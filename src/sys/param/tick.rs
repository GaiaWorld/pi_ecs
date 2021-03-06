use crate::{
	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState, NotApply},
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

impl<'w, 's> SystemParamFetch<'w, 's> for TickState {
    type Item = Tick;

    #[inline]
    unsafe fn get_param(
        _state: &'s mut Self,
        _system_state: &SystemState,
        _world: &'w World,
        change_tick: u32,
    ) -> Self::Item {
		Tick{
			change_tick: change_tick,
			last_change_tick: _world.last_change_tick()
		}
    }
}

impl NotApply for TickState {}