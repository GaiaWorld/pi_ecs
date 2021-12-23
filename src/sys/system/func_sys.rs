use std::sync::Arc;

use share::cell::TrustCell;

use crate::{
    archetype::ArchetypeComponentId,
    component::ComponentId,
    query::{Access},
    sys::param::interface::{SystemParamState, SystemParam, SystemParamFetch},
    sys::system::interface::{System, SystemState, IntoSystem, In, InputMarker},
    world::World,
};
use pi_ecs_macros::all_tuples;
// use bevy_ecs_macros::all_tuples;
use std::{borrow::Cow, marker::PhantomData};

use super::SystemId;

/// The [`System`] counter part of an ordinary function.
///
/// You get this by calling [`IntoSystem::system`]  on a function that only accepts [`SystemParam`]s.
/// The output of the system becomes the functions return type, while the input becomes the functions
/// [`In`] tagged parameter or `()` if no such paramater exists.
pub struct FunctionSystem<In, Out, Param, InMarker, F>
where
    Param: SystemParam,
{
    func: F,
    param_state: Option<Param::Fetch>,
    system_state: SystemState,
    config: Option<<Param::Fetch as SystemParamState>::Config>,
	world: Arc<TrustCell<World>>,
	id: SystemId,
    // NOTE: PhantomData<fn()-> T> gives this safe Send/Sync impls
    mark: PhantomData<fn() -> (In, Out, InMarker)>,
}

impl<In, Out, Param: SystemParam, InMarker, F> FunctionSystem<In, Out, Param, InMarker, F> {
    /// Gives mutable access to the systems config via a callback. This is useful to set up system
    /// [`Local`](crate::system::Local)s.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// # let world = &mut World::default();
    /// fn local_is_42(local: Local<usize>) {
    ///     assert_eq!(*local, 42);
    /// }
    /// let mut system = local_is_42.system().config(|config| config.0 = Some(42));
    /// system.initialize(world);
    /// system.run((), world);
    /// ```
    pub fn config(
        mut self,
        f: impl FnOnce(&mut <Param::Fetch as SystemParamState>::Config),
    ) -> Self {
        f(self.config.as_mut().unwrap());
        self
    }
}

pub struct SyncMarker;

impl<In, Out, Param, InMarker, F> IntoSystem<Param, FunctionSystem<In, Out, Param, InMarker, F>> for F
where
    In: 'static,
    Out: 'static,
    Param: SystemParam + 'static,
    InMarker: 'static,
    F: SystemParamFunction<In, Out, Param, InMarker> + Send + Sync + 'static,
{
    fn system(self, world: &mut Arc<TrustCell<World>>) -> FunctionSystem<In, Out, Param, InMarker, F> {
        let id = SystemId::new(world.borrow_mut().archetype_component_grow());
		let mut r = FunctionSystem {
            func: self,
            param_state: None,
			world: world.clone(),
            config: Some(<Param::Fetch as SystemParamState>::default_config()),
            system_state: SystemState::new::<F>(),
			id,
            mark: PhantomData,
        };
		r.initialize(&mut world.borrow_mut());
		r
    }
}



impl<In, Out, Param, InMarker, F> System for FunctionSystem<In, Out, Param, InMarker, F>
where
    In: 'static,
    Out: 'static ,
    Param: SystemParam + 'static,
    InMarker: 'static,
    F: SystemParamFunction<In, Out, Param, InMarker> + Send + Sync + 'static,
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

    // #[inline]
    // fn new_archetype(&mut self, archetype: &Archetype) {
    //     let param_state = self.param_state.as_mut().unwrap();
    //     param_state.new_archetype(archetype, &mut self.system_state);
    // }

    #[inline]
    fn component_access(&self) -> &Access<ComponentId> {
        &self.system_state.component_access_set.combined_access()
    }

    #[inline]
    fn archetype_component_access(&self) -> &Access<ArchetypeComponentId> {
        &self.system_state.archetype_component_access
    }

    #[inline]
    fn is_send(&self) -> bool {
        self.system_state.is_send
    }

    #[inline]
    unsafe fn run_unsafe(&mut self, input: Self::In) -> Self::Out {
        // let change_tick = world.increment_change_tick();
        let out = self.func.run(
            input,
            self.param_state.as_mut().unwrap(),
            &self.system_state,
            &self.world,
            0,
        );
        self.system_state.last_change_tick = 0;
        // self.system_state.last_change_tick = change_tick;
        out
    }

    #[inline]
    fn apply_buffers(&mut self, world: &mut World) {
        let param_state = self.param_state.as_mut().unwrap();
        param_state.apply(world);
    }

    #[inline]
    fn initialize(&mut self, world: &mut World) {
        self.param_state = Some(<Param::Fetch as SystemParamState>::init(
            world,
            &mut self.system_state,
            self.config.take().unwrap(),
        ));
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

/// A trait implemented for all functions that can be used as [`System`]s.
pub trait SystemParamFunction<In, Out, Param: SystemParam, InMarker>: Send + Sync + 'static {
    fn run(
        &mut self,
        input: In,
        state: &mut Param::Fetch,
        system_state: &SystemState,
        world: &Arc<TrustCell<World>>,
        change_tick: u32,
    ) -> Out;
}

macro_rules! impl_system_function {
    ($($param: ident),*) => {
        #[allow(non_snake_case)]
        impl<Out, Func, $($param: SystemParam),*> SystemParamFunction<(), Out, ($($param,)*), ()> for Func
        where
            Func:
                FnMut($($param),*) -> Out +
                FnMut($(<<$param as SystemParam>::Fetch as SystemParamFetch>::Item),*) -> Out + Send + Sync + 'static, Out: 'static
        {
            #[inline]
            fn run(&mut self, _input: (), state: &mut <($($param,)*) as SystemParam>::Fetch, system_state: &SystemState, world: &Arc<TrustCell<World>>, change_tick: u32) -> Out {
                unsafe {
                    let ($($param,)*) = <<($($param,)*) as SystemParam>::Fetch as SystemParamFetch>::get_param(state, system_state, world, change_tick);
                    self($($param),*)
                }
            }
        }

        #[allow(non_snake_case)]
        impl<Input, Out, Func, $($param: SystemParam),*> SystemParamFunction<Input, Out, ($($param,)*), InputMarker> for Func
        where
            Func:
                FnMut(In<Input>, $($param),*) -> Out +
                FnMut(In<Input>, $(<<$param as SystemParam>::Fetch as SystemParamFetch>::Item),*) -> Out + Send + Sync + 'static, Out: 'static
        {
            #[inline]
            fn run(&mut self, input: Input, state: &mut <($($param,)*) as SystemParam>::Fetch, system_state: &SystemState, world: &Arc<TrustCell<World>>, change_tick: u32) -> Out {
                unsafe {
                    let ($($param,)*) = <<($($param,)*) as SystemParam>::Fetch as SystemParamFetch>::get_param(state, system_state, world, change_tick);
                    self(In(input), $($param),*)
                }
            }
        }
    };
}

all_tuples!(impl_system_function, 0, 16, F);