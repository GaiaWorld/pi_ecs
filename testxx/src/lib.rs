#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2018::*;
#[macro_use]
extern crate std;
use pi_ecs::prelude::{Query, Res};
use pi_ecs_macros::SystemParam;
pub struct AA<'a> {
    aa: Query<usize, &'a usize>,
    bb: Res<'a, usize>,
}
impl<'a> pi_ecs::sys::param::SystemParam for AA<'a> {
    type Fetch = AAState<(
        <Query<usize, &'static usize> as pi_ecs::sys::param::SystemParam>::Fetch,
        <Res<'static, usize> as pi_ecs::sys::param::SystemParam>::Fetch,
    )>;
}
pub struct AAState<TSystemParamState> {
    state: TSystemParamState,
    marker: std::marker::PhantomData<()>,
}
unsafe impl<TSystemParamState: pi_ecs::sys::param::SystemParamState>
    pi_ecs::sys::param::SystemParamState for AAState<TSystemParamState>
{
    type Config = TSystemParamState::Config;
    fn init(
        world: &mut pi_ecs::prelude::World,
        system_state: &mut pi_ecs::prelude::SystemState,
        config: Self::Config,
    ) -> Self {
        Self {
            state: TSystemParamState::init(world, system_state, config),
            marker: std::marker::PhantomData,
        }
    }
    fn default_config() -> TSystemParamState::Config {
        TSystemParamState::default_config()
    }
    fn apply(&mut self, world: &mut pi_ecs::prelude::World) {
        self.state.apply(world)
    }
}
impl<'w, 'a> pi_ecs::sys::param::SystemParamFetch<'w, 'a>
    for AAState<(
        <Query<usize, &'static usize> as pi_ecs::sys::param::SystemParam>::Fetch,
        <Res<'static, usize> as pi_ecs::sys::param::SystemParam>::Fetch,
    )>
{
    type Item = AA<'static>;
    unsafe fn get_param(
        state: &'static mut Self,
        system_state: & pi_ecs::sys::system::SystemState,
        world: &'w pi_ecs::world::World,
        change_tick: u32,
    ) -> Self::Item {
        AA { aa : < < Query < usize , & 'static usize > as pi_ecs :: sys :: param :: SystemParam > :: Fetch as pi_ecs :: sys :: param :: SystemParamFetch > :: get_param (& mut state . state . 0 , system_state , world , change_tick) , bb : < < Res < 'static , usize > as pi_ecs :: sys :: param :: SystemParam > :: Fetch as pi_ecs :: sys :: param :: SystemParamFetch > :: get_param (& mut state . state . 1 , system_state , world , change_tick) , }    
    }
}