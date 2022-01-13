use crate::{
    archetype::{Archetype, ArchetypeId, ArchetypeComponentId},
    component::{Component, ComponentId, MultiCaseImpl},
    entity::Entity,
    query::{Access, FilteredAccess},
    storage::{ Keys, LocalVersion},
    world::World,
	pointer::Mut, WorldInner,
};

use pi_ecs_macros::all_tuples;
use share::cell::TrustCell;
use std::{
    marker::PhantomData,
	any::TypeId, sync::Arc,
};

/// WorldQuery 从world上fetch组件、实体、资源，需要实现该triat
pub trait WorldQuery {
    type Fetch: Fetch<State = Self::State>;
    type State: FetchState;
}

pub trait Fetch: Send + Sync + Sized {
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

	unsafe fn main_fetch<'a>(&'a self, _state: &Self::State, _last_change_tick: u32, _change_tick: u32) -> Option<MianFetch<'a>> {
		None
	}
}

pub struct MianFetch<'a> {
	pub(crate) value: Keys<'a, LocalVersion, ()>,
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
    fn init(world: &mut World) -> Self;
	/// 更新组件
	fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>);
    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>);
    fn matches_archetype(&mut self, archetype: &Archetype, world: &World) -> bool;
	fn get_matches(&self) -> bool;
	fn set_archetype<A: 'static + Send + Sync>(&self, _world: &mut World) {}
    // fn matches_table(&self, table: &Table) -> bool;
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
    fn init(_world: &mut World) -> Self {
        Self
    }

	fn update_component_access(&self, _access: &mut FilteredAccess<ComponentId>) {
				
	}

	#[inline]
    fn update_archetype_component_access(&self, _archetype: &Archetype, _access: &mut Access<ComponentId>) {}

    #[inline]
    fn matches_archetype(&mut self, _archetype: &Archetype, _world: &World) -> bool {
        true
    }

	fn get_matches(&self) -> bool {
		true
	}
}

impl Fetch for EntityFetch {
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
		// self.container =
		// self.iter.write(std::mem::transmute(archetype.entities.keys()));
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
}

impl<T: Component> WorldQuery for &T {
    type Fetch = ReadFetch<T>;
    type State = ReadState<T>;
}

pub struct ReadState<T> {
    pub(crate) component_id: ComponentId,
	matchs: bool,
    marker: PhantomData<T>,
}

// SAFE: component access and archetype component access are properly updated to reflect that T is
// read
unsafe impl<T: Component> FetchState for ReadState<T> {
    fn init(world: &mut World) -> Self {
		let component_id = match world.components.get_id(TypeId::of::<T>()) {
			Some(r) => r,
			None => panic!("ReadState fetch ${} fail", std::any::type_name::<T>()),
		};
        let component_info = world.components.get_info(component_id).unwrap();
        ReadState {
            component_id: component_info.id(),
			matchs: false,
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
		let archetype_component_id = unsafe { archetype.archetype_component_id(self.component_id)};
        if access.has_write(archetype_component_id) {
            panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<T>());
        }
		access.add_read(archetype_component_id);
    }


    fn matches_archetype(&mut self, archetype: &Archetype, _world: &World) -> bool {
        self.matchs = archetype.contains(self.component_id);
		self.matchs
    }

	fn get_matches(&self) -> bool {
		self.matchs
	}
}

pub struct ReadFetch<T> {
	// pub(crate) container: MaybeUninit<NonNull<u8>>,
	pub(crate) container: usize,
	mark: PhantomData<T>,
}

/// SAFE: access is read only
unsafe impl<T> ReadOnlyFetch for ReadFetch<T> {}

impl<T: Component> Fetch for ReadFetch<T> {
    type Item = &'static T;
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
}

impl<T: Component> WorldQuery for &mut T {
    type Fetch = MutFetch<T>;
    type State = MutState<T>;
}
pub struct MutFetch<T> {
	container: usize,
	mark: PhantomData<T>,
}

impl<T: Component> Fetch for MutFetch<T> {
    type Item = Mut<'static, T>;
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
}

pub struct MutState<T> {
    component_id: ComponentId,
	matchs: bool,
    marker: PhantomData<T>,
}

