// use crate::{
//     archetype::ArchetypeComponentId,
//     component::ComponentId,
//     query::{Access, FilteredAccessSet},
//     sys::system::{SystemId, System},
// };
// use std::borrow::Cow;

// /// 系统状态
// pub struct SystemState {
//     pub(crate) id: SystemId, // 系统id
//     pub(crate) name: Cow<'static, str>, // 系统名称
//     pub(crate) component_access_set: FilteredAccessSet<ComponentId>,
//     pub(crate) archetype_component_access: Access<ArchetypeComponentId>,
//     // NOTE: this must be kept private. making a SystemState non-send is irreversible to prevent
//     // SystemParams from overriding each other
//     pub(crate) is_send: bool,
//     pub(crate) last_change_tick: u32,
// }

// impl SystemState {
//     pub(crate) fn new<T>() -> Self {
//         Self {
//             name: std::any::type_name::<T>().into(),
//             archetype_component_access: Access::default(),
//             component_access_set: FilteredAccessSet::default(),
//             is_send: true,
//             id: SystemId::new(),
//             last_change_tick: 0,
//         }
//     }

//     /// Returns true if the system is [`Send`].
//     #[inline]
//     pub fn is_send(&self) -> bool {
//         self.is_send
//     }

//     /// Sets the system to be not [`Send`].
//     ///
//     /// This is irreversible.
//     #[inline]
//     pub fn set_non_send(&mut self) {
//         self.is_send = false;
//     }
// }

// /// Conversion trait to turn something into a [`System`].
// ///
// /// Use this to get a system from a function. Also note that every system implements this trait as well.
// ///
// /// # Examples
// ///
// /// ```
// /// use bevy_ecs::system::IntoSystem;
// /// use bevy_ecs::system::Res;
// ///
// /// fn my_system_function(an_usize_resource: Res<usize>) {}
// ///
// /// let system = my_system_function.system();
// /// ```
// pub trait IntoSystem<Params, SystemType: System> {
//     /// Turns this value into its corresponding [`System`].
//     fn system(self) -> SystemType;
// }

// // Systems implicitly implement IntoSystem
// impl<Sys: System> IntoSystem<(), Sys> for Sys {
//     fn system(self) -> Sys {
//         self
//     }
// }

// /// Wrapper type to mark a [`SystemParam`] as an input.
// ///
// /// [`System`]s may take an optional input which they require to be passed to them when they
// /// are being [`run`](System::run). For [`FunctionSystems`](FunctionSystem) the input may be marked
// /// with this `In` type, but only the first param of a function may be tagged as an input. This also
// /// means a system can only have one or zero input paramaters.
// ///
// /// # Examples
// ///
// /// Here is a simple example of a system that takes a [`usize`] returning the square of it.
// ///
// /// ```
// /// use bevy_ecs::prelude::*;
// ///
// /// fn main() {
// ///     let mut square_system = square.system();
// ///
// ///     let mut world = World::default();
// ///     square_system.initialize(&mut world);
// ///     assert_eq!(square_system.run(12, &mut world), 144);
// /// }
// ///
// /// fn square(In(input): In<usize>) -> usize {
// ///     input * input
// /// }
// /// ```
// pub struct In<In>(pub In);
// pub struct InputMarker;



// // async fn xx (a: u32){

// // }

// // fn aa() {
// // 	let r = xx(0);
// // }


// // asyn fn xx1 (a: u32) {

// // }
