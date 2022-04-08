use std::mem::forget;

use pi_map::Map;
use pi_null::Null;

use crate::{
	monitor::{Notify, NotifyImpl, Listener, EventType},
	entity::Entity, storage::SecondaryMap, component::ComponentId
};

// TODO 以后用宏生成
impl Notify for SingleMeta {
    fn add_create(&self, listener: Listener) {
        self.notify.add_create(listener);
    }
    fn add_delete(&self, listener: Listener) {
        self.notify.add_delete(listener)
    }
    fn add_modify(&self, listener: Listener) {
        self.notify.add_modify(listener)
    }
    fn create_event(&self, id: Entity) {
        self.notify.create_event(id);
    }
    fn delete_event(&self, id: Entity) {
        self.notify.delete_event(id);
    }
    fn modify_event(&self, id: Entity, field: &'static str, index: usize) {
        self.notify.modify_event(id, field, index);
    }
    fn remove_create(&self, listener: &Listener) {
        self.notify.remove_create(listener);
    }
    fn remove_delete(&self, listener: &Listener) {
        self.notify.remove_delete(listener);
    }
    fn remove_modify(&self, listener: &Listener) {
        self.notify.remove_modify(listener);
    }
}

pub(crate) struct SingleMeta {
    // value: usize, // *mut T
	size: usize,
	index: usize,
    notify: NotifyImpl,
	drop_fn: fn(*const u8),
}

impl SingleMeta {
    pub fn new<T>(size: usize, index: usize) -> Self {
        Self {
            size,
			index,
            notify: NotifyImpl::default(),
			drop_fn: drop::<T>,
        }
    }

    pub fn get_notify_ref(&self) -> &NotifyImpl {
        &self.notify
    }
}

pub(crate) struct Singles {
	buffer: Vec<u8>,
	metas: SecondaryMap<ComponentId, SingleMeta>,
}

impl Singles {
	pub fn new() -> Self {
		Self {
			buffer: Vec::new(),
			metas: SecondaryMap::with_capacity(0),
		}
	}

	pub fn register<T>(&mut self, component_id: ComponentId) {
		// 不存在元信息，插入元信息
		if self.metas.get(&component_id).is_none() {
			let size = std::mem::size_of::<T>();

			self.metas.insert(component_id, SingleMeta::new::<T>(usize::null(), self.buffer.len()));
			// 设置长度
			self.buffer.reserve(size);
			unsafe{self.buffer.set_len(self.buffer.len() + size)};
		};
	}

	pub fn add_listener<E: EventType, T>(&mut self, component_id: ComponentId, listener: Listener) {
		if let Some(meta) = self.metas.get(&component_id) {
			E::add(&meta.notify, listener);
		} else {
			log::warn!("add_resource_listener fail, resource is not exist: {:?}", std::any::type_name::<T>());
		}
	}

	/// 安全： 确保T和component_id的一致性, 同时存在meta
	pub unsafe fn insert<T: 'static + Send + Sync>(&mut self, component_id: ComponentId, value: T) {
		let size = std::mem::size_of::<T>();
		let meta = &mut self.metas[component_id];

		std::ptr::copy_nonoverlapping(
			&value as *const T as *const u8,
			self.buffer.as_mut_ptr().add(meta.index),
			size,
		);
		forget(value);

		if meta.size.is_null() {
			meta.size = size;
			meta.get_notify_ref().create_event(Entity::null());
		} else {
			meta.get_notify_ref().modify_event(Entity::null(), "", 0);
		}

	}

	/// 安全： 确保T和component_id的一致性
	pub unsafe fn get<T>(&self, component_id: ComponentId) -> Option<&T> {
		if let Some(meta) = self.metas.get(&component_id) {
			Some(&*(self.buffer.as_ptr().add(meta.index) as usize as *mut T))
		} else {
			None
		}
	}

	/// 安全： 确保T和component_id的一致性
	pub unsafe fn get_mut<T>(&self, component_id: ComponentId) -> Option<&mut T> {
		if let Some(meta) = self.metas.get(&component_id) {
			Some(&mut *(self.buffer.as_ptr().add(meta.index) as usize as *mut T))
		} else {
			None
		}
	}

	// ///  确保T和component_id的一致性
	// pub unsafe fn remove<T>(&mut self, component_id: ComponentId) -> Option<T> {
	// 	if let Some(meta) = self.metas.get_mut(&component_id) {
	// 		if meta.size.is_null() {
	// 			return None;
	// 		}
	// 		meta.size = usize::null();
	// 		Some(self.buffer.as_ptr().add(meta.index).cast::<T>().read_unaligned())
	// 	} else {
	// 		None
	// 	}
	// }

	pub fn get_notify_ref(&self, component_id: ComponentId) -> &NotifyImpl {
		if let Some(meta) = self.metas.get(&component_id) {
			&meta.notify
		} else {
			log::error!("get_notify err");
			panic!()
		}
	}
}

/// 销毁T
fn drop<T>(ptr: *const u8) {
	unsafe {ptr.cast::<T>().read_unaligned()};
}

impl Drop for Singles {
	fn drop(&mut self) {
		for (_local, meta) in self.metas.iter() {
			(meta.drop_fn)(unsafe { self.buffer.as_ptr().add(meta.index) });
		}
	}
}

// fn drop<T>(ptr: usize) {
// 	unsafe {ptr as usize as *mut T}
// }