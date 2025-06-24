use revm::precompile::{
    bn128::{self, run_pair, PAIR_ELEMENT_LEN},
    PrecompileError, PrecompileResult, PrecompileWithAddress,
};

pub mod pair {
    use super::*;

    pub use bn128::pair::{ADDRESS, ISTANBUL_PAIR_BASE, ISTANBUL_PAIR_PER_POINT};

    /// The number of pairing inputs per pairing operation. If the inputs provided to the precompile
    /// call are < 4, we append (G1::infinity, G2::generator) until we have the required no. of
    /// inputs.
    const BERNOULLI_LEN_LIMIT: usize = 4;

    /// The Bn128 pair precompile with BERNOULLI input rules.
    pub const BERNOULLI: PrecompileWithAddress = PrecompileWithAddress(ADDRESS, bernoulli_run);

    /// The bernoulli Bn128 pair precompile implementation.
    ///
    /// # Errors
    /// - `PrecompileError::Other("BN128PairingInputOverflow: input overflow".into())` if the input
    ///   length is greater than 768 bytes.
    fn bernoulli_run(input: &[u8], gas_limit: u64) -> PrecompileResult {
        if input.len() > BERNOULLI_LEN_LIMIT * PAIR_ELEMENT_LEN {
            return Err(PrecompileError::Other("BN128PairingInputOverflow: input overflow".into()));
        }
        run_pair(input, ISTANBUL_PAIR_PER_POINT, ISTANBUL_PAIR_BASE, gas_limit)
    }

    /// The Bn128 pair precompile in FEYNMAN hardfork.
    pub const FEYNMAN: PrecompileWithAddress = bn128::pair::ISTANBUL;
}
