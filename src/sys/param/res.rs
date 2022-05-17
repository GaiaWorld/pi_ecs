use crate::{
	resource::{Resource, ResourceId},
    entity::Entity,
    monitor::{Notify, NotifyImpl},
    sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState, NotApply},
    sys::system::interface::SystemState,
    world::{World, WorldInner, FromWorld}, component::ComponentId,
};
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut}, intrinsics::transmute,
};

/// Shared borrow of a resource.
///
/// # Panics
///
/// Panics when used as a [`SystemParameter`](SystemParam) if the resource does not exist.
///
/// Use `Option<Res<T>>` instead if the resource might not always exist.
pub struct Res<'w, T: Resource> {
    value: &'w T,
    _world: World,
    // ticks: &'w ComponentTicks,
    // last_change_tick: u32,
    // change_tick: u32,
}

impl<'w, T: Resource> Res<'w, T> {
    pub fn into_inner(self) -> &'w T {
        self.value
    }
}

pub struct ResMut<'w, T: Resource> {
    value: &'w mut T,
    resource_notify: &'w NotifyImpl,
    _world: World,
    // ticks: &'w mut ComponentTicks,
    // last_change_tick: u32,
    // change_tick: u32,
}

impl<'w, T: Resource> ResMut<'w, T> {
    pub fn create_event(&self, id: Entity) {
        self.resource_notify.create_event(id);
    }
    pub fn delete_event(&self, id: Entity) {
        self.resource_notify.delete_event(id);
    }
    pub fn modify_event(&self, id: Entity, field: &'static str, index: usize) {
        self.resource_notify.modify_event(id, field, index);
    }
}

impl<'w, T: Resource> Deref for Res<'w, T> {
    type Target = T;

    fn deref(&self) -> &'w Self::Target {
        // let v1 = self.value;
        // let r = unsafe{std::mem::transmute(v1)};
        // let t = r;
        // t
        self.value
    }
}

/// The [`SystemParamState`] of [`Res`].
pub struct ResState<T> {
    pub(crate) component_id: ResourceId,
    pub(crate) marker: PhantomData<T>,
}

impl<'w, T: Resource> SystemParam for Res<'w, T> {
    type Fetch = ResState<T>;
}

impl<T: Resource> ResState<T> {
	fn init_(world: &mut WorldInner, system_state: &mut SystemState, _config: <Self as SystemParamState>::Config, component_id: ComponentId) -> Self {
		let archetype_component_id = world.archetypes.get_archetype_resource_id::<T>().unwrap().clone();

		let combined_access = system_state.archetype_component_access.combined_access_mut();
        if combined_access.has_write(archetype_component_id) {
            panic!(
                "Res<{}> in system {} conflicts with a previous ResMut<{0}> access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                std::any::type_name::<T>(), system_state.name);
        }

        let archetype_component_id = world.archetypes.get_archetype_resource_id::<T>().unwrap();
        combined_access
            .add_read(*archetype_component_id);
        Self {
            component_id,
            marker: PhantomData,
        }
	}
}

// SAFE: Res ResourceId and ArchetypeResourceId access is applied to SystemState. If this Res
// conflicts with any prior access, a panic will occur.
unsafe impl<T: Resource> SystemParamState for ResState<T> {
    type Config = ();

    default fn init(world: &mut World, system_state: &mut SystemState, config: Self::Config) -> Self {
        let world: &mut WorldInner = world;
        let component_id = world.get_or_insert_resource_id::<T>();

        Self::init_(world, system_state, config, component_id)
    }

    fn default_config() {}
}

impl<T: Resource> NotApply for ResState<T> {}

/// 如果 T实现了Default, 则向World中插入T的默认值
unsafe impl<T: Resource + FromWorld> SystemParamState for ResState<T> {
    fn init(world: &mut World, system_state: &mut SystemState, config: Self::Config) -> Self {
        let component_id = match world.get_resource_id::<T>() {
			Some(r) => *r,
			None => {
				let value = T::from_world(world);
				world.insert_resource(value);
				*world.get_resource_id::<T>().unwrap()
			}
		};
		let world: &mut WorldInner = world;
        Self::init_(world, system_state, config, component_id)
    }
}

impl<'w, 's, T: Resource> SystemParamFetch<'w, 's> for ResState<T> {
    type Item = Res<'static, T>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        system_state: &SystemState,
        world: &'w World,
        _change_tick: u32,
    ) -> Self::Item {
        let value = world
            .archetypes
            .get_resource::<T>(state.component_id)
            .unwrap_or_else(|| {
                panic!(
                    "Component requested by {} does not exist: {}",
                    system_state.name,
                    std::any::type_name::<T>()
                )
            });
        Res {
            value: std::mem::transmute(value),
            _world: world.clone(),
            // ticks: &*column.get_ticks_mut_ptr(),
            // last_change_tick: system_state.last_change_tick,
            // change_tick,
        }
    }
}

