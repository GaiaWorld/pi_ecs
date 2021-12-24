use share::cell::TrustCell;

use crate::{
	component::{Component},
	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState},
	sys::system::interface::SystemState,
	world::{World, FromWorld}
};
use std::{ops::{Deref, DerefMut}, sync::Arc};


pub struct Local<T: Component>(&'static mut T);

impl<T: Component> Deref for Local<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
		self.0
        // unsafe{std::mem::transmute_copy(self.0)}
    }
}

impl<T: Component> DerefMut for Local<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
		self.0
		// unsafe{std::mem::transmute_copy(self.0)}
    }
}

/// The [`SystemParamState`] of [`Local`].
pub struct LocalState<T: Component>(T);

impl<T: Component + FromWorld> SystemParam for Local<T> {
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
    type Item = Local<T>;

    #[inline]
    unsafe fn get_param(
        state: &'a mut Self,
        _system_state: &'a SystemState,
        _world: &'a Arc<TrustCell<World>>,
        _change_tick: u32,
    ) -> Self::Item {
        Local(std::mem::transmute(&mut state.0))
    }
}