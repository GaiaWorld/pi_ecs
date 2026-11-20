//! 查询配对芯。
#[cfg(all(test, feature = "rc"))]
mod registration_tests {
    use super::*;
    use crate::{
        component::{Component, PlainAccess, StoragePolicy},
        query::{FetchStateResult, QueryDataDeclaration, QueryFilterDeclaration},
        query_fetch::*,
    };
    struct Value;
    impl Component for Value {
        type Access = PlainAccess;
    }
    #[test]
    fn host_query_creation_reuses_registration_contract() {
        let mut world = World::new().unwrap();
        assert!(world.query::<Tracked>().is_err());
        world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let mut filtered = world.query_filtered::<Tracked, Filter>().unwrap();
        let mut plain = world.query::<Tracked>().unwrap();
        assert!(filtered.declaration().unwrap().filter.tracking[0].changed);
        assert!(plain.declaration().unwrap().filter.tracking.is_empty());
        filtered.unregister_tracking(&mut world).unwrap();
        plain.unregister_tracking(&mut world).unwrap();
    }

    #[test]
    fn host_query_checks_world_and_reports_unpublished_entities() {
        let mut world = World::new().unwrap();
        let mut foreign = World::new().unwrap();
        world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        foreign.finish_registration().unwrap();
        let archetype = crate::entity::ArchetypeId {
            table: 0,
            variant: 0,
        };
        let storage = world.storage.as_mut().unwrap();
        let row = storage.table_mut(0).unwrap().claim_row().unwrap();
        let live = world
            .entities
            .borrow_mut()
            .allocate(
                crate::entity::AllocationDomain(0),
                crate::entity::EntityLocation::from_parts(0, 0, row.index(), 0, 0),
            )
            .unwrap();
        storage.append_member(archetype, live, row).unwrap();
        storage
            .table_mut(0)
            .unwrap()
            .publish_row(row, live)
            .unwrap();
        let mut state = world
            .query_filtered::<(crate::query_data::Has<Value>,), Filter>()
            .unwrap();
        assert!(unsafe { state.query(&mut foreign) }.is_err());
        let tick = world.read_change_tick();
        let query = unsafe {
            state.query_with_window(
                &mut world,
                crate::change_detection::ChangeWindow {
                    last_run: None,
                    this_run: tick,
                },
            )
        }
        .unwrap();
        let entity = Entity::default();
        let error = query.get(entity).unwrap_err();
        assert_eq!(*error.current_context(), crate::error::ErrorKind::NotFound);
        assert_eq!(query.get(live).unwrap(), (false,));
        assert_eq!(query.get_many([live, live]).unwrap(), [(false,), (false,)]);
        let batch = query.get_many([live, entity]).unwrap_err();
        assert_eq!(
            batch
                .downcast_ref::<crate::query_batch::BatchQueryContext>()
                .unwrap()
                .index,
            1
        );
        assert_eq!(query.iter().unwrap().collect::<Vec<_>>(), vec![(false,)]);
        let mut state = world
            .query_filtered::<crate::query_data::Has<Value>, Filter>()
            .unwrap();
        let before = world.read_change_tick();
        assert!(unsafe { state.get_mut(&mut world, entity) }.is_err());
        assert_eq!(world.read_change_tick(), before);
        let mut query = unsafe { state.query_mut_since(&mut world, Some(before)) }.unwrap();
        assert!(query.this_run() > before);
        let aliased = query.get_many_mut([live, live]).unwrap_err();
        assert_eq!(
            *aliased.current_context(),
            crate::error::ErrorKind::Conflict
        );
        assert_eq!(query.get_many_mut([live]).unwrap(), [false]);
        assert!(!query.get_mut(live).unwrap());
        assert_eq!(query.iter_mut().unwrap().collect::<Vec<_>>(), vec![false]);
        assert!(!unsafe { state.get(&mut world, live) }.unwrap());
        assert!(!unsafe { state.get_mut(&mut world, live) }.unwrap());
        let mut query = unsafe { state.query_mut(&mut world) }.unwrap();
        assert!(!query.get_mut(live).unwrap());
        assert!(unsafe { state.query(&mut world) }
            .unwrap()
            .get(live)
            .is_ok());
        let leaf =
            crate::access::AccessTree::Leaf(state.declaration().unwrap().to_access().unwrap());
        let tick = world.read_change_tick();
        let window = crate::change_detection::ChangeWindow {
            last_run: None,
            this_run: tick,
        };
        world
            .schedule_scope(|world| {
                let version = world.layout_epoch;
                // SAFETY: this leaf is the exact registered descriptor and has no sibling.
                unsafe {
                    world.system_scope(&leaf, version, |access| -> Result<(), EcsError> {
                        let access = access.refresh_query(&mut state)?;
                        let mut lookup = access.bind_lookup(&mut state, window)?;
                        assert!(!lookup.get(live)?);
                        assert!(!lookup.get_mut(live)?);
                        Ok(())
                    })??;
                    world.system_scope(&leaf, version, |access| -> Result<(), EcsError> {
                        let query = access.bind_query(&mut state, window)?;
                        assert!(!query.get(live)?);
                        Ok(())
                    })??;
                    world.system_scope(&leaf, version, |access| {
                        let mut query = access.bind_query_mut(&mut state, window)?;
                        assert_eq!(query.this_run(), tick);
                        assert!(!query.get_mut(live)?);
                        Ok(())
                    })?
                }
            })
            .unwrap();
    }

    struct Tracked;
    struct TrackedFetch;

