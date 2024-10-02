use core::cmp::max;
use revm::{
    bytecode::opcode,
    interpreter::{
        as_u64_saturated, as_usize_or_fail, gas,
        gas::warm_cold_cost,
        gas_or_fail, pop, pop_address, pop_top, push, require_non_staticcall, resize_memory,
        table::{make_instruction_table, InstructionTables},
        Host, InstructionResult, Interpreter,
    },
    primitives::{keccak256, BLOCK_HASH_HISTORY, U256},
    wiring::Block,
};

use crate::{code::ScrollCodeHost, ScrollSpec, ScrollSpecId};

/// Creates a table of instructions for the Scroll hardfork.
///
/// The following instructions are overridden:
/// - `BLOCKHASH`
/// - `BASEFEE`
/// - `EXTCODESIZE`
/// - `TSTORE`
/// - `TLOAD`
/// - `SELFDESTRUCT`
/// - `MCOPY`
pub fn make_scroll_instruction_tables<'a, H: Host + ?Sized + ScrollCodeHost, SPEC: ScrollSpec>(
) -> InstructionTables<'a, H> {
    let mut table = make_instruction_table::<H, SPEC>();

    // override the instructions
    table[opcode::BLOCKHASH as usize] = blockhash::<H>;
    table[opcode::BASEFEE as usize] = basefee::<H, SPEC>;
    table[opcode::EXTCODESIZE as usize] = extcodesize::<H>;
    table[opcode::TSTORE as usize] = tstore::<H, SPEC>;
    table[opcode::TLOAD as usize] = tload::<H, SPEC>;
    table[opcode::SELFDESTRUCT as usize] = selfdestruct::<H>;
    table[opcode::MCOPY as usize] = mcopy::<H, SPEC>;

    InstructionTables::Plain(table)
}

// SHANGHAI OPCODE IMPLEMENTATIONS
// ================================================================================================

/// Computes the blockhash for the requested block number.
///
/// The blockhash is computed as the keccak256 hash of the chain id and the block number.
/// If the requested block number is the current block number, a future block number or a block number
/// older than `BLOCK_HASH_HISTORY` we return 0.
fn blockhash<H: Host + ?Sized>(interpreter: &mut Interpreter, host: &mut H) {
    gas!(interpreter, gas::BLOCKHASH);
    pop_top!(interpreter, requested_block_number);

    // compute the diff between the current block number and the requested block number
    let current_block_number = host.env().block.number();
    let diff = as_u64_saturated!(current_block_number
        .checked_sub(*requested_block_number)
        .unwrap_or(U256::ZERO));

    *requested_block_number = match diff {
        // blockhash requested for current or future block - return 0
        0 => U256::ZERO,
        // blockhash requested for block older than BLOCK_HASH_HISTORY - return 0
        x if x > BLOCK_HASH_HISTORY => U256::ZERO,
        // blockhash requested for block in the history - return the hash
        _ => compute_block_hash(
            host.env().cfg.chain_id,
            as_u64_saturated!(requested_block_number),
        ),
    };
}

fn extcodesize<H: Host + ?Sized + ScrollCodeHost>(interpreter: &mut Interpreter, host: &mut H) {
    pop_address!(interpreter, address);
    let Some((code_size, is_cold)) = host.code_size(address) else {
        interpreter.instruction_result = InstructionResult::FatalExternalError;
        return;
    };

    gas!(interpreter, warm_cold_cost(is_cold));
    push!(interpreter, U256::from(code_size));
}

fn selfdestruct<H: Host + ?Sized>(interpreter: &mut Interpreter, _host: &mut H) {
    interpreter.instruction_result = InstructionResult::NotActivated;
}

// CURIE OPCODE IMPLEMENTATIONS
// ================================================================================================

fn basefee<H: Host + ?Sized, SPEC: ScrollSpec>(interpreter: &mut Interpreter, host: &mut H) {
    if !SPEC::scroll_enabled(ScrollSpecId::CURIE) {
        interpreter.instruction_result = InstructionResult::NotActivated;
        return;
    }

    gas!(interpreter, gas::BASE);
    push!(interpreter, *host.env().block.basefee());
}

fn tstore<H: Host + ?Sized, SPEC: ScrollSpec>(interpreter: &mut Interpreter, host: &mut H) {
    if !SPEC::scroll_enabled(ScrollSpecId::CURIE) {
        interpreter.instruction_result = InstructionResult::NotActivated;
        return;
    }

    require_non_staticcall!(interpreter);
    gas!(interpreter, gas::WARM_STORAGE_READ_COST);

    pop!(interpreter, index, value);

    host.tstore(interpreter.contract.target_address, index, value);
}

fn tload<H: Host + ?Sized, SPEC: ScrollSpec>(interpreter: &mut Interpreter, host: &mut H) {
    if !SPEC::scroll_enabled(ScrollSpecId::CURIE) {
        interpreter.instruction_result = InstructionResult::NotActivated;
        return;
    }

    gas!(interpreter, gas::WARM_STORAGE_READ_COST);

    pop_top!(interpreter, index);

    *index = host.tload(interpreter.contract.target_address, *index);
}

fn mcopy<H: Host + ?Sized, SPEC: ScrollSpec>(interpreter: &mut Interpreter, _host: &mut H) {
    if !SPEC::scroll_enabled(ScrollSpecId::CURIE) {
        interpreter.instruction_result = InstructionResult::NotActivated;
        return;
    }

    pop!(interpreter, dst, src, len);

    // into usize or fail
    let len = as_usize_or_fail!(interpreter, len);
    // deduce gas
    gas_or_fail!(interpreter, gas::copy_cost_verylow(len as u64));
    if len == 0 {
        return;
    }

    let dst = as_usize_or_fail!(interpreter, dst);
    let src = as_usize_or_fail!(interpreter, src);
    // resize memory
    resize_memory!(interpreter, max(dst, src), len);
    // copy memory in place
    interpreter.shared_memory.copy(dst, src, len);
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
