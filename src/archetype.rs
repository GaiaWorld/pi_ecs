/// 原型
use std::any::type_name;

use crate::{
    component::{ComponentId, CellMultiCase, MultiCase, Component, MultiCaseImpl},
    entity::{Entity, Entities},
    storage::{Offset, LocalVersion, Local},
	monitor::{Listener, ListenType}, 
	resource::{Singles, Resource, ResourceId},
	prelude::FilteredAccessSet,
};
use std::{
    borrow::Cow,
    hash::Hash,
    ops::{Index, IndexMut},
	any::TypeId,
	sync::Arc,
};

use pi_share::cell::TrustCell;
use pi_hash::XHashMap;
use pi_slotmap::SecondaryMap;

pub struct Archetype {
	// 原型id
    id: ArchetypeId,
	archetype_component_id: ArchetypeComponentId, // 实体访问id
	archetype_component_delete_id: ArchetypeComponentId, // 实体删除访问id
	// 该原型下的实体
    pub(crate) entities: Entities,

	// 组件（每个ComponentId对应一个MultiCase）
	// MultiCase是某个类型的组件的容器
	components: SecondaryMap<ComponentId, Arc<dyn MultiCase>>,

	// 该原型注册的组件类型id（ComponentId）
	component_ids: Vec<ComponentId>,

	// 原型组件id，每原型的每组件类型，对应一个原型组件id，资源可以认为是资源原型中的组件
	// 可以用该id区分相同原型下的不同组件、不同原型下的不同组件、不同原型下的相同组件，以及区分它们和资源
	// 常用于判断数据访问冲突
	archetype_component_ids: SecondaryMap<ComponentId, ArchetypeComponentId>,

	pub delete_list: Vec<LocalVersion>,
}

impl Archetype {
	/// 创建原型，创建的原型中还未注册组件类型，需要再调用
	pub fn new(id: ArchetypeId, archetype_component_id: ArchetypeComponentId, archetype_component_delete_id: ArchetypeComponentId) -> Self {
		Self {
			id,
			archetype_component_id,
			archetype_component_delete_id,
			entities: Entities::new(id),

			components: SecondaryMap::with_capacity(0),

			archetype_component_ids: SecondaryMap::with_capacity(0),
			component_ids: Vec::default(),
			delete_list: Vec::new(),
		}
	}

	pub fn reserve_entity(&mut self) -> LocalVersion {
		self.entities.reserve_entity()
	}

	pub fn entity_archetype_component_id(&self) -> ArchetypeComponentId {
		self.archetype_component_id
	}

	/// 取到指定组件的原型组件删除id
	pub fn entity_archetype_component_delete_id(&self) -> ArchetypeComponentId {
		self.archetype_component_delete_id
	}
	

	/// 为原型注册组件类型
	pub(crate) fn register_component_type<C: Component>(&mut self, id: ComponentId, archetype_component_id: ArchetypeComponentId){
		let container = Arc::new(TrustCell::new(MultiCaseImpl::<C>::with_capacity(0, self.id())));

		self.components.insert(
				id,
				container,
		);
		self.component_ids.push(id);
		self.archetype_component_ids.insert(id, archetype_component_id);
	}

	/// 创建实体
	pub fn create_entity(&mut self) -> Entity {
		Entity::new(self.id, self.entities.insert())
	}

	/// 移除实体
	/// 移除实体时，会连带将其拥有的组件页删除，会发出实体删除的事件，但不会发出组件销毁的事件，
	pub fn remove_entity(&mut self, local: LocalVersion) {
		if self.entities.remove(local).is_some() {
			for i in self.component_ids.iter() {
				self.components[*i].delete(local);
			}
		};
	}

	/// 即将移除的实体
	pub fn will_remove_entity(&mut self, local: LocalVersion) {
		self.delete_list.push(local);
	}

	pub fn flush(&mut self) {
		self.entities.flush(); // 插入新的实体
		// 删除实体
		if self.delete_list.len() > 0 {
			let v = std::mem::replace(&mut self.delete_list, Vec::new());
			for i in v.iter() {
				self.remove_entity(i.clone())
			}
			self.delete_list = v;
		}
	}

