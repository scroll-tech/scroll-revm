use crate::{
    exec::ScrollContextTr, instructions::ScrollInstructions, precompile::ScrollPrecompileProvider,
};

use revm::{
    context::{Cfg, ContextError, ContextSetters, ContextTr, Evm, FrameStack},
    handler::{
        instructions::InstructionProvider, EthFrame, EvmTr, FrameInitOrResult, FrameTr,
        ItemOrResult, PrecompileProvider,
    },
    interpreter::{interpreter::EthInterpreter, InterpreterResult},
    Database,
};
use revm_inspector::{Inspector, InspectorEvmTr, JournalExt};

/// The Scroll Evm instance.
pub struct ScrollEvm<
    CTX,
    INSP,
    I = ScrollInstructions<EthInterpreter, CTX>,
    P = ScrollPrecompileProvider,
    F = EthFrame<EthInterpreter>,
>(pub Evm<CTX, INSP, I, P, F>);

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
            frame_stack: FrameStack::new(),
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
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type Context = CTX;
    type Instructions = I;
    type Precompiles = P;
    type Frame = EthFrame<EthInterpreter>;

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

    fn frame_stack(&mut self) -> &mut FrameStack<Self::Frame> {
        &mut self.0.frame_stack
    }

    fn frame_init(
        &mut self,
        frame_input: <Self::Frame as FrameTr>::FrameInit,
    ) -> Result<
        ItemOrResult<&mut Self::Frame, <Self::Frame as FrameTr>::FrameResult>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        self.0.frame_init(frame_input)
    }

    fn frame_run(
        &mut self,
    ) -> Result<
        FrameInitOrResult<Self::Frame>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        self.0.frame_run()
    }

    #[doc = " Returns the result of the frame to the caller. Frame is popped from the frame stack."]
    #[doc = " Consumes the frame result or returns it if there is more frames to run."]
    fn frame_return_result(
        &mut self,
        result: <Self::Frame as FrameTr>::FrameResult,
    ) -> Result<
        Option<<Self::Frame as FrameTr>::FrameResult>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        self.0.frame_return_result(result)
    }
}

impl<CTX, INSP, I, P> InspectorEvmTr for ScrollEvm<CTX, INSP, I, P>
where
    CTX: ContextTr<Journal: JournalExt> + ContextSetters,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
    INSP: Inspector<CTX, I::InterpreterTypes>,
{
    type Inspector = INSP;

    fn inspector(&mut self) -> &mut Self::Inspector {
        &mut self.0.inspector
    }

    fn ctx_inspector(&mut self) -> (&mut Self::Context, &mut Self::Inspector) {
        (&mut self.0.ctx, &mut self.0.inspector)
    }

    fn ctx_inspector_frame(
        &mut self,
    ) -> (&mut Self::Context, &mut Self::Inspector, &mut Self::Frame) {
        (&mut self.0.ctx, &mut self.0.inspector, self.0.frame_stack.get())
    }

    fn ctx_inspector_frame_instructions(
        &mut self,
    ) -> (&mut Self::Context, &mut Self::Inspector, &mut Self::Frame, &mut Self::Instructions) {
        (&mut self.0.ctx, &mut self.0.inspector, self.0.frame_stack.get(), &mut self.0.instruction)
    }
}
