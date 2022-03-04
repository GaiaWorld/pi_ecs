use std::any::TypeId;

use pi_listener::{Listener as LibListener, Listeners as LibListeners};
use pi_map::Map;
use pi_ecs_macros::all_tuples;
use pi_share::{Share, cell::TrustCell};
use std::{ops::Deref, sync::Arc, marker::PhantomData};
use crate::{
	world::World, 
	entity::Entity, 
	component::Component, 
	query::Access,
	sys::{system::{System, IntoSystem, SystemState, InputMarker, func_sys::{FunctionSystem, SystemParamFunction, SysInput}}, 
	param::{SystemParam, SystemParamFetch, SystemParamState}}, archetype::ArchetypeComponentId};


impl SysInput for Event {}
pub trait Listeners<P, ListenerType> {
	fn listeners(&self) -> ListenerType;
}

pub struct FunctionListeners<L: ListenInit, F, P> {
	f: F,
	mark: PhantomData<( P, L)>,
}

pub trait ListenSetup {
	fn setup(self, world: &mut World);
}

impl<L: ListenInit, P: SystemParam + 'static, F> ListenSetup for FunctionListeners<L, F, P>
where
    // F: System<In=Event, Out=()>,
	F: 
		IntoSystem<P, FunctionSystem<Event, (), P, InputMarker, F>> +
		SystemParamFunction<Event, (), P, InputMarker> + Send + Sync + 'static
{
	fn setup(self, world: &mut World) {
		let sys = self.f.system(world);

		let sys = TrustCell::new(sys);
		let listener = Listener(Arc::new(move |e: Event| {
			sys.borrow_mut().run(e);
		}));
		L::init(world, listener.clone());
	}
}

pub trait ListenInit: Send + Sync + 'static {
	fn init(world: &mut World, listener: Listener);
	fn add_access(world: &mut World, access: Access<ArchetypeComponentId>);
}

pub fn add_access(world: &mut World, access: Access<ArchetypeComponentId>, a_c_id: ArchetypeComponentId) {
	let arr = world.listener_access.get_mut(&a_c_id);
	let arr = match arr {
		Some(r) => r,
		None => {
			world.listener_access.insert(a_c_id, Vec::new());
			&mut world.listener_access[a_c_id]
		}
	};
	arr.push(access);
}
pub struct ComponentListen<A, C, T>(PhantomData<(A, C, T)>);
impl<A, C, T> ListenInit for ComponentListen<A, C, T> where 
	A: 'static + Send + Sync,
	C: Component,
	T: EventType{
	fn init(world: &mut World, listener: Listener) {
		world.add_component_listener::<T, A, C>(listener);
	}
	fn add_access(world: &mut World, access: Access<ArchetypeComponentId>) {
		let arch_id = world.archetypes().get_id_by_ident(TypeId::of::<A>())
.unwrap();		
		let c_id = world.components.get_id(TypeId::of::<C>()).unwrap();
		let a_c_id = unsafe{world.archetypes()[arch_id.clone()].archetype_component_id(c_id)};

		add_access(world, access, a_c_id);
	}
}

pub struct ResourceListen<R, T>(PhantomData<(R, T)>);
impl<R, T> ListenInit for ResourceListen<R, T> where 
	R: Component,
	T: EventType{
	fn init(world: &mut World, listener: Listener) {
		world.add_resource_listener::<T, R>(listener);
	}
	fn add_access(world: &mut World, access: Access<ArchetypeComponentId>) {
		let a_c_id = world.archetypes().get_archetype_resource_id::<R>().unwrap().clone();

		add_access(world, access, a_c_id);
	}
}

pub struct EntityListen<A, T>(PhantomData<(A, T)>);
impl<A, T> ListenInit for EntityListen<A, T> where 
	A: 'static + Send + Sync,
	T: EventType{
	fn init(world: &mut World, listener: Listener) {
		world.add_entity_listener::<T, A>(listener);
	}
	
	fn add_access(world: &mut World, access: Access<ArchetypeComponentId>) {
		let arch_id = world.archetypes().get_id_by_ident(TypeId::of::<A>())
.unwrap();		
		let a_c_id = world.archetypes()[arch_id.clone()].entity_archetype_component_id();

		add_access(world, access, a_c_id);
	}
}

pub struct Listen<T: ListenInit>(PhantomData<T>);
pub struct ListenState<T: ListenInit>(PhantomData<T>);

