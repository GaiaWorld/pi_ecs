/// 组件
use std::{any::{TypeId, type_name}, ops::Index, ops::IndexMut};
use std::collections::hash_map::Entry;
use map::{Map, vecmap::VecMap};

use share::cell::TrustCell;
use thiserror::Error;
use hash::XHashMap;
use any::ArcAny;


use crate::{
	storage::{LocalVersion, Local, Offset, SecondaryMap},
	monitor::{Notify, NotifyImpl, Listener}, entity::Entity,
};

pub trait ComponentStorage {
	type Type: Map<Key = LocalVersion, Val = Self> + Index<LocalVersion, Output=Self> + IndexMut<LocalVersion, Output=Self> + Sync + Send + 'static;
}

pub trait Component: Send + Sync + 'static {
	type Storage: Map<Key = LocalVersion, Val = Self> + Index<LocalVersion, Output=Self> + IndexMut<LocalVersion, Output=Self> + Sync + Send + 'static;
}

impl<C> Component for C where C: Send + Sync + 'static {
	default type Storage = SecondaryMap<LocalVersion, Self>;
}

impl<C> Component for C where C: ComponentStorage + Send + Sync + 'static{
	type Storage = <C as ComponentStorage>::Type;
}

pub trait MultiCase: ArcAny {
    fn delete(&self, id: LocalVersion);
}

pub type CellMultiCase<C> = TrustCell<MultiCaseImpl<C>>;

pub struct MultiCaseImpl<C: Component> {
    map: C::Storage,
    notify: NotifyImpl,
	archetype_id: Local,
	ticks: VecMap<ComponentTicks>,
}

impl<C: Component> MultiCaseImpl<C> {
    pub fn get_storage(&self) -> &C::Storage {
        &self.map
    }

    pub fn get_storage_mut(&mut self) -> &mut C::Storage {
        &mut self.map
    }
}

impl<C: Component> Index<LocalVersion> for MultiCaseImpl<C> {
    type Output = C;

    fn index(&self, index: LocalVersion) -> &C {
        &self.map[index]
    }
}

impl<C: Component> IndexMut<LocalVersion> for MultiCaseImpl<C> {
    fn index_mut(&mut self, index: LocalVersion) -> &mut C {
        &mut self.map[index]
    }
}

impl< C: Component> MultiCaseImpl<C> {
    pub fn with_capacity(capacity: usize, archetype_id: Local) -> Self {
        MultiCaseImpl {
            map: C::Storage::with_capacity(capacity),
            notify: NotifyImpl::default(),
			archetype_id,
			ticks: VecMap::default(),
        }
	}

    pub fn mem_size(&self) -> usize {
        self.map.mem_size() + self.notify.mem_size()
    }
    pub fn get(&self, id: LocalVersion) -> Option<&C> {
        self.map.get(&id)
    }
    pub fn get_mut(&mut self, id: LocalVersion) -> Option<&mut C> {
        self.map.get_mut(&id)
    }
    pub unsafe fn get_unchecked(&self, id: LocalVersion) -> &C {
        self.map.get_unchecked(&id)
    }
    pub unsafe fn get_unchecked_mut(&mut self, id: LocalVersion) -> &mut C {
        self.map.get_unchecked_mut(&id)
    }
    
    pub fn insert(&mut self, id: LocalVersion, c: C, tick: u32) -> Option<C> {
        let r = self.map.insert(id, c);
        match r {
            Some(_) => {
				self.ticks[id.offset()].changed = tick;
				self.notify.modify_event(Entity::new(self.archetype_id, id), "", 0)
			}
            _ => {
				self.ticks.insert(id.offset(), ComponentTicks::new(tick));
                self.notify.create_event(Entity::new(self.archetype_id, id));
            }
        }
        r
    }

	pub fn tick(&self, id: LocalVersion) -> Option<&ComponentTicks> {
		self.ticks.get(id.offset())
	}

    pub fn insert_no_notify(&mut self, id: LocalVersion, c: C) -> Option<C> {
        let r = self.map.insert(id, c);
        r
    }

    pub fn delete(&mut self, id: LocalVersion) -> Option<C> {
		if self.map.get(&id).is_some() {
			self.notify.delete_event(Entity::new( self.archetype_id, id));
			self.map.remove(&id)
		} else {
			None
		}
    }

    pub fn get_notify(&self) -> NotifyImpl {
        self.notify.clone()
    }

    pub fn get_notify_ref(&self) -> &NotifyImpl {
        &self.notify
    }

	pub fn contains_key(&self, local: &LocalVersion) -> bool {
		self.map.contains(local)
	}
}

impl_downcast_arc!(MultiCase);

impl<C: Component> MultiCase for CellMultiCase<C> {
    fn delete(&self, id: LocalVersion) {
        // 实体删除，组件不再监听删除事件
        self.borrow_mut().map.remove(&id);
    }
}

impl<C: Component> Notify for CellMultiCase<C>{
    fn add_create(&self, listener: Listener) {
        self.borrow_mut().notify.add_create(listener);
    }
    fn add_delete(&self, listener: Listener) {
        self.borrow_mut().notify.add_delete(listener)
    }
    fn add_modify(&self, listener: Listener) {
        self.borrow_mut().notify.add_modify(listener)
    }
    fn create_event(&self, id: Entity) {
        self.borrow().notify.create_event(id);
    }
    fn delete_event(&self, id: Entity) {
        self.borrow().notify.delete_event(id);
    }
    fn modify_event(&self, id: Entity, field: &'static str, index: usize) {
        self.borrow().notify.modify_event(id, field, index);
    }
    fn remove_create(&self, listener: &Listener) {
        self.borrow_mut().notify.remove_create(listener);
    }
    fn remove_delete(&self, listener: &Listener) {
        self.borrow_mut().notify.remove_delete(listener);
    }
    fn remove_modify(&self, listener: &Listener) {
        self.borrow_mut().notify.remove_modify(listener);
    }
}


