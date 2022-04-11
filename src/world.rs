use std::any::TypeId;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use pi_map::Map;
use pi_share::cell::TrustCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::archetype::{Archetype, ArchetypeComponentId, ArchetypeId, ArchetypeIdent, Archetypes};
use crate::component::{Component, ComponentId, Components};
use crate::entity::{Entities, Entity};
use crate::monitor::{EventType, Listener};
use crate::prelude::FilterFetch;
use crate::query::Access;
use crate::query::{QueryState, WorldQuery};
use crate::storage::{Local, LocalVersion, SecondaryMap};
use crate::sys::param::res::ResState;

/// 世界
#[derive(Clone)]
pub struct World {
    pub(crate) inner: Arc<TrustCell<WorldInner>>,
}

impl World {
    /// 构造方法
    #[inline]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TrustCell::new(WorldInner::new())),
        }
    }

    /// 查询
    #[inline]
    pub fn query<A: ArchetypeIdent, Q: WorldQuery>(&mut self) -> QueryState<A, Q, ()> {
        QueryState::new(self)
    }

    /// 带过滤 的 查询
    #[inline]
    pub fn query_filtered<A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery>(
        &mut self,
    ) -> QueryState<A, Q, F>
    where
        F::Fetch: FilterFetch,
    {
        QueryState::new(self)
    }

    /// 取 res
    pub fn res<T: Component>(&self) -> ResState<T> {
        let component_id = self.get_resource_id::<T>();
        let component_id = match component_id {
            Some(r) => r.clone(),
            None => panic!(
                "Res<{}> is not exist in res_mut",
                std::any::type_name::<T>()
            ),
        };
        ResState {
            component_id,
            marker: PhantomData,
        }
    }
}

/// 世界
pub struct WorldInner {
    pub(crate) id: WorldId,
    pub(crate) components: Components,
    pub(crate) archetypes: Archetypes,

    /// 该字段描述了监听器监听的组件做访问的数据id
    pub(crate) listener_access:
        SecondaryMap<ArchetypeComponentId, Vec<Access<ArchetypeComponentId>>>,

    pub(crate) change_tick: AtomicU32,
    pub(crate) last_change_tick: u32,

