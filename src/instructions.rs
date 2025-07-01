use crate::{exec::ScrollContextTr, ScrollSpecId};
use core::cmp::max;
use revm::{
    bytecode::opcode,
    context::Cfg,
    handler::instructions::InstructionProvider,
    interpreter::{
        as_u64_saturated, as_usize_or_fail, gas, gas_or_fail, instruction_table,
        interpreter_types::{InputsTr, LoopControl, MemoryTr, RuntimeFlag, StackTr},
        popn, popn_top, push, require_non_staticcall, resize_memory, Host, InstructionResult,
        InstructionTable, Interpreter, InterpreterTypes,
    },
    primitives::{address, keccak256, Address, BLOCK_HASH_HISTORY, U256},
};
use std::rc::Rc;

const HISTORY_STORAGE_ADDRESS: Address = address!("0x0000F90827F1C53a10cb7A02335B175320002935");
const HISTORY_SERVE_WINDOW: u64 = 8191;

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
    pub fn new_mainnet() -> Self {
        Self::new(make_scroll_instruction_table::<WIRE, HOST>())
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
) -> InstructionTable<WIRE, HOST> {
    let mut table = instruction_table::<WIRE, HOST>();

    // override the instructions
    table[opcode::BLOCKHASH as usize] = blockhash::<WIRE, HOST>;
    table[opcode::BASEFEE as usize] = basefee::<WIRE, HOST>;
    table[opcode::TSTORE as usize] = tstore::<WIRE, HOST>;
    table[opcode::TLOAD as usize] = tload::<WIRE, HOST>;
    table[opcode::SELFDESTRUCT as usize] = selfdestruct::<WIRE, HOST>;
    table[opcode::MCOPY as usize] = mcopy::<WIRE, HOST>;

    table
}

// SHANGHAI OPCODE IMPLEMENTATIONS
// ================================================================================================

/// Computes the blockhash for the requested block number.
///
/// If the requested block number is the current block number, a future block number or a block
/// number older than `BLOCK_HASH_HISTORY` we return 0.
fn blockhash<WIRE: InterpreterTypes, H: ScrollContextTr>(
    interpreter: &mut Interpreter<WIRE>,
    host: &mut H,
) {
    gas!(interpreter, gas::BLOCKHASH);
    popn_top!([], requested_block_number, interpreter);

    // compute the diff between the current block number and the requested block number
    let requested_block_number_u64 = as_u64_saturated!(requested_block_number);
    let current_block_number = host.block_number();
    let diff = current_block_number.saturating_sub(requested_block_number_u64);

    *requested_block_number = match diff {
        // blockhash requested for current or future block - return 0
        0 => U256::ZERO,
        // blockhash requested for block older than BLOCK_HASH_HISTORY - return 0
        x if x > BLOCK_HASH_HISTORY => U256::ZERO,
        // blockhash requested for block in the history (pre-Feynman)
        // blockhash is computed as the keccak256 hash of the chain id and the block number
        _ if !host.cfg().spec().is_enabled_in(ScrollSpecId::FEYNMAN) => {
            let chain_id = as_u64_saturated!(host.chain_id());
            compute_block_hash(chain_id, as_u64_saturated!(requested_block_number))
        }
        // blockhash requested for block in the history (post-Feynman)
        // blockhash is loaded from the EIP-2935 history storage system contract storage.
        _ => {
            // sload assumes that the account is present in the journal
            if host.load_account_delegated(HISTORY_STORAGE_ADDRESS).is_none() {
                interpreter.control.set_instruction_result(InstructionResult::FatalExternalError);
                return;
            };

            // index in system contract ring buffer storage is block_number % HISTORY_SERVE_WINDOW
            let index = requested_block_number_u64.wrapping_rem(HISTORY_SERVE_WINDOW);

            let Some(value) = host.sload(HISTORY_STORAGE_ADDRESS, U256::from(index)) else {
                interpreter.control.set_instruction_result(InstructionResult::FatalExternalError);
                return;
            };

            value.data
        }
    };
}

