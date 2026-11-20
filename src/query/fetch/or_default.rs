use std::{
	marker::PhantomData,
};

use super::{
	interface::{WorldQuery, FetchState, Fetch, DefaultComponent, ReadOnlyFetch},
	ref_ty::{ReadState, ReadFetch},
};

use crate::{
	archetype::{Archetype, ArchetypeId, ArchetypeComponentId},
	storage::LocalVersion,
	component::{ComponentId, Component},
	query::access::FilteredAccess,
	world::World,
};

/// 如果组件不存在，则设置一个默认值
pub struct OrDefault<T>(PhantomData<T>);

impl<T: Component + Default> WorldQuery for OrDefault<T> {
    type Fetch = OrDefaultFetch<T>;
    type State = OrDefaultState<T>;
}

pub struct OrDefaultFetch<T> {
	default_id: ComponentId,
	fetch: ReadFetch<T>,
	world: World,
    matches: bool,
}

/// SAFE: OrDefaultFetch is read only because T is read only
unsafe impl<T: Component> ReadOnlyFetch for OrDefaultFetch<T> {}

pub struct OrDefaultState<T: Component>{
	default_id: ComponentId,
    state: ReadState<T>,
	matchs: bool,
}

// SAFE: component access and archetype component access are properly updated according to the
// internal Fetch
unsafe impl<T: Component + Default> FetchState for OrDefaultState<T> {
    fn init(world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self {
		let id = match world.get_resource_id::<DefaultComponent<T>>() {
			Some(r) => r.clone(),
			None => world.insert_resource(DefaultComponent(T::default())).id(),
		};

        Self {
			default_id: id,
            state: ReadState::<T>::init(world, query_id, archetype_id),
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

impl<'s, T: Component + Default> Fetch<'s> for OrDefaultFetch<T> {
    type Item = &'s T;
    type State = OrDefaultState<T>;

    unsafe fn init(
        world: &World,
        state: &Self::State
    ) -> Self {
        Self {
			default_id: state.default_id,
			world: world.clone(),
            fetch: ReadFetch::<T>::init(world, &state.state),
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
			log::warn!("component is not exist in archetype, so query fail, query: {:?}",  std::any::type_name::<OrDefault<T>>());
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
		if self.matches {
			match self.fetch.archetype_fetch(local) {
				Some(r) => Some(std::mem::transmute(r)),
				None => std::mem::transmute(self.world.archetypes().get_resource::<T>(self.default_id))
			}
		} else {
			None
		}
    }

	unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
		if self.matches {
			match self.fetch.archetype_fetch(local) {
				Some(r) => std::mem::transmute(r),
				None => std::mem::transmute(self.world.archetypes().get_resource::<T>(self.default_id).unwrap())
			}
		} else {
			std::mem::transmute(self.world.archetypes().get_resource::<T>(self.default_id))
		}
    }
}