use std::any::TypeId;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use map::Map;
use share::cell::TrustCell;

use crate::archetype::{Archetype, Archetypes, ArchetypeId, ArchetypeIdent, ArchetypeComponentId};
use crate::component::{Components, ComponentId, Component};
use crate::entity::{Entity, Entities};
use crate::monitor::{EventType, Listener};
use crate::query::{WorldQuery, QueryState};
use crate::storage::{LocalVersion, Local, SecondaryMap};
use crate::sys::param::res::ResState;
use crate::query::Access;



/// 世界
#[derive(Clone)]
pub struct World {
	pub(crate) inner: Arc<TrustCell<WorldInner>>,
}

impl World {
	/// 构造方法
	pub fn new() -> Self {
		Self{
			inner: Arc::new(TrustCell::new( WorldInner::new()))
		}
	}

	/// 查询
	pub fn query<A: ArchetypeIdent, Q: WorldQuery>(&mut self) -> QueryState<A, Q, ()> {
        QueryState::new(self)
    }

	pub fn res<T: Component>(&self) -> ResState<T> {
		let component_id = self.get_resource_id::<T>();
		let component_id = match component_id {
			Some(r) =>  r.clone(),
			None =>  panic!(
                "Res<{}> is not exist in res_mut",
                std::any::type_name::<T>()),
		};
		ResState {
			component_id,
			marker: PhantomData
		}
    }

}


/// 世界
pub struct WorldInner {
	pub(crate) id:WorldId,
	pub(crate) components: Components,
	pub(crate) archetypes: Archetypes,

	/// 该字段描述了监听器监听的组件做访问的数据id
	pub(crate) listener_access: SecondaryMap<ArchetypeComponentId, Vec<Access<ArchetypeComponentId>>>,

	pub(crate) change_tick: AtomicU32,
	pub(crate) last_change_tick: u32,
}

impl WorldInner {
	pub fn new() -> Self {
		Self {
			id: WorldId(0),
			components: Components::new(),
			archetypes: Archetypes::new(),
			listener_access: SecondaryMap::with_capacity(0),
			change_tick: AtomicU32::new(1),
			last_change_tick: 0,
		}
	}

	/// 插入资源
	pub fn insert_resource<T: Component>(&mut self, value: T) -> ComponentId {
		let id = self.components.get_or_insert_resource_id::<T>();
		self.archetypes.insert_resource::<T>(value, id);
		id
	}

	/// 取到资源id
	pub fn get_resource_id<T: Component>(&self) -> Option<&ComponentId> {
		self.components.get_resource_id::<T>()
	}

	pub fn archetype_component_grow(&mut self) -> usize {
		self.archetypes.archetype_component_grow()
	}

