use crate::{instructions::ScrollInstructions, precompile::ScrollPrecompileProvider};

use crate::exec::ScrollContextTr;
use revm::{
    context::{Cfg, ContextSetters, ContextTr, Evm},
    handler::{instructions::InstructionProvider, EvmTr, PrecompileProvider},
    interpreter::{interpreter::EthInterpreter, Interpreter, InterpreterAction, InterpreterTypes},
};
use revm_inspector::{Inspector, InspectorEvmTr, JournalExt};

/// The Scroll Evm instance.
pub struct ScrollEvm<
    CTX,
    INSP,
    I = ScrollInstructions<EthInterpreter, CTX>,
    P = ScrollPrecompileProvider,
>(pub Evm<CTX, INSP, I, P>);

impl<CTX: ScrollContextTr, INSP>
    ScrollEvm<CTX, INSP, ScrollInstructions<EthInterpreter, CTX>, ScrollPrecompileProvider>
{
    pub fn new(ctx: CTX, inspector: INSP) -> Self {
        let spec = ctx.cfg().spec();
        Self(Evm {
            ctx,
            inspector,
            instruction: ScrollInstructions::new_mainnet(),
            precompiles: ScrollPrecompileProvider::new_with_spec(spec),
        })
    }
}

impl<CTX, INSP, I, P> ScrollEvm<CTX, INSP, I, P> {
    /// Consumed self and returns a new Evm type with given Inspector.
    pub fn with_inspector<NINSP>(self, inspector: NINSP) -> ScrollEvm<CTX, NINSP, I, P> {
        ScrollEvm(self.0.with_inspector(inspector))
    }

    /// Consumes self and returns a new Evm type with given Precompiles.
    pub fn with_precompiles<NP>(self, precompiles: NP) -> ScrollEvm<CTX, INSP, I, NP> {
        ScrollEvm(self.0.with_precompiles(precompiles))
    }

    /// Consumes self and returns the inner Inspector.
    pub fn into_inspector(self) -> INSP {
        self.0.into_inspector()
    }
}

impl<CTX, INSP, I, P> EvmTr for ScrollEvm<CTX, INSP, I, P>
where
    CTX: ContextTr,
    I: InstructionProvider<
        Context = CTX,
        InterpreterTypes: InterpreterTypes<Output = InterpreterAction>,
    >,
    P: PrecompileProvider<CTX>,
{
    type Context = CTX;
    type Instructions = I;
    type Precompiles = P;

    fn run_interpreter(
        &mut self,
        interpreter: &mut Interpreter<
            <Self::Instructions as InstructionProvider>::InterpreterTypes,
        >,
    ) -> <<Self::Instructions as InstructionProvider>::InterpreterTypes as InterpreterTypes>::Output
    {
        let context = &mut self.0.ctx;
        let instructions = &mut self.0.instruction;
        interpreter.run_plain(instructions.instruction_table(), context)
    }

    fn ctx(&mut self) -> &mut Self::Context {
        &mut self.0.ctx
    }

    fn ctx_ref(&self) -> &Self::Context {
        &self.0.ctx
    }

    fn ctx_instructions(&mut self) -> (&mut Self::Context, &mut Self::Instructions) {
        (&mut self.0.ctx, &mut self.0.instruction)
    }

    fn ctx_precompiles(&mut self) -> (&mut Self::Context, &mut Self::Precompiles) {
        (&mut self.0.ctx, &mut self.0.precompiles)
    }
}

impl<CTX, INSP, I, P> InspectorEvmTr for ScrollEvm<CTX, INSP, I, P>
where
    CTX: ContextTr<Journal: JournalExt> + ContextSetters,
    I: InstructionProvider<
        Context = CTX,
        InterpreterTypes: InterpreterTypes<Output = InterpreterAction>,
    >,
    INSP: Inspector<CTX, I::InterpreterTypes>,
    P: PrecompileProvider<CTX>,
{
    type Inspector = INSP;

    fn inspector(&mut self) -> &mut Self::Inspector {
        &mut self.0.inspector
    }

    fn ctx_inspector(&mut self) -> (&mut Self::Context, &mut Self::Inspector) {
        (&mut self.0.ctx, &mut self.0.inspector)
    }

    fn run_inspect_interpreter(
        &mut self,
        interpreter: &mut Interpreter<
            <Self::Instructions as InstructionProvider>::InterpreterTypes,
        >,
    ) -> <<Self::Instructions as InstructionProvider>::InterpreterTypes as InterpreterTypes>::Output
    {
        self.0.run_inspect_interpreter(interpreter)
    }
}
