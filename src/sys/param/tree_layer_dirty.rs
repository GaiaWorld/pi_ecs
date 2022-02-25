use std::{marker::PhantomData, sync::Arc, any::TypeId};

use dirty::{LayerDirty as LayerDirty1, DirtyIterator};
use hash::XHashMap;
use map::Map;

use crate::{
	component::{Component, ComponentId},
	world::World, 
	sys::{
		system::SystemState, 
		param::{SystemParamFetch, SystemParamState}
	},
	storage::{LocalVersion, SecondaryMap, Local}, 
	monitor::{Event, ComponentListen, Create, Modify, Listen, Listeners,ListenSetup, ResourceListen},
	prelude::{FetchState, Fetch, WorldQuery, FilterFetch, filter_change::ChangedFetch}, entity::Entity, archetype::ArchetypeIdent,
};
use tree::{Tree, Empty, RecursiveIterator};

use super::{Res, assert_component_access_compatibility, SystemParam};

/// 层脏
/// 默认监听了组件的修改、创建事件、Tree的创建事件，当监听到这些事件，会添加到层次脏列表
pub struct LayerDirty<A: ArchetypeIdent, F: WorldQuery, N: 'static + Send + Sync + Default = ()> {
	state: Arc<LayerDirtyState<A, F, N>>,
}

impl<A: ArchetypeIdent, F: WorldQuery, N: 'static + Send + Sync + Default> LayerDirty<A, F, N> {
	fn new(world: &World, mut state: Arc<LayerDirtyState<A, F, N>>, last_change_tick: u32, change_tick: u32) -> Self {
			let state_ref = unsafe{&mut *(Arc::as_ptr(&mut state) as usize as *mut LayerDirtyState<A, F, N>)};
			unsafe{ state_ref.inner_fetch.setting(world, last_change_tick, change_tick)};
			Self {
				state
			}
		}
}

impl<A, F, N> SystemParam for LayerDirty<A, F, N>
where
	A: ArchetypeIdent,
	F: WorldQuery + 'static,
    F::Fetch: FilterFetch + InstallLayerListen,
	N: 'static + Send + Sync + Default,
{
    type Fetch = Arc<LayerDirtyState<A, F, N>>;
}

impl<N: 'static + Send + Sync + Default, A: ArchetypeIdent, F: WorldQuery> LayerDirty<A, F, N> {
	pub fn iter(&self) -> LayerDirtyIter<A, F, N> {
		let state = unsafe {
			&mut *(Arc::as_ptr(&self.state) as usize as *mut LayerDirtyState<A, F, N>)
		};
		let tree = unsafe{&*(self.state.tree_ptr as *const Tree<LocalVersion, N>)};
		LayerDirtyIter {
			matchs: state.is_matchs,
			iter_inner: state.layer_inner.layer_list.iter(),
			mark_inner: &mut state.layer_inner.dirty_mark,
			tree,
			pre_iter: None,
			archetype_id: state.archetype_id,
			mark: PhantomData,
		}
	}
}

pub struct LayerDirtyIter<'s, A: ArchetypeIdent, F: WorldQuery, N: 'static + Send + Sync + Default> {
	mark: PhantomData<&'s (A, F, N)>,
	matchs: bool,
	iter_inner: DirtyIterator<'s, LocalVersion>,

	mark_inner: &'s mut SecondaryMap<LocalVersion, usize>,

	tree: &'s Tree<LocalVersion, N>,
	archetype_id: Local,

	pre_iter: Option<RecursiveIterator<'s, LocalVersion, N>>,
}

impl<'s, N: 'static + Send + Sync + Default, A: ArchetypeIdent, F: WorldQuery> Iterator for LayerDirtyIter<'s, A, F, N>
where
	F::Fetch: FilterFetch,
{
    type Item = (Entity, usize);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
		if !self.matchs {
			return  None;
		}
		if let Some(r) = &mut self.pre_iter {
			// 上次迭代的脏还没完成，继续迭代
			match r.next() {
				Some(next) => {
					self.mark_inner.remove(&next.0); // 标记为不脏
					return Some((Entity::new(self.archetype_id, next.0.clone()) , next.1.layer()))
				},
				None => self.pre_iter = None
			};
		}

		// 上一个子树迭代完成，继续迭代下一个脏
		let item = self.iter_inner.next();
		if let Some((local, layer)) = item {
			if let Some(layer1) = self.mark_inner.get(local) {
				let layer1 = *layer1;
				self.mark_inner.remove(local); // 标记为不脏

				// 记录的层次和实际层次相等，并且在idtree中的层次也相等，则返回该值
				if layer == layer1{
					if let Some(r) = self.tree.get(*local) {
						if r.layer() == layer {
							// 是否判断changed？TODO
							// 记录上次迭代出的实体id，下次将对该节点在itree进行先序迭代
							self.pre_iter = Some(self.tree.recursive_iter(r.children().head));
							return Some((Entity::new(self.archetype_id, local.clone()), layer));
						}
					}
				}
			}
		}
		return None;
    }
}

