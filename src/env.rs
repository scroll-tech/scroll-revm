use super::ScrollTransaction;
use revm::{
    primitives::{Address, Bytes, TxKind, B256, U256},
    specification::{eip2930::AccessListItem, eip7702::AuthorizationList},
    wiring::{
        default::TxEnv as EthTxEnv,
        result::InvalidTransaction,
        transaction::{Transaction, TransactionValidation},
    },
};

/// The Scroll transaction environment.
pub struct TxEnv {
    /// The base transaction environment.
    pub base: EthTxEnv,
    /// Whether the transaction is an L1 message.
    pub is_l1_msg: bool,
    /// The RLP encoded transaction bytes which are used to calculate the cost associated with
    /// posting the transaction on L1.
    pub rlp_bytes: Option<Bytes>,
}

impl Transaction for TxEnv {
    fn caller(&self) -> &Address {
        self.base.caller()
    }

    fn gas_limit(&self) -> u64 {
        self.base.gas_limit()
    }

    fn gas_price(&self) -> &U256 {
        self.base.gas_price()
    }

    fn kind(&self) -> TxKind {
        self.base.kind()
    }

    fn value(&self) -> &U256 {
        self.base.value()
    }

    fn data(&self) -> &Bytes {
        self.base.data()
    }

    fn nonce(&self) -> u64 {
        self.base.nonce()
    }

    fn chain_id(&self) -> Option<u64> {
        self.base.chain_id()
    }

    fn access_list(&self) -> &[AccessListItem] {
        self.base.access_list()
    }

    fn max_priority_fee_per_gas(&self) -> Option<&U256> {
        self.base.max_priority_fee_per_gas()
    }

    fn blob_hashes(&self) -> &[B256] {
        self.base.blob_hashes()
    }

    fn max_fee_per_blob_gas(&self) -> Option<&U256> {
        self.base.max_fee_per_blob_gas()
    }

    fn authorization_list(&self) -> Option<&AuthorizationList> {
        self.base.authorization_list()
    }
}

impl ScrollTransaction for TxEnv {
    fn is_l1_msg(&self) -> bool {
        self.is_l1_msg
    }

    fn rlp_bytes(&self) -> Option<Bytes> {
        self.rlp_bytes.clone()
    }
}

impl TransactionValidation for TxEnv {
    type ValidationError = InvalidTransaction;
}
