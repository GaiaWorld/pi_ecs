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
	/// 系统状态初始化
	/// 通常做以下事情：
	/// * 检查当前参数是否与当前系统中的数据访问是否冲突（同一系统，不能同时读写相同数据）
	/// * 初始化该该参数在整个系统生命周期内不会改变的其他状态（每个系统参数会有各自不同的状态，根据自身需求初始化）
    fn init(world: &mut World, system_state: &mut SystemState, config: Self::Config) -> Self;

	/// 每个stage运行结束后，该参数应该被调用
	/// 通常用于刷新、整理前一阶段的数据（如延迟的指令操作需要flush、脏列表需要清理）}
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

pub fn assert_component_access_compatibility(
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

