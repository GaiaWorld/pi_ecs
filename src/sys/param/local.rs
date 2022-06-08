//! 本地数据
use crate::{
	component::{Component},
	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState, NotApply},
	sys::system::interface::SystemState,
	world::{World, FromWorld}
};
use std::ops::{Deref, DerefMut};


pub struct Local<'s, T: Component>(&'s mut T);

impl<'s, T: Component> Deref for Local<'s, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
		self.0
        // unsafe{std::mem::transmute_copy(self.0)}
    }
}

impl<'s, T: Component> DerefMut for Local<'s, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
		self.0
		// unsafe{std::mem::transmute_copy(self.0)}
    }
}

/// The [`SystemParamState`] of [`Local`].
pub struct LocalState<T: Component>(T);

impl<'s, T: Component + FromWorld> SystemParam for Local<'s, T> {
    type Fetch = LocalState<T>;
}

// SAFE: only local state is accessed
unsafe impl<T: Component + FromWorld> SystemParamState for LocalState<T> {
    type Config = Option<T>;

    fn init(world:  &mut World, _system_state: &mut SystemState, config: Self::Config) -> Self {
        Self(config.unwrap_or_else(|| T::from_world(world)))
    }

    fn default_config() -> Option<T> {
        None
    }
}

impl<'w, 's, T: Component + FromWorld> SystemParamFetch<'w, 's> for LocalState<T> {
    type Item = Local<'static, T>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        _system_state: &SystemState,
        _world: &'w World,
        _last_change_tick: u32,
    ) -> Self::Item {
        Local(std::mem::transmute(&mut state.0))
    }
}

impl<T: Send + Sync + 'static> NotApply for LocalState<T> {}