use crate::{exec::ScrollContextTr, ScrollSpecId};
use core::cmp::max;
use revm::{
    bytecode::opcode,
    context::Cfg,
    handler::instructions::InstructionProvider,
    interpreter::{
        as_u64_saturated, as_usize_or_fail, gas, gas_or_fail, instruction_table,
        interpreter_types::{InputsTr, MemoryTr, RuntimeFlag, StackTr},
        popn, popn_top, push, require_non_staticcall, resize_memory, Host, InstructionContext,
        InstructionResult, InstructionTable, InterpreterTypes,
    },
    primitives::{keccak256, BLOCK_HASH_HISTORY, U256},
};
use std::rc::Rc;

/// Holds the EVM instruction table for Scroll.
pub struct ScrollInstructions<WIRE: InterpreterTypes, HOST> {
    pub instruction_table: Rc<InstructionTable<WIRE, HOST>>,
}

impl<IT, CTX> InstructionProvider for ScrollInstructions<IT, CTX>
where
    IT: InterpreterTypes,
    CTX: Host,
{
    type InterpreterTypes = IT;
    type Context = CTX;

    fn instruction_table(&self) -> &InstructionTable<Self::InterpreterTypes, Self::Context> {
        &self.instruction_table
    }
}

impl<WIRE, HOST> Clone for ScrollInstructions<WIRE, HOST>
where
    WIRE: InterpreterTypes,
{
    fn clone(&self) -> Self {
        Self { instruction_table: self.instruction_table.clone() }
    }
}

impl<WIRE, HOST> ScrollInstructions<WIRE, HOST>
where
    WIRE: InterpreterTypes,
    HOST: ScrollContextTr,
{
    pub fn new_mainnet(spec: ScrollSpecId) -> Self {
        Self::new(make_scroll_instruction_table::<WIRE, HOST>(spec))
    }

    pub fn new(base_table: InstructionTable<WIRE, HOST>) -> Self {
        Self { instruction_table: Rc::new(base_table) }
    }
}

/// Creates a table of instructions for the Scroll hardfork.
///
/// The following instructions are overridden:
/// - `BLOCKHASH`
/// - `BASEFEE`
/// - `TSTORE`
/// - `TLOAD`
/// - `SELFDESTRUCT`
/// - `MCOPY`
pub fn make_scroll_instruction_table<WIRE: InterpreterTypes, HOST: ScrollContextTr>(
    spec: ScrollSpecId,
) -> InstructionTable<WIRE, HOST> {
    let mut table = instruction_table::<WIRE, HOST>();

    // override the instructions
    table[opcode::BASEFEE as usize] = basefee::<WIRE, HOST>;
    table[opcode::TSTORE as usize] = tstore::<WIRE, HOST>;
    table[opcode::TLOAD as usize] = tload::<WIRE, HOST>;
    table[opcode::SELFDESTRUCT as usize] = selfdestruct::<WIRE, HOST>;
    table[opcode::MCOPY as usize] = mcopy::<WIRE, HOST>;

    // override blockhash opcode in pre-feynman blocks
    if !spec.is_enabled_in(ScrollSpecId::FEYNMAN) {
        table[opcode::BLOCKHASH as usize] = blockhash::<WIRE, HOST>;
    }

    table
}

// SHANGHAI OPCODE IMPLEMENTATIONS
// ================================================================================================

/// Computes the blockhash for the requested block number.
///
/// The blockhash is computed as the keccak256 hash of the chain id and the block number.
/// If the requested block number is the current block number, a future block number or a block
/// number older than `BLOCK_HASH_HISTORY` we return 0.
fn blockhash<WIRE: InterpreterTypes, H: Host>(context: InstructionContext<'_, H, WIRE>) {
    let host = context.host;
    let interpreter = context.interpreter;

    gas!(interpreter, gas::BLOCKHASH);
    popn_top!([], number, interpreter);

    let requested_number = *number;
    let block_number = host.block_number();

    // compute the diff between the current block number and the requested block number
    let Some(diff) = block_number.checked_sub(requested_number) else {
        *number = U256::ZERO;
        return;
    };

    let diff = as_u64_saturated!(diff);
    *number = match diff {
        // blockhash requested for current or future block - return 0
        0 => U256::ZERO,
        // blockhash requested for block older than BLOCK_HASH_HISTORY - return 0
        x if x > BLOCK_HASH_HISTORY => U256::ZERO,
        // blockhash requested for block in the history - return the hash
        _ => {
            let chain_id = as_u64_saturated!(host.chain_id());
            compute_block_hash(chain_id, as_u64_saturated!(requested_number))
        }
    };
}

fn selfdestruct<WIRE: InterpreterTypes, H: Host>(context: InstructionContext<'_, H, WIRE>) {
    context.interpreter.halt(InstructionResult::NotActivated);
}

// CURIE OPCODE IMPLEMENTATIONS
// ================================================================================================

