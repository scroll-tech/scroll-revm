use crate::{
    builder::{DefaultScrollContext, ScrollContext},
    l1block::L1_GAS_PRICE_ORACLE_ADDRESS,
};
use revm::{
    database::{DbAccount, InMemoryDB},
    state::AccountInfo,
    Context,
};
use revm_primitives::{address, bytes, Address, Bytes, U256};
use std::vec::Vec;

pub const TX_L1_FEE_PRECISION: U256 = U256::from_limbs([1_000_000_000u64, 0, 0, 0]);
pub const CALLER: Address = address!("0x000000000000000000000000000000000000dead");
pub const TO: Address = address!("0x00000000000000000000000000000000000dead1");
pub const BENEFICIARY: Address = address!("0x0000000000000000000000000000000000000002");
pub const MIN_TRANSACTION_COST: U256 = U256::from_limbs([21_000u64, 0, 0, 0]);
pub const L1_DATA_COST: U256 = U256::from_limbs([40_000u64, 0, 0, 0]);

/// Returns a test [`ScrollContext`] which contains a basic transaction, a default block beneficiary
/// and a state with L1 gas oracle slots set.
pub fn context() -> ScrollContext<InMemoryDB> {
    Context::scroll()
        .modify_tx_chained(|tx| {
            tx.base.caller = CALLER;
            tx.base.kind = Some(TO).into();
            tx.base.gas_price = 1;
            tx.base.gas_limit = 21000;
            tx.base.gas_priority_fee = None;
            tx.rlp_bytes = Some(bytes!("01010101"));
        })
        .modify_block_chained(|block| block.beneficiary = BENEFICIARY)
        .with_db(InMemoryDB::default())
        .modify_db_chained(|db| {
            let _ = db.replace_account_storage(
                L1_GAS_PRICE_ORACLE_ADDRESS,
                (0..7)
                    .map(|n| (U256::from(n), U256::from(10_000)))
                    .chain(core::iter::once((U256::from(7), TX_L1_FEE_PRECISION)))
                    .collect(),
            );
        })
}

pub trait ScrollContextTestUtils {
    fn with_funds(self, funds: U256) -> Self;
    fn with_gas_oracle_config(self, entries: Vec<(U256, U256)>) -> Self;
    fn with_tx_payload(self, data: Bytes) -> Self;
}

impl ScrollContextTestUtils for ScrollContext<InMemoryDB> {
    fn with_funds(self, funds: U256) -> Self {
        self.modify_db_chained(|db| {
            db.cache.accounts.insert(
                CALLER,
                DbAccount {
                    info: AccountInfo { balance: funds, ..Default::default() },
                    ..Default::default()
                },
            );
        })
    }

    fn with_gas_oracle_config(self, entries: Vec<(U256, U256)>) -> Self {
        self.modify_db_chained(|db| {
            for entry in entries {
                let _ = db.insert_account_storage(L1_GAS_PRICE_ORACLE_ADDRESS, entry.0, entry.1);
            }
        })
    }

    fn with_tx_payload(self, data: Bytes) -> Self {
        self.modify_tx_chained(|tx| tx.rlp_bytes = Some(data))
    }
}
