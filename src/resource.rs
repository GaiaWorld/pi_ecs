use std::mem::forget;

use pi_map::Map;
use pi_null::Null;
use pi_share::ThreadSync;

use crate::{
	monitor::{Notify, NotifyImpl, Listener, ListenType},
	entity::Entity, storage::{SecondaryMap, Local}, component::ComponentTicks
};

pub trait Resource: ThreadSync + 'static{}

impl<T: ThreadSync + 'static> Resource for T {}

pub type ResourceId = Local;


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
	buffer: Vec<u8>,
	is_exsit: bool, // 标记组件是否存在（之一，曾经使用buffer的长度为0来表示组件不存在，但组件的大小可能就是0，因此这种方式是不可行的，因此单独使用一个bool字段来表示资源是否存在）
    notify: NotifyImpl,
	tick: ComponentTicks,
	drop_fn: fn(*const u8),
}

impl SingleMeta {
    pub fn new<T>(size: usize) -> Self {
        Self {
			buffer: Vec::with_capacity(size),
			is_exsit: false,
            notify: NotifyImpl::default(),
			tick: ComponentTicks::new(0),
			drop_fn: drop::<T>,
        }
    }

    pub fn get_notify_ref(&self) -> &NotifyImpl {
        &self.notify
    }
}

pub(crate) struct Singles {
	metas: SecondaryMap<ResourceId, SingleMeta>,
}

impl Singles {
	pub fn new() -> Self {
		Self {
			metas: SecondaryMap::with_capacity(0),
		}
	}

	pub fn register<T>(&mut self, resource_id: ResourceId) {
		// 不存在元信息，插入元信息
		if self.metas.get(&resource_id).is_none() {
			let size = std::mem::size_of::<T>();
			self.metas.insert(resource_id, SingleMeta::new::<T>(size));
		};
	}

	pub fn add_listener<E: ListenType, T>(&mut self, resource_id: ResourceId, listener: Listener) {
		if let Some(meta) = self.metas.get(&resource_id) {
			E::add(&meta.notify, listener);
		} else {
			log::warn!("add_resource_listener fail, resource is not exist: {:?}", std::any::type_name::<T>());
		}
	}

	/// 安全： 确保T和resource_id的一致性, 同时存在meta
	pub unsafe fn insert<T: Resource>(&mut self, resource_id: ResourceId, value: T, tick: u32) {
		let size = std::mem::size_of::<T>();
		let meta = &mut self.metas[resource_id];

		if meta.is_exsit {
			// 资源已经存在，销毁原有的
			(meta.drop_fn)(meta.buffer.as_ptr());
		}

		// 写入资源
		std::ptr::copy_nonoverlapping(
			&value as *const T as *const u8,
			meta.buffer.as_mut_ptr(),
			size,
		);
		forget(value);

		let is_exsit = meta.is_exsit;
		meta.is_exsit = true;
		meta.tick.changed = tick;
		// 通知
		if !is_exsit {
			meta.get_notify_ref().create_event(Entity::null());
			meta.tick.added = tick;
		} else {
			meta.get_notify_ref().modify_event(Entity::null(), "", 0);
		}

	}

	// /// 取到资源当前节拍
	// pub fn get_tick(&self, resource_id: ResourceId) -> Option<&ComponentTicks> {
	// 	if let Some(meta) = self.metas.get(&resource_id) {
	// 		if meta.is_exsit {
	// 			Some(&meta.tick)
	// 		} else {
	// 			None
	// 		}
	// 	} else {
	// 		None
	// 	}
	// }

	/// 取到资源当前节拍
	/// 如果资源不存在，将panic
	pub unsafe fn get_tick_unchecked(&self, resource_id: ResourceId) -> &ComponentTicks {
		&self.metas.get_unchecked(&resource_id).tick
	}

	/// 安全： 确保T和resource_id的一致性
	pub unsafe fn get<T>(&self, resource_id: ResourceId) -> Option<&T> {
		if let Some(meta) = self.metas.get(&resource_id) {
			if meta.is_exsit {
				Some(&*(meta.buffer.as_ptr() as usize as *mut T))
			} else {
				None
			}
		} else {
			None
		}
	}

	pub unsafe fn get_unchecked<T>(&self, resource_id: ResourceId) -> &T {
		let meta = self.metas.get_unchecked(&resource_id);
		&*(meta.buffer.as_ptr() as usize as *mut T)
	}

	pub unsafe fn get_unchecked_mut<T>(&self, resource_id: ResourceId) -> &mut T {
		let meta = self.metas.get_unchecked(&resource_id);
		&mut *(meta.buffer.as_ptr() as usize as *mut T)
	}

	/// 安全： 确保T和resource_id的一致性
	pub unsafe fn get_mut<T>(&self, resource_id: ResourceId) -> Option<&mut T> {
		if let Some(meta) = self.metas.get(&resource_id) {
			if meta.is_exsit {
				Some(&mut *(meta.buffer.as_ptr() as usize as *mut T))
			} else {
				None
			}
		} else {
			None
		}
	}

	// ///  确保T和resource_id的一致性
	// pub unsafe fn remove<T>(&mut self, resource_id: ResourceId) -> Option<T> {
	// 	if let Some(meta) = self.metas.get_mut(&resource_id) {
	// 		if meta.size.is_null() {
	// 			return None;
	// 		}
	// 		meta.size = usize::null();
	// 		Some(self.buffer.as_ptr().add(meta.index).cast::<T>().read_unaligned())
	// 	} else {
	// 		None
	// 	}
	// }

	pub fn get_notify_ref(&self, resource_id: ResourceId) -> &NotifyImpl {
		if let Some(meta) = self.metas.get(&resource_id) {
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
			if meta.is_exsit {
				(meta.drop_fn)(meta.buffer.as_ptr());
			}
		}
	}
}

// fn drop<T>(ptr: usize) {
// 	unsafe {ptr as usize as *mut T}
// }