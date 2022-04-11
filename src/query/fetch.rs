use crate::{
    archetype::{Archetype, ArchetypeId, ArchetypeComponentId},
    component::{Component, ComponentId, MultiCaseImpl, ComponentTicks},
    entity::Entity,
    query::{Access, FilteredAccess},
    storage::LocalVersion,
    world::World,
	pointer::Mut, WorldInner,
};

use derive_deref::{Deref, DerefMut};
use pi_ecs_macros::all_tuples;
use pi_share::cell::TrustCell;
use std::{
    marker::PhantomData, sync::Arc, intrinsics::transmute,
};

/// WorldQuery 从world上fetch组件、实体、资源，需要实现该triat
pub trait WorldQuery {
    type Fetch: for<'s> Fetch<'s, State = Self::State>;
    type State: FetchState;
}

pub trait Fetch<'s>: Send + Sync + Sized {
    type Item;
    type State: FetchState;

    /// 创建一个新的fetch实例.
    ///
    /// # Safety
    /// `state` must have been initialized (via [FetchState::init]) using the same `world` passed in
    /// to this function.
    unsafe fn init(
        world: &World,
        state: &Self::State
    ) -> Self;

	unsafe fn setting(&mut self, _world: &WorldInner, _last_change_tick: u32, _change_tick: u32) {

	}

    /// Adjusts internal state to account for the next [Archetype]. This will always be called on
    /// archetypes that match this [Fetch]
    ///
    /// # Safety
    /// `archetype` and `tables` must be from the [World] [Fetch::init] was called on. `state` must
    /// be the [Self::State] this was initialized with.
	/// be the [Self::State::match_archetype] was called and return `true`.
    unsafe fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype, world: &World);

    /// # Safety
    /// Must always be called _after_ [Fetch::set_archetype]. `archetype_index` must be in the range
    /// of the current archetype
    unsafe fn archetype_fetch(&mut self, archetype_index: LocalVersion) -> Option<Self::Item>;

	/// # fetch fail will panic
	unsafe fn archetype_fetch_unchecked(&mut self, archetype_index: LocalVersion) -> Self::Item;

	unsafe fn main_fetch<'a>(&'a self, _state: &Self::State, _last_change_tick: u32, _change_tick: u32) -> Option<MianFetch<'a>> {
		None
	}
}

pub struct MianFetch<'a> {
	pub(crate) value: pi_slotmap::secondary::Keys<'a, LocalVersion, ()>,
	pub(crate) next: Option<Box<MianFetch<'a>>>,
}

/// State used to construct a Fetch. This will be cached inside QueryState, so it is best to move as
/// much data / computation here as possible to reduce the cost of constructing Fetch.
/// SAFETY:
/// Implementor must ensure that [FetchState::update_component_access] and
/// [FetchState::update_archetype_component_access] exactly reflects the results of
/// [FetchState::matches_archetype], [FetchState::matches_table], [Fetch::archetype_fetch], and
/// [Fetch::table_fetch]
pub unsafe trait FetchState: Send + Sync + Sized {
	/// 创建FetchState实例
    fn init(world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self;
	/// 更新组件
	fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>);
    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>);
    fn matches_archetype(&self, archetype: &Archetype) -> bool;
	// fn get_matches(&self) -> bool;
	fn init_archetype<A: 'static + Send + Sync>(&self, _world: &mut World) {}
    // fn matches_table(&self, table: &Table) -> bool;

	fn apply(&self, _world: &mut World) {

	}
}

/// A fetch that is read only. This must only be implemented for read-only fetches.
pub unsafe trait ReadOnlyFetch {}

/// 为实例实现WorldQuery
impl WorldQuery for Entity {
    type Fetch = EntityFetch;
    type State = EntityState;
}

pub struct EntityFetch {
    // entities: *const Entity,
	// iter: MaybeUninit<Keys<'static, LocalVersion, ()>>,
	archetype_id: ArchetypeId,
}

/// SAFE: access is read only
unsafe impl ReadOnlyFetch for EntityFetch {}

pub struct EntityState;

