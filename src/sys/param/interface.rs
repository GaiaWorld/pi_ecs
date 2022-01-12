use pi_ecs_macros::all_tuples;
use crate::world::World;
use crate::component::ComponentId;
use crate::query::{FilteredAccessSet, FilteredAccess};
use crate::sys::system::interface::SystemState;

pub trait SystemParam: Sized {
    type Fetch: for<'a> SystemParamFetch<'a>;
}

pub trait SystemParamFetch<'a>: SystemParamState {
    type Item;
    /// # Safety
    ///
    /// This call might access any of the input parameters in an unsafe way. Make sure the data
    /// access is safe in the context of the system scheduler.
    unsafe fn get_param(
        state: &'a mut Self,
        system_state: &'a SystemState,
        world: &'a World,
        change_tick: u32,
    ) -> Self::Item;
}

pub unsafe trait SystemParamState: Send + Sync + 'static {
    type Config: Send + Sync;
    fn init(world: &mut World, system_state: &mut SystemState, config: Self::Config) -> Self;

	// 不会新增原型
    // #[inline]
    // fn new_archetype(&mut self, _archetype: &Archetype, _system_state: &mut SystemState) {}
    #[inline]
    fn apply(&mut self, _world: &mut World) {}
    fn default_config() -> Self::Config;
}

macro_rules! impl_system_param_tuple {
    ($($param: ident),*) => {
        impl<$($param: SystemParam),*> SystemParam for ($($param,)*) {
            type Fetch = ($($param::Fetch,)*);
        }
        #[allow(unused_variables)]
        #[allow(non_snake_case)]
        impl<'a, $($param: SystemParamFetch<'a>),*> SystemParamFetch<'a> for ($($param,)*) {
            type Item = ($($param::Item,)*);

            #[inline]
            unsafe fn get_param(
                state: &'a mut Self,
                system_state: &'a SystemState,
                world: &'a World,
                change_tick: u32,
            ) -> Self::Item {

                let ($($param,)*) = state;
                ($($param::get_param($param, system_state, world, change_tick),)*)
            }
        }

        /// SAFE: implementors of each SystemParamState in the tuple have validated their impls
        #[allow(non_snake_case)]
        unsafe impl<$($param: SystemParamState),*> SystemParamState for ($($param,)*) {
            type Config = ($(<$param as SystemParamState>::Config,)*);
            #[inline]
            fn init(_world: &mut World, _system_state: &mut SystemState, config: Self::Config) -> Self {
                let ($($param,)*) = config;
                (($($param::init(_world, _system_state, $param),)*))
            }

            #[inline]
            fn apply(&mut self, _world: &mut World) {
                let ($($param,)*) = self;
                $($param.apply(_world);)*
            }

            fn default_config() -> ($(<$param as SystemParamState>::Config,)*) {
                ($(<$param as SystemParamState>::default_config(),)*)
            }
        }
    };
}


all_tuples!(impl_system_param_tuple, 0, 16, P);

pub(crate) fn assert_component_access_compatibility(
    system_name: &str,
    query_type: &'static str,
    filter_type: &'static str,
    system_access: &FilteredAccessSet<ComponentId>,
    current: &FilteredAccess<ComponentId>,
    world: &World,
) {
    let mut conflicts = system_access.get_conflicts(current);
    if conflicts.is_empty() {
        return;
    }
    let conflicting_components = conflicts
        .drain(..)
        .map(|component_id| world.components.get_info(component_id).unwrap().name())
        .collect::<Vec<&str>>();
    let accesses = conflicting_components.join(", ");
    panic!("Query<{}, {}> in system {} accesses component(s) {} in a way that conflicts with a previous system parameter. Allowing this would break Rust's mutability rules. Consider merging conflicting Queries into a QuerySet.",
                query_type, filter_type, system_name, accesses);
}

