use crate::{
    exec::ScrollContextTr, handler::ScrollHandler, instructions::ScrollInstructions, ScrollEvm,
};

use revm::{
    context::ContextSetters,
    handler::{EthFrame, Handler, PrecompileProvider, SystemCallTx},
    interpreter::{interpreter::EthInterpreter, InterpreterResult},
    SystemCallEvm,
};
use revm_primitives::{Address, Bytes};

impl<CTX, INSP, PRECOMPILE> SystemCallEvm
    for ScrollEvm<CTX, INSP, ScrollInstructions<EthInterpreter, CTX>, PRECOMPILE>
where
    CTX: ScrollContextTr<Tx: SystemCallTx> + ContextSetters,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    fn transact_system_call_with_caller(
        &mut self,
        caller: Address,
        system_contract_address: Address,
        data: Bytes,
    ) -> Result<Self::ExecutionResult, Self::Error> {
        self.0.ctx.set_tx(CTX::Tx::new_system_tx_with_caller(
            caller,
            system_contract_address,
            data,
        ));
        let mut h = ScrollHandler::<_, _, EthFrame<EthInterpreter>>::new();
        h.run_system_call(self)
    }
}
