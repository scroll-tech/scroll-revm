use crate::ScrollSpecId;
use revm::{
    bytecode::Bytecode,
    context::{
        journaled_state::{AccountLoad, JournalCheckpoint, TransferError},
        JournalOutput, JournalTr,
    },
    interpreter::{SStoreResult, SelfDestructResult, StateLoad},
    state::Account,
    Database, Journal,
};
use revm_primitives::{hardfork::SpecId, Address, HashSet, Log, B256, U256};

/// A wrapper around the default Journal.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScrollJournal<DB> {
    /// The inner journal.
    inner: Journal<DB>,
    /// The spec id.
    spec_id: ScrollSpecId,
}

impl<DB> ScrollJournal<DB> {
    pub fn set_scroll_spec_id(&mut self, scroll_spec_id: ScrollSpecId) {
        self.spec_id = scroll_spec_id
    }
}

impl<DB: Database> JournalTr for ScrollJournal<DB> {
    type Database = DB;
    type FinalOutput = JournalOutput;

    fn new(database: Self::Database) -> Self {
        Self { inner: Journal::new(database), spec_id: ScrollSpecId::default() }
    }

    fn db_ref(&self) -> &Self::Database {
        self.inner.db_ref()
    }

    fn db(&mut self) -> &mut Self::Database {
        self.inner.db()
    }

    fn sload(
        &mut self,
        address: Address,
        key: U256,
    ) -> Result<StateLoad<U256>, <Self::Database as Database>::Error> {
        JournalTr::sload(&mut self.inner, address, key)
    }

    fn sstore(
        &mut self,
        address: Address,
        key: U256,
        value: U256,
    ) -> Result<StateLoad<SStoreResult>, <Self::Database as Database>::Error> {
        JournalTr::sstore(&mut self.inner, address, key, value)
    }

    fn tload(&mut self, address: Address, key: U256) -> U256 {
        JournalTr::tload(&mut self.inner, address, key)
    }

    fn tstore(&mut self, address: Address, key: U256, value: U256) {
        JournalTr::tstore(&mut self.inner, address, key, value)
    }

    fn log(&mut self, log: Log) {
        JournalTr::log(&mut self.inner, log)
    }

    fn selfdestruct(
        &mut self,
        address: Address,
        target: Address,
    ) -> Result<StateLoad<SelfDestructResult>, <Self::Database as Database>::Error> {
        JournalTr::selfdestruct(&mut self.inner, address, target)
    }

    fn warm_account_and_storage(
        &mut self,
        address: Address,
        storage_keys: impl IntoIterator<Item = U256>,
    ) -> Result<(), <Self::Database as Database>::Error> {
        JournalTr::warm_account_and_storage(&mut self.inner, address, storage_keys)
    }

    fn warm_account(&mut self, address: Address) {
        JournalTr::warm_account(&mut self.inner, address)
    }

    fn warm_precompiles(&mut self, addresses: HashSet<Address>) {
        JournalTr::warm_precompiles(&mut self.inner, addresses)
    }

    fn precompile_addresses(&self) -> &HashSet<Address> {
        JournalTr::precompile_addresses(&self.inner)
    }

    fn set_spec_id(&mut self, spec_id: SpecId) {
        JournalTr::set_spec_id(&mut self.inner, spec_id)
    }

    fn touch_account(&mut self, address: Address) {
        JournalTr::touch_account(&mut self.inner, address)
    }

    fn transfer(
        &mut self,
        from: Address,
        to: Address,
        balance: U256,
    ) -> Result<Option<TransferError>, <Self::Database as Database>::Error> {
        JournalTr::transfer(&mut self.inner, from, to, balance)
    }

    fn inc_account_nonce(
        &mut self,
        address: Address,
    ) -> Result<Option<u64>, <Self::Database as Database>::Error> {
        JournalTr::inc_account_nonce(&mut self.inner, address)
    }

    fn load_account(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<&mut Account>, <Self::Database as Database>::Error> {
        JournalTr::load_account(&mut self.inner, address)
    }

    fn load_account_code(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<&mut Account>, <Self::Database as Database>::Error> {
        JournalTr::load_account_code(&mut self.inner, address)
    }

    fn load_account_delegated(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<AccountLoad>, <Self::Database as Database>::Error> {
        let spec = self.spec_id;
        let is_eip7702_enabled = spec.is_enabled_in(ScrollSpecId::EUCLID);
        let db = &mut self.inner.database;

        let account = self.inner.inner.load_account_optional(db, address, is_eip7702_enabled)?;
        let is_empty = account.state_clear_aware_is_empty(spec.into());

        let mut account_load = StateLoad::new(
            AccountLoad { is_delegate_account_cold: None, is_empty },
            account.is_cold,
        );

        // load delegate code if account is EIP-7702
        if let Some(Bytecode::Eip7702(code)) = &account.info.code {
            let address = code.address();
            let delegate_account = self.inner.inner.load_account(db, address)?;
            account_load.data.is_delegate_account_cold = Some(delegate_account.is_cold);
        }

        Ok(account_load)
    }

    fn set_code_with_hash(&mut self, address: Address, code: Bytecode, hash: B256) {
        JournalTr::set_code_with_hash(&mut self.inner, address, code, hash)
    }

    fn clear(&mut self) {
        JournalTr::clear(&mut self.inner)
    }

    fn checkpoint(&mut self) -> JournalCheckpoint {
        JournalTr::checkpoint(&mut self.inner)
    }

    fn checkpoint_commit(&mut self) {
        JournalTr::checkpoint_commit(&mut self.inner)
    }

    fn checkpoint_revert(&mut self, checkpoint: JournalCheckpoint) {
        JournalTr::checkpoint_revert(&mut self.inner, checkpoint)
    }

    fn create_account_checkpoint(
        &mut self,
        caller: Address,
        address: Address,
        balance: U256,
        spec_id: SpecId,
    ) -> Result<JournalCheckpoint, TransferError> {
        JournalTr::create_account_checkpoint(&mut self.inner, caller, address, balance, spec_id)
    }

    fn depth(&self) -> usize {
        JournalTr::depth(&self.inner)
    }

    fn finalize(&mut self) -> Self::FinalOutput {
        JournalTr::finalize(&mut self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::boxed::Box;

    use revm::database::EmptyDB;
    use revm_primitives::{address, bytes};

    #[test]
    fn test_should_be_delegated() -> Result<(), Box<dyn core::error::Error>> {
        let mut journal = ScrollJournal::new(EmptyDB::new());
        let address = address!("000000000000000000000000000000000000dead");
        let delegated = address!("000000000000000000000000000000000000de1e");
        let code = Bytecode::new_eip7702(delegated);
        let code_delegated = Bytecode::new_legacy(bytes!("dead"));

        // set the accounts in the journal.
        journal.inner.inner.state.insert(
            delegated,
            Account {
                info: Default::default(),
                storage: Default::default(),
                status: Default::default(),
            },
        );
        journal.inner.inner.state.insert(
            address,
            Account {
                info: Default::default(),
                storage: Default::default(),
                status: Default::default(),
            },
        );
        journal.set_code(delegated, code_delegated);
        journal.set_code(address, code);

        // check the account is delegated.
        let db_code = journal.load_account_delegated(address)?;
        assert!(db_code.is_delegate_account_cold.is_some());

        Ok(())
    }
}