    // SAFETY: 测试抓取器只产出 ()，不访问 World 或业务值。
    unsafe impl<'w> crate::query_fetch::QueryFetch<'w, Tracked> for TrackedFetch {
        unsafe fn prepare(
            _: &crate::query_fetch::FetchAccess<'w>,
            _: FetchScope,
            _: &crate::component::ComponentId,
            _: &(),
        ) -> Result<Self, EcsError> {
            Ok(Self)
        }

        unsafe fn fetch_at(&mut self, _: &crate::query_fetch::FetchEntity) {}
    }

    // SAFETY: 此描述只声明 tick 需求，不访问业务值，Item 为 ()。
    unsafe impl QueryData for Tracked {
        type Item<'a, 'w> = ();
        type State = crate::component::ComponentId;
        type FetchState = ();
        type Fetch<'w> = TrackedFetch;
        fn init_state(world: &World) -> Result<Self::State, EcsError> {
            Ok(world.component_id::<Value>().unwrap())
        }
        fn declare_access(id: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
            Ok(QueryDataDeclaration {
                components: Box::new([]),
                tracking: vec![crate::tracking::ComponentTickDemand {
                    component: *id,
                    added: true,
                    changed: false,
                }]
                .into(),
                tick_reads: Box::new([]),
            })
        }
        fn init_fetch_state(
            _: &Self::State,
            _: FetchScope,
            _: &FetchBinding<'_>,
        ) -> Result<FetchStateResult<()>, EcsError> {
            Ok(FetchStateResult::Ready(()))
        }
    }
    struct Filter;
    // SAFETY: 过滤恒真，不读取值，只贡献本组件 changed 需求。
    unsafe impl QueryFilter for Filter {
        const IS_ARCHETYPAL: bool = true;
        type State = crate::component::ComponentId;
        type FetchState = ();
        type Fetch<'w> = NoFilterFetch;
        fn init_state(world: &World) -> Result<Self::State, EcsError> {
            Tracked::init_state(world)
        }
        fn declare_access(id: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
            Ok(QueryFilterDeclaration {
                expression: crate::query_filter::FilterExpression::All,
                tracking: vec![crate::tracking::ComponentTickDemand {
                    component: *id,
                    added: false,
                    changed: true,
                }]
                .into(),
                tick_reads: Box::new([]),
            })
        }
        fn init_fetch_state(
            _: &Self::State,
            _: FetchScope,
            _: &FetchBinding<'_>,
        ) -> Result<FetchStateResult<()>, EcsError> {
            Ok(FetchStateResult::Ready(()))
        }
        unsafe fn prepare<'w>(
            _: &FetchAccess<'w>,
            _: FetchScope,
            _: &Self::State,
            _: &(),
        ) -> Result<NoFilterFetch, EcsError> {
            Ok(NoFilterFetch)
        }
    }
    // Seam: QueryState 登记/注销到 World::apply_tracking 的完整需求链。
    #[test]
    fn rc_tracking_real_queries_keep_independent_global_demands() {
        let mut world = World::new().unwrap();
        let component = world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let mut first = QueryState::<Tracked, Filter>::new(&mut world).unwrap();
        let mut second = QueryState::<Tracked, Filter>::new(&mut world).unwrap();
        world.apply_tracking().unwrap();
        let enabled = world.read_change_tick();
        first.unregister_tracking(&mut world).unwrap();
        world.apply_tracking().unwrap();
        let demand = world.registry.component(component).unwrap().tracking;
        assert!(demand.added_tick && demand.changed_tick);
        assert_eq!(world.read_change_tick(), enabled);
        second.unregister_tracking(&mut world).unwrap();
        world.apply_tracking().unwrap();
        let demand = world.registry.component(component).unwrap().tracking;
        assert!(!demand.added_tick && !demand.changed_tick);
        assert_eq!(world.read_change_tick(), enabled);
        world.close().unwrap();
    }

    #[test]
    fn query_registration_stopped_cleanup_preserves_other_queries() {
        let mut world = World::new().unwrap();
        world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let mut state = QueryState::<Tracked, Filter>::new(&mut world).unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = world.schedule_scope::<()>(|_| panic!("停止运行"));
        }))
        .is_err());
        assert!(world.is_stopped());
        assert!(QueryState::<Tracked, Filter>::new(&mut world).is_err());
        state.unregister_tracking(&mut world).unwrap();
        assert!(world.tracking.queries.is_empty());
        world.close().unwrap();
    }
    #[test]
    fn builtin_query_descriptors_preserve_presence_and_filter_tick_reads() {
        use crate::{
            query::QueryItemPresence,
            query_data::Has,
            query_filter::{Changed, Or, With},
        };
        let mut world = World::new().unwrap();
        let id = world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        type Data = (Option<&'static Value>, Has<Value>);
        type Filter = Or<(With<Value>, Changed<Value>)>;
        let mut state = QueryState::<Data, Filter>::new(&mut world).unwrap();
        let declaration = state.declaration().unwrap();
        assert_eq!(declaration.data.components.len(), 1);
        assert_eq!(
            declaration.data.components[0].presence,
            QueryItemPresence::Optional
        );
        assert_eq!(declaration.filter.tick_reads.as_ref(), &[id]);
        assert!(declaration.filter.tracking[0].changed);
        assert!(
            matches!(declaration.filter.expression, crate::query_filter::FilterExpression::Or(children) if children.len() == 2)
        );
        assert!(QueryState::<(&Value, &mut Value)>::new(&mut world).is_err());
        state.unregister_tracking(&mut world).unwrap();
    }

    #[test]
    fn query_declaration_rejects_write_aliases_and_keeps_tick_reads_separate() {
        use crate::access::AccessMode;
        use crate::query::{QueryComponentAccess, QueryItemPresence};
        let id = crate::component::ComponentId(0);
        let mut declaration = QueryAccessDeclaration {
            data: QueryDataDeclaration {
                components: vec![QueryComponentAccess {
                    component: id,
                    mode: AccessMode::Write,
                    presence: QueryItemPresence::Required,
                }]
                .into(),
                tracking: Box::new([]),
                tick_reads: vec![id].into(),
            },
            filter: QueryFilterDeclaration {
                expression: crate::query_filter::FilterExpression::Changed(id),
                tracking: Box::new([]),
                tick_reads: vec![id].into(),
            },
        };
        let access = declaration.to_access().unwrap();
        assert_eq!(access.components.as_ref(), &[(id, AccessMode::Write)]);
        assert_eq!(access.tick_reads.as_ref(), &[id]);
        declaration.data.components = vec![
            QueryComponentAccess {
                component: id,
                mode: AccessMode::Write,
                presence: QueryItemPresence::Required,
            },
            QueryComponentAccess {
                component: id,
                mode: AccessMode::Read,
                presence: QueryItemPresence::Optional,
            },
        ]
        .into();
        assert!(declaration.to_access().is_err());
        for item in &mut declaration.data.components {
            item.mode = AccessMode::Read;
        }
        declaration.filter.expression = crate::query_filter::FilterExpression::Or(vec![
            crate::query_filter::FilterExpression::With(id),
        ]);
        let access = declaration.to_access().unwrap();
        assert_eq!(access.components.len(), 1);
        assert!(access.scope.with.is_empty() && access.scope.without.is_empty());
        declaration.filter.expression = crate::query_filter::FilterExpression::Without(id);
        assert!(declaration.to_access().is_err());
    }
    #[test]
    fn system_query_registration_uses_root_scope_and_cleans_up() {
        let mut world = World::new().unwrap();
        world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let mut state = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|mut registration| {
                    registration.query_state::<Tracked, Filter>()
                })
            })
            .unwrap();
        world
            .parameter_unregistration_scope(|mut registration| {
                registration.unregister_query(&mut state)
            })
            .unwrap();
        assert!(world.tracking.queries.is_empty());
        world.close().unwrap();
    }
    #[test]
    fn query_tracking_registration_is_independent_and_cross_world_preserves_responsibility() {
        let mut world = World::new().unwrap();
        world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let mut other = World::new().unwrap();
        other.finish_registration().unwrap();
        assert!(world
            .schedule_scope(|world| {
                assert!(QueryState::<Tracked, Filter>::new(world).is_err());
                Ok(())
            })
            .is_ok());
        let mut first = QueryState::<Tracked, Filter>::new(&mut world).unwrap();
        let declaration = first.declaration().unwrap();
        assert_eq!(declaration.data.tracking.len(), 1);
        assert_eq!(declaration.filter.tracking.len(), 1);
        let mut second = QueryState::<Tracked, Filter>::new(&mut world).unwrap();
        assert!(first.unregister_tracking(&mut other).is_err());
        assert_eq!(world.tracking.queries.len(), 2);
        let demands = world.tracking.queries.values().next().unwrap();
        assert_eq!(demands.len(), 1);
        assert!(demands[0].added && demands[0].changed);
        first.unregister_tracking(&mut world).unwrap();
        first.unregister_tracking(&mut world).unwrap();
        assert_eq!(world.tracking.queries.len(), 1);
        second.unregister_tracking(&mut world).unwrap();
        assert!(world.tracking.queries.is_empty());
        world.close().unwrap();
        other.close().unwrap();
    }
}

