use crate::ScrollSpecId;

use revm::{
    primitives::{address, Address, U256},
    Database,
};

// CONSTANTS
// ================================================================================================

/// The cost of a zero byte in calldata.
const ZERO_BYTE_COST: u64 = 4;

/// The cost of a non-zero byte in calldata.
const NON_ZERO_BYTE_COST: u64 = 16;

/// The extra cost of committing a transaction on L1.
const TX_L1_COMMIT_EXTRA_COST: U256 = U256::from_limbs([64u64, 0, 0, 0]);

/// The precision used for L1 fee calculations.
const TX_L1_FEE_PRECISION: U256 = U256::from_limbs([1_000_000_000u64, 0, 0, 0]);

/// The L1 gas price oracle address.
pub const L1_GAS_PRICE_ORACLE_ADDRESS: Address =
    address!("5300000000000000000000000000000000000002");

/// The L1 base fee storage slot.
const L1_BASE_FEE_SLOT: U256 = U256::from_limbs([1u64, 0, 0, 0]);

/// The L1 fee overhead storage slot.
const L1_OVERHEAD_SLOT: U256 = U256::from_limbs([2u64, 0, 0, 0]);

/// The L1 fee scalar storage slot.
const L1_SCALAR_SLOT: U256 = U256::from_limbs([3u64, 0, 0, 0]);

/// The L1 blob base fee storage slot.
const L1_BLOB_BASE_FEE_SLOT: U256 = U256::from_limbs([5u64, 0, 0, 0]);

/// The L1 commit scalar storage slot.
const L1_COMMIT_SCALAR_SLOT: U256 = U256::from_limbs([6u64, 0, 0, 0]);

/// The L1 blob scalar storage slot.
const L1_BLOB_SCALAR_SLOT: U256 = U256::from_limbs([7u64, 0, 0, 0]);

// L1 BLOCK INFO
// ================================================================================================

/// A struct that holds the L1 block information.
///
/// This struct is used to calculate the gas cost of a transaction based on L1 block data posted on
/// L2.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct L1BlockInfo {
    /// The current L1 base fee.
    pub l1_base_fee: U256,
    /// The current L1 fee overhead.
    pub l1_fee_overhead: U256,
    /// The current L1 fee scalar.
    pub l1_base_fee_scalar: U256,
    /// The current L1 blob base fee, None if before Curie.
    pub l1_blob_base_fee: Option<U256>,
    /// The current L1 commit scalar, None if before Curie.
    pub l1_commit_scalar: Option<U256>,
    /// The current L1 blob scalar, None if before Curie.
    pub l1_blob_scalar: Option<U256>,
    /// The current call data gas (l1_blob_scalar * l1_base_fee), None if before Curie.
    pub calldata_gas: Option<U256>,
}

impl L1BlockInfo {
    /// Try to fetch the L1 block info from the database.
    pub fn try_fetch<DB: Database>(
        db: &mut DB,
        spec_id: ScrollSpecId,
    ) -> Result<L1BlockInfo, DB::Error> {
        let l1_base_fee = db.storage(L1_GAS_PRICE_ORACLE_ADDRESS, L1_BASE_FEE_SLOT)?;
        let l1_fee_overhead = db.storage(L1_GAS_PRICE_ORACLE_ADDRESS, L1_OVERHEAD_SLOT)?;
        let l1_base_fee_scalar = db.storage(L1_GAS_PRICE_ORACLE_ADDRESS, L1_SCALAR_SLOT)?;

        // If Curie is not enabled, return the L1 block info without Curie fields.
        if !spec_id.is_enabled_in(ScrollSpecId::CURIE) {
            return Ok(L1BlockInfo {
                l1_base_fee,
                l1_fee_overhead,
                l1_base_fee_scalar,
                ..Default::default()
            });
        }

        let l1_blob_base_fee = db.storage(L1_GAS_PRICE_ORACLE_ADDRESS, L1_BLOB_BASE_FEE_SLOT)?;
        let l1_commit_scalar = db.storage(L1_GAS_PRICE_ORACLE_ADDRESS, L1_COMMIT_SCALAR_SLOT)?;
        let l1_blob_scalar = db.storage(L1_GAS_PRICE_ORACLE_ADDRESS, L1_BLOB_SCALAR_SLOT)?;
        let calldata_gas = l1_commit_scalar.saturating_mul(l1_base_fee);

        Ok(L1BlockInfo {
            l1_base_fee,
            l1_fee_overhead,
            l1_base_fee_scalar,
            l1_blob_base_fee: Some(l1_blob_base_fee),
            l1_commit_scalar: Some(l1_commit_scalar),
            l1_blob_scalar: Some(l1_blob_scalar),
            calldata_gas: Some(calldata_gas),
        })
    }

    /// Calculate the data gas for posting the transaction on L1. Calldata costs 16 gas per non-zero
    /// byte and 4 gas per zero byte.
    pub fn data_gas(&self, input: &[u8], spec_id: ScrollSpecId) -> U256 {
        if !spec_id.is_enabled_in(ScrollSpecId::CURIE) {
            U256::from(input.iter().fold(0, |acc, byte| {
                acc + if *byte == 0x00 { ZERO_BYTE_COST } else { NON_ZERO_BYTE_COST }
            }))
            .saturating_add(self.l1_fee_overhead)
            .saturating_add(TX_L1_COMMIT_EXTRA_COST)
        } else {
            U256::from(input.len())
                .saturating_mul(
                    self.l1_blob_base_fee.expect("l1_blob_base_fee should be set in Curie"),
                )
                .saturating_mul(self.l1_blob_scalar.expect("l1_blob_scalar should be set in Curie"))
        }
    }

    fn calculate_tx_l1_cost_shanghai(&self, input: &[u8], spec_id: ScrollSpecId) -> U256 {
        let tx_l1_gas = self.data_gas(input, spec_id);
        tx_l1_gas
            .saturating_mul(self.l1_base_fee)
            .saturating_mul(self.l1_base_fee_scalar)
            .wrapping_div(TX_L1_FEE_PRECISION)
    }

    fn calculate_tx_l1_cost_curie(&self, input: &[u8], spec_id: ScrollSpecId) -> U256 {
        // "commitScalar * l1BaseFee + blobScalar * _data.length * l1BlobBaseFee"
        let blob_gas = self.data_gas(input, spec_id);

        self.calldata_gas.unwrap().saturating_add(blob_gas).wrapping_div(TX_L1_FEE_PRECISION)
    }

    /// Calculate the gas cost of a transaction based on L1 block data posted on L2.
    pub fn calculate_tx_l1_cost(&self, input: &[u8], spec_id: ScrollSpecId) -> U256 {
        if !spec_id.is_enabled_in(ScrollSpecId::CURIE) {
            self.calculate_tx_l1_cost_shanghai(input, spec_id)
        } else {
            self.calculate_tx_l1_cost_curie(input, spec_id)
        }
    }
}
