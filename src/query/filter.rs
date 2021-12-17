use crate::{
    archetype::{Archetype, ArchetypeComponentId},
    // bundle::Bundle,
    component::{Component, ComponentId, /*ComponentTicks,*/ StorageType},
    query::{Fetch, ReadFetch, FetchState, Access, FilteredAccess, WorldQuery, ReadState},
    storage::{SecondaryMap, SparseSecondaryMap, Local},
    world::World,
};
use pi_ecs_macros::all_tuples;
use std::{marker::PhantomData};



/// Fetch methods used by query filters. This trait exists to allow "short circuit" behaviors for
/// relevant query filter fetches.
pub trait FilterFetch: for<'a> Fetch<'a> {
    /// # Safety
    /// Must always be called _after_ [Fetch::set_archetype]. `archetype_index` must be in the range
    /// of the current archetype
    unsafe fn archetype_filter_fetch(&mut self, archetype_index: usize) -> bool;
}

/// Filter that selects entities with a component `T`
pub struct With<T>(PhantomData<T>);

impl<T: Component> WorldQuery for With<T> {
    type Fetch = WithFetch<T>;
    type State = WithState<T>;
}

pub struct WithFetch<T> {
	pub(crate) read_fetch: ReadFetch<T>,
    marker: PhantomData<T>,
}
pub struct WithState<T> {
	pub(crate) read_state: ReadState<T>,
    marker: PhantomData<T>,
}

// SAFE: no component access or archetype component access
unsafe impl<T: Component> FetchState for WithState<T> {
    fn init(world: &mut World) -> Self {
        Self {
			read_state: ReadState::init(world),
            marker: PhantomData,
        }
    }

	fn update_component_access(&self, _access: &mut FilteredAccess<ComponentId>) {
	}

    #[inline]
    fn update_archetype_component_access(&self, _archetype: &Archetype, _access: &mut Access<ArchetypeComponentId>) {
    }
	
    fn matches_archetype(&self, archetype: &Archetype, _world: &World) -> bool {
		archetype.contains(self.read_state.component_id)
    }
}

impl<'a, T: Component> Fetch<'a> for WithFetch<T> {
    type Item = bool;
    type State = WithState<T>;

    unsafe fn init(
        world: &World,
        state: &Self::State,
    ) -> Self {
        Self {
			read_fetch: ReadFetch::init(world, &state.read_state),
            marker: PhantomData,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        _state: &Self::State,
        _archetype: &Archetype,
		_world: &World
    ) {
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, _archetype_index: usize) -> Option<Self::Item> {
        Some(true)
    }
}

impl<T: Component> FilterFetch for WithFetch<T> {
	unsafe fn archetype_filter_fetch(&mut self, archetype_index: usize) -> bool {
		match self.read_fetch.storage_type {
			StorageType::Table => std::mem::transmute((&mut *(self.read_fetch.container as *mut SecondaryMap<Local, T>)).contains_key(Local::new(archetype_index))),
			StorageType::SparseSet => std::mem::transmute((&mut *(self.read_fetch.container as *mut SparseSecondaryMap<Local, T>)).contains_key(Local::new(archetype_index)))
		}
	}
}

pub struct WithOut<T>(PhantomData<T>);

impl<T: Component> WorldQuery for WithOut<T> {
    type Fetch = WithOutFetch<T>;
    type State = WithOutState<T>;
}

pub struct WithOutFetch<T> {
	pub(crate) read_fetch: ReadFetch<T>,
	not_component: bool,
    marker: PhantomData<T>,
}
pub struct WithOutState<T> {
	pub(crate) read_state: ReadState<T>,
    marker: PhantomData<T>,
}

// SAFE: no component access or archetype component access
unsafe impl<T: Component> FetchState for WithOutState<T> {
    fn init(world: &mut World) -> Self {
        Self {
			read_state: ReadState::init(world),
            marker: PhantomData,
        }
    }

	fn update_component_access(&self, _access: &mut FilteredAccess<ComponentId>) {

	}

    #[inline]
    fn update_archetype_component_access(&self, _archetype: &Archetype, _access: &mut Access<ArchetypeComponentId>) {
        _access.add_read(self.read_state.component_id);
    }
	
    fn matches_archetype(&self, _archetype: &Archetype, _world: &World) -> bool {
		true
    }
}

impl<'a, T: Component> Fetch<'a> for WithOutFetch<T> {
    type Item = bool;
    type State = WithOutState<T>;

