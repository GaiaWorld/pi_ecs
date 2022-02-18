use std::ops::{Deref, DerefMut};

use crate::{
    sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState},
    sys::system::interface::SystemState,
    world::{World, WorldInner},
};

pub struct WorldRead(World);

impl Deref for WorldRead {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SystemParam for WorldRead {
    type Fetch = WorldRead;
}

unsafe impl SystemParamState for WorldRead {
    type Config = Option<()>;

    fn init(world: &mut World, system_state: &mut SystemState, _config: Self::Config) -> Self {
        let combined_access = system_state.component_access_set.combined_access_mut();

        // 所有的 Res 都是 读
        let w: &mut WorldInner = world;
        for (_, r) in w.components.resource_indices.iter() {
            let component_id = r.clone();
            if combined_access.has_write(component_id) {
                panic!(
                    "WorldRead in system {} conflicts with a previous access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                    system_state.name);
            }

            combined_access.add_read(component_id);
        }

        // 所有的 Component 都是 读
        let combined_access = system_state.component_access_set.combined_access_mut();
        for a in w.archetypes.iter() {
            for id in a.component_ids() {
                let component_id = id.clone();
                if combined_access.has_write(component_id) {
                    panic!(
                        "WorldRead in system {} conflicts with a previous access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                        system_state.name);
                }
                combined_access.add_read(component_id);
            }
        }

        WorldRead(world.clone())
    }

    fn default_config() -> Self::Config {
        None
    }
}

impl<'a> SystemParamFetch<'a> for WorldRead {
    type Item = WorldRead;

    #[inline]
    unsafe fn get_param(
        _state: &'a mut Self,
        _system_state: &'a SystemState,
        world: &'a World,
        _last_change_tick: u32,
    ) -> Self::Item {
        WorldRead(world.clone())
    }
}

pub struct WorldMut(World);

impl Deref for WorldMut {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for WorldMut {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl SystemParam for WorldMut {
    type Fetch = WorldMut;
}

unsafe impl SystemParamState for WorldMut {
    type Config = Option<()>;

    fn init(world: &mut World, system_state: &mut SystemState, _config: Self::Config) -> Self {
        let w: &mut WorldInner = world;

        for (_, r) in w.components.resource_indices.iter() {
            let component_id = r.clone();
            let combined_access = system_state.component_access_set.combined_access_mut();
            if combined_access.has_write(component_id) {
                panic!(
                    "WorldMut in system {} conflicts with a previous access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                    system_state.name);
            }

            if combined_access.has_read(component_id) {
                panic!(
                    "WorldMut in system {} conflicts with a previous access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                    system_state.name);
            }

            combined_access.add_write(component_id);
        }

        // 所有的 Component 都是 写
        let combined_access = system_state.component_access_set.combined_access_mut();
        for a in w.archetypes.iter() {
            for id in a.component_ids() {
                let component_id = id.clone();
                if combined_access.has_write(component_id) {
                    panic!(
                        "WorldMut in system {} conflicts with a previous access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                        system_state.name);
                }

                if combined_access.has_read(component_id) {
                    panic!(
                        "WorldMut in system {} conflicts with a previous access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                        system_state.name);
                }
                combined_access.add_write(component_id);
            }
        }

        WorldMut(world.clone())
    }

    fn default_config() -> Self::Config {
        None
    }
}

impl<'a> SystemParamFetch<'a> for WorldMut {
    type Item = WorldMut;

    #[inline]
    unsafe fn get_param(
        _state: &'a mut Self,
        _system_state: &'a SystemState,
        world: &'a World,
        _last_change_tick: u32,
    ) -> Self::Item {
        WorldMut(world.clone())
    }
}
