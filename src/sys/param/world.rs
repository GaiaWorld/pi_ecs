use std::ops::{Deref, DerefMut};

use crate::{
    sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState, NotApply},
    sys::system::interface::SystemState,
    world::{World, WorldInner}, archetype::ArchetypeComponentId,
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
        let combined_access = system_state.archetype_component_access.combined_access_mut();

        // 所有的 Res 都是 读
        let w: &mut WorldInner = world;
		
        for r in 0..w.archetypes().archetype_component_info.len() {
			if !w.archetypes().data_mark().contains(r) {
				continue;
			}
            let archetype_component_id = ArchetypeComponentId::new(r);
            if combined_access.has_write(archetype_component_id) {
                panic!(
                    "WorldRead in system {} conflicts with a previous access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                    system_state.name);
            }

            combined_access.add_read(archetype_component_id);
        }

        WorldRead(world.clone())
    }

    fn default_config() -> Self::Config {
        None
    }
}

impl<'w, 's> SystemParamFetch<'w, 's> for WorldRead {
    type Item = WorldRead;

    #[inline]
    unsafe fn get_param(
        _state: &'s mut Self,
        _system_state: &SystemState,
        world: &'w World,
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

		let combined_access = system_state.archetype_component_access.combined_access_mut();
		for r in 0..w.archetypes().archetype_component_info.len() {
			if !w.archetypes().data_mark().contains(r) {
				continue;
			}
            let archetype_component_id = ArchetypeComponentId::new(r);
            if combined_access.has_write(archetype_component_id) || combined_access.has_read(archetype_component_id) {
                panic!(
                    "WorldMut in system {} conflicts with a previous access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
                    system_state.name);
            }

            combined_access.add_write(archetype_component_id);
        }

        WorldMut(world.clone())
    }

    fn default_config() -> Self::Config {
        None
    }
}

impl<'w, 's> SystemParamFetch<'w, 's> for WorldMut {
    type Item = WorldMut;

    #[inline]
    unsafe fn get_param(
        _state: &'s mut Self,
        _system_state: &SystemState,
        world: &'w World,
        _last_change_tick: u32,
    ) -> Self::Item {
        WorldMut(world.clone())
    }
}

impl NotApply for WorldMut {}

impl NotApply for WorldRead {}
