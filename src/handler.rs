//! Handler related to Scroll chain.

use crate::{l1block::L1BlockInfo, transaction::ScrollTxTr, ScrollSpecId};
use std::{string::ToString, vec::Vec};

use revm::{
    context::{
        result::{EVMError, FromStringError, HaltReason, InvalidTransaction},
        Block, Cfg, ContextTr, Journal, Transaction,
    },
    handler::{
        post_execution, pre_execution, EvmTr, EvmTrError, Frame, FrameResult, Handler,
        MainnetHandler,
    },
    interpreter::{FrameInput, Gas},
    primitives::U256,
    state::EvmState,
};
use revm_primitives::Log;

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

pub trait IsTxError {
    fn is_tx_error(&self) -> bool;
}

impl<DB, TX> IsTxError for EVMError<DB, TX> {
    fn is_tx_error(&self) -> bool {
        matches!(self, EVMError::Transaction(_))
    }
}

/// Configure the handler for the Scroll chain.
///
/// The trait modifies the following handlers:
/// - `pre_execution.load_accounts` - Adds a hook to load `L1BlockInfo` from the database such that
///   it can be used to calculate the L1 cost of a transaction.
/// - `pre_execution.deduct_caller` - Overrides the logic to deduct the max transaction fee,
///   including the L1 fee, from the caller's balance.
/// - `post_execution.reward_beneficiary` - Overrides the logic to reward the beneficiary with the
///   gas fee.
impl<EVM, ERROR, FRAME> Handler for ScrollHandler<EVM, ERROR, FRAME>
where
    EVM: EvmTr<
        Context: ContextTr<
            Journal: Journal<FinalOutput = (EvmState, Vec<Log>)>,
            Tx: ScrollTxTr,
            Cfg: Cfg<Spec = ScrollSpecId>,
            Chain = L1BlockInfo,
        >,
    >,
    ERROR: EvmTrError<EVM> + FromStringError + IsTxError,
    FRAME: Frame<Evm = EVM, Error = ERROR, FrameResult = FrameResult, FrameInit = FrameInput>,
{
    type Evm = EVM;
    type Error = ERROR;
    type Frame = FRAME;
    type HaltReason = HaltReason;

    fn load_accounts(&self, evm: &mut Self::Evm) -> Result<(), Self::Error> {
        // only load the L1BlockInfo for txs that are not l1 messages.
        if !evm.ctx().tx().is_l1_msg() {
            let spec = evm.ctx().cfg().spec();
            let l1_block_info = L1BlockInfo::try_fetch(&mut evm.ctx().db(), spec)?;
            *evm.ctx().chain() = l1_block_info;
        }

        self.mainnet.load_accounts(evm)
    }

    fn deduct_caller(&self, evm: &mut Self::Evm) -> Result<(), Self::Error> {
        // load caller's account.
        let ctx = evm.ctx();
        let caller = ctx.tx().caller();
        let is_l1_msg = ctx.tx().is_l1_msg();
        let kind = ctx.tx().kind();
        let spec = ctx.cfg().spec();

        if !is_l1_msg {
            // We deduct caller max balance after minting and before deducing the
            // l1 cost, max values is already checked in pre_validate but l1 cost wasn't.
            pre_execution::deduct_caller(ctx)?;

            let l1_block_info = ctx.chain().clone();
            let Some(rlp_bytes) = ctx.tx().rlp_bytes() else {
                return Err(ERROR::from_string(
                    "[SCROLL] Failed to load transaction rlp_bytes.".to_string(),
                ));
            };
            // Deduct l1 fee from caller.
            let tx_l1_cost = l1_block_info.calculate_tx_l1_cost(rlp_bytes, spec);
            let caller_account = ctx.journal().load_account(caller)?;
            if tx_l1_cost.gt(&caller_account.info.balance) {
                return Err(InvalidTransaction::LackOfFundForMaxFee {
                    fee: tx_l1_cost.into(),
                    balance: caller_account.info.balance.into(),
                }
                .into());
            }
            caller_account.data.info.balance =
                caller_account.data.info.balance.saturating_sub(tx_l1_cost);
        } else {
            let caller_account = ctx.journal().load_account(caller)?;
            // bump the nonce for calls. Nonce for CREATE will be bumped in `handle_create`.
            if kind.is_call() {
                // Nonce is already checked
                caller_account.data.info.nonce = caller_account.data.info.nonce.saturating_add(1);
            }

            // touch account so we know it is changed.
            caller_account.data.mark_touch();
        }
        Ok(())
    }

    fn last_frame_result(
        &self,
        evm: &mut Self::Evm,
        frame_result: &mut <Self::Frame as Frame>::FrameResult,
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

    fn refund(
        &self,
        evm: &mut Self::Evm,
        exec_result: &mut <Self::Frame as Frame>::FrameResult,
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
        exec_result: &mut <Self::Frame as Frame>::FrameResult,
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
        let l1_block_info = ctx.chain().clone();
        let Some(rlp_bytes) = &ctx.tx().rlp_bytes() else {
            return Err(ERROR::from_string(
                "[SCROLL] Failed to load transaction rlp_bytes.".to_string(),
            ));
        };
        let l1_cost = l1_block_info.calculate_tx_l1_cost(rlp_bytes, ctx.cfg().spec());

        // reward the beneficiary with the gas fee including the L1 cost of the transaction and mark
        // the account as touched.
        let gas = exec_result.gas();
        let coinbase_account = ctx.journal().load_account(beneficiary)?;
        coinbase_account.data.info.balance = coinbase_account
            .data
            .info
            .balance
            .saturating_add(effective_gas_price * U256::from(gas.spent() - gas.refunded() as u64))
            .saturating_add(l1_cost);
        coinbase_account.data.mark_touch();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builder::{DefaultScrollContext, ScrollBuilder, ScrollContext},
        l1block::L1_GAS_PRICE_ORACLE_ADDRESS,
        transaction::L1_MESSAGE_TYPE,
    };
    use std::boxed::Box;

    use revm::{
        database::{DbAccount, InMemoryDB},
        handler::EthFrame,
        interpreter::{CallOutcome, InstructionResult, InterpreterResult},
        state::AccountInfo,
        Context,
    };
    use revm_primitives::{address, bytes, Address};

    const TX_L1_FEE_PRECISION: U256 = U256::from_limbs([1_000_000_000u64, 0, 0, 0]);
    const CALLER: Address = address!("0x000000000000000000000000000000000000dead");
    const TO: Address = address!("0x0000000000000000000000000000000000000001");
    const BENEFICIARY: Address = address!("0x0000000000000000000000000000000000000002");
    const MIN_TRANSACTION_COST: U256 = U256::from_limbs([21_000u64, 0, 0, 0]);
    const L1_DATA_COST: U256 = U256::from_limbs([4u64, 0, 0, 0]);

    fn context() -> ScrollContext<InMemoryDB> {
        Context::scroll()
            .modify_tx_chained(|tx| {
                tx.base.caller = CALLER;
                tx.base.kind = Some(TO).into();
                tx.base.gas_price = 1;
                tx.base.gas_limit = 21000;
                tx.base.gas_priority_fee = None;
                tx.rlp_bytes = Some(bytes!("01010101"));
            })
            .modify_block_chained(|block| block.beneficiary = BENEFICIARY)
            .with_db(InMemoryDB::default())
            .modify_db_chained(|db| {
                let _ = db.replace_account_storage(
                    L1_GAS_PRICE_ORACLE_ADDRESS,
                    (0..7)
                        .map(|n| (U256::from(n), U256::from(1)))
                        .chain(core::iter::once((U256::from(7), TX_L1_FEE_PRECISION)))
                        .collect(),
                );
            })
    }

    #[test]
    fn test_load_account() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context();
        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        handler.load_accounts(&mut evm)?;

        let l1_block_info = evm.ctx().chain.clone();
        assert_ne!(l1_block_info, L1BlockInfo::default());

        Ok(())
    }

    #[test]
    fn test_load_account_l1_message() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);
        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        handler.load_accounts(&mut evm)?;

        let l1_block_info = evm.ctx().chain.clone();
        assert_eq!(l1_block_info, L1BlockInfo::default());

        Ok(())
    }

    #[test]
    fn test_deduct_caller() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context().modify_db_chained(|db| {
            db.cache.accounts.insert(
                CALLER,
                DbAccount {
                    info: AccountInfo {
                        balance: MIN_TRANSACTION_COST + L1_DATA_COST,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
        });

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        handler.load_accounts(&mut evm)?;
        handler.deduct_caller(&mut evm)?;

        let caller_account = evm.ctx().journal().load_account(CALLER)?;
        assert_eq!(caller_account.info.balance, U256::ZERO);
        assert_eq!(caller_account.info.nonce, 1);

        Ok(())
    }

    #[test]
    fn test_deduct_caller_l1_message() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        handler.load_accounts(&mut evm)?;
        handler.deduct_caller(&mut evm)?;

        let caller_account = evm.ctx().journal().load_account(CALLER)?;
        assert_eq!(caller_account.info.balance, U256::ZERO);
        assert_eq!(caller_account.info.nonce, 1);

        Ok(())
    }

    #[test]
    fn test_last_frame_result() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context();

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
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
    fn test_last_frame_result_l1_message() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
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

        gas.set_refund(0);
        assert_eq!(result.gas(), &gas);

        Ok(())
    }
    #[test]
    fn test_refund() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context();

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
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
    fn test_refund_l1_message() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
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

        // gas should not have been updated
        assert_eq!(result.gas(), &gas);

        Ok(())
    }

    #[test]
    fn test_reward_beneficiary() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context();

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        let gas = Gas::new_spent(21000);
        let mut result = FrameResult::Call(CallOutcome::new(
            InterpreterResult {
                result: InstructionResult::Return,
                output: Default::default(),
                gas,
            },
            0..0,
        ));
        handler.load_accounts(&mut evm)?;
        handler.reward_beneficiary(&mut evm, &mut result)?;

        let beneficiary = evm.ctx().journal().load_account(BENEFICIARY)?;
        assert_eq!(beneficiary.info.balance, MIN_TRANSACTION_COST + L1_DATA_COST);

        Ok(())
    }

    #[test]
    fn test_reward_beneficiary_l1_message() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);

        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        let gas = Gas::new_spent(21000);
        let mut result = FrameResult::Call(CallOutcome::new(
            InterpreterResult {
                result: InstructionResult::Return,
                output: Default::default(),
                gas,
            },
            0..0,
        ));
        handler.load_accounts(&mut evm)?;
        handler.reward_beneficiary(&mut evm, &mut result)?;

        let beneficiary = evm.ctx().journal().load_account(BENEFICIARY)?;
        assert_eq!(beneficiary.info.balance, U256::ZERO);

        Ok(())
    }
}
