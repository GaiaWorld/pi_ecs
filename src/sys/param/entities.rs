//! 实体创建和删除

use std::marker::PhantomData;

use crate::{
	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState, NotApply},
	sys::system::interface::SystemState,
	world::World,
	archetype::ArchetypeId,
	entity::{Entities, Id},
};

/// 实体插入
/// 可同步创建实体
pub struct EntityInsert<T> {
    entities: usize, //*const Entities
	mark: PhantomData<T>,
}

impl<T: Send + Sync + 'static> EntityInsert<T> {
	pub fn spawn(&mut self) -> Id<T> {
		Id(
			unsafe { &mut *(self.entities as *mut Entities) }
				.insert(), 
			PhantomData
		)
	}
}

impl<T: Send + Sync + 'static> SystemParam for EntityInsert<T> {
    type Fetch = EntityInsertState<T>;
}

/// The [`SystemParamState`] of [`SystemChangeTick`].
pub struct EntityInsertState<T>(ArchetypeId,  PhantomData<T>);

unsafe impl<T: Send + Sync + 'static> SystemParamState for EntityInsertState<T> {
    type Config = ();

    fn init(world:  &mut World, system_state: &mut SystemState, _config: Self::Config) -> Self {
		let arch_id = world.archetypes_mut().get_or_create_archetype::<T>();
		let archetype_component_id = world.archetypes_mut().get(arch_id).unwrap().entity_archetype_component_id();
		// 判断实体是否访问冲突
		if system_state.archetype_component_access.combined_access().has_read(archetype_component_id) {
			panic!("init {:?} fail, entity access conflict, entity: {:?}", std::any::type_name::<Self>(), std::any::type_name::<T>());
		}
		system_state.archetype_component_access.combined_access_mut().add_write(archetype_component_id);
        Self(arch_id, PhantomData)
    }

    fn default_config() -> () {
        ()
    }
}

impl<'w, 's, T: Send + Sync + 'static> SystemParamFetch<'w, 's> for EntityInsertState<T> {
    type Item = EntityInsert<T>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        _system_state: &SystemState,
        world: &'w World,
        _change_tick: u32,
    ) -> Self::Item {
		EntityInsert{
			entities: world.entities(state.0) as *const Entities as usize,
			mark: PhantomData,
		}
    }
}

impl<T: Send + Sync + 'static> NotApply for EntityInsertState<T> {}




/// 实体删除
/// 可删除实体，但并不是同步删除（删除实体意味着删除组件，其对读写数据有更严格的要求，比如，无法与写入该类型对应组件并发执行）
/// 其也与entitycommand不同，本参数会对原型上的实体删除列表进行写入，任何其他会对删该列表进行写入的参数，都将与本参入存在写冲突
pub struct EntityDelete<T> {
    entities: usize, //*const Entities
	mark: PhantomData<T>,
}

impl<T: Send + Sync + 'static> EntityDelete<T> {
	pub fn despawn(&mut self, id: Id<T>) -> Option<()> {
		unsafe { &mut *(self.entities as *mut Entities) }.remove(id.0)
	}
}

impl<T: Send + Sync + 'static> SystemParam for EntityDelete<T> {
    type Fetch = EntityDeleteState<T>;
}

/// The [`SystemParamState`] of [`SystemChangeTick`].
pub struct EntityDeleteState<T>(ArchetypeId,  PhantomData<T>);

unsafe impl<T: Send + Sync + 'static> SystemParamState for EntityDeleteState<T> {
    type Config = ();

    fn init(world:  &mut World, system_state: &mut SystemState, _config: Self::Config) -> Self {
		let arch_id = world.archetypes_mut().get_or_create_archetype::<T>();
		let archetype_component_id = world.archetypes_mut().get(arch_id).unwrap().entity_archetype_component_delete_id();
		// 判断实体是否访问冲突
		if system_state.archetype_component_access.combined_access().has_read(archetype_component_id) {
			panic!("init {:?} fail, entity access conflict, entity: {:?}, in system: {:?}", std::any::type_name::<Self>(), std::any::type_name::<T>(), system_state.name());
		}
		system_state.archetype_component_access.combined_access_mut().add_write(archetype_component_id);
        Self(arch_id, PhantomData)
    }

    fn default_config() -> () {
        ()
    }
}

impl<'w, 's, T: Send + Sync + 'static> SystemParamFetch<'w, 's> for EntityDeleteState<T> {
    type Item = EntityDelete<T>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        _system_state: &SystemState,
        world: &'w World,
        _change_tick: u32,
    ) -> Self::Item {
		EntityDelete{
			entities: world.entities(state.0) as *const Entities as usize,
			mark: PhantomData,
		}
    }
}

impl<T: Send + Sync + 'static> NotApply for EntityDeleteState<T> {}
