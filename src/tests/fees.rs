use crate::{
    builder::ScrollBuilder,
    handler::ScrollHandler,
    l1block::*,
    test_utils::{context, ScrollContextTestUtils, BENEFICIARY, CALLER},
    transaction::SYSTEM_ADDRESS,
    ScrollSpecId,
};
use revm::{
    context::{result::EVMError, ContextTr, JournalTr},
    handler::{EthFrame, EvmTr, FrameResult, Handler},
    interpreter::{CallOutcome, Gas, InstructionResult, InterpreterResult},
};
use revm_primitives::U256;
use std::{boxed::Box, vec};

#[test]
fn test_should_deduct_correct_fees_bernoulli() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context()
        .with_funds(U256::from(30_000))
        .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::BERNOULLI);
    let mut evm = ctx.clone().build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();

    handler.pre_execution(&mut evm).unwrap();

    let ctx = evm.ctx_mut();
    let caller_account = ctx.journal_mut().load_account(CALLER)?;

    // cost is 21k + 1012 (shanghai l1 cost).
    assert_eq!(caller_account.data.info.balance, U256::from(7988));

    Ok(())
}

#[test]
fn test_should_deduct_correct_fees_curie() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context()
        .with_funds(U256::from(70_000))
        .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::CURIE);
    let mut evm = ctx.clone().build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();

    handler.pre_execution(&mut evm).unwrap();

    let ctx = evm.ctx_mut();
    let caller_account = ctx.journal_mut().load_account(CALLER)?;

    // cost is 21k + 40k (curie l1 cost).
    assert_eq!(caller_account.data.info.balance, U256::from(9000));

    Ok(())
}

#[test]
fn test_no_rollup_fee_for_system_tx() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context()
        .with_funds(U256::from(70_000))
        .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::CURIE)
        .modify_tx_chained(|tx| {
            tx.base.caller = SYSTEM_ADDRESS;
            tx.base.gas_price = 0
        });

    let mut evm = ctx.clone().build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();

    handler.pre_execution(&mut evm).unwrap();

    let ctx = evm.ctx_mut();
    let caller_account = ctx.journal_mut().load_account(CALLER)?;

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
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
    let gas = Gas::new_spent(21000);
    let mut result = FrameResult::Call(CallOutcome::new(
        InterpreterResult { result: InstructionResult::Return, output: Default::default(), gas },
        0..0,
    ));
    handler.load_accounts(&mut evm)?;
    handler.reward_beneficiary(&mut evm, &mut result)?;

    // beneficiary receives gas (if any), but not rollup fee
    let ctx = evm.ctx_mut();
    let beneficiary = ctx.journal_mut().load_account(BENEFICIARY)?;
    assert_eq!(beneficiary.info.balance, U256::from(21000));

    Ok(())
}

#[test]
fn test_should_deduct_correct_fees_feynman() -> Result<(), Box<dyn core::error::Error>> {
    let initial_funds = U256::from(70_000);
    let compression_ratio = U256::from(5_000_000_000u64);
    let tx_payload = vec![0u8; 100];

    let gas_oracle = vec![
        (L1_BASE_FEE_SLOT, U256::from(1_000_000_000u64)),
        (L1_BLOB_BASE_FEE_SLOT, U256::from(1_000_000_000u64)),
        (L1_COMMIT_SCALAR_SLOT, U256::from(10)),
        (L1_BLOB_SCALAR_SLOT, U256::from(20)),
        (PENALTY_THRESHOLD_SLOT, U256::from(6_000_000_000u64)),
        (PENALTY_FACTOR_SLOT, U256::from(2_000_000_000u64)),
    ];

    let ctx = context()
        .with_funds(initial_funds)
        .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::FEYNMAN)
        .modify_tx_chained(|tx| tx.compression_ratio = Some(compression_ratio))
        .with_gas_oracle_config(gas_oracle)
        .with_tx_payload(tx_payload.into());

    let mut evm = ctx.clone().build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();

    handler.pre_execution(&mut evm).unwrap();

    let ctx = evm.ctx_mut();
    let caller_account = ctx.journal_mut().load_account(CALLER)?;

    // cost is 21k + 6k (applying 2x penalty).
    let balance_diff = initial_funds.saturating_sub(caller_account.data.info.balance);
    assert_eq!(balance_diff, U256::from(27000));

    Ok(())
}