impl<'w, T: Resource> Deref for ResMut<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // unsafe{std::mem::transmute_copy(self.value)}
        self.value
    }
}

impl<'w, T: Resource> DerefMut for ResMut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

/// The [`SystemParamState`] of [`ResMut`].
pub struct ResMutState<T> {
    component_id: ResourceId,
    marker: PhantomData<T>,
}

impl<T: Resource> ResState<T> {
    pub fn query(&self, world: &World) -> Res<T> {
        let value = unsafe {
            world
                .archetypes
                .get_resource::<T>(self.component_id)
                .unwrap_or_else(|| {
                    panic!(
                        "Component requested by {} does not exist: ResMutState.query",
                        std::any::type_name::<T>()
                    )
                })
        };

        Res {
            value: unsafe { std::mem::transmute(value) },
            _world: world.clone(),
            // ticks: &*column.get_ticks_mut_ptr(),
            // last_change_tick: system_state.last_change_tick,
            // change_tick,
        }
    }
    pub fn query_mut(&self, world: &mut World) -> ResMut<T> {
        let (value, resource_notify) = unsafe {
            (
                world
                    .archetypes
                    .get_resource_mut::<T>(self.component_id)
                    .unwrap_or_else(|| {
                        panic!(
                            "Component requested by {} does not exist: ResMutState.query",
                            std::any::type_name::<T>()
                        )
                    }),
                world.archetypes.resources.get_notify_ref(self.component_id),
            )
        };

        ResMut {
            value: unsafe { std::mem::transmute(value) },
            _world: world.clone(),
            resource_notify: unsafe { std::mem::transmute(resource_notify) },
            // ticks: &*column.get_ticks_mut_ptr(),
            // last_change_tick: system_state.last_change_tick,
            // change_tick,
        }
    }
}

impl<'w, T: Resource> SystemParam for ResMut<'w, T> {
    type Fetch = ResMutState<T>;
}

impl<T: Resource> ResMutState<T> {
	fn init_(world: &mut WorldInner, system_state: &mut SystemState, _config: <Self as SystemParamState>::Config, component_id: ComponentId) -> Self {
		let archetype_component_id = world.archetypes.get_archetype_resource_id::<T>().unwrap().clone();
		let combined_access = &mut system_state.archetype_component_access.combined_access_mut();

        if combined_access.has_read(archetype_component_id) {
            panic!(
                "ResMut<{}> in system {} conflicts with a previous Res<{0}> access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                std::any::type_name::<T>(), system_state.name);
        }

        combined_access.add_write(archetype_component_id);

        // system_state
        //     .archetype_component_access
        //     .add_read(*archetype_component_id);
        Self {
            component_id,
            marker: PhantomData,
        }
	}
}

// SAFE: ResMut ResourceId and ArchetypeResourceId access is applied to SystemState. If this ResMut
// conflicts with any prior access, a panic will occur.
unsafe impl<T: Resource> SystemParamState for ResMutState<T> {
    type Config = ();

    default fn init(world: &mut World, system_state: &mut SystemState, config: Self::Config) -> Self {
        let world: &mut WorldInner = world;
        let component_id = world.get_or_insert_resource_id::<T>();
        Self::init_(world, system_state, config, component_id)
    }

    fn default_config() {}
}

impl<T: Resource> NotApply for ResMutState<T> {}

/// 如果 T实现了Default, 则向World中插入T的默认值
unsafe impl<T: Resource + FromWorld> SystemParamState for ResMutState<T> {
    fn init(world: &mut World, system_state: &mut SystemState, config: Self::Config) -> Self {
        let component_id = match world.get_resource_id::<T>() {
			Some(r) => *r,
			None => {
				let value = T::from_world(world);
				world.insert_resource(value);
				*world.get_resource_id::<T>().unwrap()
			}
		};
		let world: &mut WorldInner = world;
        Self::init_(world, system_state, config, component_id)
    }
}



impl<'w, 's, T: Resource> SystemParamFetch<'w, 's> for ResMutState<T> {
    type Item = ResMut<'static, T>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        system_state: &SystemState,
        world: &'w World,
        _change_tick: u32,
    ) -> Self::Item {
        let (value, resource_notify) = (
            world
                .archetypes
                .get_resource_mut::<T>(state.component_id)
                .unwrap_or_else(|| {
                    panic!(
                        "Component requested by {} does not exist: {}",
                        system_state.name,
                        std::any::type_name::<T>()
                    )
                }),
            world
                .archetypes
                .resources
                .get_notify_ref(state.component_id),
        );
        ResMut {
            value: std::mem::transmute(value),
            _world: world.clone(),
            resource_notify: std::mem::transmute(resource_notify),
        }
    }
}

pub struct WriteRes<'w, T: Resource> {
	value: Option<&'w T>,
    _resource_notify: &'w NotifyImpl,
    _world: World,
}

