use crate::{
    evm::ScrollEvm, instructions::ScrollInstructions, l1block::L1BlockInfo,
    transaction::ScrollTxTr, ScrollSpecId, ScrollTransaction,
};

use revm::{
    context::{BlockEnv, Cfg, CfgEnv, JournalOutput, JournalTr, TxEnv},
    context_interface::Block,
    database::EmptyDB,
    interpreter::interpreter::EthInterpreter,
    Context, Database, Journal, MainContext,
};

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

impl<BLOCK, TX, CFG, DB, JOURNAL> ScrollBuilder
    for Context<BLOCK, TX, CFG, DB, JOURNAL, L1BlockInfo>
where
    BLOCK: Block,
    TX: ScrollTxTr,
    CFG: Cfg<Spec = ScrollSpecId>,
    DB: Database,
    JOURNAL: JournalTr<Database = DB, FinalOutput = JournalOutput>,
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
        Context::mainnet()
            .with_tx(ScrollTransaction::default())
            .with_cfg(CfgEnv::new_with_spec(ScrollSpecId::default()))
            .with_chain(L1BlockInfo::default())
    }
}

pub type ScrollContext<DB> =
    Context<BlockEnv, ScrollTransaction<TxEnv>, CfgEnv<ScrollSpecId>, DB, Journal<DB>, L1BlockInfo>;
