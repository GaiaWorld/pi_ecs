use pi_map::Map;
use pi_share::cell::TrustCell;

use std::sync::Arc;
use std::default::Default;

use crate::{
    archetype::{Archetype, ArchetypeComponentId, ArchetypeId},
	monitor::{Event, Listen, ComponentListen, Create, Modify, Listeners, ListenSetup},
    component::{Component, ComponentId, MultiCaseImpl},
    query::{
		access::FilteredAccess,
		fetch::{Fetch, FetchState, WorldQuery, MianFetch},
		filter::FilterFetch,
	},
    storage::{SecondaryMap, Local, LocalVersion},
	sys::param::{ResMut, Tick},
    world::{World, WorldInner},
};
use std::{marker::PhantomData};

pub struct Dirty (pub(crate) SecondaryMap<Local, u32/*tick*/>, pub(crate) SecondaryMap<Local, bool>);

impl Default for Dirty {
    fn default() -> Self {
		Dirty(SecondaryMap::with_capacity(0), SecondaryMap::with_capacity(0))
	}
}

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
            fn init(world: &mut World, _query_id: usize, archetype_id: ArchetypeId) -> Self {
                let component_id = world.get_or_register_component::<T>(archetype_id);

				// 如果world上没有Dirty资源，则插入Dirty资源
				let dirty_id = world.get_resource_id::<Dirty>();
				let dirty_id = match dirty_id {
					Some(r) =>  r.clone(),
					None => world.insert_resource(Dirty::default()).id() ,
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
            fn update_archetype_component_access(
                &self,
                archetype: &Archetype,
                access: &mut FilteredAccess<ArchetypeComponentId>,
            ) {
				let archetype_component_id = unsafe{ archetype.archetype_component_id(self.component_id)};
				// if access.has_write(archetype_component_id) {
				// 	panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
				// 		std::any::type_name::<T>());
				// }
                access.add_read(archetype_component_id);
            }

            fn matches_archetype(&self, archetype: &Archetype) -> bool {
                archetype.contains(self.component_id)
            }
			fn init_archetype<A: 'static + Send + Sync>(&self, world: &mut World) {
				match unsafe{&*(self.dirty as *const Dirty)}.1.get(&self.component_id) {
					Some(r) if *r == true => (),
					_ => {
						let component_id = self.component_id;
						// 安装监听器，监听对应组件修改，并设置该组件的全局脏标记为true
						let listen = move |_event: Event, _:Listen<(ComponentListen<A, T, (Create, Modify)>, )> , mut res: ResMut<Dirty>, tick: Tick| {
							// let aa = &*res as *const Dirty as usize;
							res.0.insert(component_id, tick.change_tick);
						};
						unsafe{&mut *(self.dirty as *mut Dirty)}.1.insert(component_id, true);
						let l = listen.listeners();
						l.setup(world);
					}
				};
			}
        }

        impl<'s, T: Component> Fetch<'s> for $fetch_name<T> {
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