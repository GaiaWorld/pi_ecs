use std::marker::PhantomData;
use std::any::TypeId;

use crate::{
    archetype::{ArchetypeId, ArchetypeIdent, ArchetypeComponentId},
    component::ComponentId,
    entity::Entity,
    query::{
        Fetch, FetchState, FilterFetch, Access, FilteredAccess, QueryIter, ReadOnlyFetch,
        WorldQuery,EntityState
    },
	storage::Offset,
    world::{World, WorldId},
};
use thiserror::Error;

pub struct QueryState<A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery = ()>
where
    F::Fetch: FilterFetch,
{
    world_id: WorldId,
	pub(crate) archetype_id: ArchetypeId, // A对应的实体id
	pub(crate) component_access: FilteredAccess<ComponentId>,
    pub(crate) archetype_component_access: Access<ArchetypeComponentId>,
    pub(crate) fetch_state: Q::State,
    pub(crate) filter_state: F::State,
	pub(crate) fetch_fetch: Q::Fetch,
	pub(crate) filter_fetch: F::Fetch,

	pub(crate) entity_state: EntityState,

	pub(crate) matchs: bool,

	mark: PhantomData<A>,
}

impl<A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery> QueryState<A, Q, F>
where
    F::Fetch: FilterFetch,
{
    pub fn new(world: &World) -> Self {
		let archetype_id = match world.archetypes().get_id_by_ident(TypeId::of::<A>()) {
			Some(r) => r.clone(),
			None => panic!(),
		};
        let fetch_state = <Q::State as FetchState>::init(world);
        let filter_state = <F::State as FetchState>::init(world);
		let entity_state = <EntityState as FetchState>::init(world) ;
		let fetch_fetch =
            unsafe{ <Q::Fetch as Fetch>::init(world, &fetch_state) };
        let filter_fetch =
			unsafe{ <F::Fetch as Fetch>::init(world, &filter_state)};

        let mut component_access = Default::default();
		fetch_state.update_component_access(&mut component_access);
        filter_state.update_component_access(&mut component_access);

        let mut state = Self {
			matchs: false,
            world_id: world.id(),
			archetype_id,
            fetch_state,
            filter_state,
			fetch_fetch,
			filter_fetch,
			entity_state,
			component_access: component_access,
            archetype_component_access: Default::default(),
			mark: PhantomData
        };
		state.validate_world_and_update_archetypes(world);
        state
    }

	pub fn validate_world_and_update_archetypes(&mut self, world: &World) {
        if world.id() != self.world_id {
            panic!("Attempted to use {} with a mismatched World. QueryStates can only be used with the World they were created from.",
                std::any::type_name::<Self>());
        }
        let archetypes = world.archetypes();
		let archetype = &archetypes[self.archetype_id];

		self.fetch_state.update_archetype_component_access(archetype, &mut self.archetype_component_access);
        self.filter_state.update_archetype_component_access(archetype, &mut self.archetype_component_access);

		self.matchs = 
			self.fetch_state.matches_archetype(archetype, world) &&
			self.filter_state.matches_archetype(archetype, world) &&
			self.entity_state.matches_archetype(archetype, world);
		
		if self.matchs {
			unsafe{ self.fetch_fetch.set_archetype(&self.fetch_state, archetype, world)};
			unsafe{ self.filter_fetch.set_archetype(&self.filter_state, archetype, world)};
		}
	}

    #[inline]
    pub fn get<'w>(
        &self,
        world: &'w World,
        entity: Entity,
    ) -> Option<<Q::Fetch as Fetch>::Item>
    where
        Q::Fetch: ReadOnlyFetch,
    {
        // SAFE: query is read only
        unsafe { self.get_unchecked(world, entity) }
    }

    #[inline]
    pub fn get_mut<'w>(
        &mut self,
        world: &'w mut World,
        entity: Entity,
    ) -> Option<<Q::Fetch as Fetch>::Item> {
        // SAFE: query has unique world access
        unsafe { self.get_unchecked(world, entity) }
    }

    /// # Safety
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    #[inline]
    pub unsafe fn get_unchecked<'w>(
        &self,
        world: &'w World,
        entity: Entity,
    ) -> Option<<Q::Fetch as Fetch>::Item> {
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
        _world: &'w World,
        entity: Entity,
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> Option<<Q::Fetch as Fetch>::Item> {
		if !self.matchs || entity.archetype_id() != self.archetype_id {
			return None;
		}
        // let location = world
        //     .entities
        //     .get(entity)
        //     .ok_or(QueryEntityError::NoSuchEntity)?;

        // let archetype = &world.archetypes[entity.archetype_id()];
        if std::mem::transmute::<&F::Fetch, &mut F::Fetch>(&self.filter_fetch).archetype_filter_fetch(entity.local().offset()) {
            std::mem::transmute::<&Q::Fetch, &mut Q::Fetch>(&self.fetch_fetch).archetype_fetch(entity.local().offset())
        } else {
            None
        }
    }

    #[inline]
    pub fn iter<'w, 's>(&'s mut self, world: &'w World) -> QueryIter<'w, 's, A, Q, F>
    where
        Q::Fetch: ReadOnlyFetch,
    {
        // SAFE: query is read only
        unsafe { self.iter_unchecked(world) }
    }

    #[inline]
    pub fn iter_mut<'w, 's>(&'s mut self, world: &'w mut World) -> QueryIter<'w, 's, A, Q, F> {
        // SAFE: query has unique world access
        unsafe { self.iter_unchecked(world) }
    }

    /// # Safety
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    #[inline]
    pub unsafe fn iter_unchecked<'w, 's>(
        &'s mut self,
        world: &'w World,
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
        world: &'w World,
        last_change_tick: u32,
        change_tick: u32,
    ) -> QueryIter<'w, 's, A, Q, F> {
        QueryIter::new(world, self, last_change_tick, change_tick)
    }

    // #[inline]
    // pub fn for_each<'w>(
    //     &mut self,
    //     world: &'w World,
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
    //     world: &'w mut World,
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
    //     world: &'w World,
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
    //     world: &'w World,
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
    //     world: &'w mut World,
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
    //     world: &'w World,
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
    //     world: &'w World,
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
    //     world: &'w World,
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
}

/// An error that occurs when retrieving a specific [Entity]'s query result.
#[derive(Error, Debug)]
pub enum QueryEntityError {
    #[error("The given entity does not have the requested component.")]
    QueryDoesNotMatch,
    #[error("The requested entity does not exist.")]
    NoSuchEntity,
}
