use revm::{
    precompile::{
        modexp::{self, berlin_gas_calc, run_inner},
        utilities::right_pad_with_offset,
        PrecompileError, PrecompileResult, PrecompileWithAddress,
    },
    primitives::{Address, U256},
};

/// The MODEXP precompile address.
pub const ADDRESS: Address = modexp::BYZANTIUM.0;

/// The maximum length of the input for the MODEXP precompile in BERNOULLI hardfork.
pub const BERNOULLI_LEN_LIMIT: U256 = U256::from_limbs([32, 0, 0, 0]);

/// The MODEXP precompile with BERNOULLI length limit rule.
pub const BERNOULLI: PrecompileWithAddress = PrecompileWithAddress(ADDRESS, bernoulli_run);

/// The bernoulli MODEXP precompile implementation.
///
/// # Errors
/// - `PrecompileError::Other("ModexpBaseOverflow: modexp base overflow".into())` if the base length
///   is greater than 32 bytes.
/// - `PrecompileError::Other("ModexpExpOverflow: modexp exp overflow".into())` if the exponent
///   length is greater than 32 bytes.
/// - `PrecompileError::Other("ModexpModOverflow: modexp mod overflow".into())` if the modulus
///   length is greater than 32 bytes.
pub fn bernoulli_run(input: &[u8], gas_limit: u64) -> PrecompileResult {
    let base_len = U256::from_be_bytes(right_pad_with_offset::<32>(input, 0).into_owned());
    let exp_len = U256::from_be_bytes(right_pad_with_offset::<32>(input, 32).into_owned());
    let mod_len = U256::from_be_bytes(right_pad_with_offset::<32>(input, 64).into_owned());

    // modexp temporarily only accepts inputs of 32 bytes (256 bits) or less
    if base_len > BERNOULLI_LEN_LIMIT {
        return Err(PrecompileError::Other("ModexpBaseOverflow: modexp base overflow".into()));
    }
    if exp_len > BERNOULLI_LEN_LIMIT {
        return Err(PrecompileError::Other("ModexpExpOverflow: modexp exp overflow".into()));
    }
    if mod_len > BERNOULLI_LEN_LIMIT {
        return Err(PrecompileError::Other("ModexpModOverflow: modexp mod overflow".into()));
    }

    const OSAKA: bool = false;
    run_inner::<_, OSAKA>(input, gas_limit, 200, berlin_gas_calc)
}

/// The MODEXP precompile in the FEYNMAN hardfork.
pub const FEYNMAN: PrecompileWithAddress = modexp::OSAKA;
