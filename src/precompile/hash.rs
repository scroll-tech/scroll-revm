use super::precompile_not_implemented;

use revm::{precompile::PrecompileWithAddress, primitives::Address};

pub mod sha256 {
    use super::*;
    use revm::precompile::PrecompileResult;

    // CONSTANTS
    // ------------------------------------------------------------------------------------------------

    /// The SHA256 precompile address.
    pub const ADDRESS: Address = revm::precompile::hash::SHA256.0;

    // SHA256 SHANGHAI PRECOMPILE
    // --------------------------------------------------------------------------------------------

    /// The shanghai SHA256 precompile implementation with address.
    pub const SHANGHAI: PrecompileWithAddress = precompile_not_implemented(ADDRESS);

    // SHA256 BERNOULLI PRECOMPILE
    // --------------------------------------------------------------------------------------------

    /// The bernoulli SHA256 precompile implementation with address.
    pub const BERNOULLI: PrecompileWithAddress = PrecompileWithAddress(ADDRESS, run);

    pub fn run(input: &[u8], gas_limit: u64) -> PrecompileResult {
        cfg_if::cfg_if! {
            if #[cfg(all(target_os = "zkvm", not(target_vendor = "succinct"), target_arch = "riscv32", feature = "openvm"))] {
                use revm::precompile::{calc_linear_cost_u32, PrecompileError, PrecompileOutput};
                let cost = calc_linear_cost_u32(input.len(), 60, 12);
                if cost > gas_limit {
                        Err(PrecompileError::OutOfGas)
                } else {
                    let output = openvm_sha2::sha256(input);
                    Ok(PrecompileOutput::new(cost, output.to_vec().into()))
                }
            } else {
                revm::precompile::hash::sha256_run(input, gas_limit)
            }
        }
    }
}

pub mod ripemd160 {
    use super::*;

    // CONSTANTS
    // --------------------------------------------------------------------------------------------

    /// The RIPEMD160 precompile address.
    const ADDRESS: Address = revm::precompile::hash::RIPEMD160.0;

    // RIPEMD160 SHANGHAI PRECOMPILE
    // --------------------------------------------------------------------------------------------

    /// The shanghai RIPEMD160 precompile implementation with address.
    ///
    /// This precompile is not implemented and will return `PrecompileError::Other("Precompile not
    /// implemented".into())`.
    pub const SHANGHAI: PrecompileWithAddress = precompile_not_implemented(ADDRESS);
}
