use revm::{primitives::Bytes, wiring::TransactionValidation};

mod env;
mod handle_register;
mod instruction;
mod l1block;
mod precompile;
mod spec;

use l1block::L1BlockInfo;
pub use spec::*;

pub trait ScrollContext {
    /// A reference to the cached L1 block info.
    fn l1_block_info(&self) -> Option<&L1BlockInfo>;

    /// A mutable reference to the cached L1 block info.
    fn l1_block_info_mut(&mut self) -> &mut Option<L1BlockInfo>;
}

pub trait ScrollTransaction {
    /// Whether the transaction is an L1 message.
    fn is_l1_msg(&self) -> bool;

    /// The RLP encoded transaction bytes which are used to calculate the cost associated with
    /// posting the transaction on L1.
    fn rlp_bytes(&self) -> Option<Bytes>;
}

/// Trait for an Scroll chain spec.
pub trait ScrollWiring:
    revm::EvmWiring<
    ChainContext: ScrollContext,
    Hardfork = ScrollSpecId,
    HaltReason = revm::wiring::result::HaltReason,
    Transaction: ScrollTransaction
                     + TransactionValidation<
        ValidationError = revm::wiring::result::InvalidTransaction,
    >,
>
{
}

impl<EvmWiringT> ScrollWiring for EvmWiringT where
    EvmWiringT: revm::EvmWiring<
        ChainContext: ScrollContext,
        Hardfork = ScrollSpecId,
        HaltReason = revm::wiring::result::HaltReason,
        Transaction: ScrollTransaction
                         + TransactionValidation<
            ValidationError = revm::wiring::result::InvalidTransaction,
        >,
    >
{
}