// SAFE: no component or archetype access
unsafe impl FetchState for EntityState {
	#[inline]
    fn init(_world: &mut World, _query_id: usize, _archetype_id: ArchetypeId) -> Self {
        Self
    }

	fn update_component_access(&self, _access: &mut FilteredAccess<ComponentId>) {
				
	}

	#[inline]
    fn update_archetype_component_access(&self, _archetype: &Archetype, _access: &mut Access<ComponentId>) {}

    #[inline]
    fn matches_archetype(&self, _archetype: &Archetype,) -> bool {
        true
    }
}

impl<'s> Fetch<'s> for EntityFetch {
    type Item = Entity;
    type State = EntityState;

    unsafe fn init(
        _world: &World,
        _state: &Self::State
    ) -> Self {
        Self {
			archetype_id: ArchetypeId::default(),
            // entities: std::ptr::null::<Entity>(),
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
		Some(Entity::new(self.archetype_id, local))
		// match self.iter.assume_init_mut().next() {
		// 	Some(local) => Some(Entity::new(self.archetype_id, local)),
		// 	None => None,
		// } 
    }

	unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
		Entity::new(self.archetype_id, local)
	}
}

#[derive(Clone)]
pub struct ChangeTrackers<T: Component> {
    pub(crate) component_ticks: ComponentTicks,
    pub(crate) last_change_tick: u32,
    pub(crate) change_tick: u32,
    marker: PhantomData<T>,
}
pub struct ChangeTrackersFetch<T> {
	pub(crate) container: usize,
	pub(crate) last_change_tick: u32,
	pub(crate) change_tick: u32,
	mark: PhantomData<T>,
}

impl<T: Component> ChangeTrackers<T> {
    /// Has this component been added since the last execution of this system.
    pub fn is_added(&self) -> bool {
        self.component_ticks
            .is_added(self.last_change_tick, self.change_tick)
    }

    /// Has this component been changed since the last execution of this system.
    pub fn is_changed(&self) -> bool {
        self.component_ticks
            .is_changed(self.last_change_tick, self.change_tick)
    }
}

impl<T: Component> WorldQuery for ChangeTrackers<T> {
	type Fetch = ChangeTrackersFetch<T>;
    type State = ReadState<T>;
}

unsafe impl<T> ReadOnlyFetch for ChangeTrackersFetch<T> {}

impl<'s, T: Component> Fetch<'s> for ChangeTrackersFetch<T> {
    type Item = ChangeTrackers<T>;
    type State = ReadState<T>;

    unsafe fn init(
        _world: &World,
        _state: &Self::State
    ) -> Self {
        Self {
			container: 0,
			last_change_tick:0,
			change_tick: 0,
			mark: PhantomData,
        }
    }

	unsafe fn setting(&mut self, _world: &WorldInner, last_change_tick: u32, change_tick: u32) {
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
		let c = archetype.get_component(state.component_id);
		match c.clone().downcast() {
			Ok(r) => {
				let r: Arc<TrustCell<MultiCaseImpl<T>>> = r;
				self.container = r.as_ptr() as usize;
			},
			Err(_) => panic!("downcast fail")
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
		match (&mut *(self.container as *mut MultiCaseImpl<T>)).tick(local) {
			Some(r) => {
				Some(ChangeTrackers {
					component_ticks: r.clone(),
					last_change_tick: self.last_change_tick,
					change_tick: self.change_tick,
					marker: PhantomData
				})
			},
			None => None,
		}
    }

	unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
		ChangeTrackers {
			component_ticks: (&mut *(self.container as *mut MultiCaseImpl<T>)).tick_uncehcked(local).clone(),
			last_change_tick: self.last_change_tick,
			change_tick: self.change_tick,
			marker: PhantomData
		}
	}
}



impl<T: Component> WorldQuery for &T {
    type Fetch = ReadFetch<T>;
    type State = ReadState<T>;
}

pub struct ReadState<T> {
    pub(crate) component_id: ComponentId,
    marker: PhantomData<T>,
}

