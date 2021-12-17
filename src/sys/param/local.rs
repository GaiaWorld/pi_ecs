use crate::{
	component::{Component},
	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState},
	sys::system::interface::SystemState,
	world::{World, FromWorld}
};
use std::ops::{Deref, DerefMut};


pub struct Local<'a, T: Component>(&'a mut T);

impl<'a, T: Component> Deref for Local<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, T: Component> DerefMut for Local<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

/// The [`SystemParamState`] of [`Local`].
pub struct LocalState<T: Component>(T);

impl<'a, T: Component + FromWorld> SystemParam for Local<'a, T> {
    type Fetch = LocalState<T>;
}

// SAFE: only local state is accessed
unsafe impl<T: Component + FromWorld> SystemParamState for LocalState<T> {
    type Config = Option<T>;

    fn init(world: &mut World, _system_state: &mut SystemState, config: Self::Config) -> Self {
        Self(config.unwrap_or_else(|| T::from_world(world)))
    }

    fn default_config() -> Option<T> {
        None
    }
}

impl<'a, T: Component + FromWorld> SystemParamFetch<'a> for LocalState<T> {
    type Item = Local<'a, T>;

    #[inline]
    unsafe fn get_param(
        state: &'a mut Self,
        _system_state: &'a SystemState,
        _world: &'a World,
        _change_tick: u32,
    ) -> Self::Item {
        Local(&mut state.0)
    }
}