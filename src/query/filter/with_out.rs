use std::{
	marker::PhantomData
};

use super::interface::FilterFetch;

use crate::{
	query::{
		fetch::{Fetch, WorldQuery, FetchState, ReadFetch, ReadState},
		access::FilteredAccess,
	},
	archetype::{Archetype, ArchetypeId, ArchetypeComponentId},
	storage::LocalVersion,
	component::{Component, MultiCaseImpl},
	world::World,
};

pub struct WithOut<T>(PhantomData<T>);

impl<T: Component> WorldQuery for WithOut<T> {
    type Fetch = WithOutFetch<T>;
    type State = WithOutState<T>;
}

pub struct WithOutFetch<T> {
	pub(crate) read_fetch: ReadFetch<T>,
	not_component: bool,
    marker: PhantomData<T>,
}
pub struct WithOutState<T> {
	pub(crate) read_state: ReadState<T>,
    marker: PhantomData<T>,
}

// SAFE: no component access or archetype component access
unsafe impl<T: Component> FetchState for WithOutState<T> {
    fn init(world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self {
        Self {
			read_state: ReadState::init(world, query_id, archetype_id),
            marker: PhantomData,
        }
    }

    #[inline]
    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut FilteredAccess<ArchetypeComponentId>) {
		self.read_state.update_archetype_component_access(archetype, access);
    }
	
    fn matches_archetype(&self, _archetype: &Archetype,) -> bool {
		true
    }
}

impl<'s, T: Component> Fetch<'s> for WithOutFetch<T> {
    type Item = bool;
    type State = WithOutState<T>;

    unsafe fn init(
        world: &World,
        state: &Self::State
    ) -> Self {
        Self {
			not_component: true,
			read_fetch: ReadFetch::init(world, &state.read_state),
            marker: PhantomData,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
		_world: &World
    ) {
		self.not_component = !archetype.contains(state.read_state.component_id);
		if !self.not_component {
			self.read_fetch.set_archetype(&state.read_state, archetype, _world);
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, _archetype_index: LocalVersion) -> Option<Self::Item> {
        Some(true)
    }

	#[inline]
    unsafe fn archetype_fetch_unchecked(&mut self, _local: LocalVersion) -> Self::Item {
        true
    }
}

impl<T: Component> FilterFetch for WithOutFetch<T> {
	unsafe fn archetype_filter_fetch(&mut self, local: LocalVersion) -> bool {
		if self.not_component {
			return true;
		} else {
			let r:bool = std::mem::transmute((&mut *(self.read_fetch.container as *mut MultiCaseImpl<T>)).contains_key(&local));
			!r
		}
	}
}