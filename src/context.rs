use crate::{journal::ScrollJournal, ScrollSpecId};
use core::ops::{Deref, DerefMut};

use derive_where::derive_where;
use revm::{
    context::{
        Block, BlockEnv, Cfg, CfgEnv, ContextSetters, ContextTr, JournalTr, Transaction, TxEnv,
    },
    context_interface::context::ContextError,
    database::EmptyDB,
    Context, Database, MainContext,
};

/// Wraps a [`Context`] to correctly set the [`ScrollJournal`]'s spec.
#[derive_where(Clone, Debug; BLOCK, CFG, CHAIN, TX, DB, <DB as Database>::Error)]
pub struct ScrollContextFull<
    BLOCK = BlockEnv,
    TX = TxEnv,
    CFG = CfgEnv<ScrollSpecId>,
    DB: Database = EmptyDB,
    CHAIN = (),
> {
    pub inner: Context<BLOCK, TX, CFG, DB, ScrollJournal<DB>, CHAIN>,
}

impl<BLOCK, TX, CFG, DB: Database, CHAIN> Deref for ScrollContextFull<BLOCK, TX, CFG, DB, CHAIN> {
    type Target = Context<BLOCK, TX, CFG, DB, ScrollJournal<DB>, CHAIN>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<BLOCK, TX, CFG, DB: Database, CHAIN> DerefMut
    for ScrollContextFull<BLOCK, TX, CFG, DB, CHAIN>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<BLOCK: Block, TX: Transaction, DB: Database, CFG: Cfg, CHAIN> ContextTr
    for ScrollContextFull<BLOCK, TX, CFG, DB, CHAIN>
{
    type Block = BLOCK;
    type Tx = TX;
    type Cfg = CFG;
    type Db = DB;
    type Journal = ScrollJournal<DB>;
    type Chain = CHAIN;

    fn tx(&self) -> &Self::Tx {
        &self.inner.tx
    }

    fn block(&self) -> &Self::Block {
        &self.inner.block
    }

    fn cfg(&self) -> &Self::Cfg {
        &self.inner.cfg
    }

    fn journal(&mut self) -> &mut Self::Journal {
        &mut self.inner.journaled_state
    }

    fn journal_ref(&self) -> &Self::Journal {
        &self.inner.journaled_state
    }

    fn db(&mut self) -> &mut Self::Db {
        self.inner.journaled_state.db()
    }

    fn db_ref(&self) -> &Self::Db {
        self.inner.journaled_state.db_ref()
    }

    fn chain(&mut self) -> &mut Self::Chain {
        &mut self.inner.chain
    }

    fn error(&mut self) -> &mut Result<(), ContextError<<Self::Db as Database>::Error>> {
        &mut self.inner.error
    }

    fn tx_journal(&mut self) -> (&mut Self::Tx, &mut Self::Journal) {
        (&mut self.inner.tx, &mut self.inner.journaled_state)
    }

    // Keep Default Implementation for Instructions Host Interface
}

impl<BLOCK: Block, TX: Transaction, DB: Database, CFG: Cfg, CHAIN> ContextSetters
    for ScrollContextFull<BLOCK, TX, CFG, DB, CHAIN>
{
    fn set_tx(&mut self, tx: Self::Tx) {
        self.inner.tx = tx;
    }

    fn set_block(&mut self, block: Self::Block) {
        self.inner.block = block;
    }
}

impl MainContext for ScrollContextFull {
    fn mainnet() -> Self {
        ScrollContextFull::new(EmptyDB::new(), ScrollSpecId::default())
    }
}

impl<BLOCK: Block + Default, TX: Transaction + Default, DB: Database, CHAIN: Default>
    ScrollContextFull<BLOCK, TX, CfgEnv<ScrollSpecId>, DB, CHAIN>
{
    /// Returns a new [`ScrollContextFull`] from the provided database and spec id.
    pub fn new(db: DB, spec: ScrollSpecId) -> Self {
        let mut journaled_state = ScrollJournal::new(db);
        journaled_state.set_spec_id(spec.into());
        journaled_state.set_scroll_spec_id(spec);

        let mut cfg = CfgEnv::default();
        cfg.spec = spec;

        Self {
            inner: Context {
                tx: TX::default(),
                block: BLOCK::default(),
                cfg,
                journaled_state,
                chain: Default::default(),
                error: Ok(()),
            },
        }
    }
}

impl<BLOCK, TX, CFG, DB, CHAIN> ScrollContextFull<BLOCK, TX, CFG, DB, CHAIN>
where
    BLOCK: Block,
    TX: Transaction,
    CFG: Cfg<Spec = ScrollSpecId>,
    DB: Database,
{
    pub fn with_new_journal(
        self,
        mut journal: ScrollJournal<DB>,
    ) -> ScrollContextFull<BLOCK, TX, CFG, DB, CHAIN> {
        journal.set_spec_id(self.inner.cfg.spec().into());
        journal.set_scroll_spec_id(self.inner.cfg.spec());

        ScrollContextFull {
            inner: Context {
                tx: self.inner.tx,
                block: self.inner.block,
                cfg: self.inner.cfg,
                journaled_state: journal,
                chain: self.inner.chain,
                error: Ok(()),
            },
        }
    }

    /// Creates a new context with a new database type.
    ///
    /// This will create a new [`ScrollJournal`] object.
    pub fn with_db<ODB: Database>(self, db: ODB) -> ScrollContextFull<BLOCK, TX, CFG, ODB, CHAIN> {
        let spec = self.inner.cfg.spec();
        let mut journaled_state = ScrollJournal::new(db);
        journaled_state.set_spec_id(spec.into());
        journaled_state.set_scroll_spec_id(spec);

        ScrollContextFull {
            inner: Context {
                tx: self.inner.tx,
                block: self.inner.block,
                cfg: self.inner.cfg,
                journaled_state,
                chain: self.inner.chain,
                error: Ok(()),
            },
        }
    }

    /// Creates a new context with a new block type.
    pub fn with_block<OB: Block>(self, block: OB) -> ScrollContextFull<OB, TX, CFG, DB, CHAIN> {
        ScrollContextFull {
            inner: Context {
                tx: self.inner.tx,
                block,
                cfg: self.inner.cfg,
                journaled_state: self.inner.journaled_state,
                chain: self.inner.chain,
                error: Ok(()),
            },
        }
    }
    /// Creates a new context with a new transaction type.
    pub fn with_tx<OTX: Transaction>(
        self,
        tx: OTX,
    ) -> ScrollContextFull<BLOCK, OTX, CFG, DB, CHAIN> {
        ScrollContextFull {
            inner: Context {
                tx,
                block: self.inner.block,
                cfg: self.inner.cfg,
                journaled_state: self.inner.journaled_state,
                chain: self.inner.chain,
                error: Ok(()),
            },
        }
    }

    /// Creates a new context with a new chain type.
    pub fn with_chain<OC>(self, chain: OC) -> ScrollContextFull<BLOCK, TX, CFG, DB, OC> {
        ScrollContextFull {
            inner: Context {
                tx: self.inner.tx,
                block: self.inner.block,
                cfg: self.inner.cfg,
                journaled_state: self.inner.journaled_state,
                chain,
                error: Ok(()),
            },
        }
    }

    /// Creates a new context with a new chain type.
    pub fn with_cfg<OCFG: Cfg<Spec = ScrollSpecId>>(
        mut self,
        cfg: OCFG,
    ) -> ScrollContextFull<BLOCK, TX, OCFG, DB, CHAIN> {
        self.inner.journaled_state.set_spec_id(cfg.spec().into());
        self.inner.journaled_state.set_scroll_spec_id(cfg.spec());

        ScrollContextFull {
            inner: Context {
                tx: self.inner.tx,
                block: self.inner.block,
                cfg,
                journaled_state: self.inner.journaled_state,
                chain: self.inner.chain,
                error: Ok(()),
            },
        }
    }

    /// Modifies the context configuration.
    #[must_use]
    pub fn modify_cfg_chained<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut CFG),
    {
        f(&mut self.inner.cfg);
        self.inner.journaled_state.set_spec_id(self.inner.cfg.spec().into());
        self.inner.journaled_state.set_scroll_spec_id(self.inner.cfg.spec());
        self
    }