pub struct LayerDirtyInner{
	pub(crate) layer_list: LayerDirty1<LocalVersion>, // 脏列表
	pub(crate) dirty_mark: SecondaryMap<LocalVersion, usize>,
	// 已经安装过监听器的组件不需要再安装（用于记录已安装监听器的组件的ComponentId）
	pub(crate) is_install: XHashMap<ComponentId, ()>,
}

impl Empty for LocalVersion {
	fn empty() -> Self {
		LocalVersion(0)
	}
}

impl LayerDirtyInner {
	pub fn new() -> Self {
		Self {
			layer_list: LayerDirty1::default(),
			dirty_mark: SecondaryMap::with_capacity(0),
			is_install: XHashMap::default(),
		}
	}
	pub fn insert<N: 'static + Sync + Send + Default>(&mut self, id: LocalVersion, tree: &Tree<LocalVersion, N>) {
		match tree.get(id) {
            Some(r) => {
                if r.layer() != 0 {
					let d = match self.dirty_mark.get_mut(&id) {
						Some(r) => r,
						None => {
							// 如果dirty_mark中不存在id，需要新创建
							self.dirty_mark.insert(id, 0);
							&mut self.dirty_mark[id]
						},
					};
					// 新的layer和旧的layer不相等，则记录新的（不删除原来的，在迭代层次脏时，会重现判断层，原有的会自动失效）
                    if *d != r.layer() {
                        *d = r.layer();
                        self.layer_list.mark(id, r.layer());
                    }
                }
            }
            _ => (),
        };
	}
}

pub trait InstallLayerListen: FilterFetch {
	/// 安装监听器，收到事件设脏
	/// 安全：
	///  * 保证layer在监听器删除之前，该指针不会被销毁
	unsafe fn install<A: ArchetypeIdent, N: 'static + Sync + Send + Default>(&self, world: &mut World, layer: *const LayerDirtyInner, state: &Self::State);
}

impl<T: Component> InstallLayerListen for ChangedFetch<T>{
	
	unsafe fn install<A: ArchetypeIdent, N: 'static + Sync + Send + Default>(&self, world: &mut World, layer: *const LayerDirtyInner, state: &Self::State) {
		let layer = layer as usize;
		let layer_obj = &mut *(layer as *mut LayerDirtyInner);
		if let None = layer_obj.is_install.get(&state.component_id) {
			let component_id = state.component_id;

			// 安装监听器，监听对应组件修改，并将改变的实体插入到脏列表中
			let listen = move |
				event: Event, 
				_:Listen<ComponentListen<A, T, (Create, Modify)>>,
				idtree: Res<Tree<LocalVersion, N>>
			| {
				// 标记层脏
				unsafe{&mut *(layer as *mut LayerDirtyInner)}.insert(event.id.local(), &idtree);
			};
			// 标记监听器已经设置，下次不需要重复设置（同一个查询可能涉及到多次相同组件的过滤）
			layer_obj.is_install.insert(component_id, ());
			let l = listen.listeners();
			l.setup(world);


			// 安装监听器，监听Tree的Create事件
			let listen = move |
				event: Event, 
				_:Listen<ResourceListen<Tree<LocalVersion, N>, Create>>,
				idtree: Res<Tree<LocalVersion, N>>
			| {
				// 标记层脏
				unsafe{&mut *(layer as *mut LayerDirtyInner)}.insert(event.id.local(), &idtree);
			};
			// 标记监听器已经设置，下次不需要重复设置（同一个查询可能涉及到多次相同组件的过滤）
			layer_obj.is_install.insert(component_id, ());
			let l = listen.listeners();
			l.setup(world);
		}
	}
}

pub struct LayerDirtyState<A: ArchetypeIdent, F: WorldQuery, N: 'static + Send + Sync> {
	pub(crate) layer_inner: LayerDirtyInner, // 脏列表
	pub(crate) inner_state: F::State,
	pub(crate) inner_fetch: F::Fetch,
	pub(crate) is_matchs: bool,
	_world: World, // 抓住索引，确保在其销毁之前，World不销毁
	tree_ptr: usize,
	pub(crate) archetype_id: Local,
	mark: PhantomData<(A, N)>,
}

