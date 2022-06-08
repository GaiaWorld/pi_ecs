//! 指令，用于创建、删除实体和组件，创建和删除行为以指令的形式，缓存在指令状态中，必须通过apply方法才能应用到World中
use std::marker::PhantomData;

use pi_slotmap::Key;

use crate::{component::Component, archetype::ArchetypeIdent, entity::{Entities, Id}, world::World, storage::Local, sys::system::SystemState};

use super::{SystemParam, SystemParamState, SystemParamFetch};

/// 实体指令，用于创建和删除实体
pub struct EntityCommands<A: 'static + Send + Sync> {
	delete: &'static mut CommandQueue<EntityDelete<A>>,
	entities: &'static Entities,
	_world: World,
	mark: PhantomData<A>,
}

/// 组件指令，用于创建和删除指令
pub struct Commands<A: ArchetypeIdent, T: Component> {
	_world: World,
	queues: &'static mut CommandQueues<A, T>,
}

pub struct CommandQueues<A: ArchetypeIdent, T: Component> {
	create: CommandQueue<ComponentInsert<A, T>>,
	delete: CommandQueue<ComponentDelete<A, T>>,
}

impl<A: ArchetypeIdent, T: Component> CommandQueues<A, T> {
	fn new(world: &mut World, component_id: Local) -> Self {	
		let arch_id = world.archetypes_mut().get_or_create_archetype::<A>();
		Self{
			create: CommandQueue::new(world, arch_id, component_id),
			delete:  CommandQueue::new(world, arch_id, component_id)
		}
	}
	fn push_create(&mut self, entity: Id<A>, value: T) {
		self.create.push(ComponentInsert(entity, value));
	}

	fn push_delete(&mut self, entity: Id<A>) {
		self.delete.push(ComponentDelete(entity, PhantomData));
	}

	pub fn apply(&mut self, world: &mut World) {
		self.create.apply(world);
		self.delete.apply(world);
	}
}

impl<A: ArchetypeIdent> EntityCommands<A> {
	pub fn new(queue: &'static mut CommandQueue<EntityDelete<A>>, world: &World) -> Self {
		let arch_id = queue.arch_id;
        Self {
			_world: world.clone(),
            delete: queue,
            entities: unsafe{ std::mem::transmute(world.entities(arch_id))},
			mark: PhantomData,
        }
    }

	pub fn spawn(&mut self) -> Id<A> {
		let local = self.entities.reserve_entity();
		unsafe { Id::<A>::new(local) }
    }

	pub fn despawn(&mut self, entity: Id<A>) {
		self.delete.push(EntityDelete(entity));
	}
}

impl<A: ArchetypeIdent, T: Component> Commands<A, T> {
	/// Create a new `Commands` from a queue and a world.
    pub fn new(
		queues: &'static mut CommandQueues<A, T>, 
		world: &World
	) -> Self {
        Self {
			_world: world.clone(),
            queues,
        }
    }
    pub fn insert(&mut self, entity: Id<A>, component: T) {
		self.queues.push_create(entity, component);
    }

	pub fn delete(&mut self, entity: Id<A>) {
		self.queues.push_delete(entity);
    }

}

pub struct CommandQueue<T: 'static + Send + Sync + Command> {
	list: Vec<T>,
	ty_id: Local,
	arch_id: Local,
}

impl<T: 'static + Send + Sync + Command> CommandQueue<T> {
	pub fn new(_world: &World, arch_id: Local, component_id: Local) -> Self {
		Self {
			list: Vec::new(),
			ty_id: component_id,
			arch_id,
		}
	}
	#[inline]
	fn push(&mut self, value: T) {
		self.list.push(value);
	}

	pub fn apply(&mut self, world: &mut World) {
		for item in self.list.drain(..) {
			item.write(world, self.arch_id, self.ty_id)
		}
	}
}

/***********************Command***************************/
pub trait Command: Send + Sync + 'static {
    fn write(self, world: &mut World, arch_id: Local, type_id: Local);
}
pub struct EntityDelete<A: ArchetypeIdent>(pub(crate) Id<A>);

impl<A: ArchetypeIdent> Command for EntityDelete<A> {
	fn write(self, world: &mut World, arch_id: Local, _type_id: Local) {
		world.archetypes_mut()[arch_id].remove_entity(self.0.0);
	}
}

pub struct ComponentInsert<A: ArchetypeIdent, T>(pub(crate)Id<A>, pub(crate)T);

impl<A: ArchetypeIdent, T: Component> Command for ComponentInsert<A, T> {
	fn write(self, world: &mut World, arch_id: Local, type_id: Local) {
		let tick = world.change_tick();
		world.archetypes[arch_id].insert_component(self.0.0, self.1, type_id, tick);
	}
}

pub struct ComponentDelete<A: ArchetypeIdent, T>(pub(crate)Id<A>, PhantomData<T>);

impl<A: ArchetypeIdent, T: Component> Command for ComponentDelete<A, T> {
	fn write(self, world: &mut World, arch_id: Local, type_id: Local) {
		world.archetypes[arch_id].remove_component(self.0.0, type_id);
	}
}

/**********************SystemParam Commands***************************/


impl<A: ArchetypeIdent, T: Component> SystemParam for Commands<A, T> {
    type Fetch = CommandQueues<A, T>;
}

// SAFE: only local state is accessed
unsafe impl<A: ArchetypeIdent, T: Component> SystemParamState for CommandQueues<A, T> {
    type Config = Option<T>;

    fn init(world:  &mut World, _system_state: &mut SystemState, _config: Self::Config) -> Self {
		let component_id = world.components.get_or_insert_id::<T>();
		CommandQueues::new(world, component_id)
    }

    fn default_config() -> Option<T> {
        None
    }

	fn apply(&mut self, world: &mut World) {
		self.apply(world);
	}
}

impl<'w, 's, A: ArchetypeIdent, T: Component> SystemParamFetch<'w, 's> for CommandQueues<A, T> {
    type Item = Commands<A, T>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        _system_state: & SystemState,
        world: &'w World,
        _last_change_tick: u32,
    ) -> Self::Item {
		Commands::new(std::mem::transmute(state), world)
    }
}

/**********************SystemParam EntityCommands***************************/

impl<A: ArchetypeIdent> SystemParam for EntityCommands<A> {
    type Fetch = CommandQueue<EntityDelete<A>>;
}

// SAFE: only local state is accessed
unsafe impl<A: ArchetypeIdent> SystemParamState for CommandQueue<EntityDelete<A>> {
    type Config = Option<CommandQueue<EntityDelete<A>>>;

    fn init(world:  &mut World, _system_state: &mut SystemState, _config: Self::Config) -> Self {
		let arch_id = world.archetypes_mut().get_or_create_archetype::<A>();
		CommandQueue::new(world, arch_id, Local::null())
    }

    fn default_config() -> Option<CommandQueue<EntityDelete<A>>> {
        None
    }

	fn apply(&mut self, world: &mut World) {
		world.archetypes[self.arch_id].entities.flush();
		self.apply(world);
	}
}

impl<'w, 's, A: ArchetypeIdent> SystemParamFetch<'w, 's> for CommandQueue<EntityDelete<A>> {
    type Item = EntityCommands<A>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        _system_state: & SystemState,
        world: &'w World,
        _last_change_tick: u32,
    ) -> Self::Item {
		EntityCommands::new(std::mem::transmute(state), world)
    }
}