fn selfdestruct<WIRE: InterpreterTypes, H: Host>(
    interpreter: &mut Interpreter<WIRE>,
    _host: &mut H,
) {
    interpreter.control.set_instruction_result(InstructionResult::NotActivated);
}

// CURIE OPCODE IMPLEMENTATIONS
// ================================================================================================

fn basefee<WIRE: InterpreterTypes, H: ScrollContextTr>(
    interpreter: &mut Interpreter<WIRE>,
    host: &mut H,
) {
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::CURIE) {
        interpreter.control.set_instruction_result(InstructionResult::NotActivated);
        return;
    }

    gas!(interpreter, gas::BASE);
    push!(interpreter, U256::from(host.basefee()));
}

fn tstore<WIRE: InterpreterTypes, H: ScrollContextTr>(
    interpreter: &mut Interpreter<WIRE>,
    host: &mut H,
) {
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::CURIE) {
        interpreter.control.set_instruction_result(InstructionResult::NotActivated);
        return;
    }

    require_non_staticcall!(interpreter);
    gas!(interpreter, gas::WARM_STORAGE_READ_COST);

    popn!([index, value], interpreter);

    host.tstore(interpreter.input.target_address(), index, value);
}

fn tload<WIRE: InterpreterTypes, H: ScrollContextTr>(
    interpreter: &mut Interpreter<WIRE>,
    host: &mut H,
) {
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::CURIE) {
        interpreter.control.set_instruction_result(InstructionResult::NotActivated);
        return;
    }

    gas!(interpreter, gas::WARM_STORAGE_READ_COST);

    popn_top!([], index, interpreter);

    *index = host.tload(interpreter.input.target_address(), *index);
}

fn mcopy<WIRE: InterpreterTypes, H: ScrollContextTr>(
    interpreter: &mut Interpreter<WIRE>,
    host: &mut H,
) {
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::CURIE) {
        interpreter.control.set_instruction_result(InstructionResult::NotActivated);
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
        instructions::HISTORY_STORAGE_ADDRESS,
        ScrollSpecId::*,
    };

    use super::{compute_block_hash, make_scroll_instruction_table};

    #[test]
    fn test_blockhash_before_feynman() {
        let (chain_id, current_block, target_block, spec) = (123, 1024, 1000, EUCLID);

        let db = EmptyDB::new();
        let mut context = ScrollContext::scroll().with_db(InMemoryDB::new(db));
        context.modify_block(|block| block.number = current_block);
        context.modify_cfg(|cfg| cfg.chain_id = chain_id);
        context.modify_cfg(|cfg| cfg.spec = spec);

        let instructions = make_scroll_instruction_table();

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
        let (chain_id, current_block, target_block, spec) = (123, 1024, 1000, FEYNMAN);

        let db = EmptyDB::new();
        let mut context = ScrollContext::scroll().with_db(InMemoryDB::new(db));
        context.modify_block(|block| block.number = current_block);
        context.modify_cfg(|cfg| cfg.chain_id = chain_id);
        context.modify_cfg(|cfg| cfg.spec = spec);

        // updating the history storage system contract is not part of revm,
        // in this test we simply write the block hash to the contract storage.
        let expected_block_hash = db.block_hash_ref(target_block).expect("db contains block hash");
        context.modify_db(|db| {
            db.insert_account_storage(
                HISTORY_STORAGE_ADDRESS,
                U256::from(target_block),
                expected_block_hash.into(),
            )
            .expect("insert account should succeed")
        });

        let instructions = make_scroll_instruction_table();

        let bytecode = Bytecode::new_legacy(Bytes::from(&[BLOCKHASH, STOP]));
        let mut interpreter = Interpreter::default().with_bytecode(bytecode);
        let _ = interpreter.stack.push(U256::from(target_block));
        interpreter.run_plain(&instructions, &mut context);

        let expected = expected_block_hash.into();
        let actual = interpreter.stack.pop().expect("stack is not empty");
        assert_eq!(actual, expected);
    }
}