impl QueryAccessDeclaration {
    pub(crate) fn to_access_inner(
        _declaration: &QueryAccessDeclaration,
    ) -> Result<AccessDeclaration, EcsError> {
        use crate::access::{AccessMode, EntityScope, StructuralAccess};
        use crate::query::QueryItemPresence;
        use crate::query_filter::FilterExpression;
        let mut components = Vec::new();
        let mut with = Vec::new();
        let mut without = Vec::new();
        for item in &_declaration.data.components {
            if let Some((_, mode)) = components.iter().find(|(id, _)| *id == item.component) {
                if *mode == AccessMode::Write || item.mode == AccessMode::Write {
                    return Err(crate::error::classified_error(
                        crate::error::ErrorKind::InvalidInput,
                        "查询数据存在可变别名",
                    ));
                }
            } else {
                components.push((item.component, item.mode));
            }
            if item.presence == QueryItemPresence::Required {
                with.push(item.component);
            }
        }
        fn collect(
            expression: &FilterExpression,
            with: &mut Vec<crate::component::ComponentId>,
            without: &mut Vec<crate::component::ComponentId>,
        ) -> bool {
            match expression {
                FilterExpression::All => true,
                FilterExpression::With(id)
                | FilterExpression::Added(id)
                | FilterExpression::Changed(id) => {
                    with.push(*id);
                    true
                }
                FilterExpression::Without(id) => {
                    without.push(*id);
                    true
                }
                FilterExpression::And(children) => {
                    let mut narrow = true;
                    for child in children {
                        narrow &= collect(child, with, without);
                    }
                    narrow
                }
                FilterExpression::None | FilterExpression::Or(_) => false,
            }
        }
        if !collect(&_declaration.filter.expression, &mut with, &mut without) {
            with.clear();
            without.clear();
        }
        with.sort_unstable();
        with.dedup();
        without.sort_unstable();
        without.dedup();
        if with.iter().any(|id| without.contains(id)) {
            return Err(crate::error::classified_error(
                crate::error::ErrorKind::InvalidInput,
                "查询结构声明矛盾",
            ));
        }
        let mut tick_reads = _declaration.data.tick_reads.to_vec();
        tick_reads.extend_from_slice(&_declaration.filter.tick_reads);
        tick_reads.sort_unstable();
        tick_reads.dedup();
        Ok(AccessDeclaration {
            resources: Box::new([]),
            multi_resources: Box::new([]),
            components: components.into(),
            tick_reads: tick_reads.into(),
            reads_entities: true,
            scope: EntityScope {
                with: with.into(),
                without: without.into(),
            },
            structure: StructuralAccess::None,
            observations: Box::new([]),
        })
    }
}
impl<Q: QueryData, F: QueryFilter> QueryState<Q, F> {
    pub(crate) fn declaration_inner(
        _state: &QueryState<Q, F>,
    ) -> Result<QueryAccessDeclaration, EcsError> {
        Ok(QueryAccessDeclaration {
            data: Q::declare_access(&_state.data_state)?,
            filter: F::declare_access(&_state.filter_state)?,
        })
    }
}
impl<Q: ReadOnlyQueryData, F: QueryFilter> QueryState<Q, F> {
    pub(crate) fn query_with_window_inner<'w>(
        _state: &'w mut QueryState<Q, F>,
        _world: crate::world::WorldRef<'w>,
        _window: crate::change_detection::ChangeWindow,
    ) -> Result<Query<'w, Q, F>, EcsError> {
        host_query(_state, _world, Some(_window))
    }
}
impl<Q: QueryData, F: QueryFilter> QueryState<Q, F> {
    pub(crate) fn query_mut_since_inner<'w>(
        _state: &'w mut QueryState<Q, F>,
        _world: crate::world::WorldRef<'w>,
        _last_run: Option<crate::change_detection::ChangeTick>,
    ) -> Result<QueryMut<'w, Q, F>, EcsError> {
        host_query_mut(_state, _world, Some(_last_run))
    }
}
impl<Q: QueryData, F: QueryFilter> QueryMut<'_, Q, F> {
    pub(crate) fn this_run_inner(
        _query: &QueryMut<'_, Q, F>,
    ) -> crate::change_detection::ChangeTick {
        _query.write_tick
    }
}

