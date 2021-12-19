use std::any::TypeId;
use std::collections::HashSet;
use std::ptr::NonNull;
/// 世界

use std::sync::atomic::{AtomicU32, Ordering};
// use hash::XHashMap;
use slotmap::{SecondaryMap, SparseSecondaryMap};

use crate::archetype::{Archetype, Archetypes, ArchetypeId, ArchetypeIdent};
use crate::component::{Components, ComponentId, Component, self};
use crate::entity::Entity;
use crate::query::{WorldQuery, QueryState};
use crate::storage::{LocalVersion, Offset, Local};

pub struct World {
	pub(crate) id:WorldId,
	pub(crate) components: Components,
	pub(crate) archetypes: Archetypes,

	pub(crate) change_tick: AtomicU32,
	pub(crate) last_change_tick: u32,
}

unsafe impl Sync for World {
	
}

unsafe impl Send for World {
	
}

impl World {
	pub fn new() -> Self {
		Self {
			id: WorldId(0),
			components: Components::new(),
			archetypes: Archetypes::new(),
			change_tick: AtomicU32::new(1),
			last_change_tick: 0,
		}
	}

	pub fn insert_resource<T: Component>(&mut self, value: T) {
		let id = self.components.get_or_insert_resource_id::<T>();
		self.archetypes.insert_resource::<T>(value, id);
	}

	pub fn get_resource_id<T: Component>(&self) -> Option<&ComponentId> {
		self.components.get_resource_id::<T>()
	}

	pub fn new_archetype<T: Send + Sync + 'static>(&mut self) -> ArchetypeInfo {
		if let Some(_r) = self.archetypes.get_id_by_ident(TypeId::of::<T>()) {
			panic!("new_archetype fial");
		}
		ArchetypeInfo {
			world: self,
			type_id: TypeId::of::<T>(),
			components: HashSet::default(),
			containers: vec![Vec::new()],
		}
	}

	pub fn spawn<T: Send + Sync + 'static>(&mut self) -> EntityRef {
		let archetype_id = match self.archetypes.get_id_by_ident(TypeId::of::<T>()) {
			Some(r) => r.clone(),
			None => {
				panic!("spawn fial")
			}
		};
		let(archetypes, components) = (&mut self.archetypes, &mut self.components);
		
		let e = archetypes.spawn::<T>(archetype_id);
		EntityRef {
			local: e.local(),
			archetype_id: archetype_id,
			archetype: archetypes.get_mut(archetype_id).unwrap(),
			components,
		}
	}

	pub fn query<A: ArchetypeIdent, Q: WorldQuery>(&mut self) -> QueryState<A, Q, ()> {
        QueryState::new(self)
    }

	pub fn archetypes(&self) -> &Archetypes {
		&self.archetypes
	}

	pub fn id(&self) -> WorldId {
        self.id
    }
	pub fn read_change_tick(&self) -> u32 {
        self.change_tick.load(Ordering::Acquire)
    }

    #[inline]
    pub fn change_tick(&mut self) -> u32 {
        *self.change_tick.get_mut()
    }

    #[inline]
    pub fn last_change_tick(&self) -> u32 {
        self.last_change_tick
    }
}

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, Default)]
pub struct WorldId(pub(crate) usize);

pub struct ArchetypeInfo<'a> {
	pub(crate) world: &'a mut World,
	pub(crate) type_id: TypeId,
	pub(crate) components: HashSet<ComponentId>,
	pub(crate) containers: Vec< Vec<NonNull<u8>> >,
}

impl<'a> ArchetypeInfo<'a> {
	pub fn register<C: Component>(&mut self) -> &mut Self{
		let id = self.world.components.get_or_insert_id::<C>();
		let r = self.components.insert(id);

		if r {
			let ty = self.world.components.infos[id.offset()].storage_type();
			self.containers[0].push(
				match ty {
					component::StorageType::SparseSet => {
						NonNull::new(Box::into_raw(Box::new(SparseSecondaryMap::<Local, C>::default())).cast::<u8>()).unwrap()
					},
					component::StorageType::Table => {
						NonNull::new(Box::into_raw(Box::new(SecondaryMap::<Local, C>::default())).cast::<u8>()).unwrap()
					},
				}
			);
		}
		self
	}

	pub fn create(&mut self) {
		let components = self.components.iter().map(|r| {r.clone()}).collect();
		let c = self.containers.pop().unwrap();
		self.world.archetypes.get_id_or_insert_by_ident(self.type_id, components, c);
	}
}

pub struct EntityRef<'a> {
	pub(crate) local: LocalVersion,
	pub(crate) archetype_id: ArchetypeId,
	pub(crate) archetype: &'a mut Archetype,
	pub(crate) components: &'a mut Components,
}

impl<'a> EntityRef<'a> {
	pub fn insert<C: Component>(&mut self, value: C) -> &mut Self  {
		let id = self.components.get_or_insert_id::<C>();
		let info = unsafe { self.components.get_info_unchecked(id)};
		self.archetype.insert_component(self.local, value, id , info.storage_type);
		self
	}

	pub fn id(&self) -> Entity {
		Entity::new(self.archetype_id, self.local)
	}
}

/// Creates `Self` using data from the given [World]
pub trait FromWorld {
    /// Creates `Self` using data from the given [World]
    fn from_world(world: &mut World) -> Self;
}

impl<T: Default> FromWorld for T {
    fn from_world(_world: &mut World) -> Self {
        T::default()
    }
}