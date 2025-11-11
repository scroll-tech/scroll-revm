//! Handler related to Scroll chain.

use crate::{exec::ScrollContextTr, l1block::L1BlockInfo, transaction::ScrollTxTr, ScrollSpecId};
use std::{boxed::Box, string::ToString};

use revm::{
    bytecode::Bytecode,
    context::{
        result::{HaltReason, InvalidTransaction},
        Block, Cfg, ContextTr, JournalTr, Transaction,
    },
    handler::{
        post_execution, EthFrame, EvmTr, EvmTrError, FrameResult, FrameTr, Handler, MainnetHandler,
    },
    interpreter::{
        interpreter::EthInterpreter, interpreter_action::FrameInit, Gas, InitialAndFloorGas,
    },
    primitives::U256,
};
use revm_inspector::{Inspector, InspectorEvmTr, InspectorHandler};

/// The Scroll handler.
pub struct ScrollHandler<EVM, ERROR, FRAME> {
    pub mainnet: MainnetHandler<EVM, ERROR, FRAME>,
}

impl<EVM, ERROR, FRAME> ScrollHandler<EVM, ERROR, FRAME> {
    pub fn new() -> Self {
        Self { mainnet: MainnetHandler::default() }
    }
}

impl<EVM, ERROR, FRAME> Default for ScrollHandler<EVM, ERROR, FRAME> {
    fn default() -> Self {
        Self::new()
    }
}

