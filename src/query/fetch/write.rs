use std::{
	marker::PhantomData,
	sync::Arc,
	any::type_name,
};

use pi_share::cell::TrustCell;

use super::{interface::{WorldQuery, FetchState, Fetch, DefaultComponent}, ChangeTrackers};

use crate::{
	archetype::{Archetype, ArchetypeId, ArchetypeComponentId},
	storage::LocalVersion,
	component::{ComponentId, Component, MultiCaseImpl, ComponentTicks},
	query::access::FilteredAccess,
	world::{World, WorldInner}, resource::ResourceId,
};

pub struct Write<T>(PhantomData<T>);
pub struct WriteItem<'s, T: Component> {
	value: Option<&'s T>,
	container: usize,
	world: &'s World,
	default: ResourceId, // Option<&'s DefaultComponent<T>>
	local: LocalVersion,

	ticker: ChangeTrackers<T>,
	// tick: u32
}

impl<'s, T: Component> WriteItem<'s, T> {
	/// 取到不可变引用
    pub fn get<'a>(&'a self) -> Option<&'a T> {
		match &self.value {
			Some(r) => Some(r),
			None => None
		}
	}

	/// 取到可变引用
	pub fn get_mut<'a>(&'a mut self) -> Option<&'a mut T> {
		match self.value {
			Some(r) => Some(unsafe { &mut *(r as *const T as usize as *mut T)}),
			None => None
		}
	}

	/// 通知修改
	pub fn notify_modify(&mut self) {
		let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		c.notify_modify(self.local, self.ticker.change_tick);
	}

	pub fn notify_delete(&mut self) {
		let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		c.notify_delete(self.local, self.ticker.change_tick);
	}

	pub fn insert_no_notify(&mut self, value: T) {
		let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		c.insert_no_notify(self.local,value, self.ticker.change_tick);
		self.value = unsafe{std::mem::transmute(c.get_mut(self.local))};
	}

	/// 修改组件并通知监听函数
	pub fn write<'a>(&'a mut self, value: T) {
		let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		c.insert(self.local,value, self.ticker.change_tick);
		self.value = unsafe{std::mem::transmute(c.get_mut(self.local))};
	}

	/// 移除组件，并通知监听函数
	pub fn remove<'a>(&'a mut self) -> Option<T> {
		let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		c.delete(self.local)
	}

	pub fn get_default(&self) -> &T {
		return unsafe{std::mem::transmute(self.world.archetypes().get_resource::<DefaultComponent<T>>(self.default).unwrap())};
	}

	pub fn get_or_default(&self) -> &T {
		if let Some(r) = self.value  {
			return unsafe{std::mem::transmute(&mut *(r as *const T as usize as *mut T))}
		} else {
			return unsafe{std::mem::transmute(self.world.archetypes().get_resource::<DefaultComponent<T>>(self.default).unwrap())};
		}
		// let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		// c.insert_no_notify(self.local, default.clone());
		// let r: &'static mut T = unsafe{std::mem::transmute(c.get_unchecked_mut(self.local))};
		// self.value = Some(r);
		// self.value.as_ref().unwrap()
	}

	#[inline]
	pub fn is_changed(&self) -> bool {
		self.ticker.is_changed()
	}

	#[inline]
	pub fn is_added(&self) -> bool {
		self.ticker.is_added()
	}
}

impl<'s, T: Component + Clone> WriteItem<'s, T> {
	pub fn get_mut_or_default(&mut self) -> &mut T {
		if let Some(r) = self.value  {
			return unsafe{&mut *(r as *const T as *mut T)};
		}
		let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		match unsafe { self.world.archetypes().get_resource::<DefaultComponent<T>>(self.default) } {
			Some(d) => c.insert_no_notify(self.local, (*d).clone(), self.ticker.change_tick),
			None => panic!("get_mut_or_default fail, {:?} is not impl Default and not have DefaultComponent", type_name::<T>()),
		};
		
		let r: &'static mut T = unsafe{std::mem::transmute(c.get_unchecked_mut(self.local))};
		self.value = Some(r);
		unsafe { &mut *(r as *const T as usize as *mut T)}
	}
}

impl<T: Component> WorldQuery for Write<T> {
    type Fetch = WriteFetch<T>;
    type State = WriteState<T>;
}
pub struct WriteFetch<T: Component> {
	container: usize,
	default: ResourceId,
	world: World,
	matchs: bool,
	last_change_tick: u32,
	change_tick: u32,
	mark: PhantomData<T>,
}

