use pi_ecs_schedule::{function_adapter::FunctionSignature, system::SystemFunction};

#[test]
fn zero_parameter_function_retains_captured_state_between_calls() {
    let mut calls = 0;
    {
        let mut function = || calls += 1;
        SystemFunction::<FunctionSignature<()>>::call(&mut function, ());
        SystemFunction::<FunctionSignature<()>>::call(&mut function, ());
    }
    assert_eq!(calls, 2);
}
