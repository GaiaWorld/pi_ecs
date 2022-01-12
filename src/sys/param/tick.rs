use crate::{
	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState},
	sys::system::interface::SystemState,
	world::World,
};

#[derive(Debug)]
pub struct Tick {
    pub last_change_tick: u32,
    pub change_tick: u32,
}


impl SystemParam for Tick {
    type Fetch = TickState;
}

/// The [`SystemParamState`] of [`SystemChangeTick`].
pub struct TickState {}

unsafe impl SystemParamState for TickState {
    type Config = ();

    fn init(_world:  &mut World, _system_state: &mut SystemState, _config: Self::Config) -> Self {
        Self{}
    }

    fn default_config() -> () {
        ()
    }
}

impl<'a> SystemParamFetch<'a> for TickState {
    type Item = Tick;

    #[inline]
    unsafe fn get_param(
        _state: &'a mut Self,
        _system_state: &'a SystemState,
        _world: &'a World,
        change_tick: u32,
    ) -> Self::Item {
		Tick{
			change_tick: change_tick,
			last_change_tick: _world.last_change_tick()
		}
    }
}