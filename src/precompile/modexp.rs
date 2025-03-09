use revm::{
    precompile::{
        modexp::{berlin_gas_calc, run_inner},
        u64_to_address,
        utilities::right_pad_with_offset,
        PrecompileError, PrecompileResult, PrecompileWithAddress,
    },
    primitives::{Address, Bytes, U256},
};

// CONSTANTS
// ================================================================================================

/// The MODEXP precompile index.
const MODEXP_PRECOMPILE_INDEX: u64 = 5;

/// The MODEXP precompile address.
const MODEXP_PRECOMPILE_ADDRESS: Address = u64_to_address(MODEXP_PRECOMPILE_INDEX);

/// The maximum length of the input for the MODEXP precompile.
const SCROLL_LEN_LIMIT: U256 = U256::from_limbs([32, 0, 0, 0]);

// MODEXP PRECOMPILE
// ================================================================================================

/// The bernoulli MODEXP precompile implementation with address.
pub const BERNOULLI: PrecompileWithAddress =
    PrecompileWithAddress(MODEXP_PRECOMPILE_ADDRESS, bernoulli_run);

/// The bernoulli MODEXP precompile implementation.
///
/// # Errors
/// - `PrecompileError::Other("ModexpBaseOverflow: modexp base overflow".into())` if the base length
///   is greater than 32 bytes.
/// - `PrecompileError::Other("ModexpExpOverflow: modexp exp overflow".into())` if the exponent
///   length is greater than 32 bytes.
/// - `PrecompileError::Other("ModexpModOverflow: modexp mod overflow".into())` if the modulus
///   length is greater than 32 bytes.
fn bernoulli_run(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let base_len = U256::from_be_bytes(right_pad_with_offset::<32>(input, 0).into_owned());
    let exp_len = U256::from_be_bytes(right_pad_with_offset::<32>(input, 32).into_owned());
    let mod_len = U256::from_be_bytes(right_pad_with_offset::<32>(input, 64).into_owned());

    // modexp temporarily only accepts inputs of 32 bytes (256 bits) or less
    if base_len > SCROLL_LEN_LIMIT {
        return Err(PrecompileError::Other("ModexpBaseOverflow: modexp base overflow".into()));
    }
    if exp_len > SCROLL_LEN_LIMIT {
        return Err(PrecompileError::Other("ModexpExpOverflow: modexp exp overflow".into()));
    }
    if mod_len > SCROLL_LEN_LIMIT {
        return Err(PrecompileError::Other("ModexpModOverflow: modexp mod overflow".into()));
    }

    run_inner(input, gas_limit, 200, berlin_gas_calc)
}
