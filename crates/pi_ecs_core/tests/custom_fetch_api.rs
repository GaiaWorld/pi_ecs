//! 外部 crate 视角的声明验收；不执行设计失败桩。

#[cfg(feature = "rc")]
#[allow(dead_code)]
fn sparse_tick_install_protocol(
    storage: &mut pi_ecs_storage::storage::WorldStorage,
    component: pi_ecs_core::component::ComponentId,
    entity: pi_ecs_core::entity::Entity,
    value: pi_ecs_storage::ptr::OwningPtr<'_>,
    ticks: pi_ecs_types::change_detection::ComponentTicks,
) -> Result<(), EcsError> {
    storage.prepare_sparse_tracked(component, entity)?;
    storage.insert_sparse_tracked(component, entity, value, ticks);
    let _snapshot = storage.sparse_ticks(component, entity);
    Ok(())
}
use pi_ecs_core::{
    component::{Component, PlainAccess},
    error::EcsError,
    query::{QueryData, ReadOnlyQueryData},
    query_fetch::{FetchAccess, FetchEntity, FetchScope, QueryFetch},
    world::World,
};

struct Position;
// 外部 core crate 能取得迭代所需物理成员，不读取私有字段；仅编译，不运行桩。
#[allow(dead_code)]
fn storage_members(
    storage: &pi_ecs_storage::storage::WorldStorage,
    id: pi_ecs_core::entity::ArchetypeId,
) -> Result<(), EcsError> {
    for row in 0..storage.published_rows(id.table)? {
        let _ = storage.row_entity(id.table, pi_ecs_storage::table::TableRow::new(row));
    }
    for row in 0..storage.member_len(id)? {
        if let Some(member) = storage.member(id, row) {
            let _ = (member.entity, member.table_row);
        }
    }
    Ok(())
}
#[cfg(feature = "rc")]
#[allow(dead_code)]
fn storage_maintenance(
    storage: &mut pi_ecs_storage::storage::WorldStorage,
    targets: &[pi_ecs_storage::storage::TrackingTarget],
    tick: pi_ecs_core::change_detection::ChangeTick,
) -> Result<(), EcsError> {
    storage.apply_tracking(targets, tick)?;
    for location in storage.settle()? {
        let _ = (
            location.entity,
            location.archetype,
            location.table_row,
            location.archetype_row,
        );
    }
    Ok(())
}
impl Component for Position {
    type Access = PlainAccess;
}

struct CustomPosition;
struct CustomPositionFetch<'w>(FetchAccess<'w>);

// SAFETY: 仅委托 Position 的共享查询描述，取值仅使用受限只读入口。
unsafe impl QueryData for CustomPosition {
    type Item<'a, 'w> = &'a Position;
    type State = <&'static Position as QueryData>::State;
    type FetchState = <&'static Position as QueryData>::FetchState;
    type Fetch<'w> = CustomPositionFetch<'w>;

    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        <&Position as QueryData>::init_state(world)
    }
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &pi_ecs_core::query_fetch::FetchBinding<'_>,
    ) -> Result<pi_ecs_core::query::FetchStateResult<Self::FetchState>, EcsError> {
        <&Position as QueryData>::init_fetch_state(state, scope, binding)
    }
    fn declare_access(
        state: &Self::State,
    ) -> Result<pi_ecs_core::query::QueryDataDeclaration, EcsError> {
        <&Position as QueryData>::declare_access(state)
    }
}
// SAFETY: 结果和所有真实取值均为共享 Position，不修改 World。
unsafe impl ReadOnlyQueryData for CustomPosition {}
// SAFETY: prepare 沿用框架验证后的原授权，fetch_at 不扩权或生成写引用。
unsafe impl<'w> QueryFetch<'w, CustomPosition> for CustomPositionFetch<'w> {
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        _plan: FetchScope,
        _state: &<CustomPosition as QueryData>::State,
        _fetch_state: &<CustomPosition as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        Ok(Self(access.retain()))
    }
    unsafe fn fetch_at(&mut self, entity: &FetchEntity) -> &'w Position {
        // SAFETY: QueryFetch 的调用方保证目标、过滤、共享权限及全结果期无冲突。
        unsafe { self.0.read::<Position>(entity) }.expect("必需组件应已匹配")
    }
}

#[test]
fn external_fetch_can_name_and_use_public_protocol() {
    fn assert_read_only<Q: ReadOnlyQueryData>() {}
    assert_read_only::<CustomPosition>();
}