use crate::{
    access::AccessDeclaration,
    entity::Entity,
    error::EcsError,
    query::{
        Query, QueryAccessDeclaration, QueryData, QueryFilter, QueryMut, QueryState,
        ReadOnlyQueryData,
    },
    world::World,
};
impl<Q: ReadOnlyQueryData, F: QueryFilter> QueryState<Q, F> {
    pub(crate) fn query_inner<'w>(
        _state: &'w mut QueryState<Q, F>,
        _world: crate::world::WorldRef<'w>,
    ) -> Result<Query<'w, Q, F>, EcsError> {
        host_query(_state, _world, None)
    }
}
impl<Q: QueryData, F: QueryFilter> QueryState<Q, F> {
    pub(crate) fn query_mut_inner<'w>(
        _state: &'w mut QueryState<Q, F>,
        _world: crate::world::WorldRef<'w>,
    ) -> Result<QueryMut<'w, Q, F>, EcsError> {
        host_query_mut(_state, _world, None)
    }
}
impl<Q: ReadOnlyQueryData, F: QueryFilter> Query<'_, Q, F> {
    pub(crate) fn query_get_inner<'a>(
        _query: &'a Query<'_, Q, F>,
        _entity: Entity,
    ) -> Result<Q::Item<'a, 'a>, EcsError> {
        #[cfg(feature = "rc")]
        {
            let mut access = _query.access.fetch_access()?;
            access.window = _query.window;
            // SAFETY: this shared borrow is of a read-only query under its original authorization.
            unsafe { QueryState::<Q, F>::fetch_one(_query.state, &access, _entity) }
        }
        #[cfg(not(feature = "rc"))]
        Err(query_unsupported())
    }
}
impl<Q: QueryData, F: QueryFilter> QueryMut<'_, Q, F> {
    pub(crate) fn query_get_mut_inner<'a>(
        _query: &'a mut QueryMut<'_, Q, F>,
        _entity: Entity,
    ) -> Result<Q::Item<'a, 'a>, EcsError> {
        #[cfg(feature = "rc")]
        {
            let mut access = _query.access.fetch_access()?;
            access.world = _query.access.world.reborrow();
            access.window = _query.window;
            access.write_tick = Some(_query.write_tick);
            // SAFETY: exclusive query borrow prevents a second result for this entity.
            unsafe { QueryState::<Q, F>::fetch_one(_query.state, &access, _entity) }
        }
        #[cfg(not(feature = "rc"))]
        Err(query_unsupported())
    }
}
impl<Q: ReadOnlyQueryData, F: QueryFilter> QueryState<Q, F> {
    pub(crate) fn get_inner<'w>(
        _state: &'w mut QueryState<Q, F>,
        _world: crate::world::WorldRef<'w>,
        _entity: Entity,
    ) -> Result<Q::Item<'w, 'w>, EcsError> {
        #[cfg(feature = "rc")]
        {
            let declaration = _state.host_point_declaration(_world)?;
            let (target, scope, plan) = _state.prepare_host_target(_world, _entity)?;
            _state.host_access = Some(crate::access::AccessTree::Leaf(declaration));
            let access = _state.host_point_access(_world, None);
            // SAFETY: the point plan matches the current published entity and Q is read-only.
            unsafe { _state.consume_host_target(&access, &target, scope, &plan) }
        }
        #[cfg(not(feature = "rc"))]
        Err(query_unsupported())
    }
}
impl<Q: QueryData, F: QueryFilter> QueryState<Q, F> {
    pub(crate) fn get_mut_inner<'w>(
        _state: &'w mut QueryState<Q, F>,
        _world: crate::world::WorldRef<'w>,
        _entity: Entity,
    ) -> Result<Q::Item<'w, 'w>, EcsError> {
        #[cfg(feature = "rc")]
        {
            let declaration = _state.host_point_declaration(_world)?;
            let (target, scope, plan) = _state.prepare_host_target(_world, _entity)?;
            let next = _world.change_tick.checked_add(1).ok_or_else(|| {
                crate::error::classified_error(
                    crate::error::ErrorKind::ResourceExhausted,
                    "变化时钟耗尽",
                )
            })?;
            let tick =
                crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(next).unwrap());
            _state.host_access = Some(crate::access::AccessTree::Leaf(declaration));
            // Run the filter and prepare the data before committing the tick. The raw field
            // projection avoids replacing the whole World borrow after views have been bound.
            let access = _state.host_point_access(_world, Some(tick));
            let mut data = unsafe { _state.prepare_host_fetch(&access, &target, scope, &plan)? };
            unsafe {
                std::ptr::addr_of_mut!((*access.world.pointer.as_ptr()).change_tick).write(next);
                Ok(crate::query_fetch::QueryFetch::fetch_at(&mut data, &target))
            }
        }
        #[cfg(not(feature = "rc"))]
        Err(query_unsupported())
    }
}