	/// 为指定实体添加组件
	pub fn insert_component<C: Component>(&mut self, local: LocalVersion, value: C, id: ComponentId, tick: u32) -> Option<C> {
		if self.components.get(id).is_none() {
			return None;
		}
		unsafe {
			self.insert_component_unsafe::<C>(local, value, id, tick)
		}
	}

	/// 为指定实体添加组件
	/// 组件类型必须已经通过register_component_type方法组件到原型上,否者将panic
	pub unsafe fn  insert_component_unsafe<C: Component>(&mut self, local: LocalVersion, value: C, id: ComponentId, tick: u32) -> Option<C> {
		let container = self.components.get_unchecked(id);
		match container.clone().downcast() {
			Ok(r) => {
				let r: Arc<CellMultiCase<C>> = r;
				let mut r_ref = r.borrow_mut();
				r_ref.insert(local, value, tick)
			},
			Err(_) => panic!("downcast err"),
		}
	}

	/// 移除组件
	/// 若组件类型未通过register_component_type方法组件到原型上, 组件不能删除
	pub fn remove_component(&mut self, local: LocalVersion, id: ComponentId) {
		if self.components.get(id).is_none() {
			return;
		}
		unsafe {
			self.remove_component_unsafe(local, id);
		}
	}

	/// 移除组件
	pub unsafe fn remove_component_unsafe(&mut self, local: LocalVersion, id: ComponentId) {
		let container = self.components.get_unchecked(id);
		container.delete(local)
	}

	/// 添加组件监听器
	pub fn add_component_listener<T: ListenType, C: Component>(&mut self, listener: Listener, id: ComponentId) {
		let container = unsafe{ self.components.get_unchecked(id) };
		match container.clone().downcast_ref::<CellMultiCase<C>>() {
			Some(r) => {
				T::add(r, listener);
			},
			None => panic!("downcast err"),
		}
	}

	/// 添加实体监听器
	#[inline]
	pub fn add_entity_listener<T: ListenType>(&mut self, listener: Listener) {
		T::add(&self.entities.entity_listners, listener);
	}

	/// 取到原型id
    #[inline]
    pub fn id(&self) -> ArchetypeId {
        self.id
    }

	/// 取到实体数量
	#[inline]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

	/// 是否不存在实体
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

	/// 取到组件容器
	#[inline]
	pub unsafe fn get_component(&self, id: ComponentId) -> &Arc<dyn MultiCase> {
		self.components.get_unchecked(id)
	}

	/// 取到组件id列表
    #[inline]
    pub fn component_ids(&self) -> &[ComponentId] {
        &self.component_ids
    }

	/// 判断该原型是否包含某组件类型
	pub fn contains(&self, component_id: ComponentId) -> bool {
        self.components.contains_key(component_id)
    }

	/// 取到指定组件的原型组件id
	pub unsafe fn archetype_component_id(&self, component_id: ComponentId) -> ArchetypeComponentId {
		self.archetype_component_ids[component_id]
	}
}

/// 原型ID
pub type ArchetypeId = Local;

/// 标识符原型
pub trait ArchetypeIdent : 'static + Send + Sync {}

impl<C: Send + Sync + 'static> ArchetypeIdent for C  {}

/// 原型id生成器
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ArchetypeGeneration(usize);

impl ArchetypeGeneration {
    #[inline]
    pub fn new(generation: usize) -> Self {
        ArchetypeGeneration(generation)
    }

    #[inline]
    pub fn value(self) -> usize {
        self.0
    }
}

/// 原型唯一标识，目前只用到的Identity
/// 补充： 是否支持Components（动态原型，向beay一样）
#[derive(Hash, PartialEq, Eq)]
pub enum ArchetypeIdentity {
	Identity(TypeId),
	Components(Cow<'static, [ComponentId]>),
}

/// 原型组件id
pub type ArchetypeComponentId = Local;

/// 原型集
pub struct Archetypes {
	/// 拥有的原型
    pub(crate) archetypes: Vec<Archetype>,
	/// 原型标识映射原型id（可以通过原型类型查到原型id）
    archetype_ids: XHashMap<ArchetypeIdentity, ArchetypeId>,
	/// 原型组件的当前数量（用于生成原型组件id）
	// pub(crate) archetype_component_count: usize,
	pub(crate) archetype_component_info: Vec<String>,

