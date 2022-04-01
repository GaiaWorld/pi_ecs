use pi_map::Map;
use pi_share::cell::TrustCell;

use std::sync::Arc;
use std::any::TypeId;
use std::default::Default;

use crate::{
    archetype::{Archetype, ArchetypeComponentId},
	sys::param::{ResMut, Tick},
	monitor::{Event, Listen, ComponentListen, Create, Modify, Listeners, ListenSetup},
    // bundle::Bundle,
    component::{Component, ComponentId, MultiCaseImpl},
    query::{Fetch, ReadFetch, FetchState, Access, FilteredAccess, WorldQuery, ReadState, MianFetch},
    storage::{SecondaryMap, Local, LocalVersion},
    world::{World, WorldInner},
};
use pi_ecs_macros::all_tuples;
use std::{marker::PhantomData};



/// Fetch methods used by query filters. This trait exists to allow "short circuit" behaviors for
/// relevant query filter fetches.
pub trait FilterFetch: Fetch {
    /// # Safety
    /// Must always be called _after_ [Fetch::set_archetype]. `archetype_index` must be in the range
    /// of the current archetype
    unsafe fn archetype_filter_fetch(&mut self, local: LocalVersion) -> bool;
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
    fn init(world: &mut World, query_id: usize) -> Self {
        Self {
			read_state: ReadState::init(world, query_id),
            marker: PhantomData,
        }
    }

	fn update_component_access(&self, _access: &mut FilteredAccess<ComponentId>) {
	}

    #[inline]
    fn update_archetype_component_access(&self, _archetype: &Archetype, _access: &mut Access<ArchetypeComponentId>) {
    }
	
    fn matches_archetype(&self, archetype: &Archetype) -> bool {
		archetype.contains(self.read_state.component_id)
    }
}

impl<T: Component> Fetch for WithFetch<T> {
    type Item = bool;
    type State = WithState<T>;

    unsafe fn init(
        world: &World,
        state: &Self::State
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
		self.read_fetch.set_archetype(&_state.read_state, _archetype, _world);
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, _local: LocalVersion) -> Option<Self::Item> {
        Some(true)
    }

	#[inline]
    unsafe fn archetype_fetch_unchecked(&mut self, _local: LocalVersion) -> Self::Item {
        true
    }
}

impl<T: Component> FilterFetch for WithFetch<T> {
	unsafe fn archetype_filter_fetch(&mut self, local: LocalVersion) -> bool {
		std::mem::transmute((&*(self.read_fetch.container as *mut MultiCaseImpl<T>)).contains_key(&local))
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
    fn init(world: &mut World, query_id: usize) -> Self {
        Self {
			read_state: ReadState::init(world, query_id),
            marker: PhantomData,
        }
    }

	fn update_component_access(&self, _access: &mut FilteredAccess<ComponentId>) {

	}

    #[inline]
    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>) {
		self.read_state.update_archetype_component_access(archetype, access);
    }
	
    fn matches_archetype(&self, _archetype: &Archetype,) -> bool {
		true
    }
}

impl<T: Component> Fetch for WithOutFetch<T> {
    type Item = bool;
    type State = WithOutState<T>;

