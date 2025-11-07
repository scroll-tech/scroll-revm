use crate::{
    builder::ScrollBuilder,
    handler::ScrollHandler,
    l1block::L1BlockInfo,
    test_utils::{context, BENEFICIARY, CALLER},
    transaction::L1_MESSAGE_TYPE,
};
use std::boxed::Box;

use crate::test_utils::MIN_TRANSACTION_COST;
use revm::{
    bytecode::LegacyRawBytecode,
    context::{
        result::{EVMError, ExecutionResult, HaltReason, InvalidTransaction, ResultAndState},
        ContextTr, JournalTr,
    },
    handler::{EthFrame, EvmTr, FrameResult, Handler},
    interpreter::{CallOutcome, Gas, InstructionResult, InterpreterResult},
    state::Bytecode,
    ExecuteEvm,
};
use revm_primitives::U256;

#[test]
fn test_l1_message_validate_lacking_funds() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);
    let mut evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();

    // pre execution includes fees deduction, which should be skipped for l1 messages.
    handler.pre_execution(&mut evm)?;

    Ok(())
}

#[test]
fn test_l1_message_load_accounts() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);
    let mut evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
    handler.load_accounts(&mut evm)?;

    // l1 block info should not be loaded for l1 messages.
    let l1_block_info = evm.ctx().chain.clone();
    assert_eq!(l1_block_info, L1BlockInfo::default());

    Ok(())
}

#[test]
fn test_l1_message_should_not_deduct_caller() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);

    let mut evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
    handler.load_accounts(&mut evm)?;
    handler.validate_against_state_and_deduct_caller(&mut evm)?;

    // nonce should be increase and caller should have same balance as the start (0).
    let ctx = evm.ctx_mut();
    let caller_account = ctx.journal_mut().load_account(CALLER)?;
    assert_eq!(caller_account.info.balance, U256::ZERO);
    assert_eq!(caller_account.info.nonce, 1);

    Ok(())
}

#[test]
fn test_l1_message_last_frame_result() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);

    let mut evm = ctx.build_scroll();
    let mut handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
    let mut gas = Gas::new(21000);
    gas.set_refund(10);
    gas.set_spent(10);
    let mut result = FrameResult::Call(CallOutcome::new(
        InterpreterResult { result: InstructionResult::Return, output: Default::default(), gas },
        0..0,
    ));
    handler.last_frame_result(&mut evm, &mut result)?;

    // refund should be 0 for l1 messages.
    gas.set_refund(0);
    assert_eq!(result.gas(), &gas);

    Ok(())
}

#[test]
fn test_l1_message_should_not_refund() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);

    let mut evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
    let mut gas = Gas::new(21000);
    gas.set_refund(10);
    gas.set_spent(10);
    let mut result = FrameResult::Call(CallOutcome::new(
        InterpreterResult { result: InstructionResult::Return, output: Default::default(), gas },
        0..0,
    ));
    handler.refund(&mut evm, &mut result, 0);

    // gas should not have been updated
    assert_eq!(result.gas(), &gas);

    Ok(())
}

#[test]
fn test_l1_message_should_not_reward_beneficiary() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);

    let mut evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
    let gas = Gas::new_spent(21000);
    let mut result = FrameResult::Call(CallOutcome::new(
        InterpreterResult { result: InstructionResult::Return, output: Default::default(), gas },
        0..0,
    ));
    handler.load_accounts(&mut evm)?;
    handler.reward_beneficiary(&mut evm, &mut result)?;

    // beneficiary should not see his balance increased for l1 message execution.
    let ctx = evm.ctx_mut();
    let beneficiary = ctx.journal_mut().load_account(BENEFICIARY)?;
    assert_eq!(beneficiary.info.balance, U256::ZERO);

    Ok(())
}

#[test]
fn test_l1_message_should_revert_with_out_of_funds() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context().modify_tx_chained(|tx| {
        tx.base.tx_type = L1_MESSAGE_TYPE;
        tx.base.value = U256::ONE;
    });
    let tx = ctx.tx.clone();
    let mut evm = ctx.build_scroll();

    let ResultAndState { result, .. } = evm.transact(tx)?;

    // L1 message should pass pre-execution but revert with `OutOfFunds`.
    assert_eq!(
        result,
        ExecutionResult::Halt {
            gas_used: MIN_TRANSACTION_COST.to(),
            reason: HaltReason::OutOfFunds
        }
    );

    Ok(())
}

#[test]
fn test_l1_message_should_pass_validation() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context()
        .modify_tx_chained(|tx| {
            tx.base.tx_type = L1_MESSAGE_TYPE;
            tx.base.value = U256::ONE;
            tx.base.gas_price = 0;
        })
        // set the base fee of the block above the L1 message gas price to check it passes.
        .modify_block_chained(|block| block.basefee = 100);
    let mut evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();

    handler.validate(&mut evm)?;

    Ok(())
}

#[test]
fn test_l1_message_should_pass_pre_execution() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context()
        .modify_tx_chained(|tx| {
            tx.base.tx_type = L1_MESSAGE_TYPE;
        })
        // set the caller nonce to 1 and check pre execution passes.
        .modify_journal_chained(|journal| {
            let mut caller = journal.load_account_mut(CALLER).unwrap().data;
            caller.bump_nonce();
        });
    let mut evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();

    handler.pre_execution(&mut evm)?;

    Ok(())
}

#[test]
fn test_l1_message_eip_3607() -> Result<(), Box<dyn core::error::Error>> {
    let ctx = context()
        .modify_tx_chained(|tx| {
            tx.base.tx_type = L1_MESSAGE_TYPE;
        })
        // set the caller nonce to 1 and check pre execution passes.
        .modify_journal_chained(|journal| {
            let mut caller = journal.load_account_mut(CALLER).unwrap().data;
            let hash = revm_primitives::keccak256([1u8; 2]);
            caller.set_code(
                hash,
                Bytecode::LegacyAnalyzed(LegacyRawBytecode([1u8; 2].into()).into_analyzed()),
            )
        });
    let mut evm = ctx.build_scroll();
    let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();

    let err = handler.pre_execution(&mut evm).unwrap_err();
    assert_eq!(err, EVMError::Transaction(InvalidTransaction::RejectCallerWithCode));

    Ok(())
}
