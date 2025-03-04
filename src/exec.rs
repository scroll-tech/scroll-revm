use crate::{
    handler::ScrollHandler, instructions::ScrollInstructions, l1block::L1BlockInfo,
    transaction::ScrollTxTr, ScrollEvm, ScrollSpecId,
};
use std::vec::Vec;

use revm::{
    context::result::HaltReason,
    context_interface::{
        result::{EVMError, ExecutionResult, ResultAndState},
        Block, Cfg, ContextTr, Database, Journal,
    },
    handler::{handler::EvmTr, EthFrame, Handler},
    interpreter::interpreter::EthInterpreter,
    state::EvmState,
    Context, DatabaseCommit, ExecuteCommitEvm, ExecuteEvm,
};
use revm_inspector::{InspectCommitEvm, InspectEvm, Inspector, JournalExt};
use revm_primitives::Log;

impl<BLOCK, TX, CFG, DB, JOURNAL, INSP> ExecuteEvm
    for ScrollEvm<
        Context<BLOCK, TX, CFG, DB, JOURNAL, L1BlockInfo>,
        INSP,
        ScrollInstructions<EthInterpreter, Context<BLOCK, TX, CFG, DB, JOURNAL, L1BlockInfo>>,
    >
where
    BLOCK: Block,
    TX: ScrollTxTr,
    CFG: Cfg<Spec = ScrollSpecId>,
    DB: Database,
    JOURNAL: Journal<Database = DB, FinalOutput = (EvmState, Vec<Log>)>,
{
    type Output = Result<ResultAndState<HaltReason>, EVMError<<DB as Database>::Error>>;

    fn transact_previous(&mut self) -> Self::Output {
        let mut h = ScrollHandler::<_, _, EthFrame<_, _, _>>::new();
        h.run(self)
    }
}

impl<BLOCK, TX, CFG, DB, JOURNAL, INSP> ExecuteCommitEvm
    for ScrollEvm<
        Context<BLOCK, TX, CFG, DB, JOURNAL, L1BlockInfo>,
        INSP,
        ScrollInstructions<EthInterpreter, Context<BLOCK, TX, CFG, DB, JOURNAL, L1BlockInfo>>,
    >
where
    BLOCK: Block,
    TX: ScrollTxTr,
    CFG: Cfg<Spec = ScrollSpecId>,
    DB: Database + DatabaseCommit,
    JOURNAL: Journal<Database = DB, FinalOutput = (EvmState, Vec<Log>)> + JournalExt,
{
    type CommitOutput = Result<ExecutionResult<HaltReason>, EVMError<<DB as Database>::Error>>;

    fn transact_commit_previous(&mut self) -> Self::CommitOutput {
        self.transact_previous().map(|r| {
            self.ctx().db().commit(r.state);
            r.result
        })
    }
}

impl<BLOCK, TX, CFG, DB, JOURNAL, INSP> InspectEvm
    for ScrollEvm<
        Context<BLOCK, TX, CFG, DB, JOURNAL, L1BlockInfo>,
        INSP,
        ScrollInstructions<EthInterpreter, Context<BLOCK, TX, CFG, DB, JOURNAL, L1BlockInfo>>,
    >
where
    BLOCK: Block,
    TX: ScrollTxTr,
    CFG: Cfg<Spec = ScrollSpecId>,
    DB: Database,
    JOURNAL: Journal<Database = DB, FinalOutput = (EvmState, Vec<Log>)> + JournalExt,
    INSP: Inspector<Context<BLOCK, TX, CFG, DB, JOURNAL, L1BlockInfo>, EthInterpreter>,
{
    type Inspector = INSP;

    fn set_inspector(&mut self, inspector: Self::Inspector) {
        self.0.data.inspector = inspector;
    }

    fn inspect_previous(&mut self) -> Self::Output {
        let mut h = ScrollHandler::<_, _, EthFrame<_, _, _>>::new();
        h.run(self)
    }
}

impl<BLOCK, TX, CFG, DB, JOURNAL, INSP> InspectCommitEvm
    for ScrollEvm<
        Context<BLOCK, TX, CFG, DB, JOURNAL, L1BlockInfo>,
        INSP,
        ScrollInstructions<EthInterpreter, Context<BLOCK, TX, CFG, DB, JOURNAL, L1BlockInfo>>,
    >
where
    BLOCK: Block,
    TX: ScrollTxTr,
    CFG: Cfg<Spec = ScrollSpecId>,
    DB: Database + DatabaseCommit,
    JOURNAL: Journal<Database = DB, FinalOutput = (EvmState, Vec<Log>)> + JournalExt,
    INSP: Inspector<Context<BLOCK, TX, CFG, DB, JOURNAL, L1BlockInfo>, EthInterpreter>,
{
    fn inspect_commit_previous(&mut self) -> Self::CommitOutput {
        self.inspect_previous().map(|r| {
            self.ctx().db().commit(r.state);
            r.result
        })
    }
}
