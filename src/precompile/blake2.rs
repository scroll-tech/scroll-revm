use super::precompile_not_implemented;

use revm::{
    precompile::{u64_to_address, PrecompileWithAddress},
    primitives::Address,
};

// CONSTANTS
// ================================================================================================

/// The BLAKE2 precompile index.
const BLAKE2_PRECOMPILE_INDEX: u64 = 9;

/// The BLAKE2 precompile address.
const BLAKE2_PRECOMPILE_ADDRESS: Address = u64_to_address(BLAKE2_PRECOMPILE_INDEX);

// BLAKE2 PRECOMPILE
// ================================================================================================

/// The shanghai BLAKE2 precompile implementation with address.
///
/// This precompile is not implemented and will return `PrecompileError::Other("Precompile not
/// implemented".into())`.
pub const SHANGHAI: PrecompileWithAddress = precompile_not_implemented(BLAKE2_PRECOMPILE_ADDRESS);
