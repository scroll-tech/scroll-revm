use crate::ScrollSpecId;
use std::{boxed::Box, string::String};

use once_cell::race::OnceBox;
use revm::{
    context::{Cfg, ContextTr},
    handler::{EthPrecompiles, PrecompileProvider},
    interpreter::{InputsImpl, InterpreterResult},
    precompile::{self, secp256r1, PrecompileError, PrecompileWithAddress, Precompiles},
    primitives::Address,
};
use revm_primitives::hardfork::SpecId;

mod blake2;
mod bn128;
mod hash;
mod modexp;

/// Provides Scroll precompiles, modifying any relevant behaviour.
#[derive(Debug, Clone)]
pub struct ScrollPrecompileProvider {
    precompile_provider: EthPrecompiles,
    spec: ScrollSpecId,
}

impl ScrollPrecompileProvider {
    #[inline]
    pub fn new_with_spec(spec: ScrollSpecId) -> Self {
        let precompiles = match spec {
            ScrollSpecId::SHANGHAI => pre_bernoulli(),
            ScrollSpecId::BERNOULLI | ScrollSpecId::CURIE | ScrollSpecId::DARWIN => bernoulli(),
            ScrollSpecId::EUCLID => euclid(),
            ScrollSpecId::FEYNMAN => feynman(),
        };
        Self { precompile_provider: EthPrecompiles { precompiles, spec: SpecId::default() }, spec }
    }

    /// Precompiles getter.
    #[inline]
    pub fn precompiles(&self) -> &'static Precompiles {
        self.precompile_provider.precompiles
    }
}

/// A helper function that creates a precompile that returns `PrecompileError::Other("Precompile not
/// implemented".into())` for a given address.
const fn precompile_not_implemented(address: Address) -> PrecompileWithAddress {
    PrecompileWithAddress(address, |_input: &[u8], _gas_limit: u64| {
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
            hash::sha256::SHANGHAI,
            hash::ripemd160::SHANGHAI,
            precompile::identity::FUN,
            modexp::BERNOULLI,
            bn128::add::ISTANBUL,
            bn128::mul::ISTANBUL,
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
        precompiles.extend([hash::sha256::BERNOULLI]);
        Box::new(precompiles)
    })
}

/// Returns precompiles for Euclid spec.
pub(crate) fn euclid() -> &'static Precompiles {
    static INSTANCE: OnceBox<Precompiles> = OnceBox::new();
    INSTANCE.get_or_init(|| {
        let mut precompiles = bernoulli().clone();
        precompiles.extend([secp256r1::P256VERIFY]);
        Box::new(precompiles)
    })
}

/// Returns precompiles for Feynman spec.
pub(crate) fn feynman() -> &'static Precompiles {
    static INSTANCE: OnceBox<Precompiles> = OnceBox::new();
    INSTANCE.get_or_init(|| {
        let mut precompiles = euclid().clone();
        precompiles.extend([bn128::pair::FEYNMAN]);
        Box::new(precompiles)
    })
}

impl<CTX> PrecompileProvider<CTX> for ScrollPrecompileProvider
where
    CTX: ContextTr<Cfg: Cfg<Spec = ScrollSpecId>>,
{
    type Output = InterpreterResult;

    #[inline]
    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) -> bool {
        if spec == self.spec {
            return false;
        }
        *self = Self::new_with_spec(spec);
        true
    }

    #[inline]
    fn run(
        &mut self,
        context: &mut CTX,
        address: &Address,
        inputs: &InputsImpl,
        is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<Self::Output>, String> {
        self.precompile_provider.run(context, address, inputs, is_static, gas_limit)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::precompile::bn128::pair;
    use revm::primitives::hex;

    #[test]
    fn test_bn128_large_input() {
        // test case copied from geth
        let input = hex::decode("00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c21800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c21800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed275dc4a288d1afb3cbb1ac09187524c7db36395df7be3b99e673b13a075a65ec1d9befcd05a5323e6da4d435f3b617cdb3af83285c2df711ef39c01571827f9d00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c21800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c21800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed275dc4a288d1afb3cbb1ac09187524c7db36395df7be3b99e673b13a075a65ec1d9befcd05a5323e6da4d435f3b617cdb3af83285c2df711ef39c01571827f9d00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c21800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c21800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed275dc4a288d1afb3cbb1ac09187524c7db36395df7be3b99e673b13a075a65ec1d9befcd05a5323e6da4d435f3b617cdb3af83285c2df711ef39c01571827f9d00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c21800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c21800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed275dc4a288d1afb3cbb1ac09187524c7db36395df7be3b99e673b13a075a65ec1d9befcd05a5323e6da4d435f3b617cdb3af83285c2df711ef39c01571827f9d00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c21800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa00000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000002198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c21800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed275dc4a288d1afb3cbb1ac09187524c7db36395df7be3b99e673b13a075a65ec1d9befcd05a5323e6da4d435f3b617cdb3af83285c2df711ef39c01571827f9d").unwrap();

        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();

        // Euclid version should reject this input
        let f = euclid().get(&pair::ADDRESS).expect("precompile exists");
        let outcome = f(&input, u64::MAX);
        assert!(outcome.is_err());

        // Feynman version should accept this input
        let f = feynman().get(&pair::ADDRESS).expect("precompile exists");
        let outcome = f(&input, u64::MAX).expect("call succeeds");
        assert_eq!(outcome.bytes, expected);
    }
}
