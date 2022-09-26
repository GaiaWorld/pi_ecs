extern crate pi_ecs_macros;
extern crate pi_ecs;

use std::{marker::PhantomData};

use pi_ecs::monitor::{Create, Delete};
use pi_ecs_macros::{all_tuples, listen};


pub struct ListenerFn<T>(pub T);

pub trait Listen{}

pub struct A;

// #[derive(Default)]
// pub struct B (pub u32);

macro_rules! impl_system_function {
    ($($param: ident),*) => {
		#[allow(non_snake_case)]
        impl<Input, Out, Func, $($param: SystemParam),*> SystemParamFunction<Input, Out, ($($param ,)*), InputMarker> for Func
        where
            Func:
                FnMut(Input, $($param),*) -> Out +
                FnMut(Input, $(<<$param as SystemParam>::Fetch as SystemParamFetch>::Item),*) -> Out + ThreadSend + 'static, Out: 'static
        {
            #[inline]
            fn run(&mut self, input: Input, state: &mut <($($param,)*) as SystemParam>::Fetch, system_state: &SystemState, world: &Arc<TrustCell<World>>, change_tick: u32) -> Out {
                unsafe {
                    let ($($param,)*) = <<($($param,)*) as SystemParam>::Fetch as SystemParamFetch>::get_param(state, system_state, world, change_tick);
                    self(input, $($param),*)
                }
            }
        }

    };
}

all_tuples!(impl_system_function, 0, 2, F);
// all_tuples!(impl_system_param_tuple, 0, 2, F11);