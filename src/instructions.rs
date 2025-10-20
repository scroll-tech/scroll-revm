use crate::{exec::ScrollContextTr, ScrollSpecId};
use core::cmp::max;
use revm::{
    bytecode::opcode,
    context::Cfg,
    handler::instructions::InstructionProvider,
    interpreter::{
        _count, as_u64_saturated, as_usize_or_fail, gas, gas_or_fail, instruction_table,
        interpreter_types::{InputsTr, MemoryTr, RuntimeFlag, StackTr},
        popn, popn_top, push, require_non_staticcall, resize_memory, Host, Instruction,
        InstructionContext, InstructionResult, InstructionTable, InterpreterTypes,
    },
    primitives::{address, keccak256, Address, BLOCK_HASH_HISTORY, U256},
};
use std::rc::Rc;

const HISTORY_STORAGE_ADDRESS: Address = address!("0x0000F90827F1C53a10cb7A02335B175320002935");
const HISTORY_SERVE_WINDOW: u64 = 8191;
const DIFFICULTY: U256 = U256::ZERO;

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
/// - `DIFFICULTY`
/// - `CLZ`
pub fn make_scroll_instruction_table<WIRE: InterpreterTypes, HOST: ScrollContextTr>(
) -> InstructionTable<WIRE, HOST> {
    let mut table = instruction_table::<WIRE, HOST>();

    // override the instructions
    // static gas values taken from <https://github.com/bluealloy/revm/blob/v86/crates/interpreter/src/instructions.rs#L84>
    table[opcode::BLOCKHASH as usize] = Instruction::new(blockhash::<WIRE, HOST>, 20);
    table[opcode::BASEFEE as usize] = Instruction::new(basefee::<WIRE, HOST>, 2);
    table[opcode::TSTORE as usize] = Instruction::new(tstore::<WIRE, HOST>, 100);
    table[opcode::TLOAD as usize] = Instruction::new(tload::<WIRE, HOST>, 100);
    table[opcode::SELFDESTRUCT as usize] = Instruction::new(selfdestruct::<WIRE, HOST>, 0);
    table[opcode::MCOPY as usize] = Instruction::new(mcopy::<WIRE, HOST>, 0);
    table[opcode::DIFFICULTY as usize] = Instruction::new(difficulty::<WIRE, HOST>, 2);
    table[opcode::CLZ as usize] = Instruction::new(clz::<WIRE, HOST>, 5);

    table
}

// SHANGHAI OPCODE IMPLEMENTATIONS
// ================================================================================================

/// Computes the blockhash for the requested block number.
///
/// If the requested block number is the current block number, a future block number or a block
/// number older than `BLOCK_HASH_HISTORY` we return 0.
/// Gas is accounted in the interpreter <https://github.com/bluealloy/revm/blob/fd52a1fb531f4627ea7e69780aab56536533269d/crates/interpreter/src/interpreter.rs#L278>
fn blockhash<WIRE: InterpreterTypes, H: ScrollContextTr>(context: InstructionContext<'_, H, WIRE>) {
    let host = context.host;
    let interpreter = context.interpreter;

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
        // blockhash requested for block in the history (pre-Feynman)
        // blockhash is computed as the keccak256 hash of the chain id and the block number
        _ if !host.cfg().spec().is_enabled_in(ScrollSpecId::FEYNMAN) => {
            let chain_id = as_u64_saturated!(host.chain_id());
            compute_block_hash(chain_id, as_u64_saturated!(requested_number))
        }
        // blockhash requested for block in the history (post-Feynman)
        // blockhash is loaded from the EIP-2935 history storage system contract storage.
        _ => {
            // sload assumes that the account is present in the journal
            if host.load_account_delegated(HISTORY_STORAGE_ADDRESS).is_none() {
                interpreter.halt(InstructionResult::FatalExternalError);
                return;
            };

            // index in system contract ring buffer storage is block_number % HISTORY_SERVE_WINDOW
            let requested_block_number_u64 = as_u64_saturated!(requested_number);
            let index = requested_block_number_u64.wrapping_rem(HISTORY_SERVE_WINDOW);

            let Some(value) = host.sload(HISTORY_STORAGE_ADDRESS, U256::from(index)) else {
                interpreter.halt(InstructionResult::FatalExternalError);
                return;
            };

            value.data
        }
    };
}

/// Implements the SELFDESTRUCT instruction.
///
/// Halt execution and register account for later deletion.
fn selfdestruct<WIRE: InterpreterTypes, H: Host>(context: InstructionContext<'_, H, WIRE>) {
    context.interpreter.halt(InstructionResult::NotActivated);
}

// CURIE OPCODE IMPLEMENTATIONS
// ================================================================================================

/// EIP-3198: BASEFEE opcode
/// Gas is accounted in the interpreter <https://github.com/bluealloy/revm/blob/fd52a1fb531f4627ea7e69780aab56536533269d/crates/interpreter/src/interpreter.rs#L278>
fn basefee<WIRE: InterpreterTypes, H: ScrollContextTr>(context: InstructionContext<'_, H, WIRE>) {
    let host = context.host;
    let interpreter = context.interpreter;
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::CURIE) {
        interpreter.halt(InstructionResult::NotActivated);
        return;
    }

    push!(interpreter, U256::from(host.basefee()));
}

