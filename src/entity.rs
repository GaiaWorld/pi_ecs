use std::ops::{Deref, DerefMut};

use crate::monitor::{NotifyImpl, Notify};
use crate::storage::{LocalVersion, DelaySlotMap, Local};
use crate::archetype::ArchetypeId;

#[derive(Clone, Copy, Hash, Eq, Ord, PartialEq, PartialOrd, Debug)]
pub struct Entity {
	archetype_id: ArchetypeId,
	local: LocalVersion,
}
impl Default for Entity { 
	fn default() -> Self {
		Self{archetype_id: ArchetypeId::new(0), local: LocalVersion(0)}
	}
}

impl Entity {
	pub(crate) fn new(archetype_id: ArchetypeId, local: LocalVersion) -> Self {
		Self{archetype_id, local}
	}

	pub fn archetype_id(&self) -> ArchetypeId {
		self.archetype_id
	}

	pub fn local(&self) -> LocalVersion {
		self.local
	}
}

pub struct Entities {
	arch_id: Local,
	storage: DelaySlotMap<LocalVersion, ()>,
	//实体变化监听器
	pub(crate) entity_listners: NotifyImpl,
}

impl Deref for Entities {
	type Target = DelaySlotMap<LocalVersion, ()>;

    fn deref(&self) -> &Self::Target {
		&self.storage
	}
}


impl DerefMut for Entities {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.storage
	}
}

impl Entities {
	pub fn new(arch_id: Local) -> Self {
		Self {
			arch_id,
			storage: DelaySlotMap::default(),
			entity_listners: NotifyImpl::default(),
		}
	}

	pub fn remove(&mut self, local: LocalVersion) -> Option<()> {
		if self.storage.contains_key(local) {
			self.entity_listners.delete_event(Entity::new(self.arch_id, local));
		}
		self.storage.remove(local)
	}

	pub fn insert(&mut self) -> LocalVersion {
		let local = self.storage.insert(());
		self.entity_listners.create_event(Entity::new(self.arch_id, local));
		local
	}

	pub fn flush(&mut self) {
		let (storage, entity_listners) = (
			unsafe{&mut *(&self.storage as *const DelaySlotMap<LocalVersion, ()> as usize as *mut DelaySlotMap<LocalVersion, ()>)}, 
			&self.entity_listners
		);
		unsafe{	storage.flush( |s, local| {
				s.try_insert_with_key::<_, Never>(move |_k| {Ok(())}).unwrap_unchecked();
				entity_listners.create_event(Entity::new(self.arch_id, local));
			});
		}
	}
}

pub enum Never {}