#[cfg(feature = "rc")]
impl<Q: QueryData, F: QueryFilter> QueryState<Q, F> {
    pub(crate) fn new_registered(
        _world: &mut crate::world::World,
    ) -> Result<crate::query::QueryState<Q, F>, crate::error::EcsError> {
        query_registration_gate(_world, false)?;
        Self::register_in_scope(_world)
    }
}
#[cfg(feature = "rc")]
impl<Q: crate::query::QueryData, F: crate::query::QueryFilter> crate::query::QueryState<Q, F> {
    pub(super) fn unregister_tracking_inner(
        &mut self,
        _world: &mut crate::world::World,
    ) -> Result<(), crate::error::EcsError> {
        query_registration_gate(_world, true)?;
        Self::unregister_in_scope(self, _world)
    }
}

fn query_error(message: &'static str) -> EcsError {
    pi_ecs_types::error::classified_error(pi_ecs_types::error::ErrorKind::InvalidState, message)
}
#[cfg(feature = "rc")]
fn query_registration_gate(world: &World, unregister: bool) -> Result<(), EcsError> {
    let mode = world.access.mode.replace(crate::world::AccessMode::Stopped);
    let allowed = matches!(mode, crate::world::AccessMode::Idle)
        || (unregister && matches!(mode, crate::world::AccessMode::Stopped));
    world.access.mode.set(mode);
    if !allowed
        || world.shutdown != crate::shutdown::ShutdownState::Open
        || (!unregister && !world.registry.frozen)
    {
        return Err(query_error("查询登记要求冻结静止 World"));
    }
    Ok(())
}
#[cfg(feature = "rc")]
impl<Q: QueryData, F: QueryFilter> QueryState<Q, F> {
    pub(crate) fn register_in_scope(world: &mut World) -> Result<Self, EcsError> {
        let data_state = Q::init_state(world)?;
        let filter_state = F::init_state(world)?;
        let declaration = QueryAccessDeclaration {
            data: Q::declare_access(&data_state)?,
            filter: F::declare_access(&filter_state)?,
        };
        let access = declaration.to_access()?;
        for component in access
            .components
            .iter()
            .map(|(id, _)| id)
            .chain(access.tick_reads.iter())
            .chain(access.scope.with.iter())
            .chain(access.scope.without.iter())
        {
            if world
                .registry
                .components
                .get(component.0 as usize)
                .is_none()
            {
                return Err(query_error("查询声明组件未登记"));
            }
        }
        let mut demands: Vec<crate::tracking::ComponentTickDemand> = Vec::new();
        for demand in declaration
            .data
            .tracking
            .iter()
            .chain(declaration.filter.tracking.iter())
        {
            if world
                .registry
                .components
                .get(demand.component.0 as usize)
                .is_none()
            {
                return Err(query_error("查询追踪组件未登记"));
            }
            if !demand.added && !demand.changed {
                continue;
            }
            if let Some(existing) = demands
                .iter_mut()
                .find(|entry| entry.component == demand.component)
            {
                existing.added |= demand.added;
                existing.changed |= demand.changed;
            } else {
                demands.push(crate::tracking::ComponentTickDemand {
                    component: demand.component,
                    added: demand.added,
                    changed: demand.changed,
                });
            }
        }
        let tracking_registration = if demands.is_empty() {
            None
        } else {
            Some(crate::tracking::TickDemandRegistration {
                world_identity: world.identity,
                id: world.tracking.queries.insert(demands.into()),
            })
        };
        Ok(QueryState {
            host_access: None,
            tracking_registration,
            tracking_unregistered: false,
            data_state,
            filter_state,
            bound_world: Some(world.identity),
            cache: crate::query::WorldQueryCache {
                first_table: 0,
                tables: Box::new([]),
            },
        })
    }
    #[cfg(feature = "rc")]
    pub(crate) fn unregister_in_scope(state: &mut Self, world: &mut World) -> Result<(), EcsError> {
        if state.bound_world != Some(world.identity) {
            return Err(query_error("查询来自其他 World"));
        }
        if let Some(registration) = &state.tracking_registration {
            if registration.world_identity != world.identity
                || !world.tracking.queries.contains_key(registration.id)
            {
                return Err(query_error("查询追踪责任失效"));
            }
        }
        if let Some(registration) = state.tracking_registration.take() {
            world.tracking.queries.remove(registration.id);
        }
        state.host_access = None;
        state.cache = crate::query::WorldQueryCache {
            first_table: 0,
            tables: Box::new([]),
        };
        state.tracking_unregistered = true;
        Ok(())
    }
}
#[cfg(feature = "rc")]
pub(crate) fn missing_entity(entity: Entity) -> EcsError {
    crate::error::classified_error(crate::error::ErrorKind::NotFound, "实体无已发布位置")
        .attach(crate::query_batch::QueryEntityContext::NoSuchEntity(entity))
}