#[cfg(feature = "rc")]
#[test]
fn rc_world_write_point_query_reports_invalid_alias_and_entity_contexts() {
    use pi_ecs_core::{
        bundle::ComponentValue,
        component::StoragePolicy,
        entity::AllocationDomain,
        error::{prelude::ErrorInspectExt, ErrorKind},
        query_batch::QueryEntityContext,
    };
    let mut world = World::new().unwrap();
    world
        .register_component::<Position>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let matching = world
        .spawn(AllocationDomain(0), ComponentValue(Position))
        .unwrap()
        .forget();
    let nonmatching = world.spawn(AllocationDomain(0), ()).unwrap().forget();
    let missing = world.spawn(AllocationDomain(0), ()).unwrap().entity();
    world.flush_destruction().unwrap();
    let (alias, unmatched, missing_error) = world
        .write_scope(|write| {
            assert!(write.query_one_mut::<&Position>(matching).is_ok());
            (
                write
                    .query_one_mut::<(&mut Position, &Position)>(matching)
                    .err()
                    .unwrap(),
                write
                    .query_one_mut::<CustomPosition>(nonmatching)
                    .err()
                    .unwrap(),
                write
                    .query_one_mut::<CustomPosition>(missing)
                    .err()
                    .unwrap(),
            )
        })
        .unwrap();
    assert_eq!(
        [
            alias.current_kind(),
            unmatched.current_kind(),
            missing_error.current_kind()
        ],
        [
            &ErrorKind::InvalidInput,
            &ErrorKind::InvalidInput,
            &ErrorKind::NotFound
        ],
        "error kinds for alias, nonmatching live entity, and missing entity",
    );
    assert!(alias.downcast_ref::<QueryEntityContext>().is_none());
    assert_eq!(
        [
            matches!(unmatched.downcast_ref::<QueryEntityContext>(), Some(QueryEntityContext::NotMatched(entity)) if *entity == nonmatching),
            matches!(missing_error.downcast_ref::<QueryEntityContext>(), Some(QueryEntityContext::NoSuchEntity(entity)) if *entity == missing),
        ],
        [true, true],
        "typed contexts must preserve the exact entity",
    );
    assert!(!world.is_stopped());
    world.close().unwrap();
}

#[cfg(feature = "rc")]
#[test]
fn rc_world_write_point_query_updates_a_table_slot_with_a_custom_sparse_read() {
    use pi_ecs_core::{bundle::ComponentValue, component::StoragePolicy, entity::AllocationDomain};
    struct Score(u32);
    impl Component for Score {
        type Access = PlainAccess;
    }
    let mut world = World::new().unwrap();
    let score = world
        .register_component::<Score>(StoragePolicy::TableEligible)
        .unwrap();
    world
        .register_component::<Position>(StoragePolicy::SparseOnly)
        .unwrap();
    world.register_table(&[score]).unwrap();
    world.finish_registration().unwrap();
    let entity = world
        .spawn(
            AllocationDomain(0),
            (ComponentValue(Score(11)), ComponentValue(Position)),
        )
        .unwrap()
        .forget();
    let before = world.read_change_tick();
    world
        .write_scope(|write| {
            assert!(write.query_one_mut::<(&mut Score, &Score)>(entity).is_err());
            {
                let (mut score, _position) = write
                    .query_one_mut::<(&mut Score, CustomPosition)>(entity)
                    .unwrap();
                assert_eq!(score.0, 11);
                score.0 += 26;
            }
            assert_eq!(write.query_one_mut::<&Score>(entity).unwrap().0, 37);
        })
        .unwrap();
    assert_eq!(world.read_change_tick().get(), before.get() + 1);
    world
        .read_scope(|pin| assert_eq!(pin.get::<Score>(entity).unwrap().0, 37))
        .unwrap();
    world.close().unwrap();
}

