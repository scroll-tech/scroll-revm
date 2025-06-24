use crate::{
    builder::ScrollBuilder,
    handler::ScrollHandler,
    test_utils::{context_with_funds, CALLER},
    ScrollSpecId,
};

use revm::{
    context::{result::EVMError, ContextTr, JournalTr},
    handler::{EthFrame, EvmTr, Handler},
};
use revm_primitives::U256;

#[test]
fn test_should_deduct_correct_fees_bernoulli() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context_with_funds(U256::from(30_000))
        .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::BERNOULLI);
    let mut evm = ctx.clone().build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();

    handler.pre_execution(&mut evm).unwrap();

    let caller_account = evm.ctx().journal().load_account(CALLER)?;

    // cost is 21k + 1012 (shanghai l1 cost).
    assert_eq!(caller_account.data.info.balance, U256::from(7988));

    Ok(())
}

#[test]
fn test_should_deduct_correct_fees_curie() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context_with_funds(U256::from(70_000))
        .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::CURIE);
    let mut evm = ctx.clone().build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();

    handler.pre_execution(&mut evm).unwrap();

    let caller_account = evm.ctx().journal().load_account(CALLER)?;

    // cost is 21k + 40k (curie l1 cost).
    assert_eq!(caller_account.data.info.balance, U256::from(9000));

    Ok(())
}
