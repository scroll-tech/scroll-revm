use super::precompile_not_implemented;

use revm::{
    precompile::{blake2, PrecompileWithAddress},
    primitives::Address,
};

/// The BLAKE2 precompile address.
pub const ADDRESS: Address = blake2::FUN.0;

/// The BLAKE2 precompile is not implemented in the SHANGHAI hardfork.
///
/// This precompile is not implemented and will return `PrecompileError::Other("Precompile not
/// implemented".into())`.
pub const SHANGHAI: PrecompileWithAddress = precompile_not_implemented(ADDRESS);