#[cfg(feature = "rc")]
#[test]
fn rc_spawn_installs_table_and_sparse_values_and_reclaims_them() {
    use pi_ecs_core::{bundle::ComponentValue, component::StoragePolicy, entity::AllocationDomain};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct TableValue(u32);
    impl Component for TableValue {
        type Access = PlainAccess;
    }
    struct SparseValue(Arc<AtomicUsize>);
    impl Component for SparseValue {
        type Access = PlainAccess;
    }
    impl Drop for SparseValue {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let drops = Arc::new(AtomicUsize::new(0));
    let mut world = World::new().unwrap();
    let table = world
        .register_component::<TableValue>(StoragePolicy::TableEligible)
        .unwrap();
    world
        .register_component::<SparseValue>(StoragePolicy::SparseOnly)
        .unwrap();
    world.register_table(&[table]).unwrap();
    world.finish_registration().unwrap();
    let entity = world
        .spawn(
            AllocationDomain(0),
            (
                ComponentValue(SparseValue(drops.clone())),
                ComponentValue(TableValue(317)),
            ),
        )
        .unwrap()
        .entity();
    world
        .read_scope(|pin| {
            assert_eq!(pin.get::<TableValue>(entity).unwrap().0, 317);
            assert!(pin.get::<SparseValue>(entity).is_some());
        })
        .unwrap();
    world.flush_destruction().unwrap();
    assert_eq!(drops.load(Ordering::Relaxed), 1);
    world.close().unwrap();
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

#[cfg(feature = "rc")]
#[test]
fn rc_partial_spawn_cleans_installed_prefix_and_stops_world() {
    use pi_ecs_core::{
        bundle::{Bundle, BundleContext, BundleWriter},
        component::StoragePolicy,
        entity::AllocationDomain,
    };
    use std::{
        any::TypeId,
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };
    struct First(Arc<AtomicUsize>);
    impl Component for First {
        type Access = PlainAccess;
    }
    impl Drop for First {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    struct Second;
    impl Component for Second {
        type Access = PlainAccess;
    }
    struct Incomplete(First);
    impl Bundle for Incomplete {
        fn component_types() -> Vec<TypeId> {
            vec![TypeId::of::<First>(), TypeId::of::<Second>()]
        }
        fn validate(&self, _: &BundleContext<'_>) -> Result<(), EcsError> {
            Ok(())
        }
        fn write(self, writer: &mut BundleWriter<'_>) {
            writer.write(self.0);
        }
    }
    let drops = Arc::new(AtomicUsize::new(0));
    let mut world = World::new().unwrap();
    let first = world
        .register_component::<First>(StoragePolicy::TableEligible)
        .unwrap();
    world
        .register_component::<Second>(StoragePolicy::SparseOnly)
        .unwrap();
    world.register_table(&[first]).unwrap();
    world.finish_registration().unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = world.spawn(AllocationDomain(0), Incomplete(First(drops.clone())));
    }));
    assert!(result.is_err());
    assert!(world.is_stopped());
    assert_eq!(drops.load(Ordering::Relaxed), 1);
    world.close().unwrap();
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

#[cfg(feature = "rc")]
#[test]
fn rc_strict_add_installs_nonempty_bundle_and_preserves_identity() {
    use pi_ecs_core::{bundle::ComponentValue, component::StoragePolicy, entity::AllocationDomain};
    struct Added(u64);
    impl Component for Added {
        type Access = PlainAccess;
    }
    struct Retained(String);
    impl Component for Retained {
        type Access = PlainAccess;
    }
    struct Removed;
    impl Component for Removed {
        type Access = PlainAccess;
    }
    let mut world = World::new().unwrap();
    let id = world
        .register_component::<Added>(StoragePolicy::TableEligible)
        .unwrap();
    let retained = world
        .register_component::<Retained>(StoragePolicy::TableEligible)
        .unwrap();
    let removed = world
        .register_component::<Removed>(StoragePolicy::TableEligible)
        .unwrap();
    world.register_table(&[retained, removed]).unwrap();
    world.register_table(&[id, retained]).unwrap();
    world.finish_registration().unwrap();
    let owned = world
        .spawn(
            AllocationDomain(0),
            (
                ComponentValue(Removed),
                ComponentValue(Retained(String::from("original"))),
            ),
        )
        .unwrap();
    let entity = owned.entity();
    // Keep a positive strong count for the host operation; close reclaims the entity.
    std::mem::forget(owned);
    world
        .change_archetype(entity, ComponentValue(Added(317)), &[removed])
        .unwrap();
    world
        .read_scope(|pin| {
            assert_eq!(pin.get::<Added>(entity).unwrap().0, 317);
            assert_eq!(pin.get::<Retained>(entity).unwrap().0, "original");
            assert!(pin.get::<Removed>(entity).is_none());
        })
        .unwrap();
    world.close().unwrap();
}

#[cfg(feature = "rc")]
#[test]
fn rc_alter_installs_once_and_rejects_repeating_the_same_addition() {
    use pi_ecs_core::{
        alter::AlterState, bundle::ComponentValue, component::StoragePolicy,
        entity::AllocationDomain,
    };
    struct Value(u64);
    impl Component for Value {
        type Access = PlainAccess;
    }
    let mut world = World::new().unwrap();
    let id = world
        .register_component::<Value>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let owned = world.spawn(AllocationDomain(0), ()).unwrap();
    let entity = owned.entity();
    std::mem::forget(owned);
    let mut state = AlterState::new(&world, &[id], &[]).unwrap();
    state
        .scope(&mut world, |alter| {
            alter.alter(entity, ComponentValue(Value(317))).unwrap();
            assert!(alter.alter(entity, ComponentValue(Value(99))).is_err());
        })
        .unwrap();
    world
        .read_scope(|pin| assert_eq!(pin.get::<Value>(entity).unwrap().0, 317))
        .unwrap();
    world.close().unwrap();
}

fn assert_declaration_merges(declaration: &pi_ecs_core::query::QueryAccessDeclaration) {
    let _ = declaration.to_access();
}

#[cfg(feature = "rc")]
#[test]
fn rc_typed_query_binds_external_changed_in_and_and_or() {
    use pi_ecs_core::{
        bundle::ComponentValue,
        component::StoragePolicy,
        entity::{AllocationDomain, Entity},
        query_filter::{Or, With, Without},
    };
    let mut world = World::new().unwrap();
    world
        .register_component::<Position>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let mut and = world
        .query_filtered::<(Entity, &Position), (With<Position>, ExternalChanged)>()
        .unwrap();
    let mut or = world
        .query_filtered::<(Entity, Option<&Position>), Or<(ExternalChanged, Without<Position>)>>()
        .unwrap();
    world.apply_tracking().unwrap();
    let present = world
        .spawn(AllocationDomain(0), ComponentValue(Position))
        .unwrap()
        .forget();
    let absent = world.spawn(AllocationDomain(0), ()).unwrap().forget();
    let window = pi_ecs_core::change_detection::ChangeWindow {
        last_run: None,
        this_run: world.read_change_tick(),
    };
    // SAFETY: both queries are sequential, read-only, and bound to their own World.
    let query = unsafe { and.query_with_window(&mut world, window) }.unwrap();
    assert_eq!(query.get(present).unwrap().0, present);
    assert!(query.get(absent).is_err());
    let query = unsafe { or.query_with_window(&mut world, window) }.unwrap();
    assert!(query.get(present).unwrap().1.is_some());
    assert!(query.get(absent).unwrap().1.is_none());
    world.close().unwrap();
}

// 外部过滤器使用不同于内置表达式的长期状态。
struct ExternalChanged;
struct ExternalChangedState(
    <pi_ecs_core::query_filter::Changed<Position> as pi_ecs_core::query::QueryFilter>::State,
);
type ChangedPosition = pi_ecs_core::query_filter::Changed<Position>;
// SAFETY: 所有绑定、声明和判断均委托只读 Changed<Position>，不扩大访问。
unsafe impl pi_ecs_core::query::QueryFilter for ExternalChanged {
    const IS_ARCHETYPAL: bool = false;
    type State = ExternalChangedState;
    type FetchState = <ChangedPosition as pi_ecs_core::query::QueryFilter>::FetchState;
    type Fetch<'a> = <ChangedPosition as pi_ecs_core::query::QueryFilter>::Fetch<'a>;
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        <ChangedPosition as pi_ecs_core::query::QueryFilter>::init_state(world)
            .map(ExternalChangedState)
    }
    fn declare_access(
        state: &Self::State,
    ) -> Result<pi_ecs_core::query::QueryFilterDeclaration, EcsError> {
        <ChangedPosition as pi_ecs_core::query::QueryFilter>::declare_access(&state.0)
    }
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &pi_ecs_core::query_fetch::FetchBinding<'_>,
    ) -> Result<pi_ecs_core::query::FetchStateResult<Self::FetchState>, EcsError> {
        <ChangedPosition as pi_ecs_core::query::QueryFilter>::init_fetch_state(
            &state.0, scope, binding,
        )
    }
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        scope: FetchScope,
        state: &Self::State,
        fetch: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        unsafe {
            <ChangedPosition as pi_ecs_core::query::QueryFilter>::prepare(
                access, scope, &state.0, fetch,
            )
        }
    }
}
#[allow(dead_code)]
fn external_filter_composition_protocol(
    and: <(ExternalChanged, ChangedPosition) as pi_ecs_core::query::QueryFilter>::State,
    or: <pi_ecs_core::query_filter::Or<(ExternalChanged, ChangedPosition)> as pi_ecs_core::query::QueryFilter>::State,
    declaration: pi_ecs_core::query::QueryFilterDeclaration,
) {
    let _: pi_ecs_core::query_filter::FilterTupleState<(
        ExternalChangedState,
        <ChangedPosition as pi_ecs_core::query::QueryFilter>::State,
    )> = and;
    let _: pi_ecs_core::query_filter::FilterTupleState<(
        ExternalChangedState,
        <ChangedPosition as pi_ecs_core::query::QueryFilter>::State,
    )> = or;
    let _: Box<[pi_ecs_core::component::ComponentId]> = declaration.tick_reads;
}