impl<T: ListenInit> SystemParam for Listen<T> {
    type Fetch = ListenState<T>;
}

// SAFE: only local state is accessed
unsafe impl<T: ListenInit> SystemParamState for ListenState<T> {
    type Config = ();

    fn init(_world: &mut World, _system_state: &mut SystemState, _config: Self::Config) -> Self {
        ListenState(PhantomData)
    }

    fn default_config() -> () {
        ()
    }
}

impl<'a, T: ListenInit> SystemParamFetch<'a> for ListenState<T> {
    type Item = Listen<T>;

    #[inline]
    unsafe fn get_param(
        _state: &'a mut Self,
        _system_state: &'a SystemState,
        _world: &'a World,
        _change_tick: u32,
    ) -> Self::Item {
        Listen(PhantomData)
    }
}

pub trait EventType: Send + Sync + 'static {
	fn add(notify: &dyn Notify, listener: Listener);
}

pub struct Create;

impl EventType for Create {
	fn add(notify: &dyn Notify, listener: Listener) {
		notify.add_create(listener);
	}
}

pub struct Delete;

impl EventType for Delete {
	fn add(notify: &dyn Notify, listener: Listener) {
		notify.add_delete(listener);
	}
}

pub struct Modify;

impl EventType for Modify {
	fn add(notify: &dyn Notify, listener: Listener) {
		notify.add_modify(listener);
	}
}



#[derive(Clone)]
pub struct Event {
    pub id: Entity,
    pub field: &'static str,
    pub index: usize, // 一般无意义。 只有在数组或向量的元素被修改时，才有意义
}

#[derive(Clone)]
pub struct Listener(pub(crate) Arc<dyn Fn(Event)>);
 
unsafe impl Send for Listener {}
unsafe impl Sync for Listener {}
pub type ListenerList = LibListeners<Listener>;

impl LibListener<Event> for Listener {
	fn listen(&self, e: &Event) {
		let r = unsafe{&mut *(self as *const Listener as usize as *mut Listener)};
		r.0(e.clone());
	}
}

#[derive(Default, Clone)]
pub struct NotifyImpl(pub Share<NotifyImpl1>);

impl Notify for NotifyImpl {
	#[inline]
    fn add_create(&self, listener: Listener ) {
        unsafe { &mut *(self.0.as_ref() as *const NotifyImpl1 as *mut NotifyImpl1) }
            .create
            .push(listener)
    }
	#[inline]
    fn add_delete(&self, listener: Listener) {
        unsafe { &mut *(self.0.as_ref() as *const NotifyImpl1 as *mut NotifyImpl1) }
            .delete
            .push(listener)
    }
	#[inline]
    fn add_modify(&self, listener: Listener) {
        unsafe { &mut *(self.0.as_ref() as *const NotifyImpl1 as *mut NotifyImpl1) }
            .modify
            .push(listener)
    }

    fn remove_create(&self, _listener: &Listener) {
        // unsafe { &mut *(self.0.as_ref() as *const NotifyImpl1 as *mut NotifyImpl1) }
        //     .create
        //     .delete(listener);
    }
    fn remove_delete(&self, _listener: &Listener) {
        // unsafe { &mut *(self.0.as_ref() as *const NotifyImpl1 as *mut NotifyImpl1) }
        //     .delete
        //     .delete(listener);
    }
    fn remove_modify(&self, _listener: &Listener) {
        // unsafe { &mut *(self.0.as_ref() as *const NotifyImpl1 as *mut NotifyImpl1) }
        //     .modify
        //     .delete(listener);
    }

	fn create_event(&self, id: Entity) {
        let e = Event { id, field: "", index:0 };
        self.create.listen(&e);
    }
    fn delete_event(&self, id: Entity) {
        let e = Event { id, field: "", index:0  };
        self.delete.listen(&e);
    }
    fn modify_event(&self, id: Entity, field: &'static str, index: usize) {
        let e = Event {
            id,
            field,
            index,
        };
        self.modify.listen(&e);
    }
}

impl Deref for NotifyImpl {
    type Target = Share<NotifyImpl1>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Default)]
pub struct NotifyImpl1 {
    pub create: ListenerList,
    pub delete: ListenerList,
    pub modify: ListenerList,
}

impl NotifyImpl1 {
    pub fn mem_size(&self) -> usize {
        // self.create.mem_size() + self.delete.mem_size() + self.modify.mem_size()
		0
    }
}

