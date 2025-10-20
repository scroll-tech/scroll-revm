use crate::{
    evm::ScrollEvm, instructions::ScrollInstructions, l1block::L1BlockInfo,
    transaction::ScrollTxTr, ScrollSpecId, ScrollTransaction,
};

use revm::{
    context::{BlockEnv, Cfg, CfgEnv, JournalTr, TxEnv},
    context_interface::Block,
    database::EmptyDB,
    interpreter::interpreter::EthInterpreter,
    state::EvmState,
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
    JOURNAL: JournalTr<Database = DB, State = EvmState>,
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
        let spec = ScrollSpecId::default();
        let mut cfg = CfgEnv::new_with_spec(spec);
        cfg.enable_eip7702 = spec >= ScrollSpecId::EUCLID;
        cfg.enable_eip7623 = spec >= ScrollSpecId::FEYNMAN;

        Context::mainnet()
            .with_tx(ScrollTransaction::default())
            .with_cfg(cfg)
            .with_chain(L1BlockInfo::default())
    }
}

/// Activates specific EIP's for Euclid.
pub trait EuclidEipActivations {
    /// Activates EIP-7702 if the spec is at least at Euclid.
    fn maybe_with_eip_7702(self) -> Self;
}

/// Activates specific EIP's for Feynman.
pub trait FeynmanEipActivations: EuclidEipActivations {
    /// Activates EIP-7623 if the spec is at least at Feynman.
    fn maybe_with_eip_7623(self) -> Self;
}

impl<DB: Database> EuclidEipActivations for ScrollContext<DB> {
    fn maybe_with_eip_7702(mut self) -> Self {
        self.cfg.enable_eip7702 = self.cfg.spec >= ScrollSpecId::EUCLID;
        self
    }
}

impl<DB: Database> FeynmanEipActivations for ScrollContext<DB> {
    fn maybe_with_eip_7623(mut self) -> Self {
        self.cfg.enable_eip7623 = self.cfg.spec >= ScrollSpecId::FEYNMAN;
        self
    }
}

pub type ScrollContext<DB> =
    Context<BlockEnv, ScrollTransaction<TxEnv>, CfgEnv<ScrollSpecId>, DB, Journal<DB>, L1BlockInfo>;
