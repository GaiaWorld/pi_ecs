use crate::{
    entity::Entity,
    query::{
        Fetch, FilterFetch, QueryIter, QueryState, ReadOnlyFetch, WorldQuery,
    },
	sys::param::interface::{SystemParam, SystemParamFetch, SystemParamState, assert_component_access_compatibility},
	sys::system::interface::SystemState,
	world::World, archetype::ArchetypeIdent,
};
use std::marker::PhantomData;

/// Provides scoped access to a [`World`] according to a given [`WorldQuery`] and query filter.
pub struct Query<'w, A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery = ()>
where
    F::Fetch: FilterFetch,
{
    pub(crate) world: &'w World,
    pub(crate) state: &'w QueryState<A, Q, F>,
    pub(crate) last_change_tick: u32,
    pub(crate) change_tick: u32,
}

impl<'w, A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery> Query<'w, A, Q, F>
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
    pub(crate) unsafe fn new(
        world: &'w World,
        state: &'w QueryState<A, Q, F>,
        last_change_tick: u32,
        change_tick: u32,
    ) -> Self {
        Self {
            world,
            state,
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
                .iter_unchecked_manual(self.world, self.last_change_tick, self.change_tick)
        }
    }

    /// Returns an [`Iterator`] over the query results.
    #[inline]
    pub fn iter_mut(&mut self) -> QueryIter<'_, '_, A, Q, F> {
        // SAFE: system runs without conflicts with other systems.
        // same-system queries have runtime borrow checks when they conflict
        unsafe {
            self.state
                .iter_unchecked_manual(self.world, self.last_change_tick, self.change_tick)
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
            .iter_unchecked_manual(self.world, self.last_change_tick, self.change_tick)
    }

    /// Gets the query result for the given [`Entity`].
    ///
    /// This can only be called for read-only queries, see [`Self::get_mut`] for write-queries.
    #[inline]
    pub fn get(&self, entity: Entity) -> Option<<Q::Fetch as Fetch>::Item>
    where
        Q::Fetch: ReadOnlyFetch,
    {
        // SAFE: system runs without conflicts with other systems.
        // same-system queries have runtime borrow checks when they conflict
        unsafe {
            self.state.get_unchecked_manual(
                self.world,
                entity,
                self.last_change_tick,
                self.change_tick,
            )
        }
    }

    /// Gets the query result for the given [`Entity`].
    #[inline]
    pub fn get_mut(
        &mut self,
        entity: Entity,
    ) -> Option<<Q::Fetch as Fetch>::Item> {
        // SAFE: system runs without conflicts with other systems.
        // same-system queries have runtime borrow checks when they conflict
        unsafe {
            self.state.get_unchecked_manual(
                self.world,
                entity,
                self.last_change_tick,
                self.change_tick,
            )
        }
    }

    /// Gets the query result for the given [`Entity`].
    ///
    /// # Safety
    ///
    /// This function makes it possible to violate Rust's aliasing guarantees. You must make sure
    /// this call does not result in multiple mutable references to the same component
    #[inline]
    pub unsafe fn get_unchecked(
        &self,
        entity: Entity,
    ) -> Option<<Q::Fetch as Fetch>::Item> {
        // SEMI-SAFE: system runs without conflicts with other systems.
        // same-system queries have runtime borrow checks when they conflict
        self.state
            .get_unchecked_manual(self.world, entity, self.last_change_tick, self.change_tick)
    }
}

pub struct QueryFetch<Q, F>(PhantomData<(Q, F)>);

impl<'a, A: ArchetypeIdent, Q: WorldQuery + 'static, F: WorldQuery + 'static> SystemParam for Query<'a, A, Q, F>
where
    F::Fetch: FilterFetch,
{
    type Fetch = QueryState<A, Q, F>;
}

// SAFE: Relevant query ComponentId and ArchetypeComponentId access is applied to SystemState. If
// this QueryState conflicts with any prior access, a panic will occur.
unsafe impl<A: ArchetypeIdent, Q: WorldQuery + 'static, F: WorldQuery + 'static> SystemParamState for QueryState<A, Q, F>
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
        state
    }

    fn default_config() {}
}

impl<'a, A: ArchetypeIdent, Q: WorldQuery + 'static, F: WorldQuery + 'static> SystemParamFetch<'a> for QueryState<A, Q, F>
where
    F::Fetch: FilterFetch,
{
    type Item = Query<'a, A, Q, F>;

    #[inline]
    unsafe fn get_param(
        state: &'a mut Self,
        system_state: &'a SystemState,
        world: &'a World,
        change_tick: u32,
    ) -> Self::Item {
        Query::new(world, state, system_state.last_change_tick, change_tick)
    }
}
