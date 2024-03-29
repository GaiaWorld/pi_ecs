use std::any::type_name;

use crate::{
    archetype::ArchetypeComponentId,
    query::Access,
    sys::param::{interface::{SystemParamState, SystemParam}, SystemParamFetch},
    sys::system::interface::{System, SystemState, IntoSystem},
    world::World,
};
use derive_deref::{Deref, DerefMut};
use pi_share::{cell::TrustCell, ThreadSend, ThreadSync};
// use bevy_ecs_macros::all_tuples;
use std::{borrow::Cow, marker::PhantomData, sync::Arc};

use super::SystemId;

pub struct RunnerSystem<In, Out, Param, InMarker, F>
where
    Param: SystemParam,
{
    pub(crate) func: F,
    pub(crate) param_state: Param::Fetch,
    pub(crate) system_state: SystemState,
    // pub(crate) config: Option<<Param::Fetch as SystemParamState>::Config>,
	pub(crate) world: World,
	pub(crate) id: SystemId,
    // NOTE: PhantomData<fn()-> T> gives this safe Send/Sync impls
    pub(crate) mark: PhantomData<fn() -> (In, Out, InMarker)>,
}

// impl<In, Out, Param: SystemParam, InMarker, F> RunnerSystem<In, Out, Param, InMarker, F> {
//     /// Gives mutable access to the systems config via a callback. This is useful to set up system
//     /// [`Local`](crate::system::Local)s.
//     ///
//     /// # Examples
//     ///
//     /// ```
//     /// # use bevy_ecs::prelude::*;
//     /// # let world = &mut WorldInner::default();
//     /// fn local_is_42(local: Local<usize>) {
//     ///     assert_eq!(*local, 42);
//     /// }
//     /// let mut system = local_is_42.system().config(|config| config.0 = Some(42));
//     /// system.initialize(world);
//     /// system.run((), world);
//     /// ```
//     pub fn config(
//         mut self,
//         f: impl FnOnce(&mut <Param::Fetch as SystemParamState>::Config),
//     ) -> Self {
//         f(self.config.as_mut().unwrap());
//         self
//     }

// 	pub fn world(&self) -> &World {
// 		&self.world
// 	}
// }

// #[derive(Deref, DerefMut)]
// pub struct RunnerSystem<In, Out, Param: SystemParam, InMarker, R>(DefaultSystem<In, Out, Param, InMarker, R>);

// impl<In, Out, Param, InMarker, R> IntoSystem<Param, RunnerSystem<In, Out, Param, InMarker, ShareSystem<R>>> for ShareSystem<R>
// where
//     In: 'static + Send + Sync,
//     Out: 'static + Send + Sync,
//     Param: SystemParam + 'static,
//     InMarker: 'static,
//     R:  RunnerInner<In, Out, Param>,
// {
//     fn system(self, world: &mut World) -> RunnerSystem<In, Out, Param, InMarker, ShareSystem<R>> {
//         let id = SystemId::new(world.archetype_component_grow());
// 		let mut r = RunnerSystem {
//             func: self,
//             param_state: None,
// 			world: world.clone(),
//             config: Some(<Param::Fetch as SystemParamState>::default_config()),
//             system_state: SystemState::new::<R>(),
// 			id,
//             mark: PhantomData,
//         };
// 		r.initialize(world);
// 		r
// 		// RunnerSystem(r)
//     }
// }

impl<In, Out, Param, InMarker, R> IntoSystem<Param, RunnerSystem<In, Out, Param, InMarker, R>> for R
where
    In: ThreadSync + 'static,
    Out: ThreadSend + 'static,
    Param: SystemParam + 'static,
    InMarker: 'static,
    R:  RunnerInner<In, Out, Param>,
{
    fn system(self, world: &mut World) -> RunnerSystem<In, Out, Param, InMarker, R> {
        let id = SystemId::new(world.archetype_component_grow(type_name::<Self>(), false));
		let mut system_state =  SystemState::new::<R>();
		RunnerSystem {
            func: self,
			param_state: <Param::Fetch as SystemParamState>::init(
				world,
				&mut system_state,
				<Param::Fetch as SystemParamState>::default_config(),
			),
			world: world.clone(),
            // config: Some(<Param::Fetch as SystemParamState>::default_config()),
            system_state,
			id,
            mark: PhantomData,
        }
    }
}

