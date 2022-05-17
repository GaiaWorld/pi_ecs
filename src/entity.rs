use std::{
	marker::{PhantomData, Copy},
	hash::Hash,
	cmp::{Eq,  Ord, PartialEq, PartialOrd},
	fmt::Debug,
};
use std::ops::{Deref, DerefMut};

use pi_null::Null;
pub use pi_slotmap::{Key, KeyData};

use crate::monitor::{NotifyImpl, Notify};
use crate::storage::{LocalVersion, DelaySlotMap, Local, Offset};
use crate::archetype::ArchetypeId;

#[derive(Clone, Copy, Hash, Eq, Ord, PartialEq, PartialOrd, Debug)]
pub struct Entity {
	archetype_id: ArchetypeId,
	local: LocalVersion,
}

pub struct Id<T>(pub(crate) LocalVersion, pub(crate) PhantomData<T>);

impl<T> Id<T> {
	pub unsafe fn new(local_version: LocalVersion) -> Self {
		Self(local_version, PhantomData)
	}
}

impl<T> Offset for Id<T> {
    fn offset(&self) -> usize {
        self.0.offset()
    }
}

unsafe impl<T> Key for Id<T> {
	#[inline]
    fn data(&self) -> KeyData {
		self.0.data()
	}
}

impl<T> From<KeyData> for Id<T> {
	#[inline]
    fn from(data: KeyData) -> Self {
		Id(LocalVersion::from(data), PhantomData)
	}
}

impl<T> Default for Id<T> {
    fn default() -> Self {
        Self(LocalVersion::default(), PhantomData)
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<T> Copy for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T> Eq for Id<T> {
    fn assert_receiver_is_total_eq(&self) {
		self.0.assert_receiver_is_total_eq()
	}
}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<T> Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Id").field(&self.0).finish()
    }
}

impl<T> Null for Id<T> {
	fn null() -> Self {
		Id(<LocalVersion as Null>::null(), PhantomData)
	}
	fn is_null(&self) -> bool {
		<LocalVersion as Null>::is_null(&self.0)
	}
}

impl Null for Entity {
	fn null() -> Self {
		Self {
			archetype_id: ArchetypeId::new(0),
			local: <LocalVersion as Null>::null(),
		}
	}

	fn is_null(&self) -> bool {
		<LocalVersion as Null>::is_null(&self.local())
	}
}

impl Default for Entity { 
	fn default() -> Self {
		Self{archetype_id: ArchetypeId::new(0), local: LocalVersion(0)}
	}
}

impl Entity {
	pub fn new(archetype_id: ArchetypeId, local: LocalVersion) -> Self {
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