/// Configure the handler for the Scroll chain.
///
/// The trait modifies the following handlers:
/// - `pre_execution` - Adds a hook to load `L1BlockInfo` from the database such that it can be used
///   to calculate the L1 cost of a transaction.
/// - `validate_against_state_and_deduct_caller` - Overrides the logic to deduct the max transaction
///   fee, including the L1 fee, from the caller's balance.
/// - `last_frame_result` - Overrides the logic for gas refund in the case the transaction is a L1
///   message.
/// - `refund` - Overrides the logic for gas refund in the case the transaction is a L1 message.
/// - `post_execution.reward_beneficiary` - Overrides the logic to reward the beneficiary with the
///   gas fee and skip rewarding in case the transaction is a L1 message.
impl<EVM, ERROR, FRAME> Handler for ScrollHandler<EVM, ERROR, FRAME>
where
    EVM: EvmTr<Context: ScrollContextTr, Frame = FRAME>,
    ERROR: EvmTrError<EVM> + From<InvalidTransaction>,
    FRAME: FrameTr<FrameResult = FrameResult, FrameInit = FrameInit>,
{
    type Evm = EVM;
    type Error = ERROR;
    type HaltReason = HaltReason;

    #[inline]
    fn pre_execution(&self, evm: &mut Self::Evm) -> Result<u64, Self::Error> {
        // only load the L1BlockInfo for txs that are not l1 messages.
        if !evm.ctx().tx().is_l1_msg() && !evm.ctx().tx().is_system_tx() {
            let spec = evm.ctx().cfg().spec();
            let l1_block_info = L1BlockInfo::try_fetch(evm.ctx().db_mut(), spec)?;
            *evm.ctx().chain_mut() = l1_block_info;
        }

        self.validate_against_state_and_deduct_caller(evm)?;
        self.load_accounts(evm)?;
        let gas = self.apply_eip7702_auth_list(evm)?;
        Ok(gas)
    }

    #[inline]
    fn validate_against_state_and_deduct_caller(
        &self,
        evm: &mut Self::Evm,
    ) -> Result<(), Self::Error> {
        // load caller's account.
        let ctx_ref = evm.ctx_ref();
        let caller = ctx_ref.tx().caller();
        let is_l1_msg = ctx_ref.tx().is_l1_msg();
        let is_system_tx = ctx_ref.tx().is_system_tx();
        let spec = ctx_ref.cfg().spec();
        let is_eip3607_disabled = ctx_ref.cfg().is_eip3607_disabled();

        // execute normal checks and transaction processing logic for non-l1-msgs
        if !is_l1_msg {
            // We deduct caller max balance after minting and before deducing the
            // l1 cost, max values is already checked in pre_validate but l1 cost wasn't.
            self.mainnet.validate_against_state_and_deduct_caller(evm)?;
        }

        // process rollup fee
        let ctx = evm.ctx();
        if !is_l1_msg && !is_system_tx {
            let l1_block_info = ctx.chain().clone();
            let Some(rlp_bytes) = ctx.tx().rlp_bytes() else {
                return Err(ERROR::from_string(
                    "[SCROLL] Failed to load transaction rlp_bytes.".to_string(),
                ));
            };

            // Deduct l1 fee from caller.
            let tx_l1_cost = l1_block_info.calculate_tx_l1_cost(
                rlp_bytes,
                spec,
                ctx.tx().compression_ratio(),
                ctx.tx().compressed_size(),
            );
            let caller_account = ctx.journal_mut().load_account(caller)?;
            if tx_l1_cost.gt(&caller_account.info.balance) {
                return Err(InvalidTransaction::LackOfFundForMaxFee {
                    fee: tx_l1_cost.into(),
                    balance: caller_account.info.balance.into(),
                }
                .into());
            }
            caller_account.data.info.balance =
                caller_account.data.info.balance.saturating_sub(tx_l1_cost);
        }

        // execute l1 msg checks
        if is_l1_msg {
            // Load caller's account.
            let (tx, journal) = ctx.tx_journal_mut();
            let mut caller_account = journal.load_account(caller)?;

            // Note: we skip the balance check at pre-execution level if the transaction is a
            // L1 message and Euclid is enabled. This means the L1 message will reach execution
            // stage in revm and revert with `OutOfFunds` in the first frame, but still be included
            // in the block.
            let skip_balance_check = tx.is_l1_msg() && spec.is_enabled_in(ScrollSpecId::EUCLID);
            if !skip_balance_check {
                let max_balance_spending = tx.max_balance_spending()?;
                if max_balance_spending > caller_account.info.balance {
                    return Err(InvalidTransaction::LackOfFundForMaxFee {
                        fee: Box::new(max_balance_spending),
                        balance: Box::new(caller_account.info.balance),
                    }
                    .into());
                }
            }

            // EIP-3607: Reject transactions from senders with deployed code.
            //
            // We check the sender of the L1 message is a EOA on the L2.
            // If the sender is a (delegated) EOA on the L1, it should be a (delegated) EOA
            // on the L2.
            // If the sender is a contract on the L1, address aliasing assures with high probability
            // that the L2 sender would be an EOA.
            if !is_eip3607_disabled {
                let caller_info = &caller_account.info;
                let bytecode = match caller_info.code.as_ref() {
                    Some(bytecode) => bytecode,
                    None => &Bytecode::default(),
                };
                // Allow EOAs whose code is a valid delegation designation,
                // i.e. 0xef0100 || address, to continue to originate transactions.
                if !bytecode.is_empty() && !bytecode.is_eip7702() {
                    return Err(InvalidTransaction::RejectCallerWithCode.into());
                }
            }

            // Bump the nonce for calls. Nonce for CREATE will be bumped in `make_create_frame`.
            if tx.kind().is_call() {
                // Nonce is already checked
                caller_account.info.nonce = caller_account.info.nonce.saturating_add(1);
            }
            // touch account so we know it is changed.
            caller_account.data.mark_touch();
        }
        Ok(())
    }

    #[inline]
    fn last_frame_result(
        &mut self,
        evm: &mut Self::Evm,
        frame_result: &mut <<Self::Evm as EvmTr>::Frame as FrameTr>::FrameResult,
    ) -> Result<(), Self::Error> {
        let instruction_result = frame_result.interpreter_result().result;
        let gas = frame_result.gas_mut();
        let remaining = gas.remaining();
        let refunded = gas.refunded();

        // Spend the gas limit. Gas is reimbursed when the tx returns successfully.
        *gas = Gas::new_spent(evm.ctx().tx().gas_limit());

        if instruction_result.is_ok_or_revert() {
            gas.erase_cost(remaining);
        }

        // do not refund l1 messages.
        if !evm.ctx().tx().is_l1_msg() && instruction_result.is_ok() {
            gas.record_refund(refunded);
        }

        Ok(())
    }

    #[inline]
    fn eip7623_check_gas_floor(
        &self,
        evm: &mut Self::Evm,
        exec_result: &mut <<Self::Evm as EvmTr>::Frame as FrameTr>::FrameResult,
        init_and_floor_gas: InitialAndFloorGas,
    ) {
        // skip floor gas check for l1 messages.
        if evm.ctx().tx().is_l1_msg() {
            return;
        }
        self.mainnet.eip7623_check_gas_floor(evm, exec_result, init_and_floor_gas)
    }

    #[inline]
    fn refund(
        &self,
        evm: &mut Self::Evm,
        exec_result: &mut <<Self::Evm as EvmTr>::Frame as FrameTr>::FrameResult,
        eip7702_refund: i64,
    ) {
        // skip refund for l1 messages
        if evm.ctx().tx().is_l1_msg() {
            return;
        }
        let spec = evm.ctx().cfg().spec().into();
        post_execution::refund(spec, exec_result.gas_mut(), eip7702_refund)
    }

    fn reward_beneficiary(
        &self,
        evm: &mut Self::Evm,
        exec_result: &mut <<Self::Evm as EvmTr>::Frame as FrameTr>::FrameResult,
    ) -> Result<(), Self::Error> {
        let ctx = evm.ctx();

        // If the transaction is an L1 message, we do not need to reward the beneficiary as the
        // transaction has already been paid for on L1.
        if ctx.tx().is_l1_msg() {
            return Ok(());
        }

        // fetch the effective gas price.
        let block = ctx.block();
        let effective_gas_price = U256::from(ctx.tx().effective_gas_price(block.basefee() as u128));

        // load beneficiary's account.
        let beneficiary = block.beneficiary();

        // calculate the L1 cost of the transaction.
        let l1_cost = if !ctx.tx().is_system_tx() {
            let l1_block_info = ctx.chain().clone();
            let Some(rlp_bytes) = &ctx.tx().rlp_bytes() else {
                return Err(ERROR::from_string(
                    "[SCROLL] Failed to load transaction rlp_bytes.".to_string(),
                ));
            };
            l1_block_info.calculate_tx_l1_cost(
                rlp_bytes,
                ctx.cfg().spec(),
                ctx.tx().compression_ratio(),
                ctx.tx().compressed_size(),
            )
        } else {
            U256::from(0)
        };

        // reward the beneficiary with the gas fee including the L1 cost of the transaction and mark
        // the account as touched.
        let gas = exec_result.gas();

        let reward =
            effective_gas_price.saturating_mul(U256::from(gas.used())).saturating_add(l1_cost);
        ctx.journal_mut().balance_incr(beneficiary, reward)?;

        Ok(())
    }
}