pub trait Notify {
    fn add_create(&self, f: Listener);
    fn add_delete(&self, f: Listener);
    fn add_modify(&self, f: Listener);
	fn remove_create(&self, f: &Listener);
    fn remove_delete(&self, f: &Listener);
    fn remove_modify(&self, f: &Listener);
    fn create_event(&self, id: Entity);
    fn delete_event(&self, id: Entity);
    fn modify_event(&self, id: Entity, field: &'static str, index: usize);
}

/// 为元素满足EventType的元组，实现EventType（最多三个）
macro_rules! impl_event_function {
    ($($param: ident),*) => {
        #[allow(non_snake_case)]
		impl<$($param: EventType),*> EventType for ($($param,)*) {
			fn add(notify: &dyn Notify, listener: Listener) {
				$($param::add(notify, listener.clone());)*
			}
		}
    };
}

/// 为元素满足ListenInit的元组，实现ListenInit（最多三个）
macro_rules! impl_event_init {
    ($($param: ident),*) => {
        #[allow(non_snake_case)]
		impl<$($param: ListenInit),*> ListenInit for ($($param,)*) {
			fn init(world: &mut World, listener: Listener) {
				$($param::init(world, listener.clone());)*
			}

			fn add_access(world: &mut World, access: Access<ArchetypeComponentId>) {
				$($param::add_access(world, access.clone());)*
			}
		}
    };
}

// impl<L: ListenInit, P: SystemParam + 'static, S> Listeners<P, FunctionListeners<L, S>> for S 
// 	where S: 
// 		IntoSystem<P, FunctionSystem<Event, (), P, InputMarker, S>> +
// 		SystemParamFunction<Event, (), P, InputMarker> + Send + Sync + 'static + 
// 		FnMut(Event, Listen<L>, ) -> () +
// 		Clone,
// 		{
// 	fn listeners(&self) -> FunctionListeners<L, S> {
// 		FunctionListeners{f: self.clone(), mark: PhantomData}
// 		// let sys = self.clone().system(world);
// 		// let sys = TrustCell::new(sys);
// 		// let listener = Listener(Arc::new(move |e: Event| {
// 		// 	sys.borrow_mut().run(e);
// 		// }));
// 		// L::init(world, listener.clone());
// 	}
// }

// #[allow(non_snake_case)]
// impl<L: ListenInit, S, P0: SystemParam + 'static>
//     Listeners<(Listen<L>, P0), FunctionListeners<L, S>> for S
// where
//     S: IntoSystem<(Listen<L>, P0), FunctionSystem<Event, (), (Listen<L>, P0), InputMarker, S>>
//         + SystemParamFunction<Event, (), (Listen<L>, P0), InputMarker>
//         + Send
//         + Sync
//         + 'static
//         + FnMut(Event, Listen<L>, P0) -> ()
//         + Clone,
// {
//     fn listeners(&self) -> FunctionListeners<L, S> {
//         FunctionListeners {
//             f: self.clone(),
//             mark: PhantomData,
//         }
//     }
// }

/// 为满足条件的函数，实现ListenSetup
macro_rules! impl_event_setup {
    ($($param: ident),*) => {
		#[allow(non_snake_case)]
		impl<L: ListenInit, S, $($param: SystemParam + 'static),*> Listeners<(Listen<L>, $($param,)*), FunctionListeners<L, S, (Listen<L>, $($param,)*)>> for S 
			where S: 
				IntoSystem<(Listen<L>, $($param,)*), FunctionSystem<Event, (), (Listen<L>, $($param,)*), InputMarker, S>> +
				SystemParamFunction<Event, (), (Listen<L>, $($param,)*), InputMarker> + Send + Sync + 'static + 
				FnMut(Event, Listen<L>, $($param,)*) -> () +
				Clone,
				{
			fn listeners(&self) -> FunctionListeners<L, S, (Listen<L>, $($param,)*)> {
				FunctionListeners{f: self.clone(), mark: PhantomData}
				// let sys = self.clone().system(world);
				// let sys = TrustCell::new(sys);
				// let listener = Listener(Arc::new(move |e: Event| {
				// 	sys.borrow_mut().run(e);
				// }));
				// L::init(world, listener.clone());
			}
		}
    };
}

all_tuples!(impl_event_function, 1, 3, T);
all_tuples!(impl_event_init, 1, 3, L);
all_tuples!(impl_event_setup, 0, 15, P);

