use pi_map::Map;
use pi_share::cell::TrustCell;

use std::sync::Arc;
use std::default::Default;

use crate::{
    archetype::{Archetype, ArchetypeComponentId, ArchetypeId},
	monitor::{Event, Listen, ComponentListen, Create, Modify, Listeners, ListenSetup, Delete},
    component::{Component, ComponentId, MultiCaseImpl},
    query::{
		access::FilteredAccess,
		fetch::{Fetch, FetchState, WorldQuery, MianFetch},
		filter::FilterFetch,
	},
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
			index: Local,
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
					}).id(),
				};

				let dirty_list = unsafe{world
				.archetypes.get_resource_mut::<DirtyLists>(dirty_id).unwrap()};
				
				let is_main = match dirty_list.list.get(&Local::new(query_id)) {
					Some(_) => false,
					None => {
						dirty_list.list.insert(Local::new(query_id), DirtyList {
							init_list: SecondaryMap::with_capacity(0),
							value: SecondaryMap::with_capacity(0)
						});
						true
					}
				};
				let dirty = dirty_list as *const DirtyLists as usize;

                Self {
					dirty_list: dirty, // 脏列表的指针
					index: Local::new(query_id),
                    component_id,
					is_main,
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
				let lists = unsafe{&mut *(self.dirty_list as *mut DirtyLists)};
				let list = &mut lists.list[self.index];
				if let None = list.init_list.get(&self.component_id) {
					let component_id = self.component_id;
					let dirty_list = self.dirty_list;
					let index = self.index;

					// 安装监听器，监听对应组件修改，并将改变的实体插入到脏列表中
					let listen = move |event: Event, _:Listen<(ComponentListen<A, T, $listen>, )> | {
						let lists = unsafe{&mut *(dirty_list as *mut DirtyLists)};
						let list = &mut lists.list[index];
						list.value.insert(event.id.local(), ());
					};

					// 标记监听器已经设置，下次不需要重复设置（同一个查询可能涉及到多次相同组件的过滤）
					list.init_list.insert(component_id, ());
					let l = listen.listeners();
					l.setup(world);
				}
			}

			// 清理脏列表
			fn apply(&self, _world: &mut World) {
				if self.is_main {
					let lists = unsafe{&mut *(self.dirty_list as *mut DirtyLists)};
					let list = &mut lists.list[self.index];
					list.value.clear();
				}
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
                };
                value
            }

			unsafe fn setting(&mut self, _world: &WorldInner, last_change_tick: u32, change_tick: u32) {
				self.last_change_tick = last_change_tick;
				self.change_tick = change_tick;
			}

			unsafe fn main_fetch<'a>(&'a self, state: &Self::State, _last_change_tick: u32, _change_tick: u32) -> Option<MianFetch<'a>> {
				if state.is_main {
					let lists = &mut *(state.dirty_list as *mut DirtyLists);
					let list = &mut lists.list[state.index];
					Some(MianFetch {
						value: list.value.keys(),
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

impl_tick_filter!(
    Deleted,
    DeletedState,
    DeletedFetch,
    ComponentTicks::is_deleted,
	Delete
);
