use pi_map::Map;
use pi_share::cell::TrustCell;

use std::sync::Arc;
use std::default::Default;

use crate::{
    archetype::{Archetype, ArchetypeComponentId, ArchetypeId},
	monitor::{Event, Listen, ComponentListen, Create, Modify, Listeners, ListenSetup},
    component::{Component, ComponentId, MultiCaseImpl},
    query::{Fetch, FetchState, Access, FilteredAccess, WorldQuery, MianFetch, FilterFetch},
    storage::{SecondaryMap, Local, LocalVersion},
    world::{World, WorldInner},
};
use std::{marker::PhantomData};

/// 为每个查询创建一个脏列表
/// 在world上将DirtyList注册为一个资源，list中的每个元素代表一个查询的脏列表
pub struct DirtyLists {
	pub(crate) list: SecondaryMap<Local, DirtyList>, // 脏列表
}

pub struct DirtyList {
	pub(crate) init_list: SecondaryMap<Local,()>,
	pub(crate) value: SecondaryMap<LocalVersion,()>,
}

impl Default for DirtyLists {
    fn default() -> Self {
		DirtyLists { 
			list: SecondaryMap::with_capacity(0) 
		}
	}
}

macro_rules! impl_tick_filter {
    (
        $(#[$meta:meta])*
        $name: ident, $state_name: ident, $fetch_name: ident, $is_detected: expr, $listen: ty) => {
        $(#[$meta])*
        pub struct $name<T>(PhantomData<T>);

        pub struct $fetch_name<T> {
            // table_ticks: *mut ComponentTicks,
            // entity_table_rows: *const usize,
			container: usize, // 组件容器

            marker: PhantomData<T>,
            last_change_tick: u32,
            change_tick: u32,
        }

        pub struct $state_name<T> {
			dirty_list: usize,
            pub component_id: ComponentId,
			is_main: bool, // 是否由该过滤器添加的监听器（如果是，main_fetch会返回对应的迭代器，否则不会返回，因为重复返回会造成重复迭代）
            marker: PhantomData<T>,
        }

        impl<T: Component> WorldQuery for $name<T> {
            type Fetch = $fetch_name<T>;
            type State = $state_name<T>;
        }


        // SAFE: this reads the T component. archetype component access and component access are updated to reflect that
        unsafe impl<T: Component> FetchState for $state_name<T> {
            fn init(world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self {
                let component_id = world.get_or_register_component::<T>(archetype_id);

				// 如果world上没有Dirty资源，则插入Dirty资源
				let dirty_id = world.get_resource_id::<DirtyLists>();
				let dirty_id = match dirty_id {
					Some(r) =>  r.clone(),
					None => world.insert_resource(DirtyLists {
						list: SecondaryMap::with_capacity(0),
					}),
				};

				let dirty_list = unsafe{world
				.archetypes.get_resource_mut::<DirtyLists>(dirty_id).unwrap()};
				
				let (cur_list, is_main) = match dirty_list.list.get(&Local::new(query_id)) {
					Some(r) => (r, false),
					None => {
						dirty_list.list.insert(Local::new(query_id), DirtyList {
							init_list: SecondaryMap::with_capacity(0),
							value: SecondaryMap::with_capacity(0)
						});
						(&dirty_list.list[Local::new(query_id)], true)
					}
				};
				let dirty = cur_list as *const DirtyList as usize;

                Self {
					dirty_list: dirty, // 脏列表的指针
                    component_id,
					is_main,
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
				if let None = unsafe{&*(self.dirty_list as *const DirtyList)}.init_list.get(&self.component_id) {
					let component_id = self.component_id;
					let dirty_list = self.dirty_list;

					// 安装监听器，监听对应组件修改，并将改变的实体插入到脏列表中
					let listen = move |event: Event, _:Listen<(ComponentListen<A, T, $listen>, )> | {
						unsafe{&mut *(dirty_list as *mut DirtyList)}.value.insert(event.id.local(), ());
					};

					// 标记监听器已经设置，下次不需要重复设置（同一个查询可能涉及到多次相同组件的过滤）
					unsafe{&mut *(self.dirty_list as *mut DirtyList)}.init_list.insert(component_id, ());
					let l = listen.listeners();
					l.setup(world);
				}
			}

			// 清理脏列表
			fn apply(&self, _world: &mut World) {
				if self.is_main {
					unsafe{&mut *(self.dirty_list as *mut DirtyList)}.value.clear();
				}
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
                };
                value
            }

			unsafe fn setting(&mut self, _world: &WorldInner, last_change_tick: u32, change_tick: u32) {
				self.last_change_tick = last_change_tick;
				self.change_tick = change_tick;
			}

			unsafe fn main_fetch<'a>(&'a self, state: &Self::State, _last_change_tick: u32, _change_tick: u32) -> Option<MianFetch<'a>> {
				if state.is_main {
					Some(MianFetch {
						value: (&*(state.dirty_list as *const DirtyList)).value.keys(),
						next: None,
					})
				} else {
					None
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
    ComponentTicks::is_changed,
	(Create, Modify)

);

impl_tick_filter!(
    Added,
    AddedState,
    AddedFetch,
    ComponentTicks::is_added,
	Create
);

impl_tick_filter!(
    Modifyed,
    ModifyedState,
    ModifyedFetch,
    ComponentTicks::is_modifyed,
	Modify
);

