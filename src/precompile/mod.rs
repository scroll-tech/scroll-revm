use crate::ScrollSpecId;
use std::{boxed::Box, string::String};

use once_cell::race::OnceBox;
use revm::{
    context::{Cfg, ContextTr},
    handler::{EthPrecompiles, PrecompileProvider},
    interpreter::InterpreterResult,
    precompile::{self, PrecompileError, PrecompileWithAddress, Precompiles},
    primitives::{Address, Bytes},
};

mod blake2;
mod bn128;
mod hash;
mod modexp;

/// Provides Scroll precompiles, modifying any relevant behaviour.
pub struct ScrollPrecompileProvider {
    precompile_provider: EthPrecompiles,
}

impl Clone for ScrollPrecompileProvider {
    fn clone(&self) -> Self {
        Self { precompile_provider: self.precompile_provider.clone() }
    }
}

impl ScrollPrecompileProvider {
    pub fn new(precompiles: &'static Precompiles) -> Self {
        Self { precompile_provider: EthPrecompiles { precompiles } }
    }

    #[inline]
    pub fn new_with_spec(spec: ScrollSpecId) -> Self {
        let precompiles = match spec {
            ScrollSpecId::SHANGHAI => pre_bernoulli(),
            ScrollSpecId::BERNOULLI | ScrollSpecId::CURIE | ScrollSpecId::DARWIN => bernoulli(),
        };
        Self::new(precompiles)
    }
}

/// A helper function that creates a precompile that returns `PrecompileError::Other("Precompile not
/// implemented".into())` for a given address.
const fn precompile_not_implemented(address: Address) -> PrecompileWithAddress {
    PrecompileWithAddress(address, |_input: &Bytes, _gas_limit: u64| {
        Err(PrecompileError::Other("NotImplemented: Precompile not implemented".into()))
    })
}

/// Returns precompiles for Pre-Bernoulli spec.
pub(crate) fn pre_bernoulli() -> &'static Precompiles {
    static INSTANCE: OnceBox<Precompiles> = OnceBox::new();
    INSTANCE.get_or_init(|| {
        let mut precompiles = Precompiles::default();

        precompiles.extend([
            precompile::secp256k1::ECRECOVER,
            hash::sha256::SHA256_SHANGHAI,
            hash::ripemd160::RIPEMD160_SHANGHAI,
            precompile::identity::FUN,
            modexp::BERNOULLI,
            precompile::bn128::add::ISTANBUL,
            precompile::bn128::mul::ISTANBUL,
            bn128::pair::BERNOULLI,
            blake2::SHANGHAI,
        ]);

        Box::new(precompiles)
    })
}

/// Returns precompiles for Bernoulli spec.
pub(crate) fn bernoulli() -> &'static Precompiles {
    static INSTANCE: OnceBox<Precompiles> = OnceBox::new();
    INSTANCE.get_or_init(|| {
        let mut precompiles = pre_bernoulli().clone();
        precompiles.extend([hash::sha256::SHA256_BERNOULLI]);
        Box::new(precompiles)
    })
}

impl<CTX> PrecompileProvider<CTX> for ScrollPrecompileProvider
where
    CTX: ContextTr<Cfg: Cfg<Spec = ScrollSpecId>>,
{
    type Output = InterpreterResult;

    #[inline]
    fn set_spec(&mut self, spec: <<CTX as ContextTr>::Cfg as Cfg>::Spec) {
        *self = Self::new_with_spec(spec);
    }

    #[inline]
    fn run(
        &mut self,
        context: &mut CTX,
        address: &Address,
        bytes: &Bytes,
        gas_limit: u64,
    ) -> Result<Option<Self::Output>, String> {
        self.precompile_provider.run(context, address, bytes, gas_limit)
    }

    #[inline]
    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        self.precompile_provider.warm_addresses()
    }

    #[inline]
    fn contains(&self, address: &Address) -> bool {
        self.precompile_provider.contains(address)
    }
}

impl Default for ScrollPrecompileProvider {
    fn default() -> Self {
        Self::new_with_spec(ScrollSpecId::default())
    }
}
