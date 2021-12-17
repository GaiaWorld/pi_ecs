use crate::{
    archetype::{Archetype, ArchetypeId, ArchetypeComponentId},
    component::{Component, ComponentId, StorageType},
    entity::Entity,
    query::{Access, FilteredAccess},
    storage::{ Keys, LocalVersion, SecondaryMap, SparseSecondaryMap, Local},
    world::World,
	pointer::Mut,
};

use pi_ecs_macros::all_tuples;
use std::{
    marker::PhantomData,
    // ptr::NonNull,
	mem::MaybeUninit,
};

/// WorldQuery 从world上fetch组件、实体、资源，需要实现该triat
pub trait WorldQuery {
    type Fetch: for<'a> Fetch<'a, State = Self::State>;
    type State: FetchState;
}

pub trait Fetch<'w>: Send + Sync + Sized {
    type Item;
    type State: FetchState;

    /// 创建一个新的fetch实例.
    ///
    /// # Safety
    /// `state` must have been initialized (via [FetchState::init]) using the same `world` passed in
    /// to this function.
    unsafe fn init(
        world: &World,
        state: &Self::State,
    ) -> Self;

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
    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Option<Self::Item>;
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
    fn matches_archetype(&self, archetype: &Archetype, world: &World) -> bool;
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
	iter: MaybeUninit<Keys<'static, LocalVersion, ()>>,
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
    fn matches_archetype(&self, _archetype: &Archetype, _world: &World) -> bool {
        true
    }
}

impl<'w> Fetch<'w> for EntityFetch {
    type Item = Entity;
    type State = EntityState;

    unsafe fn init(
        _world: &World,
        _state: &Self::State,
    ) -> Self {
        Self {
			iter: MaybeUninit::uninit(),
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
		self.iter.write(std::mem::transmute(archetype.entities.keys()));
		self.archetype_id = archetype.id();
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, _archetype_index: usize) -> Option<Self::Item> {
		match self.iter.assume_init_mut().next() {
			Some(local) => Some(Entity::new(self.archetype_id, local)),
			None => None,
		} 
    }
}

impl<T: Component> WorldQuery for &T {
    type Fetch = ReadFetch<T>;
    type State = ReadState<T>;
}

pub struct ReadState<T> {
    pub(crate) component_id: ComponentId,
    pub(crate) storage_type: StorageType,
    marker: PhantomData<T>,
}

// SAFE: component access and archetype component access are properly updated to reflect that T is
// read
unsafe impl<T: Component> FetchState for ReadState<T> {
    fn init(world: &mut World) -> Self {
        let component_info = world.components.get_or_insert_info::<T>();
        ReadState {
            component_id: component_info.id(),
            storage_type: component_info.storage_type(),
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
        access.add_read(archetype_component_id);
    }


    fn matches_archetype(&self, archetype: &Archetype, _world: &World) -> bool {
        archetype.contains(self.component_id)
    }
}

pub struct ReadFetch<T> {
    pub(crate) storage_type: StorageType,
	// pub(crate) container: MaybeUninit<NonNull<u8>>,
	pub(crate) container: usize,
	mark: PhantomData<T>,
}

/// SAFE: access is read only
unsafe impl<T> ReadOnlyFetch for ReadFetch<T> {}

impl<'w, T: Component> Fetch<'w> for ReadFetch<T> {
    type Item = &'w T;
    type State = ReadState<T>;

    unsafe fn init(
        _world: &World,
        state: &Self::State,
    ) -> Self {
        Self {
            storage_type: state.storage_type,
			// container: MaybeUninit::uninit(),
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
		self.container = archetype.get_component(state.component_id).as_ptr() as usize;
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Option<Self::Item> {
        match self.storage_type {
            StorageType::Table => std::mem::transmute((&mut *(self.container as *mut SecondaryMap<Local, T>)).get(Local::new(archetype_index))),
            StorageType::SparseSet => std::mem::transmute((&mut *(self.container as *mut SparseSecondaryMap<Local, T>)).get(Local::new(archetype_index)))
        }
    }
}

impl<T: Component> WorldQuery for &mut T {
    type Fetch = WriteFetch<T>;
    type State = WriteState<T>;
}
pub struct WriteFetch<T> {
    storage_type: StorageType,
	// container: MaybeUninit<NonNull<u8>>,
	container: usize,
	mark: PhantomData<T>,
}

impl<'w, T: Component> Fetch<'w> for WriteFetch<T> {
    type Item = Mut<'w, T>;
    type State = WriteState<T>;

    unsafe fn init(
        _world: &World,
        state: &Self::State,
    ) -> Self {
        Self {
            storage_type: state.storage_type,
			// container: MaybeUninit::uninit(),
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
		self.container = archetype.get_component(state.component_id).as_ptr() as usize;
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Option<Self::Item> {
        let value: Option<&mut T> = match self.storage_type {
            StorageType::Table => std::mem::transmute((&mut *(self.container as *mut SecondaryMap<Local, T>)).get_mut(Local::new(archetype_index))) ,
            StorageType::SparseSet => std::mem::transmute((&mut *(self.container as *mut SparseSecondaryMap<Local, T>)).get_mut(Local::new(archetype_index))),
        };
		match value {
			Some(r) => Some(Mut {
				value: r,
			}),
			None => None,
		}
    }
}

pub struct WriteState<T> {
    component_id: ComponentId,
    storage_type: StorageType,
    marker: PhantomData<T>,
}

// SAFE: component access and archetype component access are properly updated to reflect that T is
// read
unsafe impl<T: Component> FetchState for WriteState<T> {
    fn init(world: &mut World) -> Self {
        let component_info = world.components.get_or_insert_info::<T>();
        WriteState {
            component_id: component_info.id(),
            storage_type: component_info.storage_type(),
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
        access.add_read(archetype_component_id)
    }


    fn matches_archetype(&self, archetype: &Archetype, _world: &World) -> bool {
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
}

// SAFE: component access and archetype component access are properly updated according to the
// internal Fetch
unsafe impl<T: FetchState> FetchState for OptionState<T> {
    fn init(world: &mut World) -> Self {
        Self {
            state: T::init(world),
        }
    }

	fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
		self.state.update_component_access(access);
	}

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>) {
        self.state.update_archetype_component_access(archetype, access);
    }

    fn matches_archetype(&self, _archetype: &Archetype, _world: &World) -> bool {
        true
	}
}

impl<'w, T: Fetch<'w>> Fetch<'w> for OptionFetch<T> {
    type Item = Option<T::Item>;
    type State = OptionState<T::State>;

    unsafe fn init(
        world: &World,
        state: &Self::State,
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
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Option<Self::Item> {
		if self.matches {
        	Some(self.fetch.archetype_fetch(archetype_index))
		} else {
			Some(None)
		}
    }
}

macro_rules! impl_tuple_fetch {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        impl<'a, $($name: Fetch<'a>),*> Fetch<'a> for ($($name,)*) {
            type Item = ($($name::Item,)*);
            type State = ($($name::State,)*);

            unsafe fn init(_world: &World, state: &Self::State) -> Self {
                let ($($name,)*) = state;
                ($($name::init(_world, $name),)*)
            }

            #[inline]
			#[allow(unused_variables)]
            unsafe fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype, world: &World) {
                let ($($name,)*) = self;
                let ($($state,)*) = _state;
                $($name.set_archetype($state, _archetype, world);)*
            }

            #[inline]
            unsafe fn archetype_fetch(&mut self, _archetype_index: usize) -> Option<Self::Item> {
                let ($($name,)*) = self;
                Some(($(match $name.archetype_fetch(_archetype_index) {
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

            fn update_archetype_component_access(&self, archetype: &Archetype, _access: &mut Access<ComponentId>) {
                let ($($name,)*) = self;
                $($name.update_archetype_component_access(archetype, _access);)*
            }


			#[allow(unused_variables)]
            fn matches_archetype(&self, _archetype: &Archetype, world: &World) -> bool {
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
