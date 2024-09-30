use super::precompile_not_implemented;
use revm::{
    precompile::{hash::sha256_run, u64_to_address, Precompile, PrecompileWithAddress},
    primitives::Address,
};

/// The SHA256 precompile index.
const SHA256_PRECOMPILE_INDEX: u64 = 2;

/// The SHA256 precompile address.
const SHA256_PRECOMPILE_ADDRESS: Address = u64_to_address(SHA256_PRECOMPILE_INDEX);

/// The shanghai SHA256 precompile implementation with address.
pub const SHA256_SHANGHAI: PrecompileWithAddress =
    precompile_not_implemented(SHA256_PRECOMPILE_ADDRESS);

/// The bernoulli SHA256 precompile implementation with address.
pub const SHA256_BERNOULLI: PrecompileWithAddress =
    PrecompileWithAddress(SHA256_PRECOMPILE_ADDRESS, Precompile::Standard(sha256_run));

/// The RIPEMD160 precompile index.
const RIPEMD160_PRECOMPILE_INDEX: u64 = 3;

/// The RIPEMD160 precompile address.
const RIPEMD160_PRECOMPILE_ADDRESS: Address = u64_to_address(RIPEMD160_PRECOMPILE_INDEX);

/// The shanghai RIPEMD160 precompile implementation with address.
pub const RIPEMD160_SHANGHAI: PrecompileWithAddress =
    precompile_not_implemented(RIPEMD160_PRECOMPILE_ADDRESS);
