use crate::ScrollSpecId;
use std::boxed::Box;

use once_cell::race::OnceBox;
use revm::{
    context::{Cfg, ContextTr},
    handler::{EthPrecompiles, PrecompileProvider},
    interpreter::InterpreterResult,
    precompile::{self, PrecompileError, PrecompileErrors, PrecompileWithAddress, Precompiles},
    primitives::{Address, Bytes},
};

mod blake2;
mod bn128;
mod hash;
mod modexp;

/// Provides Scroll precompiles, modifying any relevant behaviour.
pub struct ScrollPrecompileProvider<CTX> {
    precompile_provider: EthPrecompiles<CTX>,
}

impl<CTX> Clone for ScrollPrecompileProvider<CTX> {
    fn clone(&self) -> Self {
        Self { precompile_provider: self.precompile_provider.clone() }
    }
}

impl<CTX> ScrollPrecompileProvider<CTX> {
    pub fn new(precompiles: &'static Precompiles) -> Self {
        Self {
            precompile_provider: EthPrecompiles {
                precompiles,
                _phantom: core::marker::PhantomData,
            },
        }
    }

    #[inline]
    pub fn new_with_spec(spec: ScrollSpecId) -> Self {
        Self::new(load_precompiles(spec))
    }
}

/// A helper function that creates a precompile that returns `PrecompileError::Other("Precompile not
/// implemented".into())` for a given address.
const fn precompile_not_implemented(address: Address) -> PrecompileWithAddress {
    PrecompileWithAddress(address, |_input: &Bytes, _gas_limit: u64| {
        Err(PrecompileError::Other("NotImplemented: Precompile not implemented".into()).into())
    })
}

/// Load the precompiles for the given scroll spec.
pub fn load_precompiles(spec_id: ScrollSpecId) -> &'static Precompiles {
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

        if spec_id.is_enabled_in(ScrollSpecId::BERNOULLI) {
            precompiles.extend([hash::sha256::SHA256_BERNOULLI]);
        }

        Box::new(precompiles)
    })
}

impl<CTX> PrecompileProvider for ScrollPrecompileProvider<CTX>
where
    CTX: ContextTr<Cfg: Cfg<Spec = ScrollSpecId>>,
{
    type Context = CTX;
    type Output = InterpreterResult;

    #[inline]
    fn set_spec(&mut self, spec: <<Self::Context as ContextTr>::Cfg as Cfg>::Spec) {
        *self = Self::new_with_spec(spec);
    }

    #[inline]
    fn run(
        &mut self,
        context: &mut Self::Context,
        address: &Address,
        bytes: &Bytes,
        gas_limit: u64,
    ) -> Result<Option<Self::Output>, PrecompileErrors> {
        self.precompile_provider.run(context, address, bytes, gas_limit)
    }

    #[inline]
    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address> + '_> {
        self.precompile_provider.warm_addresses()
    }

    #[inline]
    fn contains(&self, address: &Address) -> bool {
        self.precompile_provider.contains(address)
    }
}

impl<CTX> Default for ScrollPrecompileProvider<CTX> {
    fn default() -> Self {
        Self::new_with_spec(ScrollSpecId::default())
    }
}
