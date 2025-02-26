//! Handler related to Scroll chain.

use crate::{transaction::ScrollTxTr, L1BlockInfo, ScrollSpecId};
use std::mem;

use primitives::Log;
use revm::{
    context::{
        result::{EVMError, FromStringError, HaltReason, InvalidTransaction, ResultAndState},
        Block, Cfg, ContextTr, Journal, Transaction,
    },
    handler::{
        post_execution, pre_execution, EvmTr, EvmTrError, Frame, FrameResult, Handler,
        MainnetHandler,
    },
    interpreter::FrameInput,
    primitives::{TxKind, U256},
    state::EvmState,
};

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
/// - `pre_execution.load_precompiles` - Overrides the logic to load the precompiles for the Scroll
///   chain.
/// - `post_execution.reward_beneficiary` - Overrides the logic to reward the beneficiary with the
///   gas fee.
impl<EVM, ERROR, FRAME> Handler for ScrollHandler<EVM, ERROR, FRAME>
where
    EVM: EvmTr<
        Context: ContextTr<
            Journal: Journal<FinalOutput = (EvmState, Vec<Log>)>,
            Tx: ScrollTxTr,
            Cfg: Cfg<Spec = ScrollSpecId>,
            Chain = Option<L1BlockInfo>,
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
            *evm.ctx().chain() = Some(l1_block_info);
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

            let l1_block_info =
                ctx.chain().as_ref().cloned().expect("L1BlockInfo should be loaded");
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
            if matches!(kind, TxKind::Call(_)) {
                // Nonce is already checked
                caller_account.data.info.nonce = caller_account.data.info.nonce.saturating_add(1);
            }

            // touch account so we know it is changed.
            caller_account.data.mark_touch();
        }
        Ok(())
    }

    fn reward_beneficiary(
        &self,
        evm: &mut Self::Evm,
        exec_result: &mut <Self::Frame as Frame>::FrameResult,
    ) -> Result<(), Self::Error> {
        let ctx = evm.ctx();

        // If the transaction is an L1 message, we do not need to reward the beneficiary as the
        // transaction has already been payed for on L1.
        if ctx.tx().is_l1_msg() {
            return Ok(());
        }

        // fetch the effective gas price.
        let block = ctx.block();
        let effective_gas_price = U256::from(ctx.tx().effective_gas_price(block.basefee() as u128));

        // load beneficiary's account.
        let beneficiary = block.beneficiary();

        // calculate the L1 cost of the transaction.
        let Some(l1_block_info) = ctx.chain().as_ref().cloned() else {
            return Err(ERROR::from_string(
                "[SCROLL] Failed to load L1 block information.".to_string(),
            ));
        };
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

    fn output(
        &self,
        evm: &mut Self::Evm,
        mut result: <Self::Frame as Frame>::FrameResult,
    ) -> Result<ResultAndState<Self::HaltReason>, Self::Error> {
        let ctx = evm.ctx();
        mem::replace(ctx.error(), Ok(()))?;

        // l1 messages do not get gas refunded.
        if ctx.tx().is_l1_msg() {
            let refund = result.gas().refunded();
            let spent = result.gas().spent();
            result.gas_mut().set_refund(0);
            result.gas_mut().set_spent(spent + refund as u64);
        }

        Ok(post_execution::output(ctx, result))
    }
}
