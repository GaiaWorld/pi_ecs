use std::marker::PhantomData;

use super::interface::{WorldQuery, ReadOnlyFetch, FetchState, Fetch};

use crate::{
	archetype::{Archetype, ArchetypeId, ArchetypeIdent},
	storage::LocalVersion,
	component::ComponentId,
	query::access::FilteredAccess,
	world::World,
	entity::Id,
};

/// 为实例实现WorldQuery
impl<T: ArchetypeIdent> WorldQuery for Id<T> {
    type Fetch = IdFetch<T>;
    type State = IdState;
}

pub struct IdFetch<T> {
    // entities: *const Entity,
	// iter: MaybeUninit<Keys<'static, LocalVersion, ()>>,
	archetype_id: ArchetypeId,
	mark: PhantomData<T>,
}

/// SAFE: access is read only
unsafe impl<T> ReadOnlyFetch for IdFetch<T> {}

pub struct IdState;

// SAFE: no component or archetype access
unsafe impl FetchState for IdState {
	#[inline]
    fn init(_world: &mut World, _query_id: usize, _archetype_id: ArchetypeId) -> Self {
        Self
    }

	#[inline]
    fn update_archetype_component_access(&self, _archetype: &Archetype, _access: &mut FilteredAccess<ComponentId>) {}

    #[inline]
    fn matches_archetype(&self, _archetype: &Archetype,) -> bool {
        true
    }
}

impl<'s, T: ArchetypeIdent> Fetch<'s> for IdFetch<T> {
    type Item = Id<T>;
    type State = IdState;

    unsafe fn init(
        _world: &World,
        _state: &Self::State
    ) -> Self {
        Self {
			archetype_id: ArchetypeId::default(),
            // entities: std::ptr::null::<Entity>(),
			mark: PhantomData,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        _state: &Self::State,
        archetype: &Archetype,
		_world: &World,
    ) {
		self.archetype_id = archetype.id();
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
		Some(Id(local, PhantomData))
		// match self.iter.assume_init_mut().next() {
		// 	Some(local) => Some(Entity::new(self.archetype_id, local)),
		// 	None => None,
		// } 
    }

	unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
		Id(local, PhantomData)
	}
}