    unsafe fn init(
        world: &World,
        state: &Self::State
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
		if !self.not_component {
			self.read_fetch.set_archetype(&state.read_state, archetype, _world);
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, _archetype_index: LocalVersion) -> Option<Self::Item> {
        Some(true)
    }

	#[inline]
    unsafe fn archetype_fetch_unchecked(&mut self, _local: LocalVersion) -> Self::Item {
        true
    }
}

impl<T: Component> FilterFetch for WithOutFetch<T> {
	unsafe fn archetype_filter_fetch(&mut self, local: LocalVersion) -> bool {
		if self.not_component {
			return true;
		} else {
			let r:bool = std::mem::transmute((&mut *(self.read_fetch.container as *mut MultiCaseImpl<T>)).contains_key(&local));
			!r
		}
	}
}



pub struct Or<T>(pub T);
pub struct OrFetch<T: FilterFetch> {
    pub fetch: T,
    matches: bool,
}

macro_rules! impl_query_filter_tuple {
    ($(($filter: ident, $state: ident)),*) => {
        #[allow(unused_variables)]
        #[allow(non_snake_case)]
        impl<'a, $($filter: FilterFetch),*> FilterFetch for ($($filter,)*) {

            #[inline]
            unsafe fn archetype_filter_fetch(&mut self, local: LocalVersion) -> bool {
                let ($($filter,)*) = self;
                true $(&& $filter.archetype_filter_fetch(local))*
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
        impl<$($filter: FilterFetch),*> Fetch for Or<($(OrFetch<$filter>,)*)> {
            type State = Or<($(<$filter as Fetch>::State,)*)>;
            type Item = bool;

            unsafe fn init(world: &World, state: &Self::State) -> Self {
                let ($($filter,)*) = &state.0;
                Or(($(OrFetch {
                    fetch: $filter::init(world, $filter),
                    matches: false,
                },)*))
            }

			#[allow(unused_mut)]
			unsafe fn main_fetch<'x>(&'x self, state: &Self::State, last_change_tick: u32, change_tick: u32) -> Option<MianFetch<'x>> {
				$crate::paste::item! {
					let ($([<state $filter>],)*) = &state.0;
					let ($($filter,)*) = &self.0;
					let mut k: Option<MianFetch<'x>> = None;
					$(
						if let Some(r) = $filter.fetch.main_fetch([<state $filter>], last_change_tick, change_tick) {
							if let Some(next) = &k {
								if next.value.len() == 0 {
									k = Some(r);
								} else if r.value.len() == 0 {

								} else {
									return None; // 如果有多个脏，直接遍历整个实体列表， 所以返回None
								}
							} else {
								k = Some(r);
							}
						};
					)*
					k
				}
			}

            #[inline]
            unsafe fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype, world: &World) {
                let ($($filter,)*) = &mut self.0;
                let ($($state,)*) = &state.0;
                $(
                    $filter.matches = $state.matches_archetype(archetype);
                    if $filter.matches {
                        $filter.fetch.set_archetype($state, archetype, world);
                    }
                )*
            }

            #[inline]
            unsafe fn archetype_fetch(&mut self, archetype_index: LocalVersion) -> Option<bool> {
                Some(true)
            }

			#[inline]
			unsafe fn archetype_fetch_unchecked(&mut self, _local: LocalVersion) -> Self::Item {
				true
			}
        }

        // SAFE: update_component_access and update_archetype_component_access are called for each item in the tuple
        #[allow(unused_variables)]
        #[allow(non_snake_case)]
        unsafe impl<$($filter: FetchState),*> FetchState for Or<($($filter,)*)> {
            fn init(world: &mut World, query_id: usize) -> Self {
                Or(($($filter::init(world, query_id),)*))
            }
			fn set_archetype<A: 'static + Send + Sync>(&self, _world: &mut World)  {
				// let ($($filter,)*) = &mut self.0;
                // let ($($state,)*) = &state.0;
                // $(
                //     $filter.matches = $state.matches_archetype(archetype);
                //     if $filter.matches {
                //         $filter.fetch.set_archetype( world);
                //     }
                // )*
			}

			fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
				let ($($filter,)*) = &self.0;
				$($filter.update_component_access(access);)*
			}

            fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>) {
                let ($($filter,)*) = &self.0;
                $($filter.update_archetype_component_access(archetype, access);)*
            }

            fn matches_archetype(&self, archetype: &Archetype) -> bool {
                let ($($filter,)*) = &self.0;
                false $(|| $filter.matches_archetype(archetype))*
            }
        }

		#[allow(unused_variables)]
        #[allow(non_snake_case)]
		impl<'a, $($filter: FilterFetch),*> FilterFetch for Or<($(OrFetch<$filter>,)*)> {
			unsafe fn archetype_filter_fetch(&mut self, local: LocalVersion) -> bool {
				let ($($filter,)*) = &mut self.0;
                false $(|| ($filter.matches && $filter.fetch.archetype_filter_fetch(local)))*
			}
		}
    };
}

