use revm::context::{Transaction, TxEnv};
use revm_primitives::{Address, Bytes, TxKind, B256, U256};

/// The type for a l1 message transaction.
pub const L1_MESSAGE_TYPE: u8 = 0x7E;

#[auto_impl::auto_impl(&, Arc, Box)]
pub trait ScrollTxTr: Transaction {
    /// Whether the transaction is an L1 message.
    fn is_l1_msg(&self) -> bool;

    /// The RLP encoded transaction bytes which are used to calculate the cost associated with
    /// posting the transaction on L1.
    fn rlp_bytes(&self) -> Option<&Bytes>;
}

/// A Scroll transaction. Wraps around a base transaction and provides the optional RLPed bytes for
/// the l1 fee computation.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScrollTransaction<T: Transaction> {
    pub base: T,
    pub rlp_bytes: Option<Bytes>,
}

impl<T: Transaction> ScrollTransaction<T> {
    pub fn new(base: T, rlp_bytes: Option<Bytes>) -> Self {
        Self { base, rlp_bytes }
    }
}

impl Default for ScrollTransaction<TxEnv> {
    fn default() -> Self {
        Self { base: TxEnv::default(), rlp_bytes: None }
    }
}

impl<T: Transaction> Transaction for ScrollTransaction<T> {
    type AccessList = T::AccessList;
    type Authorization = T::Authorization;

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

    fn access_list(&self) -> Option<&Self::AccessList> {
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

    fn authorization_list(&self) -> impl Iterator<Item = &Self::Authorization> {
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

    fn rlp_bytes(&self) -> Option<&Bytes> {
        self.rlp_bytes.as_ref()
    }
}
