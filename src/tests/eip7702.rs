use crate::{builder::ScrollBuilder, handler::ScrollHandler, test_utils::context};

use revm::{
    context::{
        either::Either,
        result::EVMError,
        transaction::{Authorization, SignedAuthorization},
        TransactionType,
    },
    handler::{EthFrame, Handler},
};
use revm_primitives::{eip7702, U256};

#[test]
fn test_validate_initial_gas_eip7702() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context();
    let evm = ctx.clone().build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
    let gas_empty_authorization_list = handler.validate_initial_tx_gas(&evm)?;

    let evm = ctx
        .modify_tx_chained(|tx| {
            tx.base.gas_limit += eip7702::PER_EMPTY_ACCOUNT_COST;
            tx.base.authorization_list = vec![Either::Left(SignedAuthorization::new_unchecked(
                Authorization {
                    chain_id: Default::default(),
                    address: Default::default(),
                    nonce: 0,
                },
                0,
                U256::ZERO,
                U256::ZERO,
            ))]
        })
        .build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
    let gas_with_authorization_list = handler.validate_initial_tx_gas(&evm)?;

    assert_eq!(
        gas_empty_authorization_list.initial_gas + eip7702::PER_EMPTY_ACCOUNT_COST,
        gas_with_authorization_list.initial_gas
    );

    Ok(())
}

#[test]
fn test_validate_env_eip7702() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context().modify_tx_chained(|tx| {
        tx.base.tx_type = TransactionType::Eip7702 as u8;
        tx.base.authorization_list = vec![Either::Left(SignedAuthorization::new_unchecked(
            Authorization { chain_id: Default::default(), address: Default::default(), nonce: 0 },
            0,
            U256::ZERO,
            U256::ZERO,
        ))]
    });
    let mut evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
    handler.validate_env(&mut evm)?;

    Ok(())
}

#[test]
fn test_apply_eip7702_auth_list() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context().modify_tx_chained(|tx| {
        tx.base.tx_type = TransactionType::Eip7702 as u8;
    });
    let mut evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
    handler.apply_eip7702_auth_list(&mut evm)?;

    Ok(())
}