pub struct Dirty (pub(crate) SecondaryMap<Local, u32/*tick*/>, pub(crate) SecondaryMap<Local, bool>);

impl Default for Dirty {
    fn default() -> Self {
		Dirty(SecondaryMap::with_capacity(0), SecondaryMap::with_capacity(0))
	}
}

all_tuples!(impl_query_filter_tuple, 0, 15, F, S);

macro_rules! impl_tick_filter {
    (
        $(#[$meta:meta])*
        $name: ident, $state_name: ident, $fetch_name: ident, $is_detected: expr) => {
        $(#[$meta])*
        pub struct $name<T>(PhantomData<T>);

        pub struct $fetch_name<T> {
            // table_ticks: *mut ComponentTicks,
            // entity_table_rows: *const usize,
			container: usize, // 组件容器
			dirty_container: SecondaryMap<LocalVersion, ()>,

            marker: PhantomData<T>,
            last_change_tick: u32,
            change_tick: u32,
        }

        pub struct $state_name<T> {
			dirty: usize,
            component_id: ComponentId,
            marker: PhantomData<T>,
        }

        impl<T: Component> WorldQuery for $name<T> {
            type Fetch = $fetch_name<T>;
            type State = $state_name<T>;
        }


        // SAFE: this reads the T component. archetype component access and component access are updated to reflect that
        unsafe impl<T: Component> FetchState for $state_name<T> {
            fn init(world: &mut World, _query_id: usize) -> Self {
                let component_id = match world.components.get_id(TypeId::of::<T>()){
					Some(r) => r,
					None => panic!("FetchState error: {}", std::any::type_name::<T>()),
				};

				// 如果world上没有Dirty资源，则插入Dirty资源
				let dirty_id = world.get_resource_id::<Dirty>();
				let dirty_id = match dirty_id {
					Some(r) =>  r.clone(),
					None => world.insert_resource(Dirty::default()) ,
				};

				let value = unsafe{world
				.archetypes.get_resource::<Dirty>(dirty_id).unwrap()};
				let dirty = value as *const Dirty as usize;

                Self {
					dirty,
                    component_id,
                    marker: PhantomData,
                }
            }

            #[inline]
            fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
                if access.access().has_write(self.component_id) {
                    panic!("$state_name<{}> conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                        std::any::type_name::<T>());
                }
                access.add_read(self.component_id);
            }

            #[inline]
            fn update_archetype_component_access(
                &self,
                archetype: &Archetype,
                access: &mut Access<ArchetypeComponentId>,
            ) {
				let archetype_component_id = unsafe{ archetype.archetype_component_id(self.component_id)};
				if access.has_write(archetype_component_id) {
					panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
						std::any::type_name::<T>());
				}
                access.add_read(archetype_component_id);
            }

            fn matches_archetype(&self, archetype: &Archetype) -> bool {
                archetype.contains(self.component_id)
            }
			fn set_archetype<A: 'static + Send + Sync>(&self, world: &mut World) {
				match unsafe{&*(self.dirty as *const Dirty)}.1.get(&self.component_id) {
					Some(r) if *r == true => (),
					_ => {
						let component_id = self.component_id;
						// 安装监听器，监听对应组件修改，并设置该组件的全局脏标记为true
						let listen = move |_event: Event, _:Listen<(ComponentListen<A, T, (Create, Modify)>, )> , mut res: ResMut<Dirty>, tick: Tick| {
							let aa = &*res as *const Dirty as usize;
							println!("{}", aa);
							res.0.insert(component_id, tick.change_tick);
						};
						unsafe{&mut *(self.dirty as *mut Dirty)}.1.insert(component_id, true);
						let l = listen.listeners();
						l.setup(world);
					}
				};
			}
        }

        impl<T: Component> Fetch for $fetch_name<T> {
            type State = $state_name<T>;
            type Item = bool;

            unsafe fn init(_world: &World, _state: &Self::State) -> Self {
                let value = Self {
					container: 0,
                    marker: PhantomData,
                    last_change_tick: 0,
                    change_tick: 0,
					dirty_container: SecondaryMap::with_capacity(0),
                };
                value
            }

			unsafe fn setting(&mut self, _world: &WorldInner, last_change_tick: u32, change_tick: u32) {
				self.last_change_tick = last_change_tick;
				self.change_tick = change_tick;
			}

			unsafe fn main_fetch<'a>(&'a self, state: &Self::State, last_change_tick: u32, _change_tick: u32) -> Option<MianFetch<'a>> {
				match {&*(state.dirty as *const Dirty)}.0.get(&state.component_id) {
					Some(r) => {
						if *r > last_change_tick {
							None
						} else {
							Some(MianFetch {
								value: self.dirty_container.keys(),
								next: None,
							})
						}
					},
					_ => {
						Some(MianFetch {
							value: self.dirty_container.keys(),
							next: None,
						})
					}
				}
			}

            unsafe fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype, _world: &World) {
                let c = archetype.get_component(state.component_id);
				match c.clone().downcast() {
					Ok(r) => {
						let r: Arc<TrustCell<MultiCaseImpl<T>>> = r;
						self.container = r.as_ptr() as usize;
					},
					Err(_) => panic!("downcast fail")
				}
            }

            unsafe fn archetype_fetch(&mut self, archetype_index: LocalVersion) -> Option<bool> {
				let value = (& *(self.container as *mut MultiCaseImpl<T>)).tick(archetype_index);
				match value {
					Some(r) => Some(r.is_changed(self.last_change_tick, self.change_tick) || r.is_added(self.last_change_tick, self.change_tick)),
					None => None,
				}
            }

			unsafe fn archetype_fetch_unchecked(&mut self, archetype_index: LocalVersion) -> bool {
				let value = (& *(self.container as *mut MultiCaseImpl<T>)).tick(archetype_index);
				match value {
					Some(r) => r.is_changed(self.last_change_tick, self.change_tick) || r.is_added(self.last_change_tick, self.change_tick),
					None => false,
				}
            }
        }

		impl<T: Component> FilterFetch for $fetch_name<T> {
			unsafe fn archetype_filter_fetch(&mut self, local: LocalVersion) -> bool {
				let value = (& *(self.container as *mut MultiCaseImpl<T>)).tick(local);
				match value {
					Some(r) => r.is_changed(self.last_change_tick, self.change_tick) || r.is_added(self.last_change_tick, self.change_tick),
					None => false,
				}
			}
		}
    };
}

impl_tick_filter!(
    /// Filter that retrieves components of type `T` that have been changed since the last
    /// execution of this system
    ///
    /// This filter is useful for synchronizing components, and as a performance optimization as it
    /// means that the query contains fewer items for a system to iterate over.
    ///
    /// Because the ordering of systems can change and this filter is only effective on changes
    /// before the query executes you need to use explicit dependency ordering or ordered
    /// stages to avoid frame delays.
    ///
    /// Example:
    /// ```
    /// # use bevy_ecs::system::IntoSystem;
    /// # use bevy_ecs::system::Query;
    /// # use bevy_ecs::query::Changed;
    /// #
    /// # #[derive(Debug)]
    /// # struct Name {};
    /// # struct Transform {};
    ///
    /// fn print_moving_objects_system(query: Query<&Name, Changed<Transform>>) {
    ///     for name in query.iter() {
    ///         println!("Entity Moved: {:?}", name);
    ///     }
    /// }
    ///
    /// # print_moving_objects_system.system();
    /// ```
    Changed,
    ChangedState,
    ChangedFetch,
    ComponentTicks::is_changed
);