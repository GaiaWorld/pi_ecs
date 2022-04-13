use std::sync::Arc;

use crate::{
    entity::Entity,
    query::{
        Fetch, FilterFetch, QueryIter, QueryState, ReadOnlyFetch, WorldQuery,
    },
	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState, assert_component_access_compatibility},
	sys::system::interface::SystemState,
	world::World, archetype::ArchetypeIdent, WorldInner,
};
use std::marker::PhantomData;

/// Provides scoped access to a [`World`] according to a given [`WorldQuery`] and query filter.
pub struct Query<A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery = ()>
where
    F::Fetch: FilterFetch,
{
	pub(crate) _world: World, // 抓住World， 因为Query可能在异步块中，需要保证WorldInner不被释放
	pub(crate) world_ref: &'static WorldInner,
    pub(crate) state: Arc<QueryState<A, Q, F>>, // 如果sys是异步函数，在异步函数没有执行完时，不能删除sys，否则可能造成未定义行为， TODO
    pub(crate) last_change_tick: u32,
    pub(crate) change_tick: u32,
}

impl<A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery> Query<A, Q, F>
where
    F::Fetch: FilterFetch,
{
    /// Creates a new query.
    ///
    /// # Safety
    ///
    /// This will create a query that could violate memory safety rules. Make sure that this is only
    /// called in ways that ensure the queries have unique mutable access.
    #[inline]
    pub(crate) unsafe fn new<'w>(
        world: &'w World,
        state: &Arc<QueryState<A, Q, F>>,
        last_change_tick: u32,
        change_tick: u32,
    ) -> Self {
        Self {
            _world: world.clone(),
			world_ref: std::mem::transmute(&**world),
            state: state.clone(),
            last_change_tick,
            change_tick,
        }
    }

    /// Returns an [`Iterator`] over the query results.
    ///
    /// This can only be called for read-only queries, see [`Self::iter_mut`] for write-queries.
    #[inline]
    pub fn iter(&self) -> QueryIter<'_, '_, A, Q, F>
    where
        Q::Fetch: ReadOnlyFetch,
    {
        // SAFE: system runs without conflicts with other systems.
        // same-system queries have runtime borrow checks when they conflict
        unsafe {
            self.state
                .iter_unchecked_manual(self.world_ref, self.last_change_tick, self.change_tick)
        }
    }

    /// Returns an [`Iterator`] over the query results.
    #[inline]
    pub fn iter_mut(&mut self) -> QueryIter<'_, '_, A, Q, F> {
        // SAFE: system runs without conflicts with other systems.
        // same-system queries have runtime borrow checks when they conflict
        unsafe {
            self.state
                .iter_unchecked_manual(self.world_ref, self.last_change_tick, self.change_tick)
        }
    }

    /// Returns an [`Iterator`] over the query results.
    ///
    /// # Safety
    ///
    /// This function makes it possible to violate Rust's aliasing guarantees. You must make sure
    /// this call does not result in multiple mutable references to the same component
    #[inline]
    pub unsafe fn iter_unsafe(&self) -> QueryIter<'_, '_, A, Q, F> {
        // SEMI-SAFE: system runs without conflicts with other systems.
        // same-system queries have runtime borrow checks when they conflict
        self.state
            .iter_unchecked_manual(self.world_ref, self.last_change_tick, self.change_tick)
    }

    /// Gets the query result for the given [`Entity`].
    ///
    /// This can only be called for read-only queries, see [`Self::get_mut`] for write-queries.
    #[inline]
    pub fn get<'s>(&'s self, entity: Entity) -> Option<<Q::Fetch as Fetch<'s>>::Item>
    // where
    //     Q::Fetch: ReadOnlyFetch,
    {
        // SAFE: system runs without conflicts with other systems.
        // same-system queries have runtime borrow checks when they conflict
        unsafe {
            self.state.get_unchecked_manual(
				self.world_ref,
                entity,
                self.last_change_tick,
                self.change_tick,
            )
        }
    }

	/// Gets the query result for the given [`Entity`].
    ///
    /// This can only be called for read-only queries, see [`Self::get_mut`] for write-queries.
    #[inline]
    pub fn get_unchecked<'s>(&'s self, entity: Entity) -> <Q::Fetch as Fetch<'s>>::Item
    // where
    //     Q::Fetch: ReadOnlyFetch,
    {
        // SAFE: system runs without conflicts with other systems.
        // same-system queries have runtime borrow checks when they conflict
        unsafe {
            self.state.get_unchecked(
				self.world_ref,
                entity,
            )
        }
	}

	#[inline]
    pub fn get_unchecked_mut<'s>(&'s mut self, entity: Entity) -> <Q::Fetch as Fetch>::Item
    // where
    //     Q::Fetch: ReadOnlyFetch,
    {
        // SAFE: system runs without conflicts with other systems.
        // same-system queries have runtime borrow checks when they conflict
        unsafe {
            self.state.get_unchecked(
				self.world_ref,
                entity,
            )
        }
	}

    /// Gets the query result for the given [`Entity`].
    #[inline]
    pub fn get_mut<'s>(
        &'s mut self,
        entity: Entity,
    ) -> Option<<Q::Fetch as Fetch<'s>>::Item> {
        // SAFE: system runs without conflicts with other systems.
        // same-system queries have runtime borrow checks when they conflict
        unsafe {
            self.state.get_unchecked_manual(
                self.world_ref,
                entity,
                self.last_change_tick,
                self.change_tick,
            )
        }
    }

    // /// Gets the query result for the given [`Entity`].
    // ///
    // /// # Safety
    // ///
    // /// This function makes it possible to violate Rust's aliasing guarantees. You must make sure
    // /// this call does not result in multiple mutable references to the same component
    // #[inline]
    // pub unsafe fn get_unchecked(
    //     &self,
    //     entity: Entity,
    // ) -> Option<<Q::Fetch as Fetch>::Item> {
    //     // SEMI-SAFE: system runs without conflicts with other systems.
    //     // same-system queries have runtime borrow checks when they conflict
	// 	self.state
    //         .get_unchecked_manual(self.world_ref, entity, self.last_change_tick, self.change_tick)
    //     // self.state
    //     //     .get_unchecked_manual(self.world_ref, entity)
    // }
}

