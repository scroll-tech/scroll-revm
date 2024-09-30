use revm::{
    precompile::{
        bn128::{
            pair::{ISTANBUL_PAIR_BASE, ISTANBUL_PAIR_PER_POINT},
            run_pair,
        },
        u64_to_address, Precompile, PrecompileError, PrecompileResult, PrecompileWithAddress,
    },
    primitives::{Address, Bytes},
};

pub mod pair {
    use super::*;

    /// The BN128 pairing precompile index.
    const BN128_PAIRING_PRECOMPILE_INDEX: u64 = 8;

    /// The BN128 pairing precompile address.
    const BN128_PAIRING_PRECOMPILE_ADDRESS: Address =
        u64_to_address(BN128_PAIRING_PRECOMPILE_INDEX);

    /// The BN128 PAIRING precompile with address.
    pub const BERNOULLI: PrecompileWithAddress = PrecompileWithAddress(
        BN128_PAIRING_PRECOMPILE_ADDRESS,
        Precompile::Standard(bernoulli_run),
    );

    /// The number of pairing inputs per pairing operation. If the inputs provided to the precompile
    /// call are < 4, we append (G1::infinity, G2::generator) until we have the required no. of inputs.
    const N_PAIRING_PER_OP: usize = 4;

    /// The number of bytes taken to represent a pair (G1, G2).
    const N_BYTES_PER_PAIR: usize = 192;

    /// The bernoulli BN128 PAIRING precompile implementation.
    fn bernoulli_run(input: &Bytes, gas_limit: u64) -> PrecompileResult {
        if input.len() > N_PAIRING_PER_OP * N_BYTES_PER_PAIR {
            return Err(
                PrecompileError::Other("BN128PairingInputOverflow: input overflow".into()).into(),
            );
        }
        run_pair(
            input,
            ISTANBUL_PAIR_PER_POINT,
            ISTANBUL_PAIR_BASE,
            gas_limit,
        )
    }
}