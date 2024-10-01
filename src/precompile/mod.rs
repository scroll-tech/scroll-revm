use once_cell::race::OnceBox;
use revm::{
    precompile::{self, Precompile, PrecompileError, PrecompileWithAddress, Precompiles},
    primitives::{Address, Bytes},
};

use crate::ScrollSpec;

mod blake2;
mod bn128;
mod hash;
mod modexp;

/// A helper function that creates a precompile that returns `PrecompileError::Other("Precompile not implemented".into())`
/// for a given address.
const fn precompile_not_implemented(address: Address) -> PrecompileWithAddress {
    PrecompileWithAddress(
        address,
        Precompile::Standard(|_input: &Bytes, _gas_limit: u64| {
            Err(PrecompileError::Other("NotImplemented: Precompile not implemented".into()).into())
        }),
    )
}

/// Load the precompiles for the given scroll spec.
pub fn load_precompiles<SPEC: ScrollSpec>() -> &'static Precompiles {
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

        if SPEC::scroll_enabled(crate::ScrollSpecId::BERNOULLI) {
            precompiles.extend([hash::sha256::SHA256_BERNOULLI]);
        }

        Box::new(precompiles)
    })
}
