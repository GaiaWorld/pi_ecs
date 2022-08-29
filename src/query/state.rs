use std::marker::PhantomData;

// use crate::storage::LocalVersion;
use crate::world::World;
use crate::{
    archetype::{ArchetypeId, ArchetypeIdent, ArchetypeComponentId},
    entity::Id,
    query::{
        Fetch, FetchState, FilterFetch, FilteredAccess, QueryIter, ReadOnlyFetch,
        WorldQuery
    },
    world::{WorldInner, WorldId},
};
use thiserror::Error;

// use super::EntityIter;

pub struct QueryState<A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery = ()>
where
    F::Fetch: FilterFetch,
{
    world_id: WorldId,
	pub(crate) archetype_id: ArchetypeId, // A对应的实体id
	// pub(crate) component_access: FilteredAccess<ComponentId>, // 暂时没用
    pub(crate) archetype_component_access: FilteredAccess<ArchetypeComponentId>,
    pub(crate) fetch_state: Q::State,
    pub(crate) filter_state: F::State,
	pub(crate) fetch_fetch: Q::Fetch,
	pub(crate) filter_fetch: F::Fetch,

	pub(crate) matchs: bool,

	// pub(crate) id: usize, // 为每个查询分配一个id， 一些和没查询相关联的数据可以用该id绑定

	mark: PhantomData<A>,
}

// fn aa(q: Query<(&C, &X)>, q1: Query<&B>) {
// 	for q.iter()

// 	q.get(entity);
// }

// Query Fetch SyatemState QueryState

// // [a1, a4]
// // ComponentId

// // [a1, a2, a3, a4]

// // ABC a1
// // AC a4