	/// 创建原型
	pub fn new_archetype<T: Send + Sync + 'static>(&mut self) -> ArchetypeInfo {
		ArchetypeInfo {
			archetype: self.archetypes.create_archetype_by_ident(TypeId::of::<T>()),
			world: self,
			type_id: TypeId::of::<T>(),
			components: HashSet::default(),
		}
	}

	/// 创建实体
	pub fn spawn<T: Send + Sync + 'static>(&mut self) -> EntityRef {
		let archetype_id = match self.archetypes.get_id_by_ident(TypeId::of::<T>()) {
			Some(r) => r.clone(),
			None => {
				panic!("spawn fial")
			}
		};

		let change_tick = self.read_change_tick();
		let(archetypes, components) = (&mut self.archetypes, &mut self.components);
		let e = archetypes.spawn::<T>(archetype_id);
		EntityRef {
			local: e.local(),
			archetype_id: archetype_id,
			archetype: archetypes.get_mut(archetype_id).unwrap(),
			components,
			tick:change_tick,
		}
	}

	/// 删除实体
	pub fn despawn(&mut self, entity: Entity) {
		self.archetypes[entity.archetype_id()].remove_entity(entity.local());
	}

	/// 为实体插入组件
	pub fn insert_component<C: Component>(&mut self, entity: Entity, value: C) {
		let change_tick = self.read_change_tick();
		let id = self.components.get_or_insert_id::<C>();
		self.archetypes[entity.archetype_id()].insert_component(entity.local(), value, id, change_tick);
	}

	/// 为删除组件组件
	pub fn remove_component<C: Component>(&mut self, entity: Entity) {
		let id = self.components.get_or_insert_id::<C>();
		self.archetypes[entity.archetype_id()].remove_component(entity.local(), id);
	}

	/// 添加组件监听器
	pub fn add_component_listener<T: EventType, A: 'static + Send + Sync, C: Component>(&mut self, listener: Listener) {
		let component_id = match self.components.get_id(TypeId::of::<C>()) {
			Some(r) => r,
			None => {
				panic!("add_listener fail, component is not exist: {:?}", std::any::type_name::<C>())
			},
		};
		self.archetypes.add_component_listener::<T, A, C>(listener, component_id)
	}

	/// 添加资源监听器
	pub fn add_resource_listener<T: EventType, R: Component>(&mut self, listener: Listener) {
		let component_id = match self.components.get_resource_id::<R>() {
			Some(r) => r.clone(),
			None => {
				panic!("add_listener fail, component is not exist: {:?}", std::any::type_name::<R>())
			},
		};
		self.archetypes.add_resource_listener::<T, R>(listener, component_id);
	}

	/// 添加实体监听器
	pub fn add_entity_listener<T: EventType, A: 'static + Send + Sync>(&mut self, listener: Listener) {
		self.archetypes.add_entity_listener::<T, A>(listener);
	}

	/// 取到原型
	pub fn archetypes(&self) -> &Archetypes {
		&self.archetypes
	}

	pub fn entities(&self, arch_id: Local) -> &Entities {
		&self.archetypes()[arch_id].entities
	}

	/// 取到WorldId
	pub fn id(&self) -> WorldId {
        self.id
    }

	/// 读取节拍
	pub fn read_change_tick(&self) -> u32 {
        self.change_tick.load(Ordering::Acquire)
    }

	/// 读取节拍
    #[inline]
    pub fn change_tick(&mut self) -> u32 {
        *self.change_tick.get_mut()
    }

	/// 读取上次节拍
    #[inline]
    pub fn last_change_tick(&self) -> u32 {
        self.last_change_tick
    }

	/// 节拍增加，返回增加前的节拍
	#[inline]
    pub fn increment_change_tick(&self) -> u32 {
        self.change_tick.fetch_add(1, Ordering::AcqRel)
    }

	/// 节拍增加，并修改last_change_tick
    pub fn clear_trackers(&mut self) {
        // for entities in self.removed_components.values_mut() {
        //     entities.clear();
        // }

        self.last_change_tick = self.increment_change_tick();
    }
}

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, Default)]
pub struct WorldId(pub(crate) usize);

/// 原型信息
pub struct ArchetypeInfo<'a> {
	archetype: Archetype,
	pub(crate) world: &'a mut WorldInner,
	pub(crate) type_id: TypeId,
	pub(crate) components: HashSet<ComponentId>,
}

impl<'a> ArchetypeInfo<'a> {
	/// 为原型注册组件类型
	pub fn register<C: Component>(mut self) -> Self{
		let id = self.world.components.get_or_insert_id::<C>();
		let r = self.components.insert(id);

		if r {
			self.archetype.register_component_type::<C>(
				id, 
				Local::new(self.world.archetypes.archetype_component_grow()),
			);
		}
		self
	}

	/// 创建原型
	pub fn create(self) {
		self.world.archetypes.add_archetype(self.archetype, self.type_id);
	}
}

/// 实体引用
pub struct EntityRef<'a> {
	pub(crate) local: LocalVersion,
	pub(crate) archetype_id: ArchetypeId,
	pub(crate) archetype: &'a mut Archetype,
	pub(crate) components: &'a mut Components,
	tick: u32,
}

impl<'a> EntityRef<'a> {
	/// 为实体插入组件
	pub fn insert<C: Component>(&mut self, value: C) -> &mut Self  {
		let id = self.components.get_or_insert_id::<C>();
		self.archetype.insert_component(self.local, value, id, self.tick);
		self
	}

	/// 实体id
	pub fn id(&self) -> Entity {
		Entity::new(self.archetype_id, self.local)
	}
}

/// FromWorld
pub trait FromWorld {
    /// Creates `Self` using data from the given [World]
    fn from_world(world: &mut World) -> Self;
}

impl<T: Default> FromWorld for T {
    fn from_world(_world: &mut World) -> Self {
        T::default()
    }
}

impl Deref for World {
	type Target = WorldInner;
    fn deref(&self) -> &Self::Target {
		self.inner.get()
	}
}

impl DerefMut for World {
    fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe{&mut (*self.inner.as_ptr())} 
	}
}


unsafe impl Sync for WorldInner {
	
}

unsafe impl Send for WorldInner {
	
}