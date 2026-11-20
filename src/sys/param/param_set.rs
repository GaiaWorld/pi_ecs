//! ParamSet： 参数集
//! System在初始化时，会检查System参数的冲突性，System不允许两个参数对同一个数据的读写冲突
//! ParamSet将多个参数声明在一起，他们相互之间可以存在读写冲突；system内部逻辑只能通过ParamSet的q0、q1、q0_mut、q1_mut、等方法，每次只能从ParamSet获得ParamSet中的一个参数。q0、q1、q0_mut、q1_mut等方法消耗了ParamSet的引用，只有取出来的参数归还，才能从ParamSet中获得另一个参数，因此也能从语法上限制对同一数据的读写冲突
use crate::{
	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState, NotApply},
	sys::system::interface::SystemState,
	world::World
};
use pi_ecs_macros::{all_tuples, impl_param_set_field};

pub struct ParamSet<T>(T);

pub struct ParamSetState<T>(T);

macro_rules! impl_param_set {
    ($($param: ident),*) => {
		impl<$($param: SystemParam),*> SystemParam for ParamSet<($($param,)*)> {
			type Fetch = ParamSetState<($($param::Fetch,)*)>;
		}

		impl<$($param: SystemParamState + NotApply),*> NotApply for ParamSetState<($($param,)*)> {}

		
		$crate::paste::item! {
			#[allow(non_snake_case)]
			unsafe impl<$($param: SystemParamState,)*> SystemParamState for ParamSetState<($($param,)*)> {
				type Config = ($(<$param as SystemParamState>::Config,)*);
				
				#[allow(unused_variables)]
				#[allow(unused_mut)]
				fn init(world:  &mut World, system_state: &mut SystemState, config: Self::Config) -> Self {
					let ($([<$param c>],)*) = config;
					let mut old_archetype_component_access = system_state.archetype_component_access.clone();
					let mut cur_archetype_component_access = system_state.archetype_component_access.clone();

					let r = ParamSetState(($(
						{
							let s = $param::init(world, system_state, [<$param c>]);
							cur_archetype_component_access.combined_access_mut().extend(
								&std::mem::replace(&mut system_state.archetype_component_access, old_archetype_component_access.clone()).combined_access()
							);
							s
						},
					)*));
					system_state.archetype_component_access = cur_archetype_component_access;
					r
				}
			
				fn default_config() -> Self::Config {
					($(<$param as SystemParamState>::default_config(),)*)
				}
			}
		
			#[allow(non_snake_case)]
			impl<'w, 's, $($param: SystemParamFetch<'w, 's>),*> SystemParamFetch<'w, 's> for ParamSetState<($($param,)*)> {
				type Item = ParamSet<($($param::Item,)*)>;
			
				#[inline]
				#[allow(unused_variables)]
				unsafe fn get_param(
					state: &'s mut Self,
					system_state: &SystemState,
					world: &'w World,
					last_change_tick: u32,
				) -> Self::Item {
					let ($($param,)*) = &mut state.0;
					ParamSet(($(<$param as SystemParamFetch>::get_param($param, system_state, world, last_change_tick),)*))
				}
			}
		}
	}
}

all_tuples!(impl_param_set, 2, 16, P);
impl_param_set_field!(2, 16);

