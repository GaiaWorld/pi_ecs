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

/// Filter that selects entities with a component `T`
pub struct With<T>(PhantomData<T>);

impl<T: Component> WorldQuery for With<T> {
    type Fetch = WithFetch<T>;
    type State = WithState<T>;
}

pub struct WithFetch<T> {
	pub(crate) read_fetch: ReadFetch<T>,
    marker: PhantomData<T>,
}
pub struct WithState<T> {
	pub(crate) read_state: ReadState<T>,
    marker: PhantomData<T>,
}

// SAFE: no component access or archetype component access
unsafe impl<T: Component> FetchState for WithState<T> {
    fn init(world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self {
        Self {
			read_state: ReadState::init(world, query_id, archetype_id),
            marker: PhantomData,
        }
    }

    #[inline]
    fn update_archetype_component_access(&self, _archetype: &Archetype, _access: &mut FilteredAccess<ArchetypeComponentId>) {
    }
	
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
		archetype.contains(self.read_state.component_id)
    }
}

impl<'s, T: Component> Fetch<'s> for WithFetch<T> {
    type Item = bool;
    type State = WithState<T>;

    unsafe fn init(
        world: &World,
        state: &Self::State
    ) -> Self {
        Self {
			read_fetch: ReadFetch::init(world, &state.read_state),
            marker: PhantomData,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        _state: &Self::State,
        _archetype: &Archetype,
		_world: &World
    ) {
		self.read_fetch.set_archetype(&_state.read_state, _archetype, _world);
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, _local: LocalVersion) -> Option<Self::Item> {
        Some(true)
    }

	#[inline]
    unsafe fn archetype_fetch_unchecked(&mut self, _local: LocalVersion) -> Self::Item {
        true
    }
}

impl<T: Component> FilterFetch for WithFetch<T> {
	unsafe fn archetype_filter_fetch(&mut self, local: LocalVersion) -> bool {
		std::mem::transmute((&*(self.read_fetch.container as *mut MultiCaseImpl<T>)).contains_key(&local))
	}
}