    unsafe fn init(
        world: &World,
        state: &Self::State,
    ) -> Self {
        Self {
			not_component: true,
			read_fetch: ReadFetch::init(world, &state.read_state),
            marker: PhantomData,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
		_world: &World
    ) {
		self.not_component = !archetype.contains(state.read_state.component_id);
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, _archetype_index: usize) -> Option<Self::Item> {
        Some(true)
    }
}

impl<T: Component> FilterFetch for WithOutFetch<T> {
	unsafe fn archetype_filter_fetch(&mut self, archetype_index: usize) -> bool {
		if self.not_component {
			return true;
		} else {
			!match self.read_fetch.storage_type {
				StorageType::Table => std::mem::transmute((&mut *(self.read_fetch.container as *mut SecondaryMap<Local, T>)).contains_key(Local::new(archetype_index))),
				StorageType::SparseSet => std::mem::transmute((&mut *(self.read_fetch.container as *mut SparseSecondaryMap<Local, T>)).contains_key(Local::new(archetype_index)))
			}
		}
	}
}



pub struct Or<T>(pub T);
pub struct OrFetch<T: FilterFetch> {
    fetch: T,
    matches: bool,
}

macro_rules! impl_query_filter_tuple {
    ($(($filter: ident, $state: ident)),*) => {
        #[allow(unused_variables)]
        #[allow(non_snake_case)]
        impl<'a, $($filter: FilterFetch),*> FilterFetch for ($($filter,)*) {

            #[inline]
            unsafe fn archetype_filter_fetch(&mut self, archetype_index: usize) -> bool {
                let ($($filter,)*) = self;
                true $(&& $filter.archetype_filter_fetch(archetype_index))*
            }
        }

        impl<$($filter: WorldQuery),*> WorldQuery for Or<($($filter,)*)>
            where $($filter::Fetch: FilterFetch),*
        {
            type Fetch = Or<($(OrFetch<$filter::Fetch>,)*)>;
            type State = Or<($($filter::State,)*)>;
        }


        #[allow(unused_variables)]
        #[allow(non_snake_case)]
        impl<'a, $($filter: FilterFetch),*> Fetch<'a> for Or<($(OrFetch<$filter>,)*)> {
            type State = Or<($(<$filter as Fetch<'a>>::State,)*)>;
            type Item = bool;

            unsafe fn init(world: &World, state: &Self::State) -> Self {
                let ($($filter,)*) = &state.0;
                Or(($(OrFetch {
                    fetch: $filter::init(world, $filter),
                    matches: false,
                },)*))
            }

            #[inline]
            unsafe fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype, world: &World) {
                let ($($filter,)*) = &mut self.0;
                let ($($state,)*) = &state.0;
                $(
                    $filter.matches = $state.matches_archetype(archetype, world);
                    if $filter.matches {
                        $filter.fetch.set_archetype($state, archetype, world);
                    }
                )*
            }

            #[inline]
            unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Option<bool> {
                Some(true)
            }
        }

        // SAFE: update_component_access and update_archetype_component_access are called for each item in the tuple
        #[allow(unused_variables)]
        #[allow(non_snake_case)]
        unsafe impl<$($filter: FetchState),*> FetchState for Or<($($filter,)*)> {
            fn init(world: &mut World) -> Self {
                Or(($($filter::init(world),)*))
            }

			fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
				let ($($filter,)*) = &self.0;
				$($filter.update_component_access(access);)*
			}

            fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>) {
                let ($($filter,)*) = &self.0;
                $($filter.update_archetype_component_access(archetype, access);)*
            }

            fn matches_archetype(&self, archetype: &Archetype, world: &World) -> bool {
                let ($($filter,)*) = &self.0;
                false $(|| $filter.matches_archetype(archetype, world))*
            }
        }

		#[allow(unused_variables)]
        #[allow(non_snake_case)]
		impl<'a, $($filter: FilterFetch),*> FilterFetch for Or<($(OrFetch<$filter>,)*)> {
			unsafe fn archetype_filter_fetch(&mut self, archetype_index: usize) -> bool {
				let ($($filter,)*) = &mut self.0;
                false $(|| ($filter.matches && $filter.fetch.archetype_filter_fetch(archetype_index)))*
			}
		}
    };
}

all_tuples!(impl_query_filter_tuple, 0, 15, F, S);