    /// Modifies the context block.
    #[must_use]
    pub fn modify_block_chained<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut BLOCK),
    {
        self.modify_block(f);
        self
    }

    /// Modifies the context transaction.
    #[must_use]
    pub fn modify_tx_chained<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut TX),
    {
        self.modify_tx(f);
        self
    }

    /// Modifies the context chain.
    #[must_use]
    pub fn modify_chain_chained<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut CHAIN),
    {
        self.modify_chain(f);
        self
    }

    /// Modifies the context database.
    #[must_use]
    pub fn modify_db_chained<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut DB),
    {
        self.modify_db(f);
        self
    }

    /// Modifies the context block.
    pub fn modify_block<F>(&mut self, f: F)
    where
        F: FnOnce(&mut BLOCK),
    {
        f(&mut self.inner.block);
    }

    pub fn modify_tx<F>(&mut self, f: F)
    where
        F: FnOnce(&mut TX),
    {
        f(&mut self.inner.tx);
    }

    pub fn modify_cfg<F>(&mut self, f: F)
    where
        F: FnOnce(&mut CFG),
    {
        f(&mut self.inner.cfg);
        self.inner.journaled_state.set_spec_id(self.inner.cfg.spec().into());
        self.inner.journaled_state.set_scroll_spec_id(self.inner.cfg.spec());
    }

    pub fn modify_chain<F>(&mut self, f: F)
    where
        F: FnOnce(&mut CHAIN),
    {
        f(&mut self.inner.chain);
    }

    pub fn modify_db<F>(&mut self, f: F)
    where
        F: FnOnce(&mut DB),
    {
        f(self.inner.journaled_state.db());
    }
}
