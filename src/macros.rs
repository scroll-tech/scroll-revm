#[macro_export]
macro_rules! popn {
    ([ $($x:ident),* ],$interpreterreter:expr $(,$ret:expr)? ) => {
        let Some([$( $x ),*]) = $interpreterreter.stack.popn() else {
            $interpreterreter.control.set_instruction_result(revm::interpreter::InstructionResult::StackUnderflow);
            return $($ret)?;
        };
    };
}

#[macro_export]
macro_rules! popn_top {
    ([ $($x:ident),* ], $top:ident, $interpreterreter:expr $(,$ret:expr)? ) => {
        let Some(([$( $x ),*], $top)) = $interpreterreter.stack.popn_top() else {
            $interpreterreter.control.set_instruction_result(revm::interpreter::InstructionResult::StackUnderflow);
            return $($ret)?;
        };
    };
}
