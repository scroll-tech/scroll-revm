use crate::{
    builder::{FeynmanEipActivations, ScrollBuilder},
    handler::ScrollHandler,
    test_utils::context,
    ScrollSpecId,
};

use revm::{
    context::result::{EVMError, InvalidTransaction},
    handler::{EthFrame, Handler},
};
use revm_primitives::bytes;

#[test]
fn test_should_not_apply_eip7623_calldata_gas_for_euclid() {
    const GAS_LIMIT: u64 = 21_032;

    // initiate handler.
    let ctx = context()
        .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::EUCLID)
        .modify_tx_chained(|tx| tx.base.gas_limit = GAS_LIMIT)
        .maybe_with_eip_7623();
    let evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();

    // check call passes.
    let _ = handler.validate_initial_tx_gas(&evm).unwrap();
}

#[test]
fn test_should_apply_eip7623_calldata_gas_for_feynman() {
    const GAS_LIMIT: u64 = 21_032;
    const GAS_FLOOR: u64 = 21_080;

    // initiate handler.
    let ctx = context()
        .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::FEYNMAN)
        .modify_tx_chained(|tx| {
            tx.base.gas_limit = GAS_LIMIT;
            tx.base.data = bytes!("0xdead");
        })
        .maybe_with_eip_7623();
    let evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();

    // check call errors on gas floor more than gas limit.
    let err = handler.validate_initial_tx_gas(&evm).unwrap_err();
    assert_eq!(
        err,
        EVMError::Transaction(InvalidTransaction::GasFloorMoreThanGasLimit {
            gas_limit: GAS_LIMIT,
            gas_floor: GAS_FLOOR
        })
    )
}
