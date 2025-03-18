use crate::{instructions::ScrollInstructions, precompile::ScrollPrecompileProvider};

use crate::exec::ScrollContextTr;
use revm::{
    context::{Cfg, ContextSetters, ContextTr, Evm, EvmData},
    handler::{instructions::InstructionProvider, EvmTr},
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
            data: EvmData { ctx, inspector },
            instruction: ScrollInstructions::new_mainnet(),
            precompiles: ScrollPrecompileProvider::new_with_spec(spec),
        })
    }
}

impl<CTX, INSP, I, P> EvmTr for ScrollEvm<CTX, INSP, I, P>
where
    CTX: ContextTr,
    I: InstructionProvider<
        Context = CTX,
        InterpreterTypes: InterpreterTypes<Output = InterpreterAction>,
    >,
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
        let context = &mut self.0.data.ctx;
        let instructions = &mut self.0.instruction;
        interpreter.run_plain(instructions.instruction_table(), context)
    }

    fn ctx(&mut self) -> &mut Self::Context {
        &mut self.0.data.ctx
    }

    fn ctx_ref(&self) -> &Self::Context {
        &self.0.data.ctx
    }

    fn ctx_instructions(&mut self) -> (&mut Self::Context, &mut Self::Instructions) {
        (&mut self.0.data.ctx, &mut self.0.instruction)
    }

    fn ctx_precompiles(&mut self) -> (&mut Self::Context, &mut Self::Precompiles) {
        (&mut self.0.data.ctx, &mut self.0.precompiles)
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
{
    type Inspector = INSP;

    fn inspector(&mut self) -> &mut Self::Inspector {
        &mut self.0.data.inspector
    }

    fn ctx_inspector(&mut self) -> (&mut Self::Context, &mut Self::Inspector) {
        (&mut self.0.data.ctx, &mut self.0.data.inspector)
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