impl<A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery> QueryState<A, Q, F>
where
    F::Fetch: FilterFetch,
{
    pub fn new(world:  &mut World) -> Self {
		let archetype_id = world.archetypes_mut().get_or_create_archetype::<A>();
		let q_id = world.gen_query_id();

        let fetch_state = <Q::State as FetchState>::init(world, q_id, archetype_id);
        let filter_state = <F::State as FetchState>::init(world, q_id, archetype_id);
		let fetch_fetch =
            unsafe{ <Q::Fetch as Fetch>::init(world, &fetch_state) };
        let filter_fetch =
			unsafe{ <F::Fetch as Fetch>::init(world, &filter_state)};

        // let component_access = Default::default();
		// fetch_state.update_component_access(&mut component_access);
        // filter_state.update_component_access(&mut component_access);


        let mut state = Self {
			matchs: false,
            world_id: world.id(),
			archetype_id,
            fetch_state,
            filter_state,
			fetch_fetch,
			filter_fetch,
			// component_access: component_access,
            archetype_component_access: Default::default(),
			// id: q_id,
			mark: PhantomData
        };
		state.validate_world_and_update_archetypes(world);
        state
    }

	pub fn archetype_id(&self) -> ArchetypeId {
		self.archetype_id
	}

	pub fn validate_world_and_update_archetypes(&mut self, world: &mut World) {
        if world.id() != self.world_id {
            panic!("Attempted to use {} with a mismatched WorldInner. QueryStates can only be used with the WorldInner they were created from.",
                std::any::type_name::<Self>());
        }
        let archetypes = world.archetypes();
		let archetype = &archetypes[self.archetype_id];

		// 加入实体读
		self.archetype_component_access.add_read(archetype.entity_archetype_component_id());

		self.matchs = 
			self.fetch_state.matches_archetype(archetype) &&
			self.filter_state.matches_archetype(archetype);
		
		if self.matchs {
			self.fetch_state.update_archetype_component_access(archetype, &mut self.archetype_component_access);
        	self.filter_state.update_archetype_component_access(archetype, &mut self.archetype_component_access);
		
			unsafe{ self.fetch_fetch.set_archetype(&self.fetch_state, archetype, world)};
			unsafe{ self.filter_fetch.set_archetype(&self.filter_state, archetype, world)};

			self.fetch_state.init_archetype::<A>(world);
			self.filter_state.init_archetype::<A>(world);

		}
	}

    #[inline]
    pub fn get<'w>(
        &self,
        world: &'w WorldInner,
        entity: Id<A>,
    ) -> Option<<Q::Fetch as Fetch<'w>>::Item>
    where
        Q::Fetch: ReadOnlyFetch,
    {
        // SAFE: query is read only
        unsafe { self.get_unchecked_inner(world, entity) }
    }

    #[inline]
    pub fn get_mut<'w>(
        &mut self,
        world: &'w mut WorldInner,
        entity: Id<A>,
    ) -> Option<<Q::Fetch as Fetch<'w>>::Item> {
        // SAFE: query has unique world access
        unsafe { self.get_unchecked_inner(world, entity) }
    }

	#[allow(mutable_transmutes)]
    pub unsafe fn get_unchecked<'w>(
        &self,
        _world: &'w WorldInner,
        entity: Id<A>,
    ) -> <Q::Fetch as Fetch<'w>>::Item {
		// let last_change_tick = world.last_change_tick();
        // let change_tick = world.read_change_tick();
		
        std::mem::transmute::<&Q::Fetch, &mut Q::Fetch>(&self.fetch_fetch).archetype_fetch_unchecked(entity.0)
    }
    /// # Safety
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    #[inline]
    pub unsafe fn get_unchecked_inner<'w>(
        &self,
        world: &'w WorldInner,
        entity: Id<A>,
    ) -> Option<<Q::Fetch as Fetch<'w>>::Item> {
        // self.validate_world_and_update_archetypes(world);
        self.get_unchecked_manual(
            world,
            entity,
            world.last_change_tick(),
            world.read_change_tick(),
        )
    }

    /// # Safety
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
	#[allow(mutable_transmutes)]
    pub unsafe fn get_unchecked_manual<'w>(
        &self,
        _world: &'w WorldInner,
        entity: Id<A>,
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> Option<<Q::Fetch as Fetch<'w>>::Item> {
		if !self.matchs {
			return None;
		}
        // let location = world
        //     .entities
        //     .get(entity)
        //     .ok_or(QueryEntityError::NoSuchEntity)?;

        // let archetype = &world.archetypes[entity.archetype_id()];
		
        if std::mem::transmute::<&F::Fetch, &mut F::Fetch>(&self.filter_fetch).archetype_filter_fetch(entity.0) {
            std::mem::transmute::<&Q::Fetch, &mut Q::Fetch>(&self.fetch_fetch).archetype_fetch(entity.0)
        } else {
            None
        }
    }

    #[inline]
    pub fn iter<'w, 's>(&'s mut self, world: &'w WorldInner) -> QueryIter<'w, 's, A, Q, F>
    where
        Q::Fetch: ReadOnlyFetch,
    {
        // SAFE: query is read only
        unsafe { self.iter_unchecked(world) }
    }

    #[inline]
    pub fn iter_mut<'w, 's>(&'s mut self, world: &'w mut WorldInner) -> QueryIter<'w, 's, A, Q, F> {
        // SAFE: query has unique world access
        unsafe { self.iter_unchecked(world) }
    }

	// #[inline]
    // pub async fn par_for_each<
    //     'w,
    //     's,
    //     FN: Fn(<Q::Fetch as Fetch<'s>>::Item) + Send + Sync + Clone,
    // >(
    //     &'s mut self,
    //     world: &'w World,
    //     batch_size: usize,
    //     func: FN,
	// 	last_change_tick: u32,
    //     change_tick: u32,
    // ) {
    //     // SAFETY: query is read only
    //     unsafe {
    //         // self.update_archetypes(world);
	// 		let fetch = &mut self.fetch_fetch;
		

	// 		// let mut entity = EntityFetch::init(world,
	// 		//     &query_state.entity_state);
	// 		// fetch.setting(world, last_change_tick, change_tick);
	// 		// query_state.filter_fetch.setting(world, last_change_tick, change_tick);
	
	// 		let filter = & self.filter_fetch;
	// 		let iter = match filter.main_fetch(&self.filter_state, last_change_tick, change_tick) {
	// 			Some(iter) => {
	// 				let (value, mut next) = (iter.value,iter.next);
	// 				let mut iter1 = EntityIter(Vec::new(), value);
	// 				loop {
	// 					match next {
	// 						Some(r) => {
	// 							let r = Box::into_inner(r);
	// 							let (value, next1) = (r.value,r.next);
	// 							iter1.0.push(value);
	// 							next = next1;
	// 						},
	// 						None => break
	// 					}
	// 				}
	// 				Some(iter1)
	// 			},
	// 			None => None,
	// 		};
	// 		let all_entities: pi_slotmap::dense::Keys<'s, LocalVersion, ()> = std::mem::transmute(world.archetypes()[self.archetype_id].entities.keys());



	// 		if !self.matchs {
	// 			return;
	// 		}
			
	// 		if let Some(iter) = &mut iter {
	// 			loop {
	// 				let entity = iter.next()?;
	
	// 				if !self.filter.archetype_filter_fetch(entity) {
	// 					continue;
	// 				}
	
	// 				let item = self.fetch.archetype_fetch(entity);
	// 				if let None = item {
	// 					continue;
	// 				}
	// 				return item;
	// 			}
	// 		} else {
	// 			loop {
	// 				let entity = self.all_entities_iter.next()?;
	
	// 				if !self.filter.archetype_filter_fetch(entity) {
	// 					continue;
	// 				}
	
	// 				let item = self.fetch.archetype_fetch(entity);
	// 				if let None = item {
	// 					continue;
	// 				}
	// 				return item;
	// 			}
	// 		}
    //     }
    // }

	pub fn setting<'w, 's>(
        &'s mut self,
        world: &'w WorldInner,
        last_change_tick: u32,
        change_tick: u32,
    ) {
        unsafe {
			self.fetch_fetch.setting(world, last_change_tick, change_tick);
			self.filter_fetch.setting(world, last_change_tick, change_tick);
		}
    }

    /// # Safety
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    #[inline]
    pub unsafe fn iter_unchecked<'w, 's>(
        &'s mut self,
        world: &'w WorldInner,
    ) -> QueryIter<'w, 's, A, Q, F> {
        // self.validate_world_and_update_archetypes(world);
        self.iter_unchecked_manual(world, world.last_change_tick(), world.read_change_tick())
    }

    /// # Safety
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    /// This does not validate that `world.id()` matches `self.world_id`. Calling this on a `world`
    /// with a mismatched WorldId is unsafe.
    #[inline]
    pub(crate) unsafe fn iter_unchecked_manual<'w, 's>(
        &'s self,
        world: &'w WorldInner,
        last_change_tick: u32,
        change_tick: u32,
    ) -> QueryIter<'w, 's, A, Q, F> {
        QueryIter::new(world, &mut*(self as *const Self as usize as *mut Self), last_change_tick, change_tick)
    }

    // #[inline]
    // pub fn for_each<'w>(
    //     &mut self,
    //     world: &'w WorldInner,
    //     func: impl FnMut(<Q::Fetch as Fetch<'w>>::Item),
    // ) where
    //     Q::Fetch: ReadOnlyFetch,
    // {
    //     // SAFE: query is read only
    //     unsafe {
    //         self.for_each_unchecked(world, func);
    //     }
    // }

    // #[inline]
    // pub fn for_each_mut<'w>(
    //     &mut self,
    //     world: &'w mut WorldInner,
    //     func: impl FnMut(<Q::Fetch as Fetch<'w>>::Item),
    // ) {
    //     // SAFE: query has unique world access
    //     unsafe {
    //         self.for_each_unchecked(world, func);
    //     }
    // }

    // /// # Safety
    // /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    // /// have unique access to the components they query.
    // #[inline]
    // pub unsafe fn for_each_unchecked<'w>(
    //     &mut self,
    //     world: &'w WorldInner,
    //     func: impl FnMut(<Q::Fetch as Fetch<'w>>::Item),
    // ) {
    //     self.validate_world_and_update_archetypes(world);
    //     self.for_each_unchecked_manual(
    //         world,
    //         func,
    //         world.last_change_tick(),
    //         world.read_change_tick(),
    //     );
    // }

    // #[inline]
    // pub fn par_for_each<'w>(
    //     &mut self,
    //     world: &'w WorldInner,
    //     task_pool: &TaskPool,
    //     batch_size: usize,
    //     func: impl Fn(<Q::Fetch as Fetch<'w>>::Item) + Send + Sync + Clone,
    // ) where
    //     Q::Fetch: ReadOnlyFetch,
    // {
    //     // SAFE: query is read only
    //     unsafe {
    //         self.par_for_each_unchecked(world, task_pool, batch_size, func);
    //     }
    // }

    // #[inline]
    // pub fn par_for_each_mut<'w>(
    //     &mut self,
    //     world: &'w mut WorldInner,
    //     task_pool: &TaskPool,
    //     batch_size: usize,
    //     func: impl Fn(<Q::Fetch as Fetch<'w>>::Item) + Send + Sync + Clone,
    // ) {
    //     // SAFE: query has unique world access
    //     unsafe {
    //         self.par_for_each_unchecked(world, task_pool, batch_size, func);
    //     }
    // }

    // /// # Safety
    // /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    // /// have unique access to the components they query.
    // #[inline]
    // pub unsafe fn par_for_each_unchecked<'w>(
    //     &mut self,
    //     world: &'w WorldInner,
    //     task_pool: &TaskPool,
    //     batch_size: usize,
    //     func: impl Fn(<Q::Fetch as Fetch<'w>>::Item) + Send + Sync + Clone,
    // ) {
    //     self.validate_world_and_update_archetypes(world);
    //     self.par_for_each_unchecked_manual(
    //         world,
    //         task_pool,
    //         batch_size,
    //         func,
    //         world.last_change_tick(),
    //         world.read_change_tick(),
    //     );
    // }

    // /// # Safety
    // /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    // /// have unique access to the components they query.
    // /// This does not validate that `world.id()` matches `self.world_id`. Calling this on a `world`
    // /// with a mismatched WorldId is unsafe.
    // pub(crate) unsafe fn for_each_unchecked_manual<'w, 's>(
    //     &'s self,
    //     world: &'w WorldInner,
    //     mut func: impl FnMut(<Q::Fetch as Fetch<'w>>::Item),
    //     last_change_tick: u32,
    //     change_tick: u32,
    // ) {
    //     let mut fetch =
    //         <Q::Fetch as Fetch>::init(world, &self.fetch_state, last_change_tick, change_tick);
    //     let mut filter =
    //         <F::Fetch as Fetch>::init(world, &self.filter_state, last_change_tick, change_tick);
    //     if fetch.is_dense() && filter.is_dense() {
    //         let tables = &world.storages().tables;
    //         for table_id in self.matched_table_ids.iter() {
    //             let table = &tables[*table_id];
    //             fetch.set_table(&self.fetch_state, table);
    //             filter.set_table(&self.filter_state, table);

    //             for table_index in 0..table.len() {
    //                 if !filter.table_filter_fetch(table_index) {
    //                     continue;
    //                 }
    //                 let item = fetch.table_fetch(table_index);
    //                 func(item);
    //             }
    //         }
    //     } else {
    //         let archetypes = &world.archetypes;
    //         let tables = &world.storages().tables;
    //         for archetype_id in self.matched_archetype_ids.iter() {
    //             let archetype = &archetypes[*archetype_id];
    //             fetch.set_archetype(&self.fetch_state, archetype, tables);
    //             filter.set_archetype(&self.filter_state, archetype, tables);

    //             for archetype_index in 0..archetype.len() {
    //                 if !filter.archetype_filter_fetch(archetype_index) {
    //                     continue;
    //                 }
    //                 func(fetch.archetype_fetch(archetype_index));
    //             }
    //         }
    //     }
    // }

    // /// # Safety
    // /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    // /// have unique access to the components they query.
    // /// This does not validate that `world.id()` matches `self.world_id`. Calling this on a `world`
    // /// with a mismatched WorldId is unsafe.
    // pub unsafe fn par_for_each_unchecked_manual<'w, 's>(
    //     &'s self,
    //     world: &'w WorldInner,
    //     task_pool: &TaskPool,
    //     batch_size: usize,
    //     func: impl Fn(<Q::Fetch as Fetch<'w>>::Item) + Send + Sync + Clone,
    //     last_change_tick: u32,
    //     change_tick: u32,
    // ) {
    //     task_pool.scope(|scope| {
    //         let fetch =
    //             <Q::Fetch as Fetch>::init(world, &self.fetch_state, last_change_tick, change_tick);
    //         let filter =
    //             <F::Fetch as Fetch>::init(world, &self.filter_state, last_change_tick, change_tick);

    //         if fetch.is_dense() && filter.is_dense() {
    //             let tables = &world.storages().tables;
    //             for table_id in self.matched_table_ids.iter() {
    //                 let table = &tables[*table_id];
    //                 let mut offset = 0;
    //                 while offset < table.len() {
    //                     let func = func.clone();
    //                     scope.spawn(async move {
    //                         let mut fetch = <Q::Fetch as Fetch>::init(
    //                             world,
    //                             &self.fetch_state,
    //                             last_change_tick,
    //                             change_tick,
    //                         );
    //                         let mut filter = <F::Fetch as Fetch>::init(
    //                             world,
    //                             &self.filter_state,
    //                             last_change_tick,
    //                             change_tick,
    //                         );
    //                         let tables = &world.storages().tables;
    //                         let table = &tables[*table_id];
    //                         fetch.set_table(&self.fetch_state, table);
    //                         filter.set_table(&self.filter_state, table);
    //                         let len = batch_size.min(table.len() - offset);
    //                         for table_index in offset..offset + len {
    //                             if !filter.table_filter_fetch(table_index) {
    //                                 continue;
    //                             }
    //                             let item = fetch.table_fetch(table_index);
    //                             func(item);
    //                         }
    //                     });
    //                     offset += batch_size;
    //                 }
    //             }
    //         } else {
    //             let archetypes = &world.archetypes;
    //             for archetype_id in self.matched_archetype_ids.iter() {
    //                 let mut offset = 0;
    //                 let archetype = &archetypes[*archetype_id];
    //                 while offset < archetype.len() {
    //                     let func = func.clone();
    //                     scope.spawn(async move {
    //                         let mut fetch = <Q::Fetch as Fetch>::init(
    //                             world,
    //                             &self.fetch_state,
    //                             last_change_tick,
    //                             change_tick,
    //                         );
    //                         let mut filter = <F::Fetch as Fetch>::init(
    //                             world,
    //                             &self.filter_state,
    //                             last_change_tick,
    //                             change_tick,
    //                         );
    //                         let tables = &world.storages().tables;
    //                         let archetype = &world.archetypes[*archetype_id];
    //                         fetch.set_archetype(&self.fetch_state, archetype, tables);
    //                         filter.set_archetype(&self.filter_state, archetype, tables);

    //                         let len = batch_size.min(archetype.len() - offset);
    //                         for archetype_index in offset..offset + len {
    //                             if !filter.archetype_filter_fetch(archetype_index) {
    //                                 continue;
    //                             }
    //                             func(fetch.archetype_fetch(archetype_index));
    //                         }
    //                     });
    //                     offset += batch_size;
    //                 }
    //             }
    //         }
    //     });
    // }

	pub fn apply(&self, world: &mut World) {
		self.filter_state.apply(world);
	}
}

/// An error that occurs when retrieving a specific [Entity]'s query result.
#[derive(Error, Debug)]
pub enum QueryEntityError {
    #[error("The given entity does not have the requested component.")]
    QueryDoesNotMatch,
    #[error("The requested entity does not exist.")]
    NoSuchEntity,
}
