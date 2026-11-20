use std::{
	marker::PhantomData,
	sync::Arc,
};

use pi_share::cell::TrustCell;

use super::{
	interface::{WorldQuery, ReadOnlyFetch, Fetch},
	ref_ty::ReadState,
};

use crate::{
	archetype::{Archetype},
	storage::LocalVersion,
	component::{Component, ComponentTicks, MultiCaseImpl},
	world::{World, WorldInner},
};


#[derive(Clone)]
pub struct ChangeTrackers<T: Component> {
    pub(crate) component_ticks: ComponentTicks,
    pub(crate) last_change_tick: u32,
    pub(crate) change_tick: u32,
    pub(crate) marker: PhantomData<T>,
}
pub struct ChangeTrackersFetch<T> {
	pub(crate) container: usize,
	pub(crate) last_change_tick: u32,
	pub(crate) change_tick: u32,
	mark: PhantomData<T>,
}

impl<T: Component> ChangeTrackers<T> {
    /// Has this component been added since the last execution of this system.
    pub fn is_added(&self) -> bool {
        self.component_ticks
            .is_added(self.last_change_tick, self.change_tick)
    }

    /// Has this component been changed since the last execution of this system.
    pub fn is_changed(&self) -> bool {
        self.component_ticks
            .is_changed(self.last_change_tick, self.change_tick)
    }
}

impl<T: Component> WorldQuery for ChangeTrackers<T> {
	type Fetch = ChangeTrackersFetch<T>;
    type State = ReadState<T>;
}

unsafe impl<T> ReadOnlyFetch for ChangeTrackersFetch<T> {}

impl<'s, T: Component> Fetch<'s> for ChangeTrackersFetch<T> {
    type Item = ChangeTrackers<T>;
    type State = ReadState<T>;

    unsafe fn init(
        _world: &World,
        _state: &Self::State
    ) -> Self {
        Self {
			container: 0,
			last_change_tick:0,
			change_tick: 0,
			mark: PhantomData,
        }
    }

	unsafe fn setting(&mut self, _world: &WorldInner, last_change_tick: u32, change_tick: u32) {
		self.last_change_tick = last_change_tick;
		self.change_tick = change_tick;
	}

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
		_world: &World,
    ) {
		let c = archetype.get_component(state.component_id);
		match c.clone().downcast() {
			Ok(r) => {
				let r: Arc<TrustCell<MultiCaseImpl<T>>> = r;
				self.container = r.as_ptr() as usize;
			},
			Err(_) => panic!("downcast fail")
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
		match (&mut *(self.container as *mut MultiCaseImpl<T>)).tick(local) {
			Some(r) => {
				Some(ChangeTrackers {
					component_ticks: r.clone(),
					last_change_tick: self.last_change_tick,
					change_tick: self.change_tick,
					marker: PhantomData
				})
			},
			None => Some(ChangeTrackers {
				component_ticks: ComponentTicks { added: 0, changed: 0 },
				last_change_tick: self.last_change_tick,
				change_tick: self.change_tick,
				marker: PhantomData
			}),
		}
    }

	unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
		ChangeTrackers {
			component_ticks: (&mut *(self.container as *mut MultiCaseImpl<T>)).tick_uncehcked(local).clone(),
			last_change_tick: self.last_change_tick,
			change_tick: self.change_tick,
			marker: PhantomData
		}
	}
}
