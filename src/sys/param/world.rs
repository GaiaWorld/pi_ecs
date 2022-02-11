use crate::{
	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState},
	sys::system::interface::SystemState,
	world::{World}
};

impl SystemParam for World {
    type Fetch = World;
}

unsafe impl SystemParamState for World {
    type Config = Option<World>;

    fn init(world:  &mut World, _system_state: &mut SystemState, _config: Self::Config) -> Self {
        world.clone()
    }

    fn default_config() -> Self::Config {
        None
    }
}

impl<'a> SystemParamFetch<'a> for World {
    type Item = World;

    #[inline]
    unsafe fn get_param(
        _state: &'a mut Self,
        _system_state: &'a SystemState,
        world: &'a World,
        _last_change_tick: u32,
    ) -> Self::Item {
        world.clone()
    }
}