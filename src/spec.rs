use revm::specification::hardfork::SpecId;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, enumn::N)]
#[allow(non_camel_case_types)]
pub enum ScrollSpecId {
    SHANGHAI = 1,
    BERNOULLI = 2,
    CURIE = 3,
    #[default]
    DARWIN = 4,
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
            Self::SHANGHAI | Self::BERNOULLI | Self::CURIE | Self::DARWIN => SpecId::SHANGHAI,
        }
    }
}

impl From<ScrollSpecId> for SpecId {
    fn from(spec_id: ScrollSpecId) -> Self {
        spec_id.into_eth_spec_id()
    }
}

/// String identifiers for the Scroll hardforks.
pub mod name {
    // Re-export the Ethereum hardforks.
    pub use revm::specification::hardfork::name::{LATEST, SHANGHAI};

    pub const BERNOULLI: &str = "bernoulli";
    pub const CURIE: &str = "curie";
    pub const DARWIN: &str = "darwin";
}

impl From<&str> for ScrollSpecId {
    fn from(name: &str) -> Self {
        match name {
            name::SHANGHAI => Self::SHANGHAI,
            name::BERNOULLI => Self::BERNOULLI,
            name::CURIE => Self::CURIE,
            name::DARWIN => Self::DARWIN,
            _ => Self::default(),
        }
    }
}

impl From<ScrollSpecId> for &'static str {
    fn from(value: ScrollSpecId) -> Self {
        match value {
            ScrollSpecId::SHANGHAI => name::SHANGHAI,
            ScrollSpecId::BERNOULLI => name::BERNOULLI,
            ScrollSpecId::CURIE => name::CURIE,
            ScrollSpecId::DARWIN => name::DARWIN,
        }
    }
}