    pub(crate) query_generator: usize,
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
            query_generator: 0,
        }
    }

    #[inline]
    pub fn gen_query_id(&mut self) -> usize {
        self.query_generator += 1;
        self.query_generator - 1
    }

    /// 插入资源
    #[inline]
    pub fn insert_resource<T: Component>(&mut self, value: T) -> ComponentId {
		let component_id = if let None = self.components.get_resource_id::<T>() {
			let component_id = self.components.get_or_insert_resource_id::<T>();
			let archetype_component_id = self.archetypes.archetype_component_grow();
			self.archetypes.register_resource::<T>(component_id, archetype_component_id);
			component_id
		} else {
			self.components.get_or_insert_resource_id::<T>()
		};
        self.archetypes.insert_resource::<T>(value, component_id);
        component_id
    }

    /// 取 资源id
    #[inline]
    pub fn get_resource_id<T: Component>(&self) -> Option<&ComponentId> {
        self.components.get_resource_id::<T>()
    }

	#[inline]
    pub fn get_or_insert_resource_id<T: Component>(&mut self) -> ComponentId {
		match self.components.get_resource_id::<T>() {
			Some(r) => r.clone(),
			None => {
				let archetype_component_id = self.archetypes.archetype_component_grow();
				let component_id = self.components.get_or_insert_resource_id::<T>();
				self.archetypes.register_resource::<T>(component_id, archetype_component_id);
				component_id
			},
		}
    }

    /// 取 资源
    #[inline]
    pub fn get_resource<T: Component>(&self) -> Option<&T> {
        self.get_resource_id::<T>()
            .and_then(|id| unsafe { self.archetypes.get_resource(*id) })
    }

    /// 取 资源，可变引用
    #[inline]
    pub fn get_resource_mut<T: Component>(&self) -> Option<&mut T> {
        self.get_resource_id::<T>()
            .and_then(|id| unsafe { self.archetypes.get_resource_mut(*id) })
    }

    /// 原型组件id增长
    #[inline]
    pub fn archetype_component_grow(&mut self) -> usize {
        self.archetypes.archetype_component_grow()
    }

    /// 创建原型
    pub fn new_archetype<T: Send + Sync + 'static>(&mut self) -> ArchetypeInfo {
        ArchetypeInfo {
            archetype_id: self.archetypes.get_or_create_archetype::<T>(),
            world: self,
            // type_id: TypeId::of::<T>(),
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
        let (archetypes, components) = (&mut self.archetypes, &mut self.components);
        let e = archetypes.spawn::<T>(archetype_id);
        EntityRef {
            local: e.local(),
            archetype_id: archetype_id,
            archetype: archetypes.get_mut(archetype_id).unwrap(),
            components,
            tick: change_tick,
        }
    }

    /// 删除实体
    #[inline]
    pub fn despawn(&mut self, entity: Entity) {
        self.archetypes[entity.archetype_id()].remove_entity(entity.local());
    }

    /// 为实体插入组件
    pub fn insert_component<C: Component>(&mut self, entity: Entity, value: C) {
        let change_tick = self.read_change_tick();
        let id = self.components.get_or_insert_id::<C>();
        self.archetypes[entity.archetype_id()].insert_component(
            entity.local(),
            value,
            id,
            change_tick,
        );
    }

    /// 为删除组件组件
    #[inline]
    pub fn remove_component<C: Component>(&mut self, entity: Entity) {
        let id = self.components.get_or_insert_id::<C>();
        self.archetypes[entity.archetype_id()].remove_component(entity.local(), id);
    }

    /// 添加组件监听器
    pub fn add_component_listener<T: EventType, A: 'static + Send + Sync, C: Component>(
        &mut self,
        listener: Listener,
    ) {
        let component_id = match self.components.get_id(TypeId::of::<C>()) {
            Some(r) => r,
            None => {
                panic!(
                    "add_listener fail, component is not exist: {:?}",
                    std::any::type_name::<C>()
                )
            }
        };
        self.archetypes
            .add_component_listener::<T, A, C>(listener, component_id)
    }

    /// 添加资源监听器
    pub fn add_resource_listener<T: EventType, R: Component>(&mut self, listener: Listener) {
        let component_id = match self.components.get_resource_id::<R>() {
            Some(r) => r.clone(),
            None => {
                panic!(
                    "add_listener fail, component is not exist: {:?}",
                    std::any::type_name::<R>()
                )
            }
        };
        self.archetypes
            .add_resource_listener::<T, R>(listener, component_id);
    }

    /// 添加实体监听器
    #[inline]
    pub fn add_entity_listener<T: EventType, A: 'static + Send + Sync>(
        &mut self,
        listener: Listener,
    ) {
        self.archetypes.add_entity_listener::<T, A>(listener);
    }

    /// 取到原型
    #[inline]
    pub fn archetypes(&self) -> &Archetypes {
        &self.archetypes
    }

	/// 取到原型
    #[inline]
    pub fn archetypes_mut(&mut self) -> &mut Archetypes {
        &mut self.archetypes
    }

    /// 取 所有的 Entity
    #[inline]
    pub fn entities(&self, arch_id: Local) -> &Entities {
        &self.archetypes()[arch_id].entities
    }

	/// 取 所有的 Entity
    #[inline]
    pub fn entities_mut(&mut self, arch_id: Local) -> &mut Entities {
        &mut self.archetypes[arch_id].entities
    }

    /// 取到WorldId
    #[inline]
    pub fn id(&self) -> WorldId {
        self.id
    }

    /// 读取节拍
    #[inline]
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

	/// 取到组件，如果不存在组件，则注册组件
	pub fn get_or_register_component<C: Component>(&mut self, archetype_id: ArchetypeId) -> ComponentId {
		let archetype = self.archetypes.get_mut(archetype_id);
		if let Some(archetype) = archetype {
			let id = self.components.get_or_insert_id::<C>();
			if archetype.contains(id) {
				return id;
			}
			let g = self.archetypes.archetype_component_grow();
			let archetype = &mut self.archetypes[archetype_id];
            archetype.register_component_type::<C>(
                id,
                Local::new(g),
            );
			id
		} else {
			panic!("archetype is not exist, get_or_register_component fail, archetype:{:?}", archetype_id);// 原型不存在
		}
    }
}

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, Default)]
pub struct WorldId(pub(crate) usize);

/// 原型信息
pub struct ArchetypeInfo<'a> {
    archetype_id: ArchetypeId,
    pub(crate) world: &'a mut WorldInner,
    // pub(crate) type_id: TypeId,
    pub(crate) components: HashSet<ComponentId>,
}

impl<'a> ArchetypeInfo<'a> {
    /// 为原型注册组件类型
    pub fn register<C: Component>(mut self) -> Self {
        let id = self.world.components.get_or_insert_id::<C>();
        let r = self.components.insert(id);

        if r {
			let archetype_component_id = self.world.archetypes.archetype_component_grow();
            self.world.archetypes[self.archetype_id].register_component_type::<C>(
                id,
                Local::new(archetype_component_id),
            );
        }
        self
    }

    /// 创建原型
    pub fn create(self) {
        // self.world
        //     .archetypes
        //     .add_archetype(self.archetype, self.type_id);
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
    pub fn insert<C: Component>(&mut self, value: C) -> &mut Self {
        let id = self.components.get_or_insert_id::<C>();
        self.archetype
            .insert_component(self.local, value, id, self.tick);
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
    default fn from_world(_world: &mut World) -> Self {
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
        unsafe { &mut (*self.inner.as_ptr()) }
    }
}

unsafe impl Sync for WorldInner {}

unsafe impl Send for WorldInner {}

#[cfg(test)]
mod test {
    use super::World;

	#[test]
	fn test() {
		let mut world = World::new();
		world.new_archetype::<usize>();
	}
}
