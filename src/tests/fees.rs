use crate::{
    builder::ScrollBuilder,
    handler::ScrollHandler,
    test_utils::{context, context_with_funds, BENEFICIARY, CALLER},
    transaction::SYSTEM_ADDRESS,
    ScrollSpecId,
};
use revm::{
    context::{result::EVMError, ContextTr, JournalTr},
    handler::{EthFrame, EvmTr, FrameResult, Handler},
    interpreter::{CallOutcome, Gas, InstructionResult, InterpreterResult},
};
use revm_primitives::U256;
use std::boxed::Box;

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

#[test]
fn test_no_rollup_fee_for_system_tx() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context_with_funds(U256::from(70_000))
        .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::CURIE)
        .modify_tx_chained(|tx| {
            tx.base.caller = SYSTEM_ADDRESS;
            tx.base.gas_price = 0
        });

    let mut evm = ctx.clone().build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();

    handler.pre_execution(&mut evm).unwrap();

    let caller_account = evm.ctx().journal().load_account(CALLER)?;

    // gas price is 0, no data fee => balance is unchanged.
    assert_eq!(caller_account.data.info.balance, U256::from(70_000));

    Ok(())
}

#[test]
fn test_reward_beneficiary_system_tx() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context()
        .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::CURIE)
        .modify_tx_chained(|tx| tx.base.caller = SYSTEM_ADDRESS);

    let mut evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
    let gas = Gas::new_spent(21000);
    let mut result = FrameResult::Call(CallOutcome::new(
        InterpreterResult { result: InstructionResult::Return, output: Default::default(), gas },
        0..0,
    ));
    handler.load_accounts(&mut evm)?;
    handler.reward_beneficiary(&mut evm, &mut result)?;

    // beneficiary receives gas (if any), but not rollup fee
    let beneficiary = evm.ctx().journal().load_account(BENEFICIARY)?;
    assert_eq!(beneficiary.info.balance, U256::from(21000));

    Ok(())
}