#[cfg(feature = "rc")]
pub(crate) fn unmatched_entity(entity: Entity) -> EcsError {
    crate::error::classified_error(crate::error::ErrorKind::InvalidInput, "实体不匹配查询")
        .attach(crate::query_batch::QueryEntityContext::NotMatched(entity))
}

#[cfg(not(feature = "rc"))]
fn query_unsupported() -> EcsError {
    crate::error::classified_error(crate::error::ErrorKind::Unsupported, "非 RC 查询延期")
}

fn host_query<'w, Q: ReadOnlyQueryData, F: QueryFilter>(
    state: &'w mut QueryState<Q, F>,
    world: crate::world::WorldRef<'w>,
    window: Option<crate::change_detection::ChangeWindow>,
) -> Result<Query<'w, Q, F>, EcsError> {
    #[cfg(feature = "rc")]
    {
        query_registration_gate(world, false)?;
        if window.is_some_and(|window| window.last_run.is_some_and(|last| last > window.this_run)) {
            return Err(query_error("观察窗口上界早于下界"));
        }
        let declaration = state.declaration()?.to_access()?;
        if window.is_none() && (!F::IS_ARCHETYPAL || !declaration.tick_reads.is_empty()) {
            return Err(query_error("查询需要显式观察窗口"));
        }
        state.refresh_cache(&world.registry, world.storage.as_ref(), world.identity)?;
        state.host_access = Some(crate::access::AccessTree::Leaf(declaration));
        let world = crate::unsafe_world_cell::UnsafeWorldCell::from_mut(world);
        Ok(Query {
            window,
            access: crate::access::RunAccess {
                world,
                declaration: state.host_access.as_ref().expect("宿主查询授权已准备"),
                archetype_version: world.archetype_revision().0,
            },
            state,
        })
    }
    #[cfg(not(feature = "rc"))]
    Err(query_unsupported())
}

#[cfg(feature = "rc")]
impl<Q: QueryData, F: QueryFilter> QueryState<Q, F> {
    fn host_point_declaration(&self, world: &World) -> Result<AccessDeclaration, EcsError> {
        query_registration_gate(world, false)?;
        if self.tracking_unregistered
            || self.bound_world != Some(world.identity)
            || !world.registry.frozen
        {
            return Err(query_error("查询未登记或与冻结 World 不同源"));
        }
        let declaration = self.declaration()?.to_access()?;
        if !F::IS_ARCHETYPAL || !declaration.tick_reads.is_empty() {
            return Err(query_error("查询需要显式观察窗口"));
        }
        Ok(declaration)
    }

    fn prepare_host_target(
        &self,
        world: &World,
        entity: Entity,
    ) -> Result<
        (
            crate::query_fetch::FetchEntity,
            crate::query_fetch::FetchScope,
            super::QueryArchetype<Q, F>,
        ),
        EcsError,
    > {
        use crate::{
            query::FetchStateResult::*,
            query_fetch::{FetchBinding, FetchEntity, FetchScope},
        };
        let location = world.locate(entity).ok_or_else(|| missing_entity(entity))?;
        let archetype = world
            .registry
            .archetype(location.archetype())
            .expect("发布位置必须有原型");
        let scope = FetchScope::new(archetype);
        let binding = FetchBinding {
            storage: world
                .storage
                .as_ref()
                .ok_or_else(|| query_error("冻结查询存储缺失"))?,
            world_identity: world.identity,
            archetypes: &world.registry.archetypes,
        };
        let data = Q::init_fetch_state(&self.data_state, scope, &binding)?;
        let filter = F::init_fetch_state(&self.filter_state, scope, &binding)?;
        let plan = match (data, filter) {
            (NeedsVariants, _) | (_, NeedsVariants) => {
                return Err(query_error("具体原型不能请求变体"))
            }
            (Ready(data), Ready(filter)) => super::QueryArchetype { data, filter },
            _ => return Err(unmatched_entity(entity)),
        };
        Ok((FetchEntity { entity, location }, scope, plan))
    }