pub type ComponentId = Local;

pub struct ComponentInfo {
	pub(crate) storage_type: StorageType,
	pub(crate) id: ComponentId,
	pub(crate) name: &'static str,
}

impl ComponentInfo {
	pub fn storage_type(&self) -> StorageType {
		self.storage_type
	}

	pub fn id(&self) -> ComponentId {
		self.id
	}

	pub fn name(&self) -> &'static str {
		self.name
	}
}

pub struct Components {
    pub(crate) infos: Vec<ComponentInfo>,
    indices: XHashMap<TypeId, usize>,
    resource_indices: XHashMap<TypeId, ComponentId>,
}

#[derive(Debug, Error)]
pub enum ComponentsError {
    #[error("A component of type {name:?} ({type_id:?}) already exists")]
    ComponentAlreadyExists { type_id: TypeId, name: String },
}

impl Components {
	pub fn new() -> Self {
		Self {
			infos: Vec::new(),
			indices: XHashMap::default(),
			resource_indices: XHashMap::default(),
		}
	}

	#[inline]
    pub(crate) fn get_or_insert_resource_id<T: Component>(&mut self) -> ComponentId {
		match self.resource_indices.entry(TypeId::of::<T>()) {
			Entry::Occupied(r) => *r.get(),
			Entry::Vacant(r) =>  {
				let index = self.infos.len();
				let index = ComponentId::new(index);
				self.infos.push(ComponentInfo{
					id: index, 
					storage_type: StorageType::Table,
					name: type_name::<T>(),
				});
				r.insert(index);
				index
			}
		}
    }

	pub(crate) fn get_resource_id<T: Component>(&self) -> Option<&ComponentId> {
		self.resource_indices.get(&TypeId::of::<T>())
	}

    #[inline]
    pub fn get_or_insert_id<T: Component>(&mut self) -> ComponentId {
        self.get_or_insert_with(TypeId::of::<T>(), std::any::type_name::<T>())
    }

    #[inline]
    pub fn get_or_insert_info<T: Component>(&mut self) -> &ComponentInfo {
        let id = self.get_or_insert_id::<T>();
        // SAFE: component_info with the given `id` initialized above
        unsafe { self.get_info_unchecked(id) }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.infos.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.infos.len() == 0
    }

    #[inline]
    pub fn get_info(&self, id: ComponentId) -> Option<&ComponentInfo> {
        self.infos.get(*id)
    }

    /// # Safety
    /// `id` must be a valid [ComponentId]
    #[inline]
    pub unsafe fn get_info_unchecked(&self, id: ComponentId) -> &ComponentInfo {
        debug_assert!(id.offset() < self.infos.len());
        self.infos.get_unchecked(*id)
    }

    #[inline]
    pub fn get_id(&self, type_id: TypeId) -> Option<ComponentId> {
        self.indices.get(&type_id).map(|index| ComponentId::new(*index))
    }

    #[inline]
    pub(crate) fn get_or_insert_with(
        &mut self,
        type_id: TypeId,
		name: &'static str,
    ) -> ComponentId {
        let components = &mut self.infos;
        let index = self.indices.entry(type_id).or_insert_with(|| {
            let index = components.len();
            components.push(ComponentInfo{
				id: ComponentId::new(index), 
				storage_type: StorageType::Table,
				name,
			});
            index
        });

        ComponentId::new(*index)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum StorageType {
    Table,
    SparseSet,
}

impl Default for StorageType {
    fn default() -> Self {
        StorageType::Table
    }
}


#[derive(Copy, Clone, Debug)]
pub struct ComponentTicks {
    pub(crate) added: u32,
    pub(crate) changed: u32,
}

impl ComponentTicks {
    #[inline]
    pub fn is_added(&self, last_change_tick: u32, change_tick: u32) -> bool {
        // The comparison is relative to `change_tick` so that we can detect changes over the whole
        // `u32` range. Comparing directly the ticks would limit to half that due to overflow
        // handling.
        let component_delta = change_tick.wrapping_sub(self.added);
        let system_delta = change_tick.wrapping_sub(last_change_tick);

        component_delta < system_delta
    }

    #[inline]
    pub fn is_changed(&self, last_change_tick: u32, change_tick: u32) -> bool {
        let component_delta = change_tick.wrapping_sub(self.changed);
        let system_delta = change_tick.wrapping_sub(last_change_tick);

        component_delta < system_delta
    }

    pub(crate) fn new(change_tick: u32) -> Self {
        Self {
            added: change_tick,
            changed: change_tick,
        }
    }

	#[allow(dead_code)]
    pub(crate) fn check_ticks(&mut self, _change_tick: u32) {
        // check_tick(&mut self.added, change_tick);
        // check_tick(&mut self.changed, change_tick);
    }

    /// Manually sets the change tick.
    /// Usually, this is done automatically via the [`DerefMut`](std::ops::DerefMut) implementation on [`Mut`](crate::world::Mut) or [`ResMut`](crate::system::ResMut) etc.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use bevy_ecs::{world::World, component::ComponentTicks};
    /// let world: World = unimplemented!();
    /// let component_ticks: ComponentTicks = unimplemented!();
    ///
    /// component_ticks.set_changed(world.read_change_tick());
    /// ```
    #[inline]
    pub fn set_changed(&mut self, change_tick: u32) {
        self.changed = change_tick;
    }
}
