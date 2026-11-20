// use super::interface::{WorldQuery, ReadOnlyFetch, FetchState, Fetch};

// use crate::{
// 	archetype::{Archetype, ArchetypeId},
// 	storage::LocalVersion,
// 	component::ComponentId,
// 	query::access::FilteredAccess,
// 	world::World,

// 	entity::Entity,
// };

// /// 为实例实现WorldQuery
// impl WorldQuery for Entity {
//     type Fetch = EntityFetch;
//     type State = EntityState;
// }

// pub struct EntityFetch {
//     // entities: *const Entity,
// 	// iter: MaybeUninit<Keys<'static, LocalVersion, ()>>,
// 	archetype_id: ArchetypeId,
// }

// /// SAFE: access is read only
// unsafe impl ReadOnlyFetch for EntityFetch {}

// pub struct EntityState;

// // SAFE: no component or archetype access
// unsafe impl FetchState for EntityState {
// 	#[inline]
//     fn init(_world: &mut World, _query_id: usize, _archetype_id: ArchetypeId) -> Self {
//         Self
//     }

// 	fn update_component_access(&self, _access: &mut FilteredAccess<ComponentId>) {
				
// 	}

// 	#[inline]
//     fn update_archetype_component_access(&self, _archetype: &Archetype, _access: &mut FilteredAccess<ComponentId>) {}

//     #[inline]
//     fn matches_archetype(&self, _archetype: &Archetype,) -> bool {
//         true
//     }
// }

// impl<'s> Fetch<'s> for EntityFetch {
//     type Item = Entity;
//     type State = EntityState;

//     unsafe fn init(
//         _world: &World,
//         _state: &Self::State
//     ) -> Self {
//         Self {
// 			archetype_id: ArchetypeId::default(),
//             // entities: std::ptr::null::<Entity>(),
//         }
//     }

//     #[inline]
//     unsafe fn set_archetype(
//         &mut self,
//         _state: &Self::State,
//         archetype: &Archetype,
// 		_world: &World,
//     ) {
// 		self.archetype_id = archetype.id();
//     }

//     #[inline]
//     unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
// 		Some(Entity::new(self.archetype_id, local))
// 		// match self.iter.assume_init_mut().next() {
// 		// 	Some(local) => Some(Entity::new(self.archetype_id, local)),
// 		// 	None => None,
// 		// } 
//     }

// 	unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
// 		Entity::new(self.archetype_id, local)
// 	}
// }