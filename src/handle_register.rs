//! Handler related to Scroll chain

use revm::{
    handler::{
        mainnet::{self, deduct_caller_inner},
        register::EvmHandler,
    },
    interpreter::Gas,
    primitives::{TxKind, U256},
    wiring::{
        result::{EVMError, EVMResultGeneric, InvalidTransaction},
        Block, Transaction,
    },
    Context, ContextPrecompiles,
};
use std::sync::Arc;

use crate::{
    instruction::make_scroll_instruction_tables, precompile, scroll_spec_to_generic,
    spec::ScrollSpec, L1BlockInfo, ScrollContext, ScrollTransaction, ScrollWiring,
};

/// Configure the handler for the Scroll chain.
///
/// This function modifies the following handlers:
/// - `pre_execution.load_accounts` - Adds a hook to load `L1BlockInfo` from the database such that
///    it can be used to calculate the L1 cost of a transaction.
/// - `pre_execution.deduct_caller` - Overrides the logic to deduct the max transaction fee from the
///   caller's balance.
/// - `pre_execution.load_precompiles` - Overrides the logic to load the precompiles for the Scroll chain.
/// - `post_execution.reward_beneficiary` - Overrides the logic to reward the beneficiary with the gas fee.
/// - `set_instruction_table` - Overrides the instruction table with the Scroll instruction tables.
pub fn scroll_handle_register<EvmWiringT>(handler: &mut EvmHandler<'_, EvmWiringT>)
where
    EvmWiringT: ScrollWiring,
{
    scroll_spec_to_generic!(handler.spec_id, {
        // Load l1 data
        handler.pre_execution.load_accounts = Arc::new(load_accounts::<EvmWiringT, SPEC>);
        // L1_fee is added to the gas cost.
        handler.pre_execution.deduct_caller = Arc::new(deduct_caller::<EvmWiringT, SPEC>);
        // Load precompiles for the given chain spec.
        handler.pre_execution.load_precompiles = Arc::new(load_precompiles::<EvmWiringT, SPEC>);
        // basefee is sent to coinbase
        handler.post_execution.reward_beneficiary =
            Arc::new(reward_beneficiary::<EvmWiringT, SPEC>);
        // override instruction table
        handler.set_instruction_table(make_scroll_instruction_tables::<_, SPEC>());
    });
}

/// Load the `L1BlockInfo` from the database and invoke standard `load_accounts` handler.
///
/// This function loads the `L1BlockInfo` from the database and sets it in the `Context`.
/// It also invokes the standard `load_accounts` function from the mainnet handler which is
/// responsible for loading the accounts, as defined by the spec, from the database.
#[inline]
pub fn load_accounts<EvmWiringT: ScrollWiring, SPEC: ScrollSpec>(
    context: &mut Context<EvmWiringT>,
) -> EVMResultGeneric<(), EvmWiringT> {
    // TODO: should we add a conditional here to check if it's an L1 message as I believe we do not
    // need this information for L1 transactions?
    let l1_block_info = L1BlockInfo::try_fetch(&mut context.evm.inner.db, SPEC::SCROLL_SPEC_ID)
        .map_err(EVMError::Database)?;
    *context.evm.chain.l1_block_info_mut() = Some(l1_block_info);

    mainnet::load_accounts::<EvmWiringT, SPEC>(context)
}

/// Deducts the max transaction fee from the caller's balance.
///
/// This max transaction fee also includes the L1 cost of the transaction.
#[inline]
pub fn deduct_caller<EvmWiringT: ScrollWiring, SPEC: ScrollSpec>(
    context: &mut Context<EvmWiringT>,
) -> EVMResultGeneric<(), EvmWiringT> {
    // load caller's account.
    let caller_account = context
        .evm
        .inner
        .journaled_state
        .load_account(
            *context.evm.inner.env.tx.caller(),
            &mut context.evm.inner.db,
        )
        .map_err(EVMError::Database)?;

    if !context.evm.inner.env.tx.is_l1_msg() {
        // We deduct caller max balance after minting and before deducing the
        // l1 cost, max values is already checked in pre_validate but l1 cost wasn't.
        deduct_caller_inner::<EvmWiringT, SPEC>(caller_account.data, &context.evm.inner.env);

        // TODO: extract this logic to a separate function
        let Some(rlp_bytes) = &context.evm.inner.env.tx.rlp_bytes() else {
            return Err(EVMError::Custom(
                "[SCROLL] Failed to load transaction rlp_bytes.".to_string(),
            ));
        };
        // Deduct l1 fee from caller.
        let tx_l1_cost = context
            .evm
            .inner
            .chain
            .l1_block_info()
            .as_ref()
            .expect("L1BlockInfo should be loaded")
            .calculate_tx_l1_cost(rlp_bytes, SPEC::SCROLL_SPEC_ID);
        if tx_l1_cost.gt(&caller_account.info.balance) {
            return Err(EVMError::Transaction(
                InvalidTransaction::LackOfFundForMaxFee {
                    fee: tx_l1_cost.into(),
                    balance: caller_account.info.balance.into(),
                }
                .into(),
            ));
        }
        caller_account.data.info.balance =
            caller_account.data.info.balance.saturating_sub(tx_l1_cost);
    } else {
        // bump the nonce for calls. Nonce for CREATE will be bumped in `handle_create`.
        if matches!(context.evm.inner.env.tx.kind(), TxKind::Call(_)) {
            // Nonce is already checked
            caller_account.data.info.nonce = caller_account.data.info.nonce.saturating_add(1);
        }

        // touch account so we know it is changed.
        caller_account.data.mark_touch();
    }
    Ok(())
}

/// Reward beneficiary with gas fee.
///
/// This function rewards the beneficiary with the gas fee including the L1 cost of the transaction.
#[inline]
pub fn reward_beneficiary<EvmWiringT: ScrollWiring, SPEC: ScrollSpec>(
    context: &mut Context<EvmWiringT>,
    gas: &Gas,
) -> EVMResultGeneric<(), EvmWiringT> {
    let beneficiary = *context.evm.env.block.coinbase();
    let effective_gas_price = context.evm.env.effective_gas_price();

    // transfer fee to coinbase/beneficiary.
    let coinbase_gas_price = effective_gas_price;

    let coinbase_account = context
        .evm
        .inner
        .journaled_state
        .load_account(beneficiary, &mut context.evm.inner.db)
        .map_err(EVMError::Database)?;

    if !context.evm.inner.env.tx.is_l1_msg() {
        let Some(l1_block_info) = &context.evm.inner.chain.l1_block_info() else {
            return Err(EVMError::Custom(
                "[SCROLL] Failed to load L1 block information.".to_string(),
            ));
        };

        let Some(rlp_bytes) = &context.evm.inner.env.tx.rlp_bytes() else {
            return Err(EVMError::Custom(
                "[SCROLL] Failed to load transaction rlp_bytes.".to_string(),
            ));
        };

        let l1_cost = l1_block_info.calculate_tx_l1_cost(rlp_bytes, SPEC::SCROLL_SPEC_ID);

        coinbase_account.data.mark_touch();
        coinbase_account.data.info.balance = coinbase_account
            .data
            .info
            .balance
            .saturating_add(coinbase_gas_price * U256::from(gas.spent() - gas.refunded() as u64))
            .saturating_add(l1_cost);
    }

    Ok(())
}

/// Load the precompiles for the Scroll chain.
#[inline]
pub fn load_precompiles<EvmWiringT: ScrollWiring, SPEC: ScrollSpec>(
) -> ContextPrecompiles<EvmWiringT> {
    let precompiles = precompile::load_precompiles::<SPEC>();
    ContextPrecompiles::from_static_precompiles(precompiles)
}