unsafe impl<N: 'static + Send + Sync + Default, A: ArchetypeIdent, F: WorldQuery + 'static> SystemParamState for Arc<LayerDirtyState<A, F, N>>
	where F::State: FetchState, F::Fetch: FilterFetch + InstallLayerListen{
    type Config = ();
	
	/// 检查数据访问冲突
	/// 一些状态的初始化
	/// 添加监听器监听数据的改变，进行脏设置
    fn init(world: &mut World, system_state: &mut SystemState, _config: Self::Config) -> Self {
		let (last_change_tick, change_tick) = (world.last_change_tick(), world.change_tick());
		let mut component_access = Default::default();
		let mut archetype_component_access = Default::default();
		let state = F::State::init(world, 0);
		state.update_component_access(&mut component_access);
        state.update_component_access(&mut component_access);

		let mut fetch = unsafe { F::Fetch::init(world, &state) };
		let archetype_id = match world.archetypes().get_id_by_ident(TypeId::of::<A>()) {
			Some(r) => r.clone(),
			None => panic!(),
		};

		let archetypes = world.archetypes();
		let archetype = &archetypes[archetype_id];

		let is_matchs = state.matches_archetype(archetype);
		
		if is_matchs{
			unsafe{
				fetch.set_archetype(&state, archetype, world);
				
				fetch.setting(world, last_change_tick, change_tick);
				state.update_archetype_component_access(archetype, &mut archetype_component_access);
				state.update_archetype_component_access(archetype, &mut archetype_component_access);
			}
		}
		
		let tree_resoruce_id = match world.get_resource_id::<Tree<LocalVersion, N>>() {
			Some(r) => r.clone(),
			None => panic!("systemparam init fail, {:?} is not register, in system {:?}", std::any::type_name::<Tree<LocalVersion, N>>(), &system_state.name),
		};
		let tree = unsafe{world.archetypes().get_resource::<Tree<LocalVersion, N>>(tree_resoruce_id)}.unwrap();
		let tree_ptr = tree as *const Tree<LocalVersion, N> as usize;

		let r = Arc::new(LayerDirtyState {
			layer_inner: LayerDirtyInner::new(),
			inner_state: state,
			inner_fetch: fetch,
			tree_ptr,
			mark: PhantomData,
			_world: world.clone(),
			archetype_id,
			is_matchs
        });

		// 判断访问是否冲突
		let tree_archetype_id = world.archetypes().get_archetype_resource_id::<Tree<LocalVersion, N>>().unwrap().clone();
		if archetype_component_access.has_write(tree_archetype_id) {
			panic!("systemparam init fail, {:?} read and write conflict, in system {:?}", std::any::type_name::<Tree<LocalVersion, N>>(), &system_state.name);
		}
		component_access.add_read(tree_resoruce_id);
		archetype_component_access.add_read(tree_archetype_id);

		assert_component_access_compatibility(
            &system_state.name,
            std::any::type_name::<LayerDirty::<A, F, N>>(),
            std::any::type_name::<LayerDirty::<A, F, N>>(),
            &system_state.component_access_set,
            &component_access,
            world,
        );
		// 将查询访问的组件集添加到系统访问的组件集中
        system_state
            .component_access_set
            .add(component_access);
		// 将查询访问的原型组件放入系统的原型组件集中（用于检查系统与系统的访问组件是否冲突，访问不同原型的同类型组件是允许的）
        system_state
            .archetype_component_access
            .extend(&archetype_component_access);

		if is_matchs {
			let inner = &r.layer_inner as *const LayerDirtyInner;
			let state = unsafe{&*( &r.inner_state as *const F::State) };
			// 在
			unsafe {InstallLayerListen::install::<A, N>(&r.inner_fetch, world, inner, state)};
		}

		r
    }

	fn apply(&mut self, _world: &mut World) {
		// 清理脏记录
		unsafe{ &mut *(Arc::as_ptr(self) as usize as *mut LayerDirtyState<A, F, N>)}.layer_inner.layer_list.clear();
	}

    fn default_config() {}
}

impl<'a, N: 'static + Send + Sync + Default, A: ArchetypeIdent, F: WorldQuery + 'static> SystemParamFetch<'a> for Arc<LayerDirtyState<A, F, N>> 
	where 
		F::State: FetchState,
		F::Fetch: InstallLayerListen + FilterFetch{
    type Item = LayerDirty<A, F, N>;

    #[inline]
    unsafe fn get_param(
        state: &'a mut Self,
        system_state: &'a SystemState,
        world: &'a World,
        change_tick: u32,
    ) -> Self::Item {
		LayerDirty::new(world, state.clone(), system_state.last_change_tick, change_tick)
    }
}