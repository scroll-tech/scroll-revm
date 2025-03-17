use crate::{
    handler::ScrollHandler, instructions::ScrollInstructions, l1block::L1BlockInfo,
    transaction::ScrollTxTr, ScrollEvm, ScrollSpecId,
};

use revm::{
    context::{result::HaltReason, ContextSetters, JournalOutput, JournalTr},
    context_interface::{
        result::{EVMError, ExecutionResult, ResultAndState},
        Cfg, ContextTr, Database,
    },
    handler::{EthFrame, EvmTr, Handler, PrecompileProvider},
    interpreter::{interpreter::EthInterpreter, InterpreterResult},
    DatabaseCommit, ExecuteCommitEvm, ExecuteEvm,
};
use revm_inspector::{InspectCommitEvm, InspectEvm, Inspector, InspectorHandler, JournalExt};

pub trait ScrollContextTr:
    ContextTr<
    Journal: JournalTr<FinalOutput = JournalOutput>,
    Tx: ScrollTxTr,
    Cfg: Cfg<Spec = ScrollSpecId>,
    Chain = L1BlockInfo,
>
{
}

impl<T> ScrollContextTr for T where
    T: ContextTr<
        Journal: JournalTr<FinalOutput = JournalOutput>,
        Tx: ScrollTxTr,
        Cfg: Cfg<Spec = ScrollSpecId>,
        Chain = L1BlockInfo,
    >
{
}

impl<CTX, INSP, PRECOMPILE> ExecuteEvm
    for ScrollEvm<CTX, INSP, ScrollInstructions<EthInterpreter, CTX>, PRECOMPILE>
where
    CTX: ScrollContextTr + ContextSetters,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type Output =
        Result<ResultAndState<HaltReason>, EVMError<<<CTX as ContextTr>::Db as Database>::Error>>;

    type Tx = <CTX as ContextTr>::Tx;

    type Block = <CTX as ContextTr>::Block;

    fn set_tx(&mut self, tx: Self::Tx) {
        self.0.data.ctx.set_tx(tx);
    }

    fn set_block(&mut self, block: Self::Block) {
        self.0.data.ctx.set_block(block);
    }

    fn replay(&mut self) -> Self::Output {
        let mut h = ScrollHandler::<_, _, EthFrame<_, _, _>>::new();
        h.run(self)
    }
}

impl<CTX, INSP, PRECOMPILE> ExecuteCommitEvm
    for ScrollEvm<CTX, INSP, ScrollInstructions<EthInterpreter, CTX>, PRECOMPILE>
where
    CTX: ScrollContextTr<Db: DatabaseCommit> + ContextSetters,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type CommitOutput =
        Result<ExecutionResult<HaltReason>, EVMError<<<CTX as ContextTr>::Db as Database>::Error>>;

    fn replay_commit(&mut self) -> Self::CommitOutput {
        self.replay().map(|r| {
            self.ctx().db().commit(r.state);
            r.result
        })
    }
}

impl<CTX, INSP, PRECOMPILE> InspectEvm
    for ScrollEvm<CTX, INSP, ScrollInstructions<EthInterpreter, CTX>, PRECOMPILE>
where
    CTX: ScrollContextTr<Journal: JournalExt> + ContextSetters,
    INSP: Inspector<CTX, EthInterpreter>,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type Inspector = INSP;

    fn set_inspector(&mut self, inspector: Self::Inspector) {
        self.0.data.inspector = inspector;
    }

    fn inspect_replay(&mut self) -> Self::Output {
        let mut h = ScrollHandler::<_, _, EthFrame<_, _, _>>::new();
        h.inspect_run(self)
    }
}

impl<CTX, INSP, PRECOMPILE> InspectCommitEvm
    for ScrollEvm<CTX, INSP, ScrollInstructions<EthInterpreter, CTX>, PRECOMPILE>
where
    CTX: ScrollContextTr<Journal: JournalExt, Db: DatabaseCommit> + ContextSetters,
    INSP: Inspector<CTX, EthInterpreter>,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    fn inspect_commit_previous(&mut self) -> Self::CommitOutput {
        self.inspect_replay().map(|r| {
            self.ctx().db().commit(r.state);
            r.result
        })
    }
}
