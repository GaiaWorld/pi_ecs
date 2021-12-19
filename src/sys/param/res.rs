use crate::{
	component::{ComponentId, Component},
	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState},
	sys::system::interface::SystemState,
	world::World,
};
use std::{marker::PhantomData, ops::Deref};


/// Shared borrow of a resource.
///
/// # Panics
///
/// Panics when used as a [`SystemParameter`](SystemParam) if the resource does not exist.
///
/// Use `Option<Res<T>>` instead if the resource might not always exist.
pub struct Res<'w, T: Component> {
    value: &'w T,
    // ticks: &'w ComponentTicks,
    // last_change_tick: u32,
    // change_tick: u32,
}

pub struct ResMut<'w, T: Component> {
    value: &'w mut T,
    // ticks: &'w mut ComponentTicks,
    // last_change_tick: u32,
    // change_tick: u32,
}

impl<'w, T: Component> Deref for Res<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

/// The [`SystemParamState`] of [`Res`].
pub struct ResState<T> {
    component_id: ComponentId,
    marker: PhantomData<T>,
}

impl<'a, T: Component> SystemParam for Res<'a, T> {
    type Fetch = ResState<T>;
}

// SAFE: Res ComponentId and ArchetypeComponentId access is applied to SystemState. If this Res
// conflicts with any prior access, a panic will occur.
unsafe impl<T: Component> SystemParamState for ResState<T> {
    type Config = ();

    fn init(world: &mut World, system_state: &mut SystemState, _config: Self::Config) -> Self {
        let component_id = world.get_resource_id::<T>();
		let component_id = match component_id {
			Some(r) =>  r.clone(),
			None =>  panic!(
                "Res<{}> is not exist in system {}",
                std::any::type_name::<T>(), system_state.name),
		};
        
		let combined_access = system_state.component_access_set.combined_access_mut();
        if combined_access.has_write(component_id) {
            panic!(
                "Res<{}> in system {} conflicts with a previous ResMut<{0}> access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                std::any::type_name::<T>(), system_state.name);
        }
        combined_access.add_read(component_id);

        let archetype_component_id = world.archetypes.get_archetype_resource_id::<T>().unwrap();
        system_state
            .archetype_component_access
            .add_read(*archetype_component_id);
        Self {
            component_id,
            marker: PhantomData,
        }
    }

    fn default_config() {}
}

impl<'a, T: Component> SystemParamFetch<'a> for ResState<T> {
    type Item = Res<'a, T>;

    #[inline]
    unsafe fn get_param(
        state: &'a mut Self,
        system_state: &'a SystemState,
        world: &'a World,
        _change_tick: u32,
    ) -> Self::Item {
        let value = world
            .archetypes.get_resource(state.component_id)
            .unwrap_or_else(|| {
                panic!(
                    "Component requested by {} does not exist: {}",
                    system_state.name,
                    std::any::type_name::<T>()
                )
            });
        Res {
            value: &*value.as_ptr().cast::<T>(),
            // ticks: &*column.get_ticks_mut_ptr(),
            // last_change_tick: system_state.last_change_tick,
            // change_tick,
        }
    }
}

impl<'w, T: Component> Deref for ResMut<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

/// The [`SystemParamState`] of [`ResMut`].
pub struct ResMutState<T> {
    component_id: ComponentId,
    marker: PhantomData<T>,
}

impl<'a, T: Component> SystemParam for ResMut<'a, T> {
    type Fetch = ResMutState<T>;
}

// SAFE: ResMut ComponentId and ArchetypeComponentId access is applied to SystemState. If this ResMut
// conflicts with any prior access, a panic will occur.
unsafe impl<T: Component> SystemParamState for ResMutState<T> {
    type Config = ();

    fn init(world: &mut World, system_state: &mut SystemState, _config: Self::Config) -> Self {
        let component_id = world.get_resource_id::<T>();
		let component_id = match component_id {
			Some(r) =>  r.clone(),
			None =>  panic!(
                "ResMut<{}> is not exist in system {}",
                std::any::type_name::<T>(), system_state.name),
		};
        
		let combined_access = system_state.component_access_set.combined_access_mut();
        if combined_access.has_write(component_id) {
            panic!(
                "ResMut<{}> in system {} conflicts with a previous ResMut<{0}> access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                std::any::type_name::<T>(), system_state.name);
        }
        combined_access.add_read(component_id);

        let archetype_component_id = world.archetypes.get_archetype_resource_id::<T>().unwrap();
        system_state
            .archetype_component_access
            .add_read(*archetype_component_id);
        Self {
            component_id,
            marker: PhantomData,
        }
    }

    fn default_config() {}
}

impl<'a, T: Component> SystemParamFetch<'a> for ResMutState<T> {
    type Item = ResMut<'a, T>;

    #[inline]
    unsafe fn get_param(
        state: &'a mut Self,
        system_state: &'a SystemState,
        world: &'a World,
        _change_tick: u32,
    ) -> Self::Item {
        let value = world
            .archetypes.get_resource(state.component_id)
            .unwrap_or_else(|| {
                panic!(
                    "Component requested by {} does not exist: {}",
                    system_state.name,
                    std::any::type_name::<T>()
                )
            });
        ResMut {
            value: &mut *value.as_ptr().cast::<T>(),
            // ticks: &*column.get_ticks_mut_ptr(),
            // last_change_tick: system_state.last_change_tick,
            // change_tick,
        }
    }
}