impl<In, Out, Param, InMarker, R> System for RunnerSystem<In, Out, Param, InMarker, R>
where
	In: ThreadSync + 'static,
	Out: ThreadSend + 'static,
	Param: SystemParam + 'static,
	InMarker: 'static,
	R:  RunnerInner<In, Out, Param>,
{
    type In = In;
    type Out = Out;

    #[inline]
    fn name(&self) -> Cow<'static, str> {
        self.system_state.name.clone()
    }

    #[inline]
    fn id(&self) -> SystemId {
        self.id
    }

    #[inline]
    fn archetype_component_access(&self) -> &Access<ArchetypeComponentId> {
        &self.system_state.archetype_component_access.combined_access()
    }

    #[inline]
    fn is_send(&self) -> bool {
        self.system_state.is_send
    }

    #[inline]
    unsafe fn run_unsafe(&mut self, input: Self::In) -> Self::Out {
        // let change_tick = world.increment_change_tick();
		let change_tick = self.world.read_change_tick();

		let p = <<Param as SystemParam>::Fetch as SystemParamFetch>::get_param(&mut self.param_state, &self.system_state, &self.world, change_tick);
        let out = self.func.run(
            input,
            p
        );
        self.system_state.last_change_tick = change_tick;
        // self.system_state.last_change_tick = change_tick;
        out
    }

    #[inline]
    fn apply_buffers(&mut self) {
        let param_state = &mut self.param_state;
        param_state.apply(&mut self.world);
    }

    #[inline]
    fn check_change_tick(&mut self, _change_tick: u32) {
        // check_system_change_tick(
        //     &mut self.system_state.last_change_tick,
        //     change_tick,
        //     self.system_state.name.as_ref(),
        // );
    }
}


// impl<In, Out, Param, InMarker, R> System for RunnerSystem<In, Out, Param, InMarker, ShareSystem<R>>
// where
// 	In: 'static + Send + Sync,
// 	Out: 'static + Send + Sync,
// 	Param: SystemParam + 'static,
// 	InMarker: 'static,
// 	R:  RunnerInner<In, Out, Param>,
// {
//     type In = In;
//     type Out = Out;

//     #[inline]
//     fn name(&self) -> Cow<'static, str> {
//         self.system_state.name.clone()
//     }

//     #[inline]
//     fn id(&self) -> SystemId {
//         self.id
//     }

//     // #[inline]
//     // fn new_archetype(&mut self, archetype: &Archetype) {
//     //     let param_state = self.param_state.as_mut().unwrap();
//     //     param_state.new_archetype(archetype, &mut self.system_state);
//     // }

//     #[inline]
//     fn component_access(&self) -> &Access<ComponentId> {
//         &self.system_state.component_access_set.combined_access()
//     }

//     #[inline]
//     fn archetype_component_access(&self) -> &FilteredAccess<ArchetypeComponentId> {
//         &self.system_state.archetype_component_access
//     }

//     #[inline]
//     fn is_send(&self) -> bool {
//         self.system_state.is_send
//     }

//     #[inline]
//     unsafe fn run_unsafe(&mut self, input: Self::In) -> Self::Out {
//         // let change_tick = world.increment_change_tick();
// 		let change_tick = self.world.read_change_tick();

// 		let p = <<Param as SystemParam>::Fetch as SystemParamFetch>::get_param(self.param_state.as_mut().unwrap(), &self.system_state, &self.world, change_tick);
//         let out = self.func.0.borrow_mut().run(
//             input,
//             p
//         );
//         self.system_state.last_change_tick = change_tick;
//         // self.system_state.last_change_tick = change_tick;
//         out
//     }

//     #[inline]
//     fn apply_buffers(&mut self) {
//         let param_state = self.param_state.as_mut().unwrap();
//         param_state.apply(&mut self.world);
//     }

//     #[inline]
//     fn initialize(&mut self, world: &mut World) {
//         self.param_state = Some(<Param::Fetch as SystemParamState>::init(
//             world,
//             &mut self.system_state,
//             self.config.take().unwrap(),
//         ));
//     }

//     #[inline]
//     fn check_change_tick(&mut self, _change_tick: u32) {
//         // check_system_change_tick(
//         //     &mut self.system_state.last_change_tick,
//         //     change_tick,
//         //     self.system_state.name.as_ref(),
//         // );
//     }
// }

pub trait Runner: ThreadSync + 'static{
	type In: ThreadSync + 'static = ();
	type Out: ThreadSend + 'static = ();
	type Param: SystemParam = ();
	fn run(&mut self, input: Self::In, param: <<Self::Param as SystemParam>::Fetch as SystemParamFetch>::Item ) -> Self::Out;
}

impl<In, Out, Param, S> RunnerInner<In, Out, Param> for ShareSystem<S> where
	In: ThreadSync + 'static,
	Out: ThreadSend + 'static,
	Param: SystemParam,
	S: Runner<In = In, Out = Out, Param = Param> {

	fn run(&mut self, input: In, param: <<Param as SystemParam>::Fetch as SystemParamFetch>::Item ) -> Out {
		// let r: &mut S = ;
		Runner::run(&mut *(*self.0).borrow_mut(), input, param)
    }
}

pub trait RunnerInner<In, Out, Param>: ThreadSync + 'static
	where
		In: ThreadSync + 'static,
		Out: ThreadSend + 'static,
		Param: SystemParam {
	fn run(&mut self, input: In, param: <<Param as SystemParam>::Fetch as SystemParamFetch>::Item ) -> Out;
}

#[derive(Deref, DerefMut)]
pub struct ShareSystem<R>(Arc<TrustCell<R>>);

unsafe impl<R: Send> Send for ShareSystem<R> {}

impl<R> ShareSystem<R> {
	pub fn new(r: R) -> Self {
		ShareSystem(Arc::new(TrustCell::new(r)))
	}
}
impl<R> Clone for ShareSystem<R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}