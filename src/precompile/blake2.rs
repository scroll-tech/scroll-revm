use super::precompile_not_implemented;

use revm::{precompile::PrecompileWithAddress, primitives::Address};

// CONSTANTS
// ================================================================================================

/// The BLAKE2 precompile address.
const ADDRESS: Address = revm::precompile::blake2::FUN.0;

// BLAKE2 PRECOMPILE
// ================================================================================================

/// The shanghai BLAKE2 precompile implementation with address.
///
/// This precompile is not implemented and will return `PrecompileError::Other("Precompile not
/// implemented".into())`.
pub const SHANGHAI: PrecompileWithAddress = precompile_not_implemented(ADDRESS);
