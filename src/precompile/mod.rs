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
            precompile::secp256k1::ECRECOVER, // 0x01
            hash::SHA256_SHANGHAI,            // 0x02
            hash::RIPEMD160_SHANGHAI,         // 0x03
            precompile::identity::FUN,        // 0x04
            modexp::BERNOULLI,                // 0x05
            precompile::bn128::add::ISTANBUL, // 0x06
            precompile::bn128::mul::ISTANBUL, // 0x07
            bn128::pair::BERNOULLI,           // 0x08
            blake2::SHANGHAI,                 // 0x09
        ]);

        if SPEC::scroll_enabled(crate::ScrollSpecId::BERNOULLI) {
            precompiles.extend([hash::SHA256_BERNOULLI]);
        }

        Box::new(precompiles)
    })
}