/// Store transient storage tied to the account.
///
/// If values is different add entry to the journal
/// so that old state can be reverted if that action is needed.
///
/// EIP-1153: Transient storage opcodes
/// Gas is accounted in the interpreter <https://github.com/bluealloy/revm/blob/fd52a1fb531f4627ea7e69780aab56536533269d/crates/interpreter/src/interpreter.rs#L278>
fn tstore<WIRE: InterpreterTypes, H: ScrollContextTr>(context: InstructionContext<'_, H, WIRE>) {
    let host = context.host;
    let interpreter = context.interpreter;
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::CURIE) {
        interpreter.halt(InstructionResult::NotActivated);
        return;
    }

    require_non_staticcall!(interpreter);

    popn!([index, value], interpreter);

    host.tstore(interpreter.input.target_address(), index, value);
}

/// Read transient storage tied to the account.
///
/// EIP-1153: Transient storage opcodes
/// Gas is accounted in the interpreter <https://github.com/bluealloy/revm/blob/fd52a1fb531f4627ea7e69780aab56536533269d/crates/interpreter/src/interpreter.rs#L278>
fn tload<WIRE: InterpreterTypes, H: ScrollContextTr>(context: InstructionContext<'_, H, WIRE>) {
    let host = context.host;
    let interpreter = context.interpreter;
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::CURIE) {
        interpreter.halt(InstructionResult::NotActivated);
        return;
    }

    popn_top!([], index, interpreter);

    *index = host.tload(interpreter.input.target_address(), *index);
}

/// Implements the MCOPY instruction.
///
/// EIP-5656: Memory copying instruction that copies memory from one location to another.
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

/// Implements the DIFFICULTY instruction.
///
/// Pushes the block difficulty(default to 0) onto the stack.
/// Gas is accounted in the interpreter <https://github.com/bluealloy/revm/blob/fd52a1fb531f4627ea7e69780aab56536533269d/crates/interpreter/src/interpreter.rs#L278>
pub fn difficulty<WIRE: InterpreterTypes, H: Host + ?Sized>(
    context: InstructionContext<'_, H, WIRE>,
) {
    push!(context.interpreter, DIFFICULTY);
}

/// Implements the CLZ instruction
///
/// EIP-7939 count leading zeros.
fn clz<WIRE: InterpreterTypes, H: ScrollContextTr>(context: InstructionContext<'_, H, WIRE>) {
    let host = context.host;
    let interpreter = context.interpreter;
    if !host.cfg().spec().is_enabled_in(ScrollSpecId::GALILEO) {
        interpreter.halt(InstructionResult::NotActivated);
        return;
    }

    popn_top!([], op1, context.interpreter);

    let leading_zeros = op1.leading_zeros();
    *op1 = U256::from(leading_zeros);
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
    use super::{compute_block_hash, make_scroll_instruction_table};
    use crate::{
        builder::{DefaultScrollContext, ScrollContext},
        instructions::HISTORY_STORAGE_ADDRESS,
        ScrollSpecId::*,
    };

    use revm::{
        bytecode::{opcode::*, Bytecode},
        database::{EmptyDB, InMemoryDB},
        interpreter::Interpreter,
        primitives::{Bytes, U256},
        DatabaseRef,
    };
    use rstest::rstest;

    #[test]
    fn test_blockhash_before_feynman() {
        let (chain_id, current_block, target_block, spec) = (123, U256::from(1024), 1000, EUCLID);

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
        let (chain_id, current_block, target_block, spec) = (123, U256::from(1024), 1000, FEYNMAN);

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

    #[rstest]
    #[case(BLOCKHASH, 20)]
    #[case(BASEFEE, 2)]
    #[case(TSTORE, 100)]
    #[case(TLOAD, 100)]
    #[case(MCOPY, 9)]
    #[case(SELFDESTRUCT, 0)]
    #[case(DIFFICULTY, 2)]
    fn test_gas_used(#[case] opcode: u8, #[case] expected_gas_used: u64) {
        let (chain_id, current_block, spec) = (123, U256::from(1024), FEYNMAN);

        let db = EmptyDB::new();
        let mut context = ScrollContext::scroll().with_db(InMemoryDB::new(db));
        context.modify_block(|block| block.number = current_block);
        context.modify_cfg(|cfg| cfg.chain_id = chain_id);
        context.modify_cfg(|cfg| cfg.spec = spec);

        let instructions = make_scroll_instruction_table();

        let bytecode = Bytecode::new_legacy(Bytes::from([opcode, STOP].to_vec()));
        let mut interpreter = Interpreter::default().with_bytecode(bytecode);
        let _ = interpreter.stack.push(U256::from(1));
        let _ = interpreter.stack.push(U256::from(0));
        let _ = interpreter.stack.push(U256::from(0));
        interpreter.run_plain(&instructions, &mut context);

        let actual_gas_used = interpreter.gas.used();
        assert_eq!(actual_gas_used, expected_gas_used);
    }
}