impl<'s, T: Component> Fetch<'s> for WriteFetch<T> {
    type Item = WriteItem<'s, T>;
    type State = WriteState<T>;

    unsafe fn init(
        world: &World,
        state: &Self::State
    ) -> Self {
        Self {
			container: 0,
			matchs: false,
			last_change_tick: 0,
			change_tick: 0,
			default: state.default,
			world: world.clone(),
			mark: PhantomData,
        }
    }

	unsafe fn setting(
		&mut self, 
		_world: &WorldInner, last_change_tick: u32, change_tick: u32) {
		self.last_change_tick = last_change_tick;
		self.change_tick = change_tick;
	}

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
		_world: &World,
    ) {
		self.matchs = archetype.contains(state.component_id);
		// 没有对应的原型，则跳过
		if !self.matchs {
			log::warn!("component is not exist in archetype, so query fail, query: {:?}",  std::any::type_name::<Write<T>>());
			return;
		}

		let c = archetype.get_component(state.component_id);
		match c.clone().downcast() {
			Ok(r) => {
				let r: Arc<TrustCell<MultiCaseImpl<T>>> = r;
				self.container = (*r).as_ptr() as usize;
			},
			Err(_) => panic!("downcast fail")
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
        if !self.matchs {
			return None;
		}
		let value = std::mem::transmute((&mut *(self.container as *mut MultiCaseImpl<T>)).get_mut(local));
		let tick = (&mut *(self.container as *mut MultiCaseImpl<T>)).tick(local);
		Some(WriteItem {
			value,
			container: self.container,
			world: std::mem::transmute(&self.world),
			default: self.default,
			local,
			ticker: ChangeTrackers {
				component_ticks: tick.map_or(ComponentTicks{added: 0, changed: 0}, |r| {r.clone()}),
				last_change_tick: self.last_change_tick,
				change_tick: self.change_tick,
				marker: PhantomData,
			},
		})
    }

	#[inline]
    unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
        let value: Option<&'static T> = std::mem::transmute((&mut *(self.container as *mut MultiCaseImpl<T>)).get(local));
		let tick = (&mut *(self.container as *mut MultiCaseImpl<T>)).tick(local);
		WriteItem {
			value,
			container: self.container,
			world: std::mem::transmute(&self.world),
			local,
			default: self.default,
			ticker: ChangeTrackers {
				component_ticks: tick.map_or(ComponentTicks{added: 0, changed: 0}, |r| {r.clone()}),
				last_change_tick: self.last_change_tick,
				change_tick: self.change_tick,
				marker: PhantomData,
			},
		}
    }
}

pub struct WriteState<T: Component> {
    component_id: ComponentId,
	default: ResourceId,
    marker: PhantomData<T>,
}

// SAFE: component access and archetype component access are properly updated to reflect that T is
// read
unsafe impl<T: Component> FetchState for WriteState<T> {
    default fn init(world: &mut World, _query_id: usize, archetype_id: ArchetypeId) -> Self {
		
		let component_id = world.get_or_register_component::<T>(archetype_id);
		
		let r_id = world.get_or_insert_resource_id::<DefaultComponent<T>>();
        WriteState {
            component_id,
			default:r_id,
            marker: PhantomData,
        }
    }

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut FilteredAccess<ArchetypeComponentId>) {
		if archetype.contains(self.component_id) {
			let archetype_component_id = unsafe { archetype.archetype_component_id(self.component_id)};
			if access.has_read(archetype_component_id) {
				panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
					std::any::type_name::<T>());
			}
			access.add_modify(archetype_component_id)
		}
    }


    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        archetype.contains(self.component_id)
    }
}


unsafe impl<T: Component + Default> FetchState for WriteState<T> {
    fn init(world: &mut World, _query_id: usize, archetype_id: ArchetypeId) -> Self {
		// DefaultComponent<T>永远不能被销毁
		let r_id = match world.get_resource::<DefaultComponent<T>>() {
			Some(_r) => world.get_resource_id::<DefaultComponent<T>>().unwrap().clone(),
			None => world.insert_resource(DefaultComponent(T::default())).id(),
		};
		let component_id = world.get_or_register_component::<T>(archetype_id);
		
        WriteState {
            component_id,
			default: r_id,
            marker: PhantomData,
        }
    }
}