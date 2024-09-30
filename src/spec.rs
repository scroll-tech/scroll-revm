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

// TODO: Refactor this logic so we only have Scroll specific forks.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, enumn::N)]
#[allow(non_camel_case_types)]
pub enum ScrollSpecId {
    FRONTIER = 0,
    FRONTIER_THAWING = 1,
    HOMESTEAD = 2,
    DAO_FORK = 3,
    TANGERINE = 4,
    SPURIOUS_DRAGON = 5,
    BYZANTIUM = 6,
    CONSTANTINOPLE = 7,
    PETERSBURG = 8,
    ISTANBUL = 9,
    MUIR_GLACIER = 10,
    BERLIN = 11,
    LONDON = 12,
    ARROW_GLACIER = 13,
    GRAY_GLACIER = 14,
    MERGE = 15,
    SHANGHAI = 16,
    /// The scroll network initially started with Shanghai with some features disabled.
    PRE_BERNOULLI = 17,
    /// Bernoulli update introduces:
    ///   - Enable `SHA-256` precompile.
    ///   - Use `EIP-4844` blobs for Data Availability (not part of layer2).
    BERNOULLI = 18,
    /// Curie update introduces:
    ///   - Support `EIP-1559` transactions.
    ///   - Support the `BASEFEE`, `MCOPY`, `TLOAD`, `TSTORE` opcodes.
    ///
    /// Although the Curie update include new opcodes in Cancun, the most important change
    /// `EIP-4844` is not included. So we sort it before Cancun.
    CURIE = 19,
    CANCUN = 20,
    PRAGUE = 21,
    PRAGUE_EOF = 22,
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
            Self::FRONTIER => SpecId::FRONTIER,
            Self::FRONTIER_THAWING => SpecId::FRONTIER_THAWING,
            Self::HOMESTEAD => SpecId::HOMESTEAD,
            Self::DAO_FORK => SpecId::DAO_FORK,
            Self::TANGERINE => SpecId::TANGERINE,
            Self::SPURIOUS_DRAGON => SpecId::SPURIOUS_DRAGON,
            Self::BYZANTIUM => SpecId::BYZANTIUM,
            Self::CONSTANTINOPLE => SpecId::CONSTANTINOPLE,
            Self::PETERSBURG => SpecId::PETERSBURG,
            Self::ISTANBUL => SpecId::ISTANBUL,
            Self::MUIR_GLACIER => SpecId::MUIR_GLACIER,
            Self::BERLIN => SpecId::BERLIN,
            Self::LONDON => SpecId::LONDON,
            Self::ARROW_GLACIER => SpecId::ARROW_GLACIER,
            Self::GRAY_GLACIER => SpecId::GRAY_GLACIER,
            Self::MERGE => SpecId::MERGE,
            Self::SHANGHAI | Self::PRE_BERNOULLI | Self::BERNOULLI | Self::CURIE => {
                SpecId::SHANGHAI
            }
            Self::CANCUN => SpecId::CANCUN,
            Self::PRAGUE => SpecId::PRAGUE,
            Self::PRAGUE_EOF => SpecId::PRAGUE_EOF,
            Self::LATEST => SpecId::LATEST,
        }
    }
}

impl From<ScrollSpecId> for SpecId {
    fn from(spec_id: ScrollSpecId) -> Self {
        spec_id.into_eth_spec_id()
    }
}

impl From<SpecId> for ScrollSpecId {
    fn from(value: SpecId) -> Self {
        match value {
            SpecId::FRONTIER => Self::FRONTIER,
            SpecId::FRONTIER_THAWING => Self::FRONTIER_THAWING,
            SpecId::HOMESTEAD => Self::HOMESTEAD,
            SpecId::DAO_FORK => Self::DAO_FORK,
            SpecId::TANGERINE => Self::TANGERINE,
            SpecId::SPURIOUS_DRAGON => Self::SPURIOUS_DRAGON,
            SpecId::BYZANTIUM => Self::BYZANTIUM,
            SpecId::CONSTANTINOPLE => Self::CONSTANTINOPLE,
            SpecId::PETERSBURG => Self::PETERSBURG,
            SpecId::ISTANBUL => Self::ISTANBUL,
            SpecId::MUIR_GLACIER => Self::MUIR_GLACIER,
            SpecId::BERLIN => Self::BERLIN,
            SpecId::LONDON => Self::LONDON,
            SpecId::ARROW_GLACIER => Self::ARROW_GLACIER,
            SpecId::GRAY_GLACIER => Self::GRAY_GLACIER,
            SpecId::MERGE => Self::MERGE,
            SpecId::SHANGHAI => Self::SHANGHAI,
            SpecId::CANCUN => Self::CANCUN,
            SpecId::PRAGUE => Self::PRAGUE,
            SpecId::PRAGUE_EOF => Self::PRAGUE_EOF,
            SpecId::LATEST => Self::LATEST,
        }
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
    pub use revm::specification::hardfork::id::*;

    pub const PRE_BERNOULLI: &str = "pre-bernoulli";
    pub const BERNOULLI: &str = "bernoulli";
    pub const CURIE: &str = "curie";
}

impl From<&str> for ScrollSpecId {
    fn from(name: &str) -> Self {
        match name {
            id::FRONTIER => Self::FRONTIER,
            id::FRONTIER_THAWING => Self::FRONTIER_THAWING,
            id::HOMESTEAD => Self::HOMESTEAD,
            id::DAO_FORK => Self::DAO_FORK,
            id::TANGERINE => Self::TANGERINE,
            id::SPURIOUS_DRAGON => Self::SPURIOUS_DRAGON,
            id::BYZANTIUM => Self::BYZANTIUM,
            id::CONSTANTINOPLE => Self::CONSTANTINOPLE,
            id::PETERSBURG => Self::PETERSBURG,
            id::ISTANBUL => Self::ISTANBUL,
            id::MUIR_GLACIER => Self::MUIR_GLACIER,
            id::BERLIN => Self::BERLIN,
            id::LONDON => Self::LONDON,
            id::ARROW_GLACIER => Self::ARROW_GLACIER,
            id::GRAY_GLACIER => Self::GRAY_GLACIER,
            id::MERGE => Self::MERGE,
            id::SHANGHAI => Self::SHANGHAI,
            id::PRE_BERNOULLI => Self::PRE_BERNOULLI,
            id::BERNOULLI => Self::BERNOULLI,
            id::CURIE => Self::CURIE,
            id::CANCUN => Self::CANCUN,
            id::PRAGUE => Self::PRAGUE,
            id::PRAGUE_EOF => Self::PRAGUE_EOF,
            _ => Self::LATEST,
        }
    }
}

impl From<ScrollSpecId> for &'static str {
    fn from(value: ScrollSpecId) -> Self {
        match value {
            ScrollSpecId::FRONTIER
            | ScrollSpecId::FRONTIER_THAWING
            | ScrollSpecId::HOMESTEAD
            | ScrollSpecId::DAO_FORK
            | ScrollSpecId::TANGERINE
            | ScrollSpecId::SPURIOUS_DRAGON
            | ScrollSpecId::BYZANTIUM
            | ScrollSpecId::CONSTANTINOPLE
            | ScrollSpecId::PETERSBURG
            | ScrollSpecId::ISTANBUL
            | ScrollSpecId::MUIR_GLACIER
            | ScrollSpecId::BERLIN
            | ScrollSpecId::LONDON
            | ScrollSpecId::ARROW_GLACIER
            | ScrollSpecId::GRAY_GLACIER
            | ScrollSpecId::MERGE
            | ScrollSpecId::SHANGHAI
            | ScrollSpecId::CANCUN
            | ScrollSpecId::PRAGUE
            | ScrollSpecId::PRAGUE_EOF => value.into_eth_spec_id().into(),
            ScrollSpecId::PRE_BERNOULLI => id::PRE_BERNOULLI,
            ScrollSpecId::BERNOULLI => id::BERNOULLI,
            ScrollSpecId::CURIE => id::CURIE,
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

spec!(FRONTIER, FrontierSpec);
// FRONTIER_THAWING no EVM spec change
spec!(HOMESTEAD, HomesteadSpec);
// DAO_FORK no EVM spec change
spec!(TANGERINE, TangerineSpec);
spec!(SPURIOUS_DRAGON, SpuriousDragonSpec);
spec!(BYZANTIUM, ByzantiumSpec);
// CONSTANTINOPLE was overridden with PETERSBURG
spec!(PETERSBURG, PetersburgSpec);
spec!(ISTANBUL, IstanbulSpec);
// MUIR_GLACIER no EVM spec change
spec!(BERLIN, BerlinSpec);
spec!(LONDON, LondonSpec);
// ARROW_GLACIER no EVM spec change
// GRAY_GLACIER no EVM spec change
spec!(MERGE, MergeSpec);
spec!(SHANGHAI, ShanghaiSpec);
spec!(CANCUN, CancunSpec);
spec!(PRAGUE, PragueSpec);
spec!(PRAGUE_EOF, PragueEofSpec);

spec!(LATEST, LatestSpec);

// Scroll Hardforks
spec!(PRE_BERNOULLI, PreBernoulliSpec);
spec!(BERNOULLI, BernoulliSpec);
spec!(CURIE, CurieSpec);

#[macro_export]
macro_rules! scroll_spec_to_generic {
    ($spec_id:expr, $e:expr) => {{
        // We are transitioning from var to generic spec.
        match $spec_id {
            $crate::ScrollSpecId::FRONTIER | $crate::ScrollSpecId::FRONTIER_THAWING => {
                use $crate::FrontierSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::HOMESTEAD | $crate::ScrollSpecId::DAO_FORK => {
                use $crate::HomesteadSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::TANGERINE => {
                use $crate::TangerineSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::SPURIOUS_DRAGON => {
                use $crate::SpuriousDragonSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::BYZANTIUM => {
                use $crate::ByzantiumSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::PETERSBURG | $crate::ScrollSpecId::CONSTANTINOPLE => {
                use $crate::PetersburgSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::ISTANBUL | $crate::ScrollSpecId::MUIR_GLACIER => {
                use $crate::IstanbulSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::BERLIN => {
                use $crate::BerlinSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::LONDON
            | $crate::ScrollSpecId::ARROW_GLACIER
            | $crate::ScrollSpecId::GRAY_GLACIER => {
                use $crate::LondonSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::MERGE => {
                use $crate::MergeSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::SHANGHAI => {
                use $crate::ShanghaiSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::CANCUN => {
                use $crate::CancunSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::LATEST => {
                use $crate::LatestSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::PRAGUE => {
                use $crate::PragueSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::PRAGUE_EOF => {
                use $crate::PragueEofSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::PRE_BERNOULLI => {
                use $crate::PreBernoulliSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::BERNOULLI => {
                use $crate::BernoulliSpec as SPEC;
                $e
            }
            $crate::ScrollSpecId::CURIE => {
                use $crate::CurieSpec as SPEC;
                $e
            }
        }
    }};
}
