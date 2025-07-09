use crate::{
    handler::ScrollHandler, instructions::ScrollInstructions, l1block::L1BlockInfo,
    transaction::ScrollTxTr, ScrollEvm, ScrollSpecId,
};

use revm::{
    context::{
        result::{ExecResultAndState, HaltReason},
        ContextSetters, JournalTr,
    },
    context_interface::{
        result::{EVMError, ExecutionResult},
        Cfg, ContextTr, Database,
    },
    handler::{EthFrame, Handler, PrecompileProvider},
    interpreter::{interpreter::EthInterpreter, InterpreterResult},
    state::EvmState,
    DatabaseCommit, ExecuteCommitEvm, ExecuteEvm,
};
use revm_inspector::{InspectCommitEvm, InspectEvm, Inspector, InspectorHandler, JournalExt};

pub trait ScrollContextTr:
    ContextTr<
    Journal: JournalTr<State = EvmState>,
    Tx: ScrollTxTr,
    Cfg: Cfg<Spec = ScrollSpecId>,
    Chain = L1BlockInfo,
>
{
}

impl<T> ScrollContextTr for T where
    T: ContextTr<
        Journal: JournalTr<State = EvmState>,
        Tx: ScrollTxTr,
        Cfg: Cfg<Spec = ScrollSpecId>,
        Chain = L1BlockInfo,
    >
{
}

/// Type alias for the error type of the ScrollEvm.
pub type ScrollError<CTX> = EVMError<<<CTX as ContextTr>::Db as Database>::Error>;

impl<CTX, INSP, PRECOMPILE> ExecuteEvm
    for ScrollEvm<CTX, INSP, ScrollInstructions<EthInterpreter, CTX>, PRECOMPILE>
where
    CTX: ScrollContextTr + ContextSetters,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type Tx = <CTX as ContextTr>::Tx;
    type Block = <CTX as ContextTr>::Block;
    type State = EvmState;
    type Error = ScrollError<CTX>;
    type ExecutionResult = ExecutionResult<HaltReason>;

    fn set_block(&mut self, block: Self::Block) {
        self.0.ctx.set_block(block);
    }

    fn transact_one(&mut self, tx: Self::Tx) -> Result<Self::ExecutionResult, Self::Error> {
        self.0.ctx.set_tx(tx);
        let mut h = ScrollHandler::<_, _, EthFrame<EthInterpreter>>::new();
        h.run(self)
    }

    fn finalize(&mut self) -> Self::State {
        self.0.ctx.journal_mut().finalize()
    }

    fn replay(
        &mut self,
    ) -> Result<ExecResultAndState<Self::ExecutionResult, Self::State>, Self::Error> {
        let mut h = ScrollHandler::<_, _, EthFrame<EthInterpreter>>::new();
        h.run(self).map(|result| {
            let state = self.finalize();
            ExecResultAndState::new(result, state)
        })
    }
}

impl<CTX, INSP, PRECOMPILE> ExecuteCommitEvm
    for ScrollEvm<CTX, INSP, ScrollInstructions<EthInterpreter, CTX>, PRECOMPILE>
where
    CTX: ScrollContextTr<Db: DatabaseCommit> + ContextSetters,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    fn commit(&mut self, state: Self::State) {
        self.0.ctx.db_mut().commit(state)
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
        self.0.inspector = inspector;
    }

    fn inspect_one_tx(&mut self, tx: Self::Tx) -> Result<Self::ExecutionResult, Self::Error> {
        self.0.ctx.set_tx(tx);
        let mut h = ScrollHandler::<_, _, EthFrame<_>>::new();
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
}