    fn host_point_access<'w>(
        &'w self,
        world: &'w mut World,
        write_tick: Option<crate::change_detection::ChangeTick>,
    ) -> crate::query_fetch::FetchAccess<'w> {
        let Some(crate::access::AccessTree::Leaf(declaration)) = self.host_access.as_ref() else {
            unreachable!("宿主查询无叶子声明");
        };
        crate::query_fetch::FetchAccess {
            world: crate::unsafe_world_cell::UnsafeWorldCell::from_mut(world),
            declaration,
            window: None,
            write_tick,
        }
    }

    unsafe fn prepare_host_fetch<'w>(
        &self,
        access: &crate::query_fetch::FetchAccess<'w>,
        target: &crate::query_fetch::FetchEntity,
        scope: crate::query_fetch::FetchScope,
        plan: &super::QueryArchetype<Q, F>,
    ) -> Result<Q::Fetch<'w>, EcsError> {
        use crate::query_fetch::{FilterFetch, QueryFetch};
        unsafe {
            let mut filter = F::prepare(access, scope, &self.filter_state, &plan.filter)?;
            if !filter.matches(target) {
                return Err(unmatched_entity(target.entity));
            }
            Q::Fetch::prepare(access, scope, &self.data_state, &plan.data)
        }
    }

    unsafe fn consume_host_target<'w>(
        &self,
        access: &crate::query_fetch::FetchAccess<'w>,
        target: &crate::query_fetch::FetchEntity,
        scope: crate::query_fetch::FetchScope,
        plan: &super::QueryArchetype<Q, F>,
    ) -> Result<Q::Item<'w, 'w>, EcsError> {
        use crate::query_fetch::QueryFetch;
        unsafe {
            Ok(self
                .prepare_host_fetch(access, target, scope, plan)?
                .fetch_at(target))
        }
    }

    pub(crate) fn refresh_cache(
        &mut self,
        registry: &crate::registry::TypeRegistry,
        storage: Option<&crate::storage::WorldStorage>,
        identity: u64,
    ) -> Result<(), EcsError> {
        use crate::query::{QueryTable, WorldQueryCache};
        if !registry.frozen || self.tracking_unregistered || self.bound_world != Some(identity) {
            return Err(query_error("查询未登记或与冻结 World 不同源"));
        }
        let binding = crate::query_fetch::FetchBinding {
            storage: storage.ok_or_else(|| query_error("冻结查询存储缺失"))?,
            world_identity: identity,
            archetypes: &registry.archetypes,
        };
        if !self.cache.tables.is_empty() {
            // Append only unseen variants; retain the old prefix if any new binding fails.
            let first = usize::from(self.cache.first_table);
            let mut touched = Vec::new();
            for archetype in &registry.archetypes {
                let Some(index) = usize::from(archetype.id().table).checked_sub(first) else {
                    continue;
                };
                let Some(table) = self.cache.tables.get_mut(index) else {
                    continue;
                };
                if let QueryTable::Variants { map, entries } = table {
                    if usize::from(archetype.id().variant) < map.len() {
                        continue;
                    }
                    if !touched.iter().any(|(saved, _, _)| *saved == index) {
                        touched.push((index, map.len(), entries.len()));
                    }
                } else {
                    continue;
                }
                if let Err(error) =
                    table.bind_variant(&self.data_state, &self.filter_state, archetype, &binding)
                {
                    for (index, map_len, entry_len) in touched {
                        if let QueryTable::Variants { map, entries } = &mut self.cache.tables[index]
                        {
                            map.truncate(map_len);
                            entries.truncate(entry_len);
                        }
                    }
                    return Err(error);
                }
            }
            return Ok(());
        }
        // RC storage cannot be replaced; its fixed container roots survive variant append.
        let mut tables = registry
            .tables
            .iter()
            .map(|layout| {
                QueryTable::<Q, F>::bind_table(
                    &self.data_state,
                    &self.filter_state,
                    layout,
                    &binding,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        for archetype in &registry.archetypes {
            tables[usize::from(archetype.id().table)].bind_variant(
                &self.data_state,
                &self.filter_state,
                archetype,
                &binding,
            )?;
        }
        let first = tables
            .iter()
            .position(|table| !matches!(table, QueryTable::Excluded));
        let (first_table, tables) = if let Some(first) = first {
            let last = tables
                .iter()
                .rposition(|table| !matches!(table, QueryTable::Excluded))
                .unwrap();
            (
                u16::try_from(first).expect("已登记 Table 编号"),
                tables.drain(first..=last).collect(),
            )
        } else {
            (0, Box::new([]) as Box<[_]>)
        };
        self.cache = WorldQueryCache {
            first_table,
            tables,
        };
        Ok(())
    }
}

#[cfg(feature = "rc")]
impl<Q: QueryData, F: QueryFilter> QueryState<Q, F> {
    pub(crate) unsafe fn fetch_one<'w>(
        state: &Self,
        access: &crate::query_fetch::FetchAccess<'w>,
        entity: Entity,
    ) -> Result<Q::Item<'w, 'w>, EcsError> {
        use crate::query_fetch::QueryFetch;
        // SAFETY: caller maintains the original authorization for this result.
        let (target, mut fetch) = unsafe { Self::prepare_cached_target(state, access, entity)? };
        Ok(unsafe { fetch.fetch_at(&target) })
    }

    pub(crate) unsafe fn prepare_cached_target<'w>(
        state: &Self,
        access: &crate::query_fetch::FetchAccess<'w>,
        entity: Entity,
    ) -> Result<(crate::query_fetch::FetchEntity, Q::Fetch<'w>), EcsError> {
        unsafe { Self::prepare_optional_cached_target(state, access, entity) }?
            .ok_or_else(|| unmatched_entity(entity))
    }

    pub(crate) unsafe fn prepare_optional_cached_target<'w>(
        state: &Self,
        access: &crate::query_fetch::FetchAccess<'w>,
        entity: Entity,
    ) -> Result<Option<(crate::query_fetch::FetchEntity, Q::Fetch<'w>)>, EcsError> {
        use crate::{
            query::QueryTable,
            query_fetch::{FetchEntity, FetchScope, FilterFetch, QueryFetch},
        };
        // SAFETY: the authorization keeps metadata from the same World alive.
        let entities = unsafe { &*std::ptr::addr_of!((*access.world.pointer.as_ptr()).entities) };
        let location = entities
            .borrow()
            .locate(entity)
            .ok_or_else(|| missing_entity(entity))?;
        let archetype = location.archetype();
        let Some(table) = archetype
            .table
            .checked_sub(state.cache.first_table)
            .and_then(|offset| state.cache.tables.get(usize::from(offset)))
        else {
            return Ok(None);
        };
        let (scope, entry) = match table {
            QueryTable::Excluded => return Ok(None),
            QueryTable::Uniform { fetch } => (
                FetchScope::Table {
                    table: archetype.table,
                },
                fetch,
            ),
            QueryTable::Variants { map, entries } => {
                let Some(index) = map.get(usize::from(archetype.variant)).copied().flatten() else {
                    return Ok(None);
                };
                (FetchScope::Archetype { archetype }, &entries[index].1)
            }
        };
        let target = FetchEntity { entity, location };
        // SAFETY: the target and both plans share the original authorization. Drop the
        // dynamic filter before preparing a potentially writable data fetch.
        let matched = unsafe {
            F::prepare(access, scope, &state.filter_state, &entry.filter)?.matches(&target)
        };
        if !matched {
            return Ok(None);
        }
        let data = unsafe { Q::Fetch::prepare(access, scope, &state.data_state, &entry.data)? };
        Ok(Some((target, data)))
    }
}