// SAFE: component access and archetype component access are properly updated to reflect that T is
// read
unsafe impl<T: Component> FetchState for MutState<T> {
    fn init(world: &mut World) -> Self {
		let component_id = match world.components.get_id(TypeId::of::<T>()) {
			Some(r) => r,
			None => panic!("MutState fetch ${} fail", std::any::type_name::<T>()),
		};
        let component_info = world.components.get_info(component_id).unwrap();
        MutState {
            component_id: component_info.id(),
			matchs: false,
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


    fn matches_archetype(&mut self, archetype: &Archetype, _world: &World) -> bool {
        self.matchs = archetype.contains(self.component_id);
		self.matchs
    }

	fn get_matches(&self) -> bool {
		self.matchs
	}
}

pub struct Write<T>(PhantomData<T>);
pub struct WriteItem<T: Component> {
	value: Option<&'static mut T>,
	container: usize,
	local: LocalVersion,
	tick: u32
}

impl<T: Component> WriteItem<T> {
    pub fn get<'a>(&'a self) -> Option<&'a T> {
		match &self.value {
			Some(r) => Some(r),
			None => None
		}
	}

	pub fn get_mut<'a>(&'a mut self) -> Option<&'a mut T> {
		match &mut self.value {
			Some(r) => Some(r),
			None => None
		}
	}

	pub fn write<'a>(&'a mut self, value: T) {
		let c = unsafe{&mut *(self.container as *mut MultiCaseImpl<T>)};
		c.insert(self.local,value, self.tick);
		self.value = Some(unsafe{std::mem::transmute(c.get_mut(self.local))});
	}
}

impl<T: Component> WorldQuery for Write<T> {
    type Fetch = WriteFetch<T>;
    type State = WriteState<T>;
}
pub struct WriteFetch<T> {
	container: usize,
	matchs: bool,
	tick: u32,
	mark: PhantomData<T>,
}

impl<T: Component> Fetch for WriteFetch<T> {
    type Item = WriteItem<T>;
    type State = WriteState<T>;

    unsafe fn init(
        _world: &World,
        _state: &Self::State
    ) -> Self {
        Self {
			container: 0,
			matchs: false,
			tick: 0,
			mark: PhantomData,
        }
    }

	unsafe fn setting(&mut self, _world: &WorldInner, _last_change_tick: u32, change_tick: u32) {
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
			local: local,
			tick: self.tick,
		})
    }
}

pub struct WriteState<T> {
    component_id: ComponentId,
    marker: PhantomData<T>,
}

// SAFE: component access and archetype component access are properly updated to reflect that T is
// read
unsafe impl<T: Component> FetchState for WriteState<T> {
    fn init(world: &mut World) -> Self {
		let component_id = match world.components.get_id(TypeId::of::<T>()) {
			Some(r) => r,
			None => panic!("WriteState fetch ${} fail", std::any::type_name::<T>()),
		};
        let component_info = world.components.get_info(component_id).unwrap();
        WriteState {
            component_id: component_info.id(),
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


    fn matches_archetype(&mut self, archetype: &Archetype, _world: &World) -> bool {
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
    fn init(world: &mut World) -> Self {
        Self {
            state: T::init(world),
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

    fn matches_archetype(&mut self, archetype: &Archetype, world: &World) -> bool {
		self.matchs = self.state.matches_archetype(archetype, world);
        true
	}
}

impl<T: Fetch> Fetch for OptionFetch<T> {
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
		self.matches = state.state.matches_archetype(archetype, world);
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
}

macro_rules! impl_tuple_fetch {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        impl<'a, $($name: Fetch),*> Fetch for ($($name,)*) {
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
        }

        // SAFE: update_component_access and update_archetype_component_access are called for each item in the tuple
        #[allow(non_snake_case)]
		#[allow(unused_variables)]
        unsafe impl<$($name: FetchState),*> FetchState for ($($name,)*) {
            fn init(_world: &mut World) -> Self {
                ($($name::init(_world),)*)
            }

			fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
				let ($($name,)*) = self;
				$($name.update_component_access(access);)*
			}

			fn set_archetype<A: 'static + Send + Sync>(&self, _world: &mut World)  {
				let ($($name,)*) = self;
                $($name.set_archetype::<A>(_world);)*
			}

            fn update_archetype_component_access(&self, archetype: &Archetype, _access: &mut Access<ComponentId>) {
                let ($($name,)*) = self;
                $($name.update_archetype_component_access(archetype, _access);)*
            }


			#[allow(unused_variables)]
            fn matches_archetype(&mut self, _archetype: &Archetype, world: &World) -> bool {
                let ($($name,)*) = self;
                true $(&& $name.matches_archetype(_archetype, world))*
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
