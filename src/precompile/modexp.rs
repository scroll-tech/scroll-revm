use revm::{
    precompile::{
        modexp,
        modexp::{berlin_gas_calc, run_inner},
        u64_to_address,
        utilities::right_pad_with_offset,
        Precompile, PrecompileError, PrecompileId, PrecompileResult,
    },
    primitives::{Address, U256},
};

/// The MODEXP precompile address.
pub const ADDRESS: Address = u64_to_address(5);

/// The maximum length of the input for the MODEXP precompile in BERNOULLI hardfork.
pub const BERNOULLI_LEN_LIMIT: U256 = U256::from_limbs([32, 0, 0, 0]);

/// The MODEXP precompile with BERNOULLI length limit rule.
pub const BERNOULLI: Precompile = Precompile::new(PrecompileId::ModExp, ADDRESS, bernoulli_run);

/// The Galileo MODEXP precompile.
pub const GALILEO: Precompile = Precompile::new(PrecompileId::ModExp, ADDRESS, galileo_run);

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

/// The MODEXP precompile with Galileo (OSAKA) implementation.
///
/// This version removes the 32-byte length limit that was present in BERNOULLI,
/// allowing modexp operations with larger inputs according to the OSAKA specification.
pub fn galileo_run(input: &[u8], gas_limit: u64) -> PrecompileResult {
    modexp::osaka_run(input, gas_limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use revm::primitives::hex;
    use std::vec;

    #[test]
    fn test_galileo_modexp_backward_compatibility() {
        // Test case: verify that Galileo modexp doesn't affect behavior before FEYNMAN
        // Using a standard modexp test case: base=3, exp=5, mod=7
        // Expected result: 3^5 mod 7 = 243 mod 7 = 5

        // Input format: [base_len(32 bytes)][exp_len(32 bytes)][mod_len(32 bytes)][base][exp][mod]
        // base_len = 1, exp_len = 1, mod_len = 1
        // base = 3, exp = 5, mod = 7
        let input = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000001\
             0000000000000000000000000000000000000000000000000000000000000001\
             0000000000000000000000000000000000000000000000000000000000000001\
             03\
             05\
             07",
        )
        .unwrap();

        let gas_limit = 100000u64;

        // Test BERNOULLI behavior
        let bernoulli_result = bernoulli_run(&input, gas_limit);
        assert!(bernoulli_result.is_ok(), "BERNOULLI modexp should succeed");
        let bernoulli_output = bernoulli_result.unwrap();

        // Test Galileo behavior
        let galileo_result = galileo_run(&input, gas_limit);
        assert!(galileo_result.is_ok(), "Galileo modexp should succeed");
        let galileo_output = galileo_result.unwrap();

        // Verify both produce the same result
        assert_eq!(
            bernoulli_output.bytes, galileo_output.bytes,
            "Galileo modexp should produce the same result as BERNOULLI for valid inputs"
        );

        // Verify that Galileo uses more gas (OSAKA gas rules vs Berlin gas rules)
        assert!(
            galileo_output.gas_used >= bernoulli_output.gas_used,
            "Galileo should use at least as much gas as BERNOULLI (OSAKA gas rules)"
        );

        // Expected result: 3^5 mod 7 = 5
        let expected = vec![5u8];
        assert_eq!(bernoulli_output.bytes.as_ref(), &expected);
        assert_eq!(galileo_output.bytes.as_ref(), &expected);
    }

    #[test]
    fn test_galileo_modexp_with_32_byte_limit() {
        // Test that Galileo handles the 32-byte limit differently than BERNOULLI
        // Input with base_len = 33 (exceeds BERNOULLI limit)
        let input = hex::decode(
            "0000000000000000000000000000000000000000000000000000000000000021\
             0000000000000000000000000000000000000000000000000000000000000001\
             0000000000000000000000000000000000000000000000000000000000000001\
             030303030303030303030303030303030303030303030303030303030303030303\
             05\
             07",
        )
        .unwrap();

        let gas_limit = 100000u64;

        // BERNOULLI should reject this (base length > 32)
        let bernoulli_result = bernoulli_run(&input, gas_limit);
        assert!(bernoulli_result.is_err(), "BERNOULLI should reject base_len > 32");
        assert!(
            matches!(bernoulli_result.unwrap_err(), PrecompileError::Other(msg) if msg.contains("ModexpBaseOverflow")),
            "BERNOULLI should return ModexpBaseOverflow error"
        );

        // Galileo should accept this (no 32-byte limit)
        let galileo_result = galileo_run(&input, gas_limit);
        assert!(galileo_result.is_ok(), "Galileo should accept base_len > 32");
    }

    #[test]
    fn test_galileo_modexp_preserves_feynman_behavior() {
        // Test with inputs that should work in all versions
        // This ensures Galileo doesn't break existing functionality
        let test_cases = vec![
            // Small values
            (
                "0000000000000000000000000000000000000000000000000000000000000001\
                 0000000000000000000000000000000000000000000000000000000000000001\
                 0000000000000000000000000000000000000000000000000000000000000001\
                 02\
                 03\
                 05",
                vec![3u8], // 2^3 mod 5 = 8 mod 5 = 3
            ),
            // Zero exponent (should return 1)
            (
                "0000000000000000000000000000000000000000000000000000000000000001\
                 0000000000000000000000000000000000000000000000000000000000000001\
                 0000000000000000000000000000000000000000000000000000000000000001\
                 05\
                 00\
                 0d",
                vec![1u8], // 5^0 mod 13 = 1
            ),
        ];

        for (input_hex, expected) in test_cases {
            let input = hex::decode(input_hex).unwrap();
            let gas_limit = 100000u64;

            let bernoulli_result = bernoulli_run(&input, gas_limit).unwrap();
            let galileo_result = galileo_run(&input, gas_limit).unwrap();

            assert_eq!(
                bernoulli_result.bytes.as_ref(),
                &expected,
                "BERNOULLI should produce expected result"
            );
            assert_eq!(
                galileo_result.bytes.as_ref(),
                &expected,
                "Galileo should produce expected result"
            );
            assert_eq!(
                bernoulli_result.bytes, galileo_result.bytes,
                "Galileo and BERNOULLI should produce identical results for valid inputs"
            );
        }
    }
}