impl<EVM, ERROR> InspectorHandler for ScrollHandler<EVM, ERROR, EthFrame<EthInterpreter>>
where
    EVM: InspectorEvmTr<
        Context: ScrollContextTr,
        Frame = EthFrame<EthInterpreter>,
        Inspector: Inspector<<<Self as Handler>::Evm as EvmTr>::Context, EthInterpreter>,
    >,
    ERROR: EvmTrError<EVM>,
{
    type IT = EthInterpreter;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builder::ScrollBuilder,
        test_utils::{
            context, ScrollContextTestUtils, BENEFICIARY, CALLER, L1_DATA_COST,
            MIN_TRANSACTION_COST,
        },
    };
    use std::boxed::Box;

    use revm::{
        context::result::EVMError,
        handler::EthFrame,
        interpreter::{CallOutcome, InstructionResult, InterpreterResult},
    };

    #[test]
    fn test_validate_lacking_funds() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context();
        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
        let err = handler.validate_against_state_and_deduct_caller(&mut evm).unwrap_err();
        assert_eq!(
            err,
            EVMError::Transaction(InvalidTransaction::LackOfFundForMaxFee {
                fee: Box::new(U256::from(21000)),
                balance: Box::default()
            })
        );

        Ok(())
    }

    #[test]
    fn test_load_account() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context().with_funds(MIN_TRANSACTION_COST + L1_DATA_COST);
        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
        handler.pre_execution(&mut evm)?;

        let l1_block_info = evm.ctx().chain.clone();
        assert_ne!(l1_block_info, L1BlockInfo::default());

        Ok(())
    }

    #[test]
    fn test_deduct_caller() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context().with_funds(MIN_TRANSACTION_COST + L1_DATA_COST);

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
        handler.pre_execution(&mut evm)?;

        let ctx = evm.ctx_mut();
        let caller_account = ctx.journal_mut().load_account(CALLER)?;
        assert_eq!(caller_account.info.balance, U256::ZERO);
        assert_eq!(caller_account.info.nonce, 1);

        Ok(())
    }

    #[test]
    fn test_last_frame_result() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context();

        let mut evm = ctx.build_scroll();
        let mut handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
        let mut gas = Gas::new(21000);
        gas.set_refund(10);
        gas.set_spent(10);
        let mut result = FrameResult::Call(CallOutcome::new(
            InterpreterResult {
                result: InstructionResult::Return,
                output: Default::default(),
                gas,
            },
            0..0,
        ));
        handler.last_frame_result(&mut evm, &mut result)?;

        assert_eq!(result.gas(), &gas);

        Ok(())
    }

    #[test]
    fn test_refund() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context();

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
        let mut gas = Gas::new(21000);
        gas.set_refund(10);
        gas.set_spent(10);
        let mut result = FrameResult::Call(CallOutcome::new(
            InterpreterResult {
                result: InstructionResult::Return,
                output: Default::default(),
                gas,
            },
            0..0,
        ));
        handler.refund(&mut evm, &mut result, 0);

        gas.set_refund(2);
        assert_eq!(result.gas(), &gas);

        Ok(())
    }

    #[test]
    fn test_reward_beneficiary() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context().with_funds(MIN_TRANSACTION_COST + L1_DATA_COST);

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
        let gas = Gas::new_spent(21000);
        let mut result = FrameResult::Call(CallOutcome::new(
            InterpreterResult {
                result: InstructionResult::Return,
                output: Default::default(),
                gas,
            },
            0..0,
        ));
        handler.pre_execution(&mut evm)?;
        handler.reward_beneficiary(&mut evm, &mut result)?;

        let ctx = evm.ctx_mut();
        let beneficiary = ctx.journal_mut().load_account(BENEFICIARY)?;
        assert_eq!(beneficiary.info.balance, MIN_TRANSACTION_COST + L1_DATA_COST);

        Ok(())
    }

    #[test]
    fn test_transaction_pre_execution() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context().with_funds(MIN_TRANSACTION_COST + L1_DATA_COST);

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_>>::new();
        handler.pre_execution(&mut evm)?;

        Ok(())
    }
}
