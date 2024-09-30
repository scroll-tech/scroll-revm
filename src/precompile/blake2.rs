use revm::{
    precompile::{u64_to_address, PrecompileWithAddress},
    primitives::Address,
};

use super::precompile_not_implemented;

/// The BLAKE2 precompile index.
const BLAKE2_PRECOMPILE_INDEX: u64 = 9;

/// The BLAKE2 precompile address.
const BLAKE2_PRECOMPILE_ADDRESS: Address = u64_to_address(BLAKE2_PRECOMPILE_INDEX);

/// The shanghai BLAKE2 precompile implementation with address.
pub const SHANGHAI: PrecompileWithAddress = precompile_not_implemented(BLAKE2_PRECOMPILE_ADDRESS);