fn basefee<WIRE: InterpreterTypes, H: ScrollContextTr>(context: InstructionContext<'_, H, WIRE>) {
    let host = context.host;
    let interpreter = context.interpreter;
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::CURIE) {
        interpreter.halt(InstructionResult::NotActivated);
        return;
    }

    gas!(interpreter, gas::BASE);
    push!(interpreter, U256::from(host.basefee()));
}

fn tstore<WIRE: InterpreterTypes, H: ScrollContextTr>(context: InstructionContext<'_, H, WIRE>) {
    let host = context.host;
    let interpreter = context.interpreter;
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::CURIE) {
        interpreter.halt(InstructionResult::NotActivated);
        return;
    }

    require_non_staticcall!(interpreter);
    gas!(interpreter, gas::WARM_STORAGE_READ_COST);

    popn!([index, value], interpreter);

    host.tstore(interpreter.input.target_address(), index, value);
}

fn tload<WIRE: InterpreterTypes, H: ScrollContextTr>(context: InstructionContext<'_, H, WIRE>) {
    let host = context.host;
    let interpreter = context.interpreter;
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::CURIE) {
        interpreter.halt(InstructionResult::NotActivated);
        return;
    }

    gas!(interpreter, gas::WARM_STORAGE_READ_COST);

    popn_top!([], index, interpreter);

    *index = host.tload(interpreter.input.target_address(), *index);
}

fn mcopy<WIRE: InterpreterTypes, H: ScrollContextTr>(context: InstructionContext<'_, H, WIRE>) {
    let host = context.host;
    let interpreter = context.interpreter;
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::CURIE) {
        interpreter.halt(InstructionResult::NotActivated);
        return;
    }

    popn!([dst, src, len], interpreter);

    // into usize or fail
    let len = as_usize_or_fail!(interpreter, len);
    // deduce gas
    gas_or_fail!(interpreter, gas::copy_cost_verylow(len));
    if len == 0 {
        return;
    }

    let dst = as_usize_or_fail!(interpreter, dst);
    let src = as_usize_or_fail!(interpreter, src);
    // resize memory
    resize_memory!(interpreter, max(dst, src), len);
    // copy memory in place
    interpreter.memory.copy(dst, src, len);
}

// HELPER FUNCTIONS
// ================================================================================================

/// Helper function to compute the block hash.
///
/// The block hash is computed as the keccak256 hash of the chain id and the block number.
fn compute_block_hash(chain_id: u64, block_number: u64) -> U256 {
    let mut input = [0u8; 16];
    input[..8].copy_from_slice(&chain_id.to_be_bytes());
    input[8..].copy_from_slice(&block_number.to_be_bytes());
    U256::from_be_bytes(keccak256(input).into())
}

#[cfg(test)]
mod tests {
    use revm::{
        bytecode::{opcode::*, Bytecode},
        database::{EmptyDB, InMemoryDB},
        interpreter::Interpreter,
        primitives::{Bytes, U256},
        DatabaseRef,
    };

    use crate::{
        builder::{DefaultScrollContext, ScrollContext},
        ScrollSpecId::*,
    };

    use super::{compute_block_hash, make_scroll_instruction_table};

    #[test]
    fn test_blockhash_before_feynman() {
        let (chain_id, current_block, target_block, spec) = (123, U256::from(1024), 1000, EUCLID);

        let db = EmptyDB::new();
        let mut context = ScrollContext::scroll().with_db(InMemoryDB::new(db));
        context.modify_block(|block| block.number = current_block);
        context.modify_cfg(|cfg| cfg.chain_id = chain_id);
        context.modify_cfg(|cfg| cfg.spec = spec);

        let instructions = make_scroll_instruction_table(spec);

        let bytecode = Bytecode::new_legacy(Bytes::from(&[BLOCKHASH, STOP]));
        let mut interpreter = Interpreter::default().with_bytecode(bytecode);
        let _ = interpreter.stack.push(U256::from(target_block));
        interpreter.run_plain(&instructions, &mut context);

        let expected = compute_block_hash(chain_id, target_block);
        let actual = interpreter.stack.pop().expect("stack is not empty");
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_blockhash_after_feynman() {
        let (chain_id, current_block, target_block, spec) = (123, U256::from(1024), 1000, FEYNMAN);

        let db = EmptyDB::new();
        let mut context = ScrollContext::scroll().with_db(InMemoryDB::new(db));
        context.modify_block(|block| block.number = current_block);
        context.modify_cfg(|cfg| cfg.chain_id = chain_id);
        context.modify_cfg(|cfg| cfg.spec = spec);

        let instructions = make_scroll_instruction_table(spec);

        let bytecode = Bytecode::new_legacy(Bytes::from(&[BLOCKHASH, STOP]));
        let mut interpreter = Interpreter::default().with_bytecode(bytecode);
        let _ = interpreter.stack.push(U256::from(target_block));
        interpreter.run_plain(&instructions, &mut context);

        let expected = db.block_hash_ref(target_block).expect("db contains block hash").into();
        let actual = interpreter.stack.pop().expect("stack is not empty");
        assert_eq!(actual, expected);
    }
}
