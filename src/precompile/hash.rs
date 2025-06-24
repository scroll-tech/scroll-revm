use super::precompile_not_implemented;

use revm::{
    precompile::{hash, PrecompileWithAddress},
    primitives::Address,
};

pub mod sha256 {
    use super::*;

    /// SHA-256 precompile address
    pub const ADDRESS: Address = hash::SHA256.0;

    /// The SHA256 precompile is not implemented in the Shanghai hardfork.
    pub const SHANGHAI: PrecompileWithAddress = precompile_not_implemented(ADDRESS);

    /// The bernoulli SHA256 precompile implementation with address.
    pub const BERNOULLI: PrecompileWithAddress = PrecompileWithAddress(ADDRESS, hash::sha256_run);
}

pub mod ripemd160 {
    use super::*;

    /// The RIPEMD160 precompile address.
    pub const ADDRESS: Address = hash::RIPEMD160.0;

    /// The shanghai RIPEMD160 precompile is not implemented in the Shanghai hardfork.
    ///
    /// This precompile is not implemented and will return `PrecompileError::Other("Precompile not
    /// implemented".into())`.
    pub const SHANGHAI: PrecompileWithAddress = precompile_not_implemented(ADDRESS);
}
