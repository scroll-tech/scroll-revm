//! Handler related to Scroll chain.

use crate::{exec::ScrollContextTr, l1block::L1BlockInfo, transaction::ScrollTxTr, ScrollSpecId};
use std::string::ToString;

use revm::{
    bytecode::Bytecode,
    context::{
        result::{EVMError, HaltReason, InvalidTransaction},
        transaction::AuthorizationTr,
        Block, Cfg, ContextTr, JournalTr, Transaction, TransactionType,
    },
    handler::{
        post_execution, pre_execution, validation, validation::validate_priority_fee_tx, EvmTr,
        EvmTrError, Frame, FrameResult, Handler, MainnetHandler,
    },
    interpreter::{gas, interpreter::EthInterpreter, FrameInput, Gas, InitialAndFloorGas},
    primitives::U256,
};
use revm_inspector::{Inspector, InspectorEvmTr, InspectorFrame, InspectorHandler};
use revm_primitives::{eip7702, hardfork::SpecId, KECCAK_EMPTY};

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

pub trait MatchesInvalidTransactionVariantError {
    fn matches_variant(&self, invalid_transaction: InvalidTransaction) -> bool;
}

impl<DB> MatchesInvalidTransactionVariantError for EVMError<DB> {
    fn matches_variant(&self, invalid_transaction: InvalidTransaction) -> bool {
        if let EVMError::Transaction(tx_err) = self {
            return core::mem::discriminant(tx_err) == core::mem::discriminant(&invalid_transaction);
        }
        false
    }
}

/// Configure the handler for the Scroll chain.
///
/// The trait modifies the following handlers:
/// - `validate` - Catches any `LackOfFundForMaxFee` error emitted from `validate_tx_against_state`
///   and verifies if the target transaction is a L1 message, in which case it ignores the error.
/// - `validate_initial_tx_gas` - Adds the EIP-7702 gas due to the authorization list.
/// - `validate_env` - Catches any `Eip7702NotSupported` error emitted from `validate_env` and
///   proceeds with the code from `validation::validate_env`, swapping the `SpecId` for
///   `ScrollSpecId`.
/// - `load_accounts` - Adds a hook to load `L1BlockInfo` from the database such that it can be used
///   to calculate the L1 cost of a transaction.
/// - `apply_eip7702_auth_list` - Remove the conversion of the ScrollSpecId to a SpecId, which
///   required the Euclid hardfork to convert to Prague.
/// - `deduct_caller` - Overrides the logic to deduct the max transaction fee, including the L1 fee,
///   from the caller's balance.
/// - `last_frame_result` - Overrides the logic for gas refund in the case the transaction is a L1
///   message.
/// - `refund` - Overrides the logic for gas refund in the case the transaction is a L1 message.
/// - `post_execution.reward_beneficiary` - Overrides the logic to reward the beneficiary with the
///   gas fee and skip rewarding in case the transaction is a L1 message.
impl<EVM, ERROR, FRAME> Handler for ScrollHandler<EVM, ERROR, FRAME>
where
    EVM: EvmTr<Context: ScrollContextTr>,
    ERROR: EvmTrError<EVM> + MatchesInvalidTransactionVariantError + From<InvalidTransaction>,
    FRAME: Frame<Evm = EVM, Error = ERROR, FrameResult = FrameResult, FrameInit = FrameInput>,
{
    type Evm = EVM;
    type Error = ERROR;
    type Frame = FRAME;
    type HaltReason = HaltReason;

    #[inline]
    fn validate(&self, evm: &mut Self::Evm) -> Result<InitialAndFloorGas, Self::Error> {
        self.validate_env(evm)?;
        let initial_and_floor_gas = self.validate_initial_tx_gas(evm)?;
        let res = self.validate_tx_against_state(evm);

        let default_lack_of_funds_for_max_fee_error = InvalidTransaction::LackOfFundForMaxFee {
            fee: Box::default(),
            balance: Box::default(),
        };
        let is_lack_of_funds_error = res
            .as_ref()
            .err()
            .map(|err| err.matches_variant(default_lack_of_funds_for_max_fee_error))
            .unwrap_or(false);
        let should_skip_lack_of_funds_error = evm.ctx().tx().is_l1_msg() &&
            evm.ctx().cfg().spec().is_enabled_in(ScrollSpecId::EUCLID);

        // if the error is not a `LackOfFundForMaxFee` or if we shouldn't skip lack of funds error,
        // propagate the error.
        if !is_lack_of_funds_error || !should_skip_lack_of_funds_error {
            res?;
        }

        Ok(initial_and_floor_gas)
    }

    #[inline]
    fn validate_initial_tx_gas(&self, evm: &Self::Evm) -> Result<InitialAndFloorGas, Self::Error> {
        let ctx = evm.ctx_ref();
        let tx = ctx.tx();
        let scroll_spec = ctx.cfg().spec();
        let spec = ctx.cfg().spec().into();

        let mut gas = gas::calculate_initial_tx_gas_for_tx(tx, spec);

        // Add EIP-7702 gas which was skipped due to SpecId::PRAGUE not being active for
        // ScrollSpecId::EUCLID.
        if scroll_spec.is_enabled_in(ScrollSpecId::EUCLID) {
            let authorization_list_num = tx.authorization_list_len() as u64;
            gas.initial_gas += authorization_list_num * eip7702::PER_EMPTY_ACCOUNT_COST;
        }

        // Additional check to see if limit is big enough to cover initial gas.
        if gas.initial_gas > tx.gas_limit() {
            return Err(InvalidTransaction::CallGasCostMoreThanGasLimit {
                gas_limit: tx.gas_limit(),
                initial_gas: gas.initial_gas,
            }
            .into());
        }

        // EIP-7623: Increase calldata cost
        // floor gas should be less than gas limit.
        if spec.is_enabled_in(SpecId::PRAGUE) && gas.floor_gas > tx.gas_limit() {
            return Err(InvalidTransaction::GasFloorMoreThanGasLimit {
                gas_floor: gas.floor_gas,
                gas_limit: tx.gas_limit(),
            }
            .into());
        };

        Ok(gas)
    }

    #[inline]
    fn validate_env(&self, evm: &mut Self::Evm) -> Result<(), Self::Error> {
        let res = validation::validate_env(evm.ctx());

        // In the case of the `Eip7702NotSupported` error, we duplicate the code from
        // `validation::validate_tx_env` here, replacing the check on SpecId::PRAGUE by
        // ScrollSpecId::EUCLID.
        let is_eip_7702_not_supported_error = res
            .as_ref()
            .err()
            .map(|err: &ERROR| err.matches_variant(InvalidTransaction::Eip7702NotSupported))
            .unwrap_or(false);
        if is_eip_7702_not_supported_error {
            let ctx = evm.ctx();
            let spec_id = ctx.cfg().spec();
            let tx = ctx.tx();
            let base_fee = if ctx.cfg().is_base_fee_check_disabled() {
                None
            } else {
                Some(ctx.block().basefee() as u128)
            };

            // --- EIP 7702 VALIDATION ---
            if !spec_id.is_enabled_in(ScrollSpecId::EUCLID) {
                return Err(InvalidTransaction::Eip7702NotSupported.into());
            }

            if Some(ctx.cfg().chain_id()) != tx.chain_id() {
                return Err(InvalidTransaction::InvalidChainId.into());
            }

            validate_priority_fee_tx(
                tx.max_fee_per_gas(),
                tx.max_priority_fee_per_gas().unwrap_or_default(),
                base_fee,
            )?;

            let auth_list_len = tx.authorization_list_len();
            // The transaction is considered invalid if the length of authorization_list is zero.
            if auth_list_len == 0 {
                return Err(InvalidTransaction::EmptyAuthorizationList.into());
            }

            // --- TX VALIDATION ---

            // Check if gas_limit is more than block_gas_limit
            if !ctx.cfg().is_block_gas_limit_disabled() && tx.gas_limit() > ctx.block().gas_limit()
            {
                return Err(InvalidTransaction::CallerGasLimitMoreThanBlock.into());
            }

            // EIP-3860: Limit and meter initcode
            let spec_id: SpecId = spec_id.into();
            if spec_id.is_enabled_in(SpecId::SHANGHAI) && tx.kind().is_create() {
                let max_initcode_size = ctx.cfg().max_code_size().saturating_mul(2);
                if ctx.tx().input().len() > max_initcode_size {
                    return Err(InvalidTransaction::CreateInitCodeSizeLimit.into());
                }
            }

            Ok(())
        } else {
            res
        }
    }

    #[inline]
    fn load_accounts(&self, evm: &mut Self::Evm) -> Result<(), Self::Error> {
        // only load the L1BlockInfo for txs that are not l1 messages.
        if !evm.ctx().tx().is_l1_msg() {
            let spec = evm.ctx().cfg().spec();
            let l1_block_info = L1BlockInfo::try_fetch(&mut evm.ctx().db(), spec)?;
            *evm.ctx().chain() = l1_block_info;
        }

        self.mainnet.load_accounts(evm)
    }

    // TODO: issue #24
    #[inline]
    fn apply_eip7702_auth_list(&self, evm: &mut Self::Evm) -> Result<u64, Self::Error> {
        let context = evm.ctx();
        let tx = context.tx();
        // Return if there is no auth list.
        if tx.tx_type() != TransactionType::Eip7702 {
            return Ok(0);
        }

        let chain_id = context.cfg().chain_id();
        let (tx, journal) = context.tx_journal();

        let mut refunded_accounts = 0;
        for authorization in tx.authorization_list() {
            // 1. Verify the chain id is either 0 or the chain's current ID.
            let auth_chain_id = authorization.chain_id();
            if !auth_chain_id.is_zero() && auth_chain_id != U256::from(chain_id) {
                continue;
            }

            // 2. Verify the `nonce` is less than `2**64 - 1`.
            if authorization.nonce() == u64::MAX {
                continue;
            }

            // recover authority and authorized addresses.
            // 4. `authority = ecrecover(keccak(MAGIC || rlp([chain_id, address, nonce])), y_parity,
            //    r, s]`
            let Some(authority) = authorization.authority() else {
                continue;
            };

            // warm authority account and check nonce.
            // 4. Add `authority` to `accessed_addresses` (as defined in [EIP-2929](./eip-2929.md).)
            let mut authority_acc = journal.load_account_code(authority)?;

            // 5. Verify the code of `authority` is either empty or already delegated.
            if let Some(bytecode) = &authority_acc.info.code {
                // if it is not empty and it is not eip7702
                if !bytecode.is_empty() && !bytecode.is_eip7702() {
                    continue;
                }
            }

            // 6. Verify the nonce of `authority` is equal to `nonce`. In case `authority` does not
            //    exist in the trie, verify that `nonce` is equal to `0`.
            if authorization.nonce() != authority_acc.info.nonce {
                continue;
            }

            // 7. Add `PER_EMPTY_ACCOUNT_COST - PER_AUTH_BASE_COST` gas to the global refund counter
            //    if `authority` exists in the trie.
            if !authority_acc.is_empty() {
                refunded_accounts += 1;
            }

            // 8. Set the code of `authority` to be `0xef0100 || address`. This is a delegation
            //    designation.
            //  * As a special case, if `address` is `0x0000000000000000000000000000000000000000` do
            //    not write the designation. Clear the accounts code and reset the account's code
            //    hash to the empty hash
            //    `0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470`.
            let address = authorization.address();
            let (bytecode, hash) = if address.is_zero() {
                (Bytecode::default(), KECCAK_EMPTY)
            } else {
                let bytecode = Bytecode::new_eip7702(address);
                let hash = bytecode.hash_slow();
                (bytecode, hash)
            };
            authority_acc.info.code_hash = hash;
            authority_acc.info.code = Some(bytecode);

            // 9. Increase the nonce of `authority` by one.
            authority_acc.info.nonce = authority_acc.info.nonce.saturating_add(1);
            authority_acc.mark_touch();
        }

        let refunded_gas =
            refunded_accounts * (eip7702::PER_EMPTY_ACCOUNT_COST - eip7702::PER_AUTH_BASE_COST);

        Ok(refunded_gas)
    }

    #[inline]
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

    #[inline]
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

    #[inline]
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

impl<EVM, ERROR, FRAME> InspectorHandler for ScrollHandler<EVM, ERROR, FRAME>
where
    EVM: InspectorEvmTr<
        Context: ScrollContextTr,
        Inspector: Inspector<<<Self as Handler>::Evm as EvmTr>::Context, EthInterpreter>,
    >,
    ERROR: EvmTrError<EVM> + MatchesInvalidTransactionVariantError,
    FRAME: InspectorFrame<
        Evm = EVM,
        Error = ERROR,
        FrameResult = FrameResult,
        FrameInit = FrameInput,
        IT = EthInterpreter,
    >,
{
    type IT = EthInterpreter;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builder::{DefaultScrollContext, ScrollBuilder, ScrollContext},
        journal::ScrollJournal,
        l1block::L1_GAS_PRICE_ORACLE_ADDRESS,
        transaction::L1_MESSAGE_TYPE,
    };
    use std::boxed::Box;

    use revm::{
        context::transaction::{Authorization, SignedAuthorization},
        database::{DbAccount, InMemoryDB},
        handler::EthFrame,
        interpreter::{CallOutcome, InstructionResult, InterpreterResult},
        state::AccountInfo,
    };
    use revm_primitives::{address, bytes, Address};

    const TX_L1_FEE_PRECISION: U256 = U256::from_limbs([1_000_000_000u64, 0, 0, 0]);
    const CALLER: Address = address!("0x000000000000000000000000000000000000dead");
    const TO: Address = address!("0x0000000000000000000000000000000000000001");
    const BENEFICIARY: Address = address!("0x0000000000000000000000000000000000000002");
    const MIN_TRANSACTION_COST: U256 = U256::from_limbs([21_000u64, 0, 0, 0]);
    const L1_DATA_COST: U256 = U256::from_limbs([4u64, 0, 0, 0]);

    fn context() -> ScrollContext<InMemoryDB> {
        ScrollContext::scroll()
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
            .with_new_journal(ScrollJournal::new(InMemoryDB::default()))
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
    fn test_validate_lacking_funds() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context();
        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        let err = handler.validate(&mut evm).unwrap_err();
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
    fn test_validate_lacking_funds_l1_message() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context()
            .modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE)
            .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::EUCLID);
        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        handler.validate(&mut evm)?;

        Ok(())
    }

    #[test]
    fn test_validate_initial_gas_eip7702() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context();
        let evm = ctx.clone().build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        let gas_empty_authorization_list = handler.validate_initial_tx_gas(&evm)?;

        let evm = ctx
            .modify_tx_chained(|tx| {
                tx.base.gas_limit += eip7702::PER_EMPTY_ACCOUNT_COST;
                tx.base.authorization_list = vec![SignedAuthorization::new_unchecked(
                    Authorization {
                        chain_id: Default::default(),
                        address: Default::default(),
                        nonce: 0,
                    },
                    0,
                    U256::ZERO,
                    U256::ZERO,
                )]
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
        let ctx = context()
            .modify_tx_chained(|tx| {
                tx.base.tx_type = TransactionType::Eip7702 as u8;
                tx.base.authorization_list = vec![SignedAuthorization::new_unchecked(
                    Authorization {
                        chain_id: Default::default(),
                        address: Default::default(),
                        nonce: 0,
                    },
                    0,
                    U256::ZERO,
                    U256::ZERO,
                )]
            })
            .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::EUCLID);
        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        handler.validate_env(&mut evm)?;

        Ok(())
    }

    #[test]
    fn test_load_account() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context();
        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        handler.load_accounts(&mut evm)?;

        let l1_block_info = evm.ctx().inner.chain.clone();
        assert_ne!(l1_block_info, L1BlockInfo::default());

        Ok(())
    }

    #[test]
    fn test_load_account_l1_message() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context().modify_tx_chained(|tx| tx.base.tx_type = L1_MESSAGE_TYPE);
        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        handler.load_accounts(&mut evm)?;

        let l1_block_info = evm.ctx().inner.chain.clone();
        assert_eq!(l1_block_info, L1BlockInfo::default());

        Ok(())
    }

    #[test]
    fn test_apply_eip7702_auth_list() -> Result<(), Box<dyn core::error::Error>> {
        let ctx = context()
            .modify_tx_chained(|tx| {
                tx.base.tx_type = TransactionType::Eip7702 as u8;
            })
            .modify_cfg_chained(|cfg| cfg.spec = ScrollSpecId::EUCLID);
        let mut evm = ctx.build_scroll();
        let handler = ScrollHandler::<_, EVMError<_>, EthFrame<_, _, _>>::new();
        handler.apply_eip7702_auth_list(&mut evm)?;

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
