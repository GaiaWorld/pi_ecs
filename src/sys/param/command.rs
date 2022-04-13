use std::{marker::PhantomData, any::TypeId};

use crate::{component::Component, entity::{Entities, Entity}, world::World, storage::Local, sys::system::SystemState};

use super::{SystemParam, SystemParamState, SystemParamFetch};

pub struct EntityCommands<T: 'static + Send + Sync> {
	delete: &'static mut CommandQueue<EntityDelete<T>>,
	entities: &'static Entities,
	arch_id: Local,
	_world: World,
	mark: PhantomData<T>,
}

pub struct Commands<T: Component> {
	_world: World,
	queues: &'static mut CommandQueues<T>,
}

pub struct CommandQueues<T: Component> {
	create: CommandQueue<ComponentInsert<T>>,
	delete: CommandQueue<ComponentDelete<T>>,
}

impl<T: Component> CommandQueues<T> {
	fn new(world: &mut World, component_id: Local) -> Self {	
		Self{
			create: CommandQueue::new(world, component_id),
			delete:  CommandQueue::new(world, component_id)
		}
	}
	fn push_create(&mut self, entity: Entity, value: T) {
		self.create.push(ComponentInsert(entity, value));
	}

	fn push_delete(&mut self, entity: Entity) {
		self.delete.push(ComponentDelete(entity, PhantomData));
	}

	pub fn apply(&mut self, world: &mut World) {
		self.create.apply(world);
		self.delete.apply(world);
	}
}

impl<T: 'static + Send + Sync> EntityCommands<T> {
	pub fn new(queue: &'static mut CommandQueue<EntityDelete<T>>, world: &World) -> Self {
		let arch_id = match world.archetypes.get_id_by_ident(TypeId::of::<T>()) {
			Some(r) => *r,
			None => panic!("fetch entity fail，entity is not exist, in EntityCommands<{}>", std::any::type_name::<T>())
		};
        Self {
			_world: world.clone(),
            delete: queue,
			arch_id,
            entities: unsafe{ std::mem::transmute(world.entities(arch_id))},
			mark: PhantomData,
        }
    }

	pub fn spawn(&mut self) -> Entity {
		let local = self.entities.reserve_entity();
		Entity::new(self.arch_id, local)
    }

	pub fn despawn(&mut self, entity: Entity) {
		self.delete.push(EntityDelete(entity, PhantomData));
	}
}

impl<T: Component> Commands<T> {
	/// Create a new `Commands` from a queue and a world.
    pub fn new(
		queues: &'static mut CommandQueues<T>, 
		world: &World
	) -> Self {
        Self {
			_world: world.clone(),
            queues,
			// component_type_id: world.components.get_id(TypeId::of::<T>()).unwrap(),
        }
    }
    pub fn insert(&mut self, entity: Entity, component: T) {
		self.queues.push_create(entity, component);
    }

	pub fn delete(&mut self, entity: Entity) {
		self.queues.push_delete(entity);
    }

}

pub struct CommandQueue<T: 'static + Send + Sync + Command> {
	list: Vec<T>,
	ty_id: Local,
}

impl<T: 'static + Send + Sync + Command> CommandQueue<T> {
	pub fn new(_world: &World, component_id: Local) -> Self {
		Self {
			list: Vec::new(),
			ty_id: component_id,
		}
	}
	#[inline]
	fn push(&mut self, value: T) {
		self.list.push(value);
	}

	pub fn apply(&mut self, world: &mut World) {
		for item in self.list.drain(..) {
			item.write(world, self.ty_id)
		}
	}
}

/***********************Command***************************/
pub trait Command: Send + Sync + 'static {
    fn write(self, world: &mut World, type_id: Local);
}
pub struct EntityDelete<T>(pub(crate) Entity, PhantomData<T>);

impl<T: 'static + Send + Sync> Command for EntityDelete<T> {
	fn write(self, world: &mut World, _type_id: Local) {
		world.despawn(self.0);
	}
}

pub struct ComponentInsert<T>(pub(crate)Entity, pub(crate)T);

impl<T: Component> Command for ComponentInsert<T> {
	fn write(self, world: &mut World, type_id: Local) {
		let tick = world.change_tick();
		world.archetypes[self.0.archetype_id()].insert_component(self.0.local(), self.1, type_id, tick);
	}
}

pub struct ComponentDelete<T>(pub(crate)Entity, PhantomData<T>);

impl<T: Component> Command for ComponentDelete<T> {
	fn write(self, world: &mut World, type_id: Local) {
		world.archetypes[self.0.archetype_id()].remove_component(self.0.local(), type_id);
	}
}

/**********************SystemParam Commands***************************/


impl<T: Component> SystemParam for Commands<T> {
    type Fetch = CommandQueues<T>;
}

// SAFE: only local state is accessed
unsafe impl<T: Component> SystemParamState for CommandQueues<T> {
    type Config = Option<T>;

    fn init(world:  &mut World, _system_state: &mut SystemState, _config: Self::Config) -> Self {
		let component_id = match world.components.get_id(TypeId::of::<T>()) {
			Some(r) => r, 
			None => panic!("fetch component fail, component is not exist, in Commands<{}>", std::any::type_name::<T>()),
		};
		CommandQueues::new(world, component_id)
    }

    fn default_config() -> Option<T> {
        None
    }

	fn apply(&mut self, world: &mut World) {
		self.apply(world);
	}
}

impl<'w, 's, T: Component> SystemParamFetch<'w, 's> for CommandQueues<T> {
    type Item = Commands<T>;

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

impl<T: 'static + Send + Sync> SystemParam for EntityCommands<T> {
    type Fetch = CommandQueue<EntityDelete<T>>;
}

// SAFE: only local state is accessed
unsafe impl<T: 'static + Send + Sync> SystemParamState for CommandQueue<EntityDelete<T>> {
    type Config = Option<CommandQueue<EntityDelete<T>>>;

    fn init(world:  &mut World, _system_state: &mut SystemState, _config: Self::Config) -> Self {
		let arch_id = match world.archetypes().get_id_by_ident(TypeId::of::<T>()) {
			Some(r) => r,
			None => panic!("fetch archetype fail, {} id not exist, in EntityCommands<{}>", std::any::type_name::<T>(), std::any::type_name::<T>()),
		};
		CommandQueue::new(world, *arch_id)
    }

    fn default_config() -> Option<CommandQueue<EntityDelete<T>>> {
        None
    }

	fn apply(&mut self, world: &mut World) {
		world.archetypes[self.ty_id].entities.flush();
		self.apply(world);
	}
}

impl<'w, 's, T: 'static + Send + Sync> SystemParamFetch<'w, 's> for CommandQueue<EntityDelete<T>> {
    type Item = EntityCommands<T>;

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