// SAFE: component access and archetype component access are properly updated to reflect that T is
// read
unsafe impl<T: Component> FetchState for ReadState<T> {
    fn init(world: &mut World, _query_id: usize, archetype_id: ArchetypeId) -> Self {
		let component_id = world.get_or_register_component::<T>(archetype_id);
        ReadState {
            component_id,
            marker: PhantomData,
        }
    }

	fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
		if access.access().has_write(self.component_id) {
            panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<T>());
        }
		access.add_read(self.component_id)
	}

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>) {
		// world.components.get_or_insert_id::<T>()
		let archetype_component_id = unsafe { archetype.archetype_component_id(self.component_id)};
        if access.has_write(archetype_component_id) {
            panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<T>());
        }
		access.add_read(archetype_component_id);
    }


    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        archetype.contains(self.component_id)
    }
}

pub struct ReadFetch<T> {
	// pub(crate) container: MaybeUninit<NonNull<u8>>,
	pub(crate) container: usize,
	mark: PhantomData<T>,
}

/// SAFE: access is read only
unsafe impl<T> ReadOnlyFetch for ReadFetch<T> {}

impl<'s, T: Component> Fetch<'s> for ReadFetch<T> {
    type Item = &'s T;
    type State = ReadState<T>;

    unsafe fn init(
        _world: &World,
        _state: &Self::State
    ) -> Self {
        Self {
			container: 0,
			mark: PhantomData,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
		_world: &World,
    ) {
		let c = archetype.get_component(state.component_id);
		match c.clone().downcast() {
			Ok(r) => {
				let r: Arc<TrustCell<MultiCaseImpl<T>>> = r;
				self.container = r.as_ptr() as usize;
			},
			Err(_) => panic!("downcast fail")
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
		std::mem::transmute((&mut *(self.container as *mut MultiCaseImpl<T>)).get(local))
    }

	unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
		std::mem::transmute((&mut *(self.container as *mut MultiCaseImpl<T>)).get_unchecked(local))
	}
}

impl<T: Component> WorldQuery for &mut T {
    type Fetch = MutFetch<T>;
    type State = MutState<T>;
}
pub struct MutFetch<T> {
	container: usize,
	mark: PhantomData<T>,
}

impl<'s, T: Component> Fetch<'s> for MutFetch<T> {
    type Item = Mut<'s, T>;
    type State = MutState<T>;

    unsafe fn init(
        _world: &World,
        _state: &Self::State
    ) -> Self {
        Self {
			container: 0,
			mark: PhantomData,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
		_world: &World,
    ) {
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
        let value = std::mem::transmute((&mut *(self.container as *mut MultiCaseImpl<T>)).get_mut(local));
		match value {
			Some(r) => Some(Mut {
				value: r,
			}),
			None => None,
		}
    }

	#[inline]
    unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
        let value = std::mem::transmute((&mut *(self.container as *mut MultiCaseImpl<T>)).get_unchecked_mut(local));
		Mut {
			value,
		}
    }
}

pub struct MutState<T> {
    component_id: ComponentId,
    marker: PhantomData<T>,
}

// SAFE: component access and archetype component access are properly updated to reflect that T IS
// read
unsafe impl<T: Component> FetchState for MutState<T> {
    fn init(world: &mut World, _query_id: usize, archetype_id: ArchetypeId) -> Self {
		let component_id = world.get_or_register_component::<T>(archetype_id);
        MutState {
            component_id,
            marker: PhantomData,
        }
    }

	fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
		if access.access().has_write(self.component_id) {
            panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<T>());
        }
		access.add_write(self.component_id)
	}

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>) {
		let archetype_component_id = unsafe { archetype.archetype_component_id(self.component_id)};
        if access.has_write(archetype_component_id) {
            panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<T>());
        }
		access.add_write(archetype_component_id)
    }


    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        archetype.contains(self.component_id)
    }
}

pub struct Write<T>(PhantomData<T>);
pub struct WriteItem<'s, T: Component> {
	value: Option<&'s T>,
	container: usize,
	default: &'s T,
	local: LocalVersion,
	tick: u32
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
		c.notify_modify(self.local, self.tick);
	}

	/// 修改组件并通知监听函数
	pub fn write<'a>(&'a mut self, value: T) {
		let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		c.insert(self.local,value, self.tick);
		let xx = c.get_mut(self.local);
		if xx.is_some() {
			self.value = unsafe{std::mem::transmute(c.get_mut(self.local))};
		}
		
	}

	/// 移除组件，并通知监听函数
	pub fn remove<'a>(&'a mut self) -> Option<T> {
		let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		c.delete(self.local)
	}

	pub fn get_default(&self) -> &T {
		return unsafe{std::mem::transmute(self.default)};
	}

	pub fn get_or_default(&self) -> &T {
		if let Some(r) = self.value  {
			return unsafe{std::mem::transmute(&mut *(r as *const T as usize as *mut T))}
		} else {
			return unsafe{std::mem::transmute(self.default)};
		}
		// let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		// c.insert_no_notify(self.local, default.clone());
		// let r: &'static mut T = unsafe{std::mem::transmute(c.get_unchecked_mut(self.local))};
		// self.value = Some(r);
		// self.value.as_ref().unwrap()
	}
}

impl<'s, T: Component + Clone> WriteItem<'s, T> {
	pub fn get_mut_or_default(&mut self) -> &mut T {
		if let Some(r) = &mut self.value  {
			return unsafe{std::mem::transmute(r)};
		}
		let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		c.insert_no_notify(self.local, self.default.clone());
		let r: &'static mut T = unsafe{std::mem::transmute(c.get_unchecked_mut(self.local))};
		self.value = Some(r);
		unsafe { &mut *(r as *const T as usize as *mut T)}
	}
}

impl<T: Component + Default> WorldQuery for Write<T> {
    type Fetch = WriteFetch<T>;
    type State = WriteState<T>;
}
pub struct WriteFetch<T: Component> {
	container: usize,
	default: &'static DefaultComponent<T>,
	matchs: bool,
	tick: u32,
	mark: PhantomData<T>,
}

impl<'s, T: Component + Default> Fetch<'s> for WriteFetch<T> {
    type Item = WriteItem<'s, T>;
    type State = WriteState<T>;

    unsafe fn init(
        _world: &World,
        state: &Self::State
    ) -> Self {
        Self {
			container: 0,
			matchs: false,
			tick: 0,
			default: state.default,
			mark: PhantomData,
        }
    }

	unsafe fn setting(
		&mut self, 
		_world: &WorldInner, _last_change_tick: u32, change_tick: u32) {
		self.tick = change_tick;
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
		Some(WriteItem {
			value,
			container: self.container,
			default: self.default,
			local: local,
			tick: self.tick,
		})
    }

	#[inline]
    unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
        let value: Option<&'static T> = std::mem::transmute((&mut *(self.container as *mut MultiCaseImpl<T>)).get(local));
		WriteItem {
			value,
			container: self.container,
			local: local,
			default: self.default,
			tick: self.tick,
		}
    }
}

pub struct WriteState<T: Component> {
    component_id: ComponentId,
	default: &'static DefaultComponent<T>,
    marker: PhantomData<T>,
}

// SAFE: component access and archetype component access are properly updated to reflect that T is
// read
unsafe impl<T: Component + Default> FetchState for WriteState<T> {
    fn init(world: &mut World, _query_id: usize, archetype_id: ArchetypeId) -> Self {
		// DefaultComponent<T>永远不能被销毁
		match world.get_resource_id::<DefaultComponent<T>>() {
			Some(r) => r.clone(),
			None => world.insert_resource(DefaultComponent(T::default())),
		};

		let component_id = world.get_or_register_component::<T>(archetype_id);

		let default_value = world.get_resource_mut::<DefaultComponent<T>>().unwrap();
        WriteState {
            component_id,
			default: unsafe {transmute(default_value)},
            marker: PhantomData,
        }
    }

	fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
		if access.access().has_write(self.component_id) {
            panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<T>());
        }
		access.add_write(self.component_id)
	}

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>) {
		if archetype.contains(self.component_id) {
			let archetype_component_id = unsafe { archetype.archetype_component_id(self.component_id)};
			if access.has_write(archetype_component_id) {
				panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
					std::any::type_name::<T>());
			}
			access.add_write(archetype_component_id)
		}
    }


    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        archetype.contains(self.component_id)
    }
}

impl<T: WorldQuery> WorldQuery for Option<T> {
    type Fetch = OptionFetch<T::Fetch>;
    type State = OptionState<T::State>;
}

pub struct OptionFetch<T> {
    fetch: T,
    matches: bool,
}

/// SAFE: OptionFetch is read only because T is read only
unsafe impl<T: ReadOnlyFetch> ReadOnlyFetch for OptionFetch<T> {}

pub struct OptionState<T: FetchState> {
    state: T,
	matchs: bool,
}

// SAFE: component access and archetype component access are properly updated according to the
// internal Fetch
unsafe impl<T: FetchState> FetchState for OptionState<T> {
    fn init(world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self {
        Self {
            state: T::init(world, query_id, archetype_id),
			matchs: false
        }
    }

	fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
		self.state.update_component_access(access);
	}

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>) {
		if self.matchs {
			self.state.update_archetype_component_access(archetype, access);
		}
    }

    fn matches_archetype(&self, _archetype: &Archetype) -> bool {
        true
	}
}

impl<'s, T: Fetch<'s>> Fetch<'s> for OptionFetch<T> {
    type Item = Option<T::Item>;
    type State = OptionState<T::State>;

    unsafe fn init(
        world: &World,
        state: &Self::State
    ) -> Self {
        Self {
            fetch: T::init(world, &state.state),
            matches: false,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
		world: &World,
    ) {
		self.matches = state.state.matches_archetype(archetype);
		if self.matches {
        	self.fetch.set_archetype(&state.state, archetype, world);
		} else {
			log::warn!("component is not exist in archetype, so query fail, query: {:?}",  std::any::type_name::<Option<T>>());
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
		if self.matches {
        	Some(self.fetch.archetype_fetch(local))
		} else {
			None
		}
    }

	#[inline]
    unsafe fn archetype_fetch_unchecked (&mut self, local: LocalVersion) -> Self::Item {
		if self.matches {
        	self.fetch.archetype_fetch(local)
		} else {
			None
		}
    }
}

/// 如果组件不存在，则设置一个默认值
pub struct OrDefault<T>(PhantomData<T>);

impl<T: Component + Default> WorldQuery for OrDefault<T> {
    type Fetch = OrDefaultFetch<T>;
    type State = OrDefaultState<T>;
}

pub struct OrDefaultFetch<T> {
	default_id: ComponentId,
	fetch: ReadFetch<T>,
	world: World,
    matches: bool,
}

/// SAFE: OrDefaultFetch is read only because T is read only
unsafe impl<T: Component> ReadOnlyFetch for OrDefaultFetch<T> {}

pub struct OrDefaultState<T: Component>{
	default_id: ComponentId,
    state: ReadState<T>,
	matchs: bool,
}

#[derive(Deref, DerefMut)]
pub struct DefaultComponent<T: Component>(T);

// SAFE: component access and archetype component access are properly updated according to the
// internal Fetch
unsafe impl<T: Component + Default> FetchState for OrDefaultState<T> {
    fn init(world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self {
		let id = match world.get_resource_id::<DefaultComponent<T>>() {
			Some(r) => r.clone(),
			None => world.insert_resource(DefaultComponent(T::default())),
		};

        Self {
			default_id: id,
            state: ReadState::<T>::init(world, query_id, archetype_id),
			matchs: false
        }
    }

	fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
		self.state.update_component_access(access);
	}

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>) {
		if self.matchs {
			self.state.update_archetype_component_access(archetype, access);
		}
    }

    fn matches_archetype(&self, _archetype: &Archetype) -> bool {
        true
	}
}

impl<'s, T: Component + Default> Fetch<'s> for OrDefaultFetch<T> {
    type Item = &'s T;
    type State = OrDefaultState<T>;