	/// 资源map， 通过资源id查询到资源
	pub(crate) resources: Singles,

	/// 资源类型到原型组件id的映射
	pub(crate) archetype_resource_indices: XHashMap<TypeId, ArchetypeComponentId>,

	/// todo, 手机监听器的资源访问，设置再system上，以便有正确的数据访问依赖
	pub listener_component_access: XHashMap<ArchetypeComponentId, Vec<FilteredAccessSet<ArchetypeComponentId>>>,
}

impl Archetypes {
	/// 构造方法
	pub(crate) fn new() -> Self {
		Self {
			archetypes: Vec::new(),
			archetype_ids: XHashMap::default(),

			archetype_resource_indices: XHashMap::default(),
			resources: Singles::new(),
			listener_component_access: XHashMap::default(),
			archetype_component_info: Vec::default(),
		}
	}

	/// 创建原型
	/// * `type_id`为原型类型的TypeId，返回原型实例
	/// 该方法仅仅创建的一个原型实例，必须调用Archetypes.init_archetype原型方法，才能将原型由world管理起来。
	pub(crate) fn create_archetype_by_ident<A: Send + Sync + 'static>(&mut self) -> Archetype {
		let type_id = TypeId::of::<A>();
		if let Some(_) = self.archetype_ids.get(&ArchetypeIdentity::Identity(type_id)) {
			panic!("archetype is exist");
		}

		let id = ArchetypeId::new(self.archetypes.len());

		let archetype = Archetype::new(
			id, 
			Local::new(self.archetype_component_grow(type_name::<A>().to_string())),
			Local::new(self.archetype_component_grow(type_name::<A>().to_string() + "-Delete")),
		);
		archetype
    }

	/// 取到原型，如果原型不存在，则创建原型
	pub fn get_or_create_archetype<T: Send + Sync + 'static>(&mut self) -> ArchetypeId {
		let type_id = TypeId::of::<T>();
		if let Some(archetype) = self.archetype_ids.get(&ArchetypeIdentity::Identity(type_id)) {
			return archetype.clone();
		}

