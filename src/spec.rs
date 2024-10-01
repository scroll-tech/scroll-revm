use crate::{env::TxEnv, handle_register::scroll_handle_register, ScrollContext};
use core::marker::PhantomData;
use revm::{
    database_interface::Database,
    handler::register::HandleRegisters,
    precompile::PrecompileSpecId,
    specification::hardfork::{Spec, SpecId},
    wiring::{default::block::BlockEnv, result::HaltReason, EvmWiring},
    EvmHandler,
};

use crate::l1block::L1BlockInfo;

pub struct ScrollEvmWiring<DB: Database, EXT> {
    _phantom: PhantomData<(DB, EXT)>,
}

impl<DB: Database, EXT> EvmWiring for ScrollEvmWiring<DB, EXT> {
    type Block = BlockEnv;
    type Database = DB;
    type ChainContext = Context;
    type ExternalContext = EXT;
    type Hardfork = ScrollSpecId;
    type HaltReason = HaltReason;
    type Transaction = TxEnv;
}

impl<DB: Database, EXT> revm::EvmWiring for ScrollEvmWiring<DB, EXT> {
    fn handler<'evm>(hardfork: Self::Hardfork) -> revm::EvmHandler<'evm, Self> {
        let mut handler = EvmHandler::mainnet_with_spec(hardfork);

        handler.append_handler_register(HandleRegisters::Plain(scroll_handle_register::<Self>));

        handler
    }
}

/// Context for the Scroll chain.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Context {
    l1_block_info: Option<L1BlockInfo>,
}

impl ScrollContext for Context {
    fn l1_block_info(&self) -> Option<&L1BlockInfo> {
        self.l1_block_info.as_ref()
    }

    fn l1_block_info_mut(&mut self) -> &mut Option<L1BlockInfo> {
        &mut self.l1_block_info
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, enumn::N)]
#[allow(non_camel_case_types)]
pub enum ScrollSpecId {
    SHANGHAI = 1,
    BERNOULLI = 2,
    CURIE = 3,
    DARWIN = 4,
    #[default]
    LATEST = u8::MAX,
}

impl ScrollSpecId {
    /// Returns the `ScrollSpecId` for the given `u8`.
    #[inline]
    pub fn try_from_u8(spec_id: u8) -> Option<Self> {
        Self::n(spec_id)
    }

    /// Returns `true` if the given specification ID is enabled in this spec.
    #[inline]
    pub const fn is_enabled_in(self, other: Self) -> bool {
        Self::enabled(self, other)
    }

    /// Returns `true` if the provided specification ID is enabled in the other spec.
    #[inline]
    pub const fn enabled(our: Self, other: Self) -> bool {
        our as u8 >= other as u8
    }

    /// Converts the `ScrollSpecId` to a `SpecId`.
    const fn into_eth_spec_id(self) -> SpecId {
        match self {
            Self::SHANGHAI | Self::BERNOULLI => SpecId::SHANGHAI,
            Self::CURIE | Self::DARWIN => SpecId::CANCUN,
            Self::LATEST => SpecId::CANCUN,
        }
    }
}

impl From<ScrollSpecId> for SpecId {
    fn from(spec_id: ScrollSpecId) -> Self {
        spec_id.into_eth_spec_id()
    }
}

impl From<ScrollSpecId> for PrecompileSpecId {
    fn from(value: ScrollSpecId) -> Self {
        PrecompileSpecId::from_spec_id(value.into_eth_spec_id())
    }
}

/// String identifiers for the Scroll hardforks.
pub mod id {
    // Re-export the Ethereum hardforks.
    pub use revm::specification::hardfork::id::{LATEST, SHANGHAI};

    pub const BERNOULLI: &str = "bernoulli";
    pub const CURIE: &str = "curie";
    pub const DARWIN: &str = "darwin";
}

impl From<&str> for ScrollSpecId {
    fn from(name: &str) -> Self {
        match name {
            id::SHANGHAI => Self::SHANGHAI,
            id::BERNOULLI => Self::BERNOULLI,
            id::CURIE => Self::CURIE,
            id::DARWIN => Self::DARWIN,
            _ => Self::LATEST,
        }
    }
}

impl From<ScrollSpecId> for &'static str {
    fn from(value: ScrollSpecId) -> Self {
        match value {
            ScrollSpecId::SHANGHAI => id::SHANGHAI,
            ScrollSpecId::BERNOULLI => id::BERNOULLI,
            ScrollSpecId::CURIE => id::CURIE,
            ScrollSpecId::DARWIN => id::DARWIN,
            ScrollSpecId::LATEST => id::LATEST,
        }
    }
}

pub trait ScrollSpec: Spec + Sized + 'static {
    /// The specification ID for scroll.
    const SCROLL_SPEC_ID: ScrollSpecId;

    /// Returns whether the provided `ScrollSpecId` is enabled by this spec.
    #[inline]
    fn scroll_enabled(spec_id: ScrollSpecId) -> bool {
        ScrollSpecId::enabled(Self::SCROLL_SPEC_ID, spec_id)
    }
}

macro_rules! spec {
    ($spec_id:ident, $spec_name:ident) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $spec_name;

        impl ScrollSpec for $spec_name {
            const SCROLL_SPEC_ID: ScrollSpecId = ScrollSpecId::$spec_id;
        }

        impl Spec for $spec_name {
            const SPEC_ID: SpecId = $spec_name::SCROLL_SPEC_ID.into_eth_spec_id();
        }
    };
}

spec!(SHANGHAI, ShanghaiSpec);
spec!(BERNOULLI, BernoulliSpec);
spec!(CURIE, CurieSpec);
// DARWIN no EVM spec change
spec!(LATEST, LatestSpec);

#[macro_export]
macro_rules! scroll_spec_to_generic {
    ($spec_id:expr, $e:expr) => {{
        match $spec_id {
            $crate::ScrollSpecId::SHANGHAI => {
                use $crate::ShanghaiSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::BERNOULLI => {
                use $crate::BernoulliSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::CURIE | $crate::ScrollSpecId::DARWIN => {
                use $crate::CurieSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::LATEST => {
                use $crate::LatestSpec as SPEC;
                $e
            }
        }
    }};
}
