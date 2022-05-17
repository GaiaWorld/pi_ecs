use super::interface::{WorldQuery, FetchState, Fetch, ReadOnlyFetch};

use crate::{
	archetype::{Archetype, ArchetypeId, ArchetypeComponentId},
	storage::LocalVersion,
	query::access::FilteredAccess,
	world::World,
};

impl<T: WorldQuery> WorldQuery for Option<T> {
    type Fetch = OptionFetch<T::Fetch>;
    type State = OptionState<T::State>;
}

pub struct OptionFetch<T> {
    fetch: T,
    matches: bool,
}

/// SAFE: OptionFetch is read only because T is read only
unsafe impl<T: ReadOnlyFetch> ReadOnlyFetch for OptionFetch<T> {}

pub struct OptionState<T: FetchState> {
    state: T,
	matchs: bool,
}

// SAFE: component access and archetype component access are properly updated according to the
// internal Fetch
unsafe impl<T: FetchState> FetchState for OptionState<T> {
    fn init(world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self {
        Self {
            state: T::init(world, query_id, archetype_id),
			matchs: false
        }
    }

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut FilteredAccess<ArchetypeComponentId>) {
		if self.matchs {
			self.state.update_archetype_component_access(archetype, access);
		}
    }

    fn matches_archetype(&self, _archetype: &Archetype) -> bool {
        true
	}
}

impl<'s, T: Fetch<'s>> Fetch<'s> for OptionFetch<T> {
    type Item = Option<T::Item>;
    type State = OptionState<T::State>;

    unsafe fn init(
        world: &World,
        state: &Self::State
    ) -> Self {
        Self {
            fetch: T::init(world, &state.state),
            matches: false,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
		world: &World,
    ) {
		self.matches = state.state.matches_archetype(archetype);
		if self.matches {
        	self.fetch.set_archetype(&state.state, archetype, world);
		} else {
			log::warn!("component is not exist in archetype, so query fail, query: {:?}",  std::any::type_name::<Option<T>>());
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
		if self.matches {
        	Some(self.fetch.archetype_fetch(local))
		} else {
			None
		}
    }

	#[inline]
    unsafe fn archetype_fetch_unchecked (&mut self, local: LocalVersion) -> Self::Item {
		if self.matches {
        	self.fetch.archetype_fetch(local)
		} else {
			None
		}
    }
}