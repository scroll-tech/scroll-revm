use revm::{
    context::{Transaction, TxEnv},
    handler::SystemCallTx,
};
use revm_primitives::{address, Address, Bytes, TxKind, B256, U256};

/// The type for a l1 message transaction.
pub const L1_MESSAGE_TYPE: u8 = 0x7E;

/// The caller address of EIP-2935 system transactions.
pub const SYSTEM_ADDRESS: Address = address!("0xfffffffffffffffffffffffffffffffffffffffe");

#[auto_impl::auto_impl(&, Arc, Box)]
pub trait ScrollTxTr: Transaction {
    /// Whether the transaction is an L1 message.
    fn is_l1_msg(&self) -> bool;

    /// Whether the transaction is a system transaction (e.g. EIP-2935).
    fn is_system_tx(&self) -> bool;

    /// The RLP encoded transaction bytes which are used to calculate the cost associated with
    /// posting the transaction on L1.
    fn rlp_bytes(&self) -> Option<&Bytes>;

    /// The compression ratio of the transaction which is used to calculate the cost associated
    /// with posting the transaction on L1.
    /// Note: compression_ratio(tx) = size(tx) * 1e9 / size(zstd(tx))
    fn compression_ratio(&self) -> Option<U256>;

    /// The size of the full rlp-encoded transaction after compression.
    /// This is used for calculating the cost associated with posting the transaction on L1.
    /// Note: compressed_size(tx) = min(size(zstd(rlp(tx))), size(rlp(tx)))
    fn compressed_size(&self) -> Option<usize>;
}

/// A Scroll transaction. Wraps around a base transaction and provides the optional RLPed bytes for
/// the l1 fee computation.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScrollTransaction<T: Transaction> {
    pub base: T,
    pub rlp_bytes: Option<Bytes>,
    pub compression_ratio: Option<U256>,
    pub compressed_size: Option<usize>,
}

impl<T: Transaction> ScrollTransaction<T> {
    pub fn new(
        base: T,
        rlp_bytes: Option<Bytes>,
        compression_ratio: Option<U256>,
        compressed_size: Option<usize>,
    ) -> Self {
        Self { base, rlp_bytes, compression_ratio, compressed_size }
    }
}

impl Default for ScrollTransaction<TxEnv> {
    fn default() -> Self {
        Self {
            base: TxEnv::default(),
            rlp_bytes: None,
            compression_ratio: None,
            compressed_size: None,
        }
    }
}

impl<T: Transaction> Transaction for ScrollTransaction<T> {
    type AccessListItem<'a>
        = T::AccessListItem<'a>
    where
        T: 'a;
    type Authorization<'a>
        = T::Authorization<'a>
    where
        T: 'a;

    fn tx_type(&self) -> u8 {
        self.base.tx_type()
    }

    fn caller(&self) -> Address {
        self.base.caller()
    }

    fn gas_limit(&self) -> u64 {
        self.base.gas_limit()
    }

    fn value(&self) -> U256 {
        self.base.value()
    }

    fn input(&self) -> &Bytes {
        self.base.input()
    }

    fn nonce(&self) -> u64 {
        self.base.nonce()
    }

    fn kind(&self) -> TxKind {
        self.base.kind()
    }

    fn chain_id(&self) -> Option<u64> {
        self.base.chain_id()
    }

    fn gas_price(&self) -> u128 {
        self.base.gas_price()
    }

    fn access_list(&self) -> Option<impl Iterator<Item = Self::AccessListItem<'_>>> {
        self.base.access_list()
    }

    fn blob_versioned_hashes(&self) -> &[B256] {
        self.base.blob_versioned_hashes()
    }

    fn max_fee_per_blob_gas(&self) -> u128 {
        self.base.max_fee_per_blob_gas()
    }

    fn authorization_list_len(&self) -> usize {
        self.base.authorization_list_len()
    }

    fn authorization_list(&self) -> impl Iterator<Item = Self::Authorization<'_>> {
        self.base.authorization_list()
    }

    fn max_priority_fee_per_gas(&self) -> Option<u128> {
        self.base.max_priority_fee_per_gas()
    }
}

impl<T: Transaction> ScrollTxTr for ScrollTransaction<T> {
    fn is_l1_msg(&self) -> bool {
        self.tx_type() == L1_MESSAGE_TYPE
    }

    fn is_system_tx(&self) -> bool {
        self.caller() == SYSTEM_ADDRESS
    }

    fn rlp_bytes(&self) -> Option<&Bytes> {
        self.rlp_bytes.as_ref()
    }

    fn compression_ratio(&self) -> Option<U256> {
        self.compression_ratio
    }

    fn compressed_size(&self) -> Option<usize> {
        self.compressed_size
    }
}

impl<TX: Transaction + SystemCallTx> SystemCallTx for ScrollTransaction<TX> {
    fn new_system_tx_with_caller(
        caller: Address,
        system_contract_address: Address,
        data: Bytes,
    ) -> Self {
        // System transactions do not require a rollup fee, as such we don't provide the RLP bytes
        // nor the compression ratio for it.
        ScrollTransaction::new(
            TX::new_system_tx_with_caller(caller, system_contract_address, data),
            None,
            None,
            None,
        )
    }
}