impl<'w, T: Resource> WriteRes<'w, T> {
	/// 取到不可变引用
    pub fn get(&self) -> Option<&T> {
		match &self.value {
			Some(r) => Some(r),
			None => None
		}
	}

	/// 取到可变引用
	pub fn get_mut(&mut self) -> Option<&mut T> {
		match self.value {
			Some(r) => Some(unsafe { &mut *(r as *const T as usize as *mut T)}),
			None => None
		}
	}

	// 插入
	pub fn write(&mut self, v: T) {
		self._world.insert_resource(v);
		self.value = unsafe { transmute(self._world.get_resource::<T>()) };
	}
}

/// The [`SystemParamState`] of [`ResMut`].
pub struct WriteResState<T> {
    component_id: ResourceId,
    marker: PhantomData<T>,
}

impl<'w, T: Resource> SystemParam for WriteRes<'w, T> {
    type Fetch = WriteResState<T>;
}

impl<T: Resource> WriteResState<T> {
	fn init_(world: &mut WorldInner, system_state: &mut SystemState, _config: <Self as SystemParamState>::Config, component_id: ComponentId) -> Self {
		let archetype_component_id = world.archetypes.get_archetype_resource_id::<T>().unwrap().clone();
		let combined_access = &mut system_state.archetype_component_access.combined_access_mut();
        if combined_access.has_read(archetype_component_id) {
            panic!(
                "ResMut<{}> in system {} conflicts with a previous Res<{0}> access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                std::any::type_name::<T>(), system_state.name);
        }

        combined_access.add_modify(archetype_component_id);

        // system_state
        //     .archetype_component_access
        //     .add_read(*archetype_component_id);
        Self {
            component_id,
            marker: PhantomData,
        }
	}
}

// SAFE: ResMut ResourceId and ArchetypeResourceId access is applied to SystemState. If this ResMut
// conflicts with any prior access, a panic will occur.
unsafe impl<T: Resource> SystemParamState for WriteResState<T> {
    type Config = ();

    fn init(world: &mut World, system_state: &mut SystemState, config: Self::Config) -> Self {
        let world: &mut WorldInner = world;
        let component_id = world.get_or_insert_resource_id::<T>();
        Self::init_(world, system_state, config, component_id)
    }

    fn default_config() {}
}

impl<'w, 's, T: Resource> SystemParamFetch<'w, 's> for WriteResState<T> {
    type Item = WriteRes<'static, T>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        _system_state: &SystemState,
        world: &'w World,
        _change_tick: u32,
    ) -> Self::Item {
        let (value, resource_notify) = (
            world
                .archetypes
                .get_resource_mut::<T>(state.component_id),
            world
                .archetypes
                .resources
                .get_notify_ref(state.component_id),
        );
        WriteRes {
            value: std::mem::transmute(value),
            _world: world.clone(),
            _resource_notify: std::mem::transmute(resource_notify),
        }
    }
}

pub struct OptionResState<T>(ResState<T>);

impl<'w, T: Resource> SystemParam for Option<Res<'w, T>> {
    type Fetch = OptionResState<T>;
}

unsafe impl<T: Resource> SystemParamState for OptionResState<T> {
    type Config = ();

    fn init(world: &mut World, system_state: &mut SystemState, _config: Self::Config) -> Self {
        Self(ResState::init(world, system_state, ()))
    }

    fn default_config() {}
}

impl<'w, 's, T: Resource> SystemParamFetch<'w, 's> for OptionResState<T> {
    type Item = Option<Res<'static, T>>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        _system_state: &SystemState,
        world: &'w World,
        _change_tick: u32,
    ) -> Self::Item {
        match world.archetypes.get_resource_mut::<T>(state.0.component_id) {
            Some(value) => Some(Res {
                value: std::mem::transmute(value),
                _world: world.clone(),
            }),
            None => None,
        }
    }
}

pub struct OptionResMutState<T>(ResMutState<T>);

impl<'w, T: Resource> SystemParam for Option<ResMut<'w, T>> {
    type Fetch = OptionResMutState<T>;
}

unsafe impl<T: Resource> SystemParamState for OptionResMutState<T> {
    type Config = ();

    fn init(world: &mut World, system_state: &mut SystemState, _config: Self::Config) -> Self {
        Self(ResMutState::init(world, system_state, ()))
    }

    fn default_config() {}
}

impl<'w, 's, T: Resource> SystemParamFetch<'w, 's> for OptionResMutState<T> {
    type Item = Option<ResMut<'static, T>>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        _system_state: &SystemState,
        world: &'w World,
        _change_tick: u32,
    ) -> Self::Item {
        match world.archetypes.get_resource_mut::<T>(state.0.component_id) {
            Some(value) => {
                let resource_notify = world
                    .archetypes
                    .resources
                    .get_notify_ref(state.0.component_id);
                Some(ResMut {
                    value: std::mem::transmute(value),
                    _world: world.clone(),
                    resource_notify: std::mem::transmute(resource_notify),
                })
            }
            None => None,
        }
    }
}