		let archetype = self.create_archetype_by_ident::<T>();
		let archetype_id = archetype.id();
		self.add_archetype(archetype, type_id);
		archetype_id
	}

	/// 创建原型
	pub fn create_archetype<T: Send + Sync + 'static>(&mut self) -> ArchetypeId {
		let type_id = TypeId::of::<T>();
		let archetype = self.create_archetype_by_ident::<T>();
		let archetype_id = archetype.id();
		self.add_archetype(archetype, type_id);
		archetype_id
	}

	/// 添加原型，将原型管理起来
	pub(crate) fn add_archetype(&mut self, archetype: Archetype, type_id: TypeId) {
		self.archetype_ids.insert(ArchetypeIdentity::Identity(type_id), archetype.id);
		self.archetypes.push(archetype);
	}

	/// 创建实体
	pub(crate) fn spawn<E: Send + Sync + 'static>(&mut self, id: ArchetypeId) -> Entity {
		self.archetypes[id.offset()].create_entity()
    }

	/// 插入资源
	pub(crate) fn insert_resource<T: Resource>(&mut self, value: T, id: ResourceId, tick: u32) {
		// self.archetype_resource_indices.insert(TypeId::of::<T>(), ArchetypeComponentId::new(archetype_component_id));
		unsafe { self.resources.insert(id, value, tick); };
	}

	pub(crate) fn register_resource<T: Resource>(&mut self, resource_id: ResourceId, archetype_component_id: usize) {
		self.archetype_resource_indices.insert(TypeId::of::<T>(), ArchetypeComponentId::new(archetype_component_id));
		self.resources.register::<T>(resource_id);
	}
	

	/// 取到原型组件id（不同原型相同类型的组件，id不同）
	pub fn get_archetype_resource_id<T: Resource>(&self) -> Option<&ArchetypeComponentId> {
		self.archetype_resource_indices.get(&TypeId::of::<T>())
	}

	/// 根据资源的id, 取到资源的只读引用
	pub unsafe fn get_resource<T: Resource>(&self, id: ResourceId) -> Option<&T> {
		self.resources.get(id)
	}

	/// 根据资源的id, 取到资源的只读引用
	pub unsafe fn get_resource_mut<T: Resource>(&self, id: ResourceId) -> Option<&mut T> {
		self.resources.get_mut(id)
	}

	#[inline]
	pub unsafe fn get_resource_unchecked<T: Resource>(&self, id: ResourceId) -> &T {
		self.resources.get_unchecked(id)
	}

	/// 根据资源的id, 取到资源的只读引用
	#[inline]
	pub unsafe fn get_resource_unchecked_mut<T: Resource>(&self, id: ResourceId) -> &mut T {
		self.resources.get_unchecked_mut(id)
	}

	// pub unsafe fn get_resource_notify<T: Component>(&self, id: ComponentId) -> Option<NotifyImpl> {
	// 	// self.resources.get_notify(id)
	// }

	/// 原型组件id增长
	#[inline]
	pub(crate) fn archetype_component_grow(&mut self, info: String) -> usize {
		self.archetype_component_info.push(info);
		self.archetype_component_info.len()
	}

	/// 添加组件监听器
	pub fn add_component_listener<T: ListenType, A: 'static + Send + Sync, C: Component>(&mut self, listener: Listener, id: ComponentId) {
		let archetype_id = self.get_or_create_archetype::<A>();
		if self.archetypes[archetype_id.offset()].components.get(id).is_none() {
			let archetype_component_id = self.archetype_component_grow(std::any::type_name::<C>().to_string());
			self.archetypes[archetype_id.offset()].register_component_type::<C>(id, ArchetypeComponentId::new(archetype_component_id))
		}
		
		self.archetypes[archetype_id.offset()].add_component_listener::<T, C>(listener, id)
	}

	/// 添加原型监听器
	pub fn add_entity_listener<T: ListenType, A: 'static + Send + Sync>(&mut self, listener: Listener) {
		let archetype_id = self.get_or_create_archetype::<A>();
		self.archetypes[archetype_id.offset()].add_entity_listener::<T>(listener);
	}

	/// 添加资源监听器
	pub fn add_resource_listener<T: ListenType, R: Component>(&mut self, listener: Listener, id: ComponentId) {
		self.resources.add_listener::<T, R>(id, listener);
	}

	/// 取到当前原型id
    #[inline]
    pub fn generation(&self) -> ArchetypeGeneration {
        ArchetypeGeneration(self.archetypes.len())
    }

	/// 原型数量
    #[inline]
    pub fn len(&self) -> usize {
        self.archetypes.len()
    }

	// 是否不存在原型
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.archetypes.is_empty()
    }

	/// 取到原型的只读引用
    #[inline]
    pub fn get(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.archetypes.get(id.offset())
    }

	/// 取到原型的可写引用
    #[inline]
    pub fn get_mut(&mut self, id: ArchetypeId) -> Option<&mut Archetype> {
        self.archetypes.get_mut(id.offset())
    }

	// 迭代原型
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Archetype> {
        self.archetypes.iter()
    }

	/// 根据原型类型TypeId，取到ArchetypeId
	#[inline]
    pub fn get_id_by_ident(&self, type_id: TypeId) -> Option<&ArchetypeId> {
        self.archetype_ids.get(&ArchetypeIdentity::Identity(type_id))
    }

	pub fn archetype_component_info(&self)  -> &Vec<String>{
		&self.archetype_component_info
	}
}

impl Index<ArchetypeId> for Archetypes {
    type Output = Archetype;

    #[inline]
    fn index(&self, index: ArchetypeId) -> &Self::Output {
        &self.archetypes[index.offset()]
    }
}

impl IndexMut<ArchetypeId> for Archetypes {
    #[inline]
    fn index_mut(&mut self, index: ArchetypeId) -> &mut Self::Output {
        &mut self.archetypes[index.offset()]
    }
}