    unsafe fn init(
        world: &World,
        state: &Self::State
    ) -> Self {
        Self {
			default_id: state.default_id,
			world: world.clone(),
            fetch: ReadFetch::<T>::init(world, &state.state),
            matches: false,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
		world: &World,
    ) {
		self.matches = state.state.matches_archetype(archetype);
		if self.matches {
        	self.fetch.set_archetype(&state.state, archetype, world);
		} else {
			log::warn!("component is not exist in archetype, so query fail, query: {:?}",  std::any::type_name::<OrDefault<T>>());
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
		if self.matches {
			match self.fetch.archetype_fetch(local) {
				Some(r) => Some(std::mem::transmute(r)),
				None => std::mem::transmute(self.world.archetypes().get_resource::<T>(self.default_id))
			}
		} else {
			None
		}
    }

	unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
		if self.matches {
			match self.fetch.archetype_fetch(local) {
				Some(r) => std::mem::transmute(r),
				None => std::mem::transmute(self.world.archetypes().get_resource::<T>(self.default_id).unwrap())
			}
		} else {
			std::mem::transmute(self.world.archetypes().get_resource::<T>(self.default_id))
		}
    }
}

macro_rules! impl_tuple_fetch {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        impl<'s, $($name: Fetch<'s>),*> Fetch<'s> for ($($name,)*) {
            type Item = ($($name::Item,)*);
            type State = ($($name::State,)*);

            unsafe fn init(_world: &World, state: &Self::State) -> Self {
                let ($($name,)*) = state;
                ($($name::init(_world, $name),)*)
            }

			unsafe fn main_fetch<'x>(&'x self, state: &Self::State, _last_change_tick: u32, _change_tick: u32) -> Option<MianFetch<'x>> {
				$crate::paste::item! {
					let ($([<state $name>],)*) = state;
					let ($($name,)*) = self;
					$(
						if let Some(r) = $name.main_fetch([<state $name>], _last_change_tick, _change_tick) {
							return Some(r)
						};
					)*
					None
				}
			}

            #[inline]
			#[allow(unused_variables)]
            unsafe fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype, world: &World) {
                let ($($name,)*) = self;
                let ($($state,)*) = _state;
                $($name.set_archetype($state, _archetype, world);)*
            }

            #[inline]
			#[allow(unused_variables)]
            unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
                let ($($name,)*) = self;
                Some(($(match $name.archetype_fetch(local) {
					Some(r) => r,
					None => return None
				},)*))
            }

			#[inline]
			#[allow(unused_variables)]
            unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
                let ($($name,)*) = self;
                ($($name.archetype_fetch_unchecked(local),)*)
            }
        }

        // SAFE: update_component_access and update_archetype_component_access are called for each item in the tuple
        #[allow(non_snake_case)]
		#[allow(unused_variables)]
        unsafe impl<$($name: FetchState),*> FetchState for ($($name,)*) {
            fn init(_world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self {
                ($($name::init(_world, query_id, archetype_id),)*)
            }

			fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
				let ($($name,)*) = self;
				$($name.update_component_access(access);)*
			}

			fn init_archetype<A: 'static + Send + Sync>(&self, _world: &mut World)  {
				let ($($name,)*) = self;
                $($name.init_archetype::<A>(_world);)*
			}

            fn update_archetype_component_access(&self, archetype: &Archetype, _access: &mut Access<ComponentId>) {
                let ($($name,)*) = self;
                $($name.update_archetype_component_access(archetype, _access);)*
            }


			#[allow(unused_variables)]
            fn matches_archetype(&self, _archetype: &Archetype) -> bool {
                let ($($name,)*) = self;
                true $(&& $name.matches_archetype(_archetype))*
            }
        }

        impl<$($name: WorldQuery),*> WorldQuery for ($($name,)*) {
            type Fetch = ($($name::Fetch,)*);
            type State = ($($name::State,)*);
        }

        /// SAFE: each item in the tuple is read only
        unsafe impl<$($name: ReadOnlyFetch),*> ReadOnlyFetch for ($($name,)*) {}

    };
}

all_tuples!(impl_tuple_fetch, 0, 15, F, S);