pub struct QueryFetch<Q, F>(PhantomData<(Q, F)>);

impl<A: ArchetypeIdent, Q: WorldQuery + 'static, F: WorldQuery + 'static> SystemParam for Query< A, Q, F>
where
    F::Fetch: FilterFetch,
{
    type Fetch = Arc<QueryState<A, Q, F>>;
}

// SAFE: Relevant query ComponentId and ArchetypeComponentId access is applied to SystemState. If
// this QueryState conflicts with any prior access, a panic will occur.
unsafe impl<A: ArchetypeIdent, Q: WorldQuery + 'static, F: WorldQuery + 'static> SystemParamState for Arc<QueryState<A, Q, F>>
where
    F::Fetch: FilterFetch,
{
    type Config = ();

    fn init(world: &mut World, system_state: &mut SystemState, _config: Self::Config) -> Self {
		// 创建查询状态
        let state = QueryState::new(world);
		// 检查system内部，组件访问是否冲突（无法在system中的两个查询中，同时使用组件的读和写查询）
        assert_component_access_compatibility(
            &system_state.name,
            std::any::type_name::<Q>(),
            std::any::type_name::<F>(),
            &system_state.component_access_set,
            &state.component_access,
            world,
        );
		// 将查询访问的组件集添加到系统访问的组件集中
        system_state
            .component_access_set
            .add(state.component_access.clone());
		// 将查询访问的原型组件放入系统的原型组件集中（用于检查系统与系统的访问组件是否冲突，访问不同原型的同类型组件是允许的）
        system_state
            .archetype_component_access
            .extend(&state.archetype_component_access);
        Arc::new(state)
    }

    fn default_config() {}

	// 
	fn apply(&mut self, world: &mut World) {
		(**self).apply(world)
	}
}

impl<'w, 's, A: ArchetypeIdent, Q: WorldQuery + 'static, F: WorldQuery + 'static> SystemParamFetch<'w, 's> for Arc<QueryState<A, Q, F>>
where
    F::Fetch: FilterFetch,
{
    type Item = Query<A, Q, F>;

    #[inline]
    unsafe fn get_param(
        state: &'s mut Self,
        system_state: &SystemState,
        world: &'w World,
        change_tick: u32,
    ) -> Self::Item {
        Query::new(world, state, system_state.last_change_tick, change_tick)
    }
}
