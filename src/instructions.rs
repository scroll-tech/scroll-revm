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
/// The blockhash is computed as the keccak256 hash of the chain id and the block number.
/// If the requested block number is the current block number, a future block number or a block
/// number older than `BLOCK_HASH_HISTORY` we return 0.
fn blockhash<WIRE: InterpreterTypes, H: Host>(interpreter: &mut Interpreter<WIRE>, host: &mut H) {
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
        // blockhash requested for block in the history - return the hash
        _ => {
            let chain_id = as_u64_saturated!(host.chain_id());
            compute_block_hash(chain_id, as_u64_saturated!(requested_block_number))
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
