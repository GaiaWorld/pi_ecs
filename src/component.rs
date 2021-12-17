/// 组件

use std::any::{TypeId, type_name};
use std::collections::hash_map::Entry;

use thiserror::Error;
use hash::XHashMap;


use crate::storage::{Local, Offset};

pub trait Component: Send + Sync + 'static {}

impl<C: Send + Sync + 'static> Component for C  {}

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
    // pub(crate) fn add(
    //     &mut self,
    //     descriptor: ComponentDescriptor,
    // ) -> Result<ComponentId, ComponentsError> {
    //     let index = self.components.len();
    //     if let Some(type_id) = descriptor.type_id {
    //         let index_entry = self.indices.entry(type_id);
    //         if let Entry::Occupied(_) = index_entry {
    //             return Err(ComponentsError::ComponentAlreadyExists {
    //                 type_id,
    //                 name: descriptor.name,
    //             });
    //         }
    //         self.indices.insert(type_id, index);
    //     }
    //     self.components
    //         .push(ComponentInfo::new(ComponentId::new(index), descriptor));

    //     Ok(ComponentId::new(index))
    // }

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
