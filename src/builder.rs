use crate::{
    context::ScrollContextFull, evm::ScrollEvm, instructions::ScrollInstructions,
    journal::ScrollJournal, l1block::L1BlockInfo, transaction::ScrollTxTr, ScrollSpecId,
    ScrollTransaction,
};

use revm::{
    context::{BlockEnv, Cfg, CfgEnv, JournalTr, TxEnv},
    context_interface::Block,
    database::EmptyDB,
    interpreter::interpreter::EthInterpreter,
    Database, MainContext,
};
use revm_primitives::hardfork::SpecId;

pub trait ScrollBuilder: Sized {
    type Context;

    fn build_scroll(
        self,
    ) -> ScrollEvm<Self::Context, (), ScrollInstructions<EthInterpreter, Self::Context>>;

    fn build_scroll_with_inspector<INSP>(
        self,
        inspector: INSP,
    ) -> ScrollEvm<Self::Context, INSP, ScrollInstructions<EthInterpreter, Self::Context>>;
}

impl<BLOCK, TX, CFG, DB> ScrollBuilder for ScrollContextFull<BLOCK, TX, CFG, DB, L1BlockInfo>
where
    BLOCK: Block,
    TX: ScrollTxTr,
    CFG: Cfg<Spec = ScrollSpecId>,
    DB: Database,
{
    type Context = Self;

    fn build_scroll(
        self,
    ) -> ScrollEvm<Self::Context, (), ScrollInstructions<EthInterpreter, Self::Context>> {
        ScrollEvm::new(self, ())
    }

    fn build_scroll_with_inspector<INSP>(
        self,
        inspector: INSP,
    ) -> ScrollEvm<Self::Context, INSP, ScrollInstructions<EthInterpreter, Self::Context>> {
        ScrollEvm::new(self, inspector)
    }
}

/// Allows to build a default Scroll [`Context`].
pub trait DefaultScrollContext {
    fn scroll() -> ScrollContext<EmptyDB>;
}

impl DefaultScrollContext for ScrollContext<EmptyDB> {
    fn scroll() -> ScrollContext<EmptyDB> {
        let scroll_spec_id = ScrollSpecId::default();

        let mut journal = ScrollJournal::new(EmptyDB::new());
        journal.set_spec_id(SpecId::default());
        journal.set_scroll_spec_id(scroll_spec_id);

        ScrollContextFull::mainnet()
            .with_tx(ScrollTransaction::default())
            .with_cfg(CfgEnv::new_with_spec(scroll_spec_id))
            .with_new_journal(journal)
            .with_chain(L1BlockInfo::default())
    }
}

pub type ScrollContext<DB> =
    ScrollContextFull<BlockEnv, ScrollTransaction<TxEnv>, CfgEnv<ScrollSpecId>, DB, L1BlockInfo>;
