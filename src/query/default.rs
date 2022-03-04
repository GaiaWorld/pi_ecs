// use pi_share::cell::TrustCell;

// use crate::{
// 	component::{ComponentId, Component},
// 	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState},
// 	sys::system::interface::SystemState,
// 	world::World,
// };
// use std::{marker::PhantomData, ops::Deref, sync::Arc};

// pub struct Changed<T: Component> {
// 	_world: World,
// 	mark: PhantomData<T>,
// }

// /// The [`SystemParamState`] of [`Res`].
// pub struct ChangedState<T> {
//     component_id: ComponentId,
//     marker: PhantomData<T>,
// }

// impl<T: Component> SystemParam for Changed<T> {
//     type Fetch = ChangedState<T>;
// }

// // SAFE: Res ComponentId and ArchetypeComponentId access is applied to SystemState. If this Res
// // conflicts with any prior access, a panic will occur.
// unsafe impl<T: Component> SystemParamState for ChangedState<T> {
//     type Config = ();

//     fn init(world: &mut World, system_state: &mut SystemState, _config: Self::Config) -> Self {
//         let component_id = world.get_resource_id::<T>();
// 		let component_id = match component_id {
// 			Some(r) =>  r.clone(),
// 			None =>  panic!(
//                 "Res<{}> is not exist in system {}",
//                 std::any::type_name::<T>(), system_state.name),
// 		};
        
// 		let combined_access = system_state.component_access_set.combined_access_mut();
//         if combined_access.has_write(component_id) {
//             panic!(
//                 "Res<{}> in system {} conflicts with a previous ResMut<{0}> access. Allowing this would break Rust's mutability rules. Consider removing the duplicate access.",
//                 std::any::type_name::<T>(), system_state.name);
//         }
//         combined_access.add_read(component_id);

//         let archetype_component_id = world.archetypes.get_archetype_resource_id::<T>().unwrap();
//         system_state
//             .archetype_component_access
//             .add_read(*archetype_component_id);
//         Self {
//             component_id,
//             marker: PhantomData,
//         }
//     }

//     fn default_config() {}
// }

// impl<'a, T: Component> SystemParamFetch<'a> for ChangedState<T> {
//     type Item = Changed<T>;

//     #[inline]
//     unsafe fn get_param(
//         _state: &'a mut Self,
//         _system_state: &'a SystemState,
//         world: &'a World,
//         _change_tick: u32,
//     ) -> Self::Item {
// 		Changed {
// 			_world: world.clone(),
// 			mark: PhantomData,
//         }
//     }
// }