fn host_query_mut<'w, Q: QueryData, F: QueryFilter>(
    state: &'w mut QueryState<Q, F>,
    world: crate::world::WorldRef<'w>,
    last_run: Option<Option<crate::change_detection::ChangeTick>>,
) -> Result<QueryMut<'w, Q, F>, EcsError> {
    #[cfg(feature = "rc")]
    {
        query_registration_gate(world, false)?;
        let declaration = state.declaration()?.to_access()?;
        if last_run.is_none() && (!F::IS_ARCHETYPAL || !declaration.tick_reads.is_empty()) {
            return Err(query_error("查询需要显式观察窗口"));
        }
        let next = world.change_tick.checked_add(1).ok_or_else(|| {
            crate::error::classified_error(
                crate::error::ErrorKind::ResourceExhausted,
                "变化时钟耗尽",
            )
        })?;
        let write_tick =
            crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(next).unwrap());
        if last_run.flatten().is_some_and(|last| last > write_tick) {
            return Err(query_error("观察窗口上界早于下界"));
        }
        state.refresh_cache(&world.registry, world.storage.as_ref(), world.identity)?;
        state.host_access = Some(crate::access::AccessTree::Leaf(declaration));
        world.change_tick = next;
        let world = crate::unsafe_world_cell::UnsafeWorldCell::from_mut(world);
        Ok(QueryMut {
            window: last_run.map(|last_run| crate::change_detection::ChangeWindow {
                last_run,
                this_run: write_tick,
            }),
            write_tick,
            access: crate::access::RunAccess {
                world,
                declaration: state.host_access.as_ref().expect("宿主查询授权已准备"),
                archetype_version: world.archetype_revision().0,
            },
            state,
        })
    }
    #[cfg(not(feature = "rc"))]
    Err(query_unsupported())
}

use super::{NoFilter, NoFilterFetch};

impl NoFilterFetch {
    pub(super) unsafe fn matches_inner(
        &mut self,
        _entity: &crate::query_fetch::FetchEntity,
    ) -> bool {
        true
    }
}

impl NoFilter {
    pub(super) fn init_state_inner(_world: &World) -> Result<(), EcsError> {
        Ok(())
    }
}

impl NoFilter {}

impl NoFilter {}

impl NoFilter {
    pub(super) unsafe fn prepare_inner<'w>(
        _access: &crate::query_fetch::FetchAccess<'w>,
        _plan: crate::query_fetch::FetchScope,
        _state: &(),
        _fetch_state: &(),
    ) -> Result<NoFilterFetch, EcsError> {
        Ok(NoFilterFetch)
    }
}

impl<Q: QueryData, F: QueryFilter> super::QueryTable<Q, F> {
    pub(super) fn bind_table_inner(
        _data: &Q::State,
        _filter: &F::State,
        _layout: &pi_ecs_storage::table::TableLayout,
        _binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<Self, EcsError> {
        use crate::query::{FetchStateResult::*, QueryArchetype};
        let scope = crate::query_fetch::FetchScope::Table {
            table: _layout.id().table,
        };
        let data = Q::init_fetch_state(_data, scope, _binding)?;
        let filter = F::init_fetch_state(_filter, scope, _binding)?;
        Ok(match (data, filter) {
            (NotMatched, _) | (_, NotMatched) => Self::Excluded,
            (Ready(data), Ready(filter)) => Self::Uniform {
                fetch: QueryArchetype { data, filter },
            },
            _ => Self::Variants {
                map: Vec::new(),
                entries: Vec::new(),
            },
        })
    }
}

impl<Q: QueryData, F: QueryFilter> super::QueryTable<Q, F> {
    pub(super) fn bind_variant_inner(
        &mut self,
        _data: &Q::State,
        _filter: &F::State,
        _archetype: &crate::archetype::Archetype,
        _binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<(), EcsError> {
        use crate::query::{FetchStateResult::*, QueryArchetype};
        let Self::Variants { map, entries } = self else {
            return Ok(());
        };
        let variant = _archetype.id().variant;
        match usize::from(variant).cmp(&map.len()) {
            std::cmp::Ordering::Less => return Ok(()),
            std::cmp::Ordering::Greater => return Err(query_error("查询变体后缀不连续")),
            std::cmp::Ordering::Equal => {}
        }
        let scope = crate::query_fetch::FetchScope::new(_archetype);
        let data = Q::init_fetch_state(_data, scope, _binding)?;
        let filter = F::init_fetch_state(_filter, scope, _binding)?;
        match (data, filter) {
            (NeedsVariants, _) | (_, NeedsVariants) => {
                return Err(query_error("具体原型返回了表级变体请求"))
            }
            (Ready(data), Ready(filter)) => {
                entries
                    .try_reserve(1)
                    .map_err(|_| query_error("无法准备查询变体条目"))?;
                map.try_reserve(1)
                    .map_err(|_| query_error("无法准备查询变体索引"))?;
                let index = entries.len();
                entries.push((variant, QueryArchetype { data, filter }));
                map.push(Some(index));
            }
            _ => map.push(None),
        }
        Ok(())
    }
}
