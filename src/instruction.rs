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
    specification::hardfork::{Spec, SpecId},
    wiring::Block,
};

use crate::{ScrollSpec, ScrollSpecId};

fn blockhash<H: Host + ?Sized, SPEC: Spec>(interpreter: &mut Interpreter, host: &mut H) {
    gas!(interpreter, gas::BLOCKHASH);
    pop_top!(interpreter, number);

    let block_number = host.env().block.number();

    match block_number.checked_sub(*number) {
        Some(diff) if !diff.is_zero() => {
            let diff = as_u64_saturated!(diff);
            let block_number = as_u64_saturated!(number);

            // TODO:
            //       - Do we need to check the spec?
            //       - can we add the diff check to he match guard and convert this to if let pattern?
            if SPEC::enabled(SpecId::SHANGHAI) && diff <= BLOCK_HASH_HISTORY {
                let mut input = [0u8; 16];
                input[..8].copy_from_slice(&host.env().cfg.chain_id.to_be_bytes());
                input[8..].copy_from_slice(&block_number.to_be_bytes());
                *number = U256::from_be_bytes(keccak256(input).into());
                return;
            }
        }
        _ => {
            // If blockhash is requested for the current block, the hash should be 0, so we fall
            // through.
        }
    }

    *number = U256::ZERO;
}

fn basefee<H: Host + ?Sized, SPEC: ScrollSpec>(interpreter: &mut Interpreter, host: &mut H) {
    if !SPEC::scroll_enabled(ScrollSpecId::CURIE) {
        interpreter.instruction_result = InstructionResult::NotActivated;
        return;
    }

    gas!(interpreter, gas::BASE);
    push!(interpreter, *host.env().block.basefee());
}

fn extcodesize<H: Host + ?Sized, SPEC: Spec>(interpreter: &mut Interpreter, host: &mut H) {
    pop_address!(interpreter, address);
    let Some(code) = host.code(address) else {
        interpreter.instruction_result = InstructionResult::FatalExternalError;
        return;
    };

    let (code, load) = code.into_components();

    gas!(interpreter, warm_cold_cost(load.state_load.is_cold));

    push!(interpreter, U256::from(code.len()));
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

fn selfdestruct<H: Host + ?Sized, SPEC: ScrollSpec>(interpreter: &mut Interpreter, _host: &mut H) {
    interpreter.instruction_result = InstructionResult::NotActivated;
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

pub fn make_scroll_instruction_tables<'a, H: Host + ?Sized, SPEC: ScrollSpec>(
) -> InstructionTables<'a, H> {
    let mut table = make_instruction_table::<H, SPEC>();

    // override the instructions
    table[opcode::BLOCKHASH as usize] = blockhash::<H, SPEC>;
    table[opcode::BASEFEE as usize] = basefee::<H, SPEC>;
    table[opcode::EXTCODESIZE as usize] = extcodesize::<H, SPEC>;
    table[opcode::TSTORE as usize] = tstore::<H, SPEC>;
    table[opcode::TLOAD as usize] = tload::<H, SPEC>;
    table[opcode::SELFDESTRUCT as usize] = selfdestruct::<H, SPEC>;
    table[opcode::MCOPY as usize] = mcopy::<H, SPEC>;

    InstructionTables::Plain(table)
}