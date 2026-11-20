//! 类型化系统参数生命周期：保留参数位置和 Core 授权边界。

#[cfg(all(test, feature = "rc"))]
mod reader_tests {
    use crate::{
        notification_reader::AddedReader,
        system_param::{ParamRegistration, SystemParam},
    };
    use pi_ecs_core::{
        access::{AccessTree, OrderingArchetypes},
        component::{Component, PlainAccess, StoragePolicy},
        notification::NoticeKind,
        world::World,
    };

    #[test]
    fn rc_reader_declaration_and_ordering_cover_only_the_subscribed_stream() {
        struct Tag;
        impl Component for Tag {
            type Access = PlainAccess;
        }
        type Reader = AddedReader<'static, Tag>;
        let mut world = World::new().unwrap();
        let id = world
            .register_component::<Tag>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let state = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|access| {
                    Reader::initialize((), &mut ParamRegistration::new(access))
                })
            })
            .unwrap();
        let AccessTree::Leaf(leaf) = Reader::declaration(&state, &mut world).unwrap() else {
            panic!("a reader has a single leaf");
        };
        assert_eq!(leaf.observations.as_ref(), &[(id, NoticeKind::Added)]);
        assert!(leaf.components.is_empty() && leaf.tick_reads.is_empty());
        let logical = OrderingArchetypes {
            components: vec![Box::<[_]>::from([id])].into(),
        };
        let ordering = Reader::ordering_access(&state, &logical).unwrap();
        assert_eq!(ordering.len(), 1);
        assert_eq!(ordering[0].declaration.observations, leaf.observations);
        assert!(ordering[0].entities.is_empty() && ordering[0].targets.is_empty());
        world
            .parameter_unregistration_scope(|access| {
                Reader::unregister(state, &mut ParamRegistration::new(access))
            })
            .unwrap();
        world.close().unwrap();
    }
}

use super::*;
use pi_ecs_core::access::{AccessTree, OrderingAccess, OrderingArchetypes};

// This private protocol dispatches on the parameter descriptor, not on its State type.
// In particular Res and its optional form may share State without sharing fetch semantics.
pub(super) trait ParamImpl: SystemParam {
    fn initialize_inner(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError>;
    fn prepare_inner(state: &mut Self::State, world: WorldRef<'_>)
        -> Result<ParamAccess, EcsError>;
    fn declaration_inner(state: &Self::State, world: WorldRef<'_>) -> Result<AccessTree, EcsError>;
    fn ordering_inner(
        state: &Self::State,
        logical: &OrderingArchetypes,
    ) -> Result<Vec<OrderingAccess>, EcsError>;
    fn begin_inner(state: &mut Self::State) -> Self::Pending;
    unsafe fn fetch_inner<'r>(
        state: &'r mut Self::State,
        pending: &'r mut Self::Pending,
        context: SystemRunContext<'r>,
    ) -> Self::Item<'r>;
    fn finish_inner(
        state: &mut Self::State,
        pending: Self::Pending,
        context: &SystemRunContext<'_>,
    );
    fn unregister_inner(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError>;
}

macro_rules! unit_pending {
    () => {
        fn begin_inner(_: &mut Self::State) {}
        fn finish_inner(_: &mut Self::State, _: (), _: &SystemRunContext<'_>) {}
    };
}

impl<T: Send + 'static> ParamImpl for Local<'_, T> {
    fn initialize_inner(
        config: LocalConfig<T>,
        _: &mut ParamRegistration<'_>,
    ) -> Result<LocalState<T>, EcsError> {
        Ok(LocalState {
            value: config.initial,
            recovery: config.recovery,
        })
    }
    fn prepare_inner(_: &mut LocalState<T>, _: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        Ok(ParamAccess::Together(Vec::new()))
    }
    fn declaration_inner(_: &LocalState<T>, _: WorldRef<'_>) -> Result<AccessTree, EcsError> {
        Ok(AccessTree::Together(Vec::new()))
    }
    fn ordering_inner(
        _: &LocalState<T>,
        _: &OrderingArchetypes,
    ) -> Result<Vec<OrderingAccess>, EcsError> {
        Ok(Vec::new())
    }
    unit_pending!();
    unsafe fn fetch_inner<'r>(
        state: &'r mut LocalState<T>,
        _: &'r mut (),
        _: SystemRunContext<'r>,
    ) -> Local<'r, T> {
        Local {
            value: &mut state.value,
        }
    }
    fn unregister_inner(_: LocalState<T>, _: &mut ParamRegistration<'_>) -> Result<(), EcsError> {
        Ok(())
    }
}

macro_rules! composition_lifecycle {
    ($node:ident; $($P:ident $index:tt $value:ident),+) => {
        fn initialize_inner(config: Self::Config, registration: &mut ParamRegistration<'_>) -> Result<Self::State, EcsError> {
            // Each Option is populated only after that position succeeds. On Err, clear every
            // earlier position in declaration order and keep the original initialization error.
            $(let mut $value = None;)+
            let result = (|| {
                $($value = Some(registration.initialize_parameter::<$P>(config.$index)?);)+
                Ok(())
            })();
            if let Err(error) = result {
                $(if let Some(state) = $value { let _ = $P::unregister(state, registration); })+
                return Err(error);
            }
            Ok(($($value.expect("successful parameter initialization"),)+))
        }
        fn prepare_inner(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
            Ok(ParamAccess::$node(vec![$($P::prepare(&mut state.$index, world)?,)+]))
        }
        fn declaration_inner(state: &Self::State, world: WorldRef<'_>) -> Result<AccessTree, EcsError> {
            Ok(AccessTree::$node(vec![$($P::declaration(&state.$index, world)?,)+]))
        }
        fn ordering_inner(state: &Self::State, logical: &OrderingArchetypes) -> Result<Vec<OrderingAccess>, EcsError> {
            let mut output = Vec::new();
            $(output.extend($P::ordering_access(&state.$index, logical)?);)+
            Ok(output)
        }
        fn begin_inner(state: &mut Self::State) -> Self::Pending {
            ($($P::begin(&mut state.$index),)+)
        }
        fn unregister_inner(state: Self::State, registration: &mut ParamRegistration<'_>) -> Result<(), EcsError> {
            let mut first = Ok(());
            $(first = first.and($P::unregister(state.$index, registration));)+
            first
        }
    };
}

macro_rules! tuple_impl {
    ($($P:ident $index:tt $value:ident),+) => {
        impl<$($P: SystemParam),+> ParamImpl for ($($P,)+) {
            composition_lifecycle!(Together; $($P $index $value),+);
            unsafe fn fetch_inner<'r>(state: &'r mut Self::State, pending: &'r mut Self::Pending, context: SystemRunContext<'r>) -> Self::Item<'r> {
                // SAFETY: each position receives its own Together child exactly once.
                ($(unsafe { $P::fetch(&mut state.$index, &mut pending.$index, context.child($index).expect("invalid Together fetch position")) },)+)
            }
            fn finish_inner(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
                $($P::finish(&mut state.$index, pending.$index, &context.child($index).expect("invalid Together finish position"));)+
            }
        }
    };
}
tuple_impl!(P0 0 a);
tuple_impl!(P0 0 a, P1 1 b);
tuple_impl!(P0 0 a, P1 1 b, P2 2 c);
tuple_impl!(P0 0 a, P1 1 b, P2 2 c, P3 3 d);
tuple_impl!(P0 0 a, P1 1 b, P2 2 c, P3 3 d, P4 4 e);
tuple_impl!(P0 0 a, P1 1 b, P2 2 c, P3 3 d, P4 4 e, P5 5 f);
tuple_impl!(P0 0 a, P1 1 b, P2 2 c, P3 3 d, P4 4 e, P5 5 f, P6 6 g);
tuple_impl!(P0 0 a, P1 1 b, P2 2 c, P3 3 d, P4 4 e, P5 5 f, P6 6 g, P7 7 h);

impl<A: SequentialParam, B: SequentialParam> ParamImpl for ParamSet<(A, B)> {
    composition_lifecycle!(Exclusive; A 0 first, B 1 second);

    unsafe fn fetch_inner<'r>(
        state: &'r mut Self::State,
        pending: &'r mut Self::Pending,
        context: SystemRunContext<'r>,
    ) -> Self::Item<'r> {
        ParamSetItem {
            first: &mut state.0,
            second: &mut state.1,
            first_pending: &mut pending.0,
            second_pending: &mut pending.1,
            context,
        }
    }

    fn finish_inner(
        state: &mut Self::State,
        pending: Self::Pending,
        context: &SystemRunContext<'_>,
    ) {
        // SAFETY: the ParamSet and all its member items are gone. Each child is used
        // only for finish, in order; no old binding or business reference is retained.
        let first =
            unsafe { context.exclusive_child(0) }.expect("invalid ParamSet finish position 0");
        A::finish(&mut state.0, pending.0, &first);
        let second =
            unsafe { context.exclusive_child(1) }.expect("invalid ParamSet finish position 1");
        B::finish(&mut state.1, pending.1, &second);
    }
}

impl<P: SystemParam, M: ParamView<P>> ParamImpl for NamedParam<P, M> {
    fn initialize_inner(
        config: P::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<P::State, EcsError> {
        P::initialize(config, registration)
    }
    fn prepare_inner(state: &mut P::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        P::prepare(state, world)
    }
    fn declaration_inner(state: &P::State, world: WorldRef<'_>) -> Result<AccessTree, EcsError> {
        P::declaration(state, world)
    }
    fn ordering_inner(
        state: &P::State,
        logical: &OrderingArchetypes,
    ) -> Result<Vec<OrderingAccess>, EcsError> {
        P::ordering_access(state, logical)
    }
    fn begin_inner(state: &mut P::State) -> P::Pending {
        P::begin(state)
    }
    unsafe fn fetch_inner<'r>(
        state: &'r mut P::State,
        pending: &'r mut P::Pending,
        context: SystemRunContext<'r>,
    ) -> M::Item<'r> {
        // SAFETY: P owns precisely the same authorization tree as NamedParam.
        M::from_param(unsafe { P::fetch(state, pending, context) })
    }
    fn finish_inner(state: &mut P::State, pending: P::Pending, context: &SystemRunContext<'_>) {
        P::finish(state, pending, context);
    }
    fn unregister_inner(
        state: P::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        P::unregister(state, registration)
    }
}

impl<T: Send + 'static> Local<'_, T> {
    pub(super) fn local_rebind<'r>(
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        // Local only borrows its own position's state. No World plan or value needs refresh.
        Ok(context)
    }
}

pub(super) trait RebindImpl: SystemParam {
    unsafe fn rebind_inner<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError>;
}

use pi_ecs_core::access::{
    AccessDeclaration, AccessMode, EntityScope, ResolvedAccess, StructuralAccess,
};
use pi_ecs_core::error::{classified_error, ErrorKind};
#[cfg(feature = "rc")]
use pi_ecs_core::unsafe_world_cell::UnsafeWorldCell;

#[cfg(not(feature = "rc"))]
fn unsupported(feature: &'static str) -> EcsError {
    classified_error(ErrorKind::Unsupported, feature)
}

fn empty_leaf() -> AccessDeclaration {
    AccessDeclaration {
        resources: Box::new([]),
        multi_resources: Box::new([]),
        components: Box::new([]),
        tick_reads: Box::new([]),
        reads_entities: false,
        scope: EntityScope {
            with: Box::new([]),
            without: Box::new([]),
        },
        structure: StructuralAccess::None,
        observations: Box::new([]),
    }
}

fn non_entity_access(leaf: AccessDeclaration) -> ParamAccess {
    ParamAccess::Leaf(vec![ResolvedAccess {
        resources: leaf.resources,
        multi_resources: leaf.multi_resources,
        observations: leaf.observations,
        values: Box::new([]),
        tick_reads: Box::new([]),
        entities: Box::new([]),
        structure: None,
    }])
}

fn non_entity_ordering(declaration: AccessDeclaration) -> Vec<OrderingAccess> {
    vec![OrderingAccess {
        declaration,
        sources: Box::new([]),
        entities: Box::new([]),
        targets: Box::new([]),
    }]
}

#[cfg(feature = "rc")]
fn unique_leaf(state: &ResourceParamState, mode: AccessMode) -> AccessDeclaration {
    AccessDeclaration {
        resources: Box::new([(state.resource, mode)]),
        ..empty_leaf()
    }
}

#[cfg(feature = "rc")]
fn prepare_unique<T: Send + Sync + 'static>(
    state: &ResourceParamState,
    world: WorldRef<'_>,
    mode: AccessMode,
    required: bool,
) -> Result<ParamAccess, EcsError> {
    let leaf = AccessTree::Leaf(unique_leaf(state, mode));
    let revision = UnsafeWorldCell::from_ref(world).archetype_revision().0;
    // SAFETY: prepare has the sole RC World borrow. The only value access is a
    // short shared existence check against this parameter's own leaf; nothing escapes.
    unsafe {
        world.system_scope(&leaf, revision, |access| {
            let exists = access.resource_parts::<T>(state.resource)?.is_some();
            require_unique(exists, required)
        })?
    }?;
    Ok(non_entity_access(unique_leaf(state, mode)))
}

#[cfg(feature = "rc")]
fn require_unique(exists: bool, required: bool) -> Result<(), EcsError> {
    if required && !exists {
        return Err(classified_error(
            ErrorKind::NotFound,
            "required system resource is absent",
        ));
    }
    Ok(())
}

#[cfg(feature = "rc")]
unsafe fn read_unique<'r, T: Send + Sync + 'static>(
    state: &ResourceParamState,
    context: SystemRunContext<'r>,
) -> Option<Res<'r, T>> {
    // SAFETY: only this position's leaf is used; the caller excludes conflicting writes.
    unsafe { context.access.resource_parts::<T>(state.resource) }
        .expect("invalid resource read authority")
        .map(|(value, ticks)| Res {
            value,
            ticks,
            window: context.window,
        })
}

#[cfg(feature = "rc")]
unsafe fn write_unique<'r, T: Send + Sync + 'static>(
    state: &ResourceParamState,
    context: SystemRunContext<'r>,
) -> Option<ResMut<'r, T>> {
    // SAFETY: the caller guarantees exclusive use of this position's resource and tick slots.
    unsafe {
        context
            .access
            .resource_mut::<T>(state.resource, context.window.this_run)
    }
    .expect("invalid resource write authority")
    .map(|value| ResMut {
        value,
        window: context.window,
    })
}

macro_rules! unique_impl {
    ($parameter:ty, $mode:ident, $required:literal, $bind:ident $(, $missing:literal)?) => {
        impl<T: Send + Sync + 'static> ParamImpl for $parameter {
            fn initialize_inner(_: (), registration: &mut ParamRegistration<'_>) -> Result<ResourceParamState, EcsError> {
                let resource = registration.access().resource_id::<T>().ok_or_else(|| classified_error(ErrorKind::NotFound, "system resource type is not registered"))?;
                Ok(ResourceParamState { resource })
            }
            fn prepare_inner(state: &mut ResourceParamState, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
                #[cfg(feature = "rc")]
                { prepare_unique::<T>(state, world, AccessMode::$mode, $required) }
                #[cfg(not(feature = "rc"))]
                { let _ = (state, world); Err(unsupported("non-RC resource binding is unavailable")) }
            }
            fn declaration_inner(state: &ResourceParamState, _: WorldRef<'_>) -> Result<AccessTree, EcsError> {
                #[cfg(feature = "rc")]
                { Ok(AccessTree::Leaf(unique_leaf(state, AccessMode::$mode))) }
                #[cfg(not(feature = "rc"))]
                { let _ = state; Err(unsupported("non-RC resource binding is unavailable")) }
            }
            fn ordering_inner(state: &ResourceParamState, _: &OrderingArchetypes) -> Result<Vec<OrderingAccess>, EcsError> {
                #[cfg(feature = "rc")]
                { Ok(non_entity_ordering(unique_leaf(state, AccessMode::$mode))) }
                #[cfg(not(feature = "rc"))]
                { let _ = state; Err(unsupported("non-RC resource binding is unavailable")) }
            }
            unit_pending!();
            unsafe fn fetch_inner<'r>(state: &'r mut ResourceParamState, _: &'r mut (), context: SystemRunContext<'r>) -> Self::Item<'r> {
                #[cfg(feature = "rc")]
                { unsafe { $bind::<T>(state, context) } $(.expect($missing))? }
                #[cfg(not(feature = "rc"))]
                { let _ = (state, context); panic!("non-RC resource binding is unavailable") }
            }
            fn unregister_inner(_: ResourceParamState, _: &mut ParamRegistration<'_>) -> Result<(), EcsError> { Ok(()) }
        }

        impl<T: Send + Sync + 'static> RebindImpl for $parameter {
            unsafe fn rebind_inner<'r>(state: &mut ResourceParamState, context: SystemRunContext<'r>) -> Result<SystemRunContext<'r>, EcsError> {
                #[cfg(feature = "rc")]
                {
                    // SAFETY: no sibling result survives the ParamSet short borrow. This
                    // temporary shared binding ends before fetch can issue a write binding.
                    let exists = unsafe { context.access.resource_parts::<T>(state.resource)? }.is_some();
                    require_unique(exists, $required)?;
                    Ok(context)
                }
                #[cfg(not(feature = "rc"))]
                { let _ = (state, context); Err(unsupported("non-RC resource binding is unavailable")) }
            }
        }
    };
}

unique_impl!(
    Res<'_, T>,
    Read,
    true,
    read_unique,
    "required Res is absent"
);
unique_impl!(Option<Res<'_, T>>, Read, false, read_unique);
unique_impl!(
    ResMut<'_, T>,
    Write,
    true,
    write_unique,
    "required ResMut is absent"
);
unique_impl!(Option<ResMut<'_, T>>, Write, false, write_unique);

#[cfg(feature = "rc")]
fn matches_query(
    query: &pi_ecs_core::query::QueryAccessDeclaration,
    has: &impl Fn(pi_ecs_core::component::ComponentId) -> bool,
) -> bool {
    use pi_ecs_core::query::QueryItemPresence::Required;
    use pi_ecs_core::query_filter::FilterExpression;
    fn matches_filter(
        filter: &FilterExpression,
        has: &impl Fn(pi_ecs_core::component::ComponentId) -> bool,
    ) -> bool {
        use FilterExpression::*;
        match filter {
            All => true,
            None => false,
            With(id) | Added(id) | Changed(id) => has(*id),
            Without(id) => !has(*id),
            And(terms) => terms.iter().all(|term| matches_filter(term, has)),
            Or(terms) => terms.iter().any(|term| matches_filter(term, has)),
        }
    }
    query
        .data
        .components
        .iter()
        .all(|item| item.presence != Required || has(item.component))
        && matches_filter(&query.filter.expression, has)
}

#[cfg(feature = "rc")]
fn alter_target(
    source: &[pi_ecs_core::component::ComponentId],
    added: &[pi_ecs_core::component::ComponentId],
    removed: &[pi_ecs_core::component::ComponentId],
) -> Option<Vec<pi_ecs_core::component::ComponentId>> {
    let has = |id| source.binary_search(id).is_ok();
    if (added.is_empty() && removed.is_empty()) || added.iter().any(has) || !removed.iter().all(has)
    {
        return None;
    }
    let mut target: Vec<_> = source
        .iter()
        .copied()
        .filter(|id| !removed.contains(id))
        .chain(added.iter().copied())
        .collect();
    target.sort_unstable();
    Some(target)
}

#[cfg(feature = "rc")]
fn prepare_query<Q: QueryData, F: QueryFilter>(
    state: &mut QueryState<Q, F>,
    world: WorldRef<'_>,
) -> Result<ParamAccess, EcsError> {
    let query = state.declaration()?;
    prepare_query_access(world, &query, query.to_access()?, |access| {
        // SAFETY: preparation has no live parameter results or cache views.
        unsafe { access.refresh_query(state) }.map(|_| ())
    })
}

#[cfg(feature = "rc")]
fn prepare_query_access(
    world: WorldRef<'_>,
    query: &pi_ecs_core::query::QueryAccessDeclaration,
    access: pi_ecs_core::access::AccessDeclaration,
    refresh: impl FnOnce(pi_ecs_core::access::RunAccess<'_>) -> Result<(), EcsError>,
) -> Result<ParamAccess, EcsError> {
    use pi_ecs_core::access::{ResolvedStructureAccess, ResolvedValueAccess, StructuralAccess};
    let leaf = AccessTree::Leaf(access);
    let revision = UnsafeWorldCell::from_ref(world).archetype_revision().0;
    // SAFETY: this short RC scope performs only the metadata refresh of this exact leaf.
    unsafe { world.system_scope(&leaf, revision, refresh)? }?;
    let AccessTree::Leaf(access) = leaf else {
        unreachable!()
    };
    let archetypes = world.archetypes()?;
    let mut matched: Vec<_> = archetypes
        .iter()
        .filter(|arch| {
            matches_query(query, &|id| {
                arch.components()
                    .binary_search_by_key(&id, |part| part.component())
                    .is_ok()
            })
        })
        .collect();
    matched.sort_unstable_by_key(|arch| (arch.id().table, arch.id().variant));
    let expand = |component, mode| ResolvedValueAccess {
        component,
        mode,
        archetypes: matched
            .iter()
            .filter(|arch| {
                arch.components()
                    .binary_search_by_key(&component, |part| part.component())
                    .is_ok()
            })
            .map(|arch| arch.id())
            .collect(),
    };
    let structure = match access.structure {
        StructuralAccess::None => None,
        StructuralAccess::Spawn { .. } => {
            return Err(pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::InvalidState,
                "Spawn 不使用查询参数准备入口",
            ));
        }
        StructuralAccess::Alter { added, removed } => {
            let published: std::collections::HashMap<Box<[_]>, _> = archetypes
                .iter()
                .map(|arch| {
                    let components: Box<[_]> = arch
                        .components()
                        .iter()
                        .map(|part| part.component())
                        .collect();
                    (components, arch.id())
                })
                .collect();
            let mut sources = Vec::new();
            let mut targets = Vec::new();
            for source in &matched {
                let components: Vec<_> = source
                    .components()
                    .iter()
                    .map(|part| part.component())
                    .collect();
                if let Some(target) = alter_target(&components, &added, &removed) {
                    sources.push(source.id());
                    targets.extend(published.get(target.as_slice()).copied());
                }
            }
            targets.sort_unstable_by_key(|id| (id.table, id.variant));
            targets.dedup();
            Some(ResolvedStructureAccess {
                sources: sources.into_boxed_slice(),
                targets: targets.into_boxed_slice(),
                added,
                removed,
            })
        }
    };
    Ok(ParamAccess::Leaf(vec![ResolvedAccess {
        resources: access.resources,
        multi_resources: access.multi_resources,
        observations: access.observations,
        values: access
            .components
            .iter()
            .map(|&(component, mode)| expand(component, mode))
            .collect(),
        tick_reads: access
            .tick_reads
            .iter()
            .map(|&component| expand(component, AccessMode::Read))
            .collect(),
        entities: matched.iter().map(|arch| arch.id()).collect(),
        structure,
    }]))
}

#[cfg(feature = "rc")]
fn order_query<Q: QueryData, F: QueryFilter>(
    state: &QueryState<Q, F>,
    logical: &OrderingArchetypes,
) -> Result<Vec<OrderingAccess>, EcsError> {
    let query = state.declaration()?;
    Ok(order_query_access(&query, query.to_access()?, logical))
}

#[cfg(feature = "rc")]
fn order_query_access(
    query: &pi_ecs_core::query::QueryAccessDeclaration,
    mut declaration: pi_ecs_core::access::AccessDeclaration,
    logical: &OrderingArchetypes,
) -> Vec<OrderingAccess> {
    use pi_ecs_core::access::StructuralAccess;
    let entities: Box<[_]> = logical
        .components
        .iter()
        .enumerate()
        .filter_map(|(index, ids)| {
            matches_query(query, &|id| ids.binary_search(&id).is_ok()).then_some(index)
        })
        .collect();
    let structure = std::mem::replace(&mut declaration.structure, StructuralAccess::None);
    // Keep the full query match range for member and structural comparisons.
    // StagePlan intersects it with component presence for value and live tick access.
    // The separate structural entry keeps strict conversion from narrowing query access.
    let mut structural = None;
    if let StructuralAccess::Alter { added, removed } = &structure {
        let by_components: std::collections::HashMap<_, _> = logical
            .components
            .iter()
            .enumerate()
            .map(|(index, ids)| (ids.as_ref(), index))
            .collect();
        let mut sources = Vec::new();
        let mut targets = Vec::new();
        for &source in &entities {
            if let Some(target) = alter_target(&logical.components[source], added, removed) {
                sources.push(source);
                targets.extend(by_components.get(target.as_slice()).copied());
            }
        }
        targets.sort_unstable();
        targets.dedup();
        let mut declaration = empty_leaf();
        declaration.structure = structure;
        structural = Some(OrderingAccess {
            declaration,
            sources: sources.into_boxed_slice(),
            entities: Box::new([]),
            targets: targets.into_boxed_slice(),
        });
    }
    let mut ordering = vec![OrderingAccess {
        declaration,
        sources: entities.clone(),
        entities,
        targets: Box::new([]),
    }];
    ordering.extend(structural);
    ordering
}

macro_rules! query_impl {
    ($parameter:ident, $data:ident, $bind:ident) => {
        impl<Q: $data + 'static, F: QueryFilter + 'static> ParamImpl for $parameter<'_, Q, F> {
            fn initialize_inner(
                _: (),
                registration: &mut ParamRegistration<'_>,
            ) -> Result<Self::State, EcsError> {
                #[cfg(feature = "rc")]
                {
                    registration.access().query_state::<Q, F>()
                }
                #[cfg(not(feature = "rc"))]
                {
                    let _ = registration;
                    Err(unsupported("non-RC query registration is unavailable"))
                }
            }
            fn prepare_inner(
                state: &mut Self::State,
                world: WorldRef<'_>,
            ) -> Result<ParamAccess, EcsError> {
                #[cfg(feature = "rc")]
                {
                    prepare_query(state, world)
                }
                #[cfg(not(feature = "rc"))]
                {
                    let _ = (state, world);
                    Err(unsupported("non-RC query preparation is unavailable"))
                }
            }
            fn declaration_inner(
                state: &Self::State,
                _: WorldRef<'_>,
            ) -> Result<AccessTree, EcsError> {
                Ok(AccessTree::Leaf(state.declaration()?.to_access()?))
            }
            fn ordering_inner(
                state: &Self::State,
                logical: &OrderingArchetypes,
            ) -> Result<Vec<OrderingAccess>, EcsError> {
                #[cfg(feature = "rc")]
                {
                    order_query(state, logical)
                }
                #[cfg(not(feature = "rc"))]
                {
                    let _ = (state, logical);
                    Err(unsupported("non-RC query ordering is unavailable"))
                }
            }
            unit_pending!();
            unsafe fn fetch_inner<'r>(
                state: &'r mut Self::State,
                _: &'r mut (),
                context: SystemRunContext<'r>,
            ) -> Self::Item<'r> {
                #[cfg(feature = "rc")]
                {
                    unsafe { context.access.$bind(state, context.window) }
                        .expect("invalid query binding")
                }
                #[cfg(not(feature = "rc"))]
                {
                    let _ = (state, context);
                    panic!("non-RC query binding is unavailable")
                }
            }
            fn unregister_inner(
                mut state: Self::State,
                registration: &mut ParamRegistration<'_>,
            ) -> Result<(), EcsError> {
                #[cfg(feature = "rc")]
                {
                    registration.access().unregister_query(&mut state)
                }
                #[cfg(not(feature = "rc"))]
                {
                    let _ = (&mut state, registration);
                    Err(unsupported("non-RC query registration is unavailable"))
                }
            }
        }

        impl<Q: $data + 'static, F: QueryFilter + 'static> RebindImpl for $parameter<'_, Q, F> {
            unsafe fn rebind_inner<'r>(
                state: &mut Self::State,
                context: SystemRunContext<'r>,
            ) -> Result<SystemRunContext<'r>, EcsError> {
                let (access, window) = context.into_parts();
                // SAFETY: ParamSet ends all results before entering this leaf again.
                SystemRunContext::new(unsafe { access.refresh_query(state)? }, window)
            }
        }
    };
}

query_impl!(Query, ReadOnlyQueryData, bind_query);
query_impl!(QueryMut, QueryData, bind_query_mut);
query_impl!(Lookup, QueryData, bind_lookup);

#[cfg(feature = "rc")]
fn command_world(expected: u64, actual: u64) -> Result<(), EcsError> {
    if expected != actual {
        return Err(classified_error(
            ErrorKind::InvalidState,
            "command parameter belongs to a different World",
        ));
    }
    Ok(())
}

#[cfg(feature = "rc")]
macro_rules! command_impl {
    ($parameter:ty, $state:ident, $handle:ident, $get:ident, $($mutable:tt)?) => {
        impl<C: Send + 'static> ParamImpl for $parameter {
            fn initialize_inner(_: (), registration: &mut ParamRegistration<'_>) -> Result<Self::State, EcsError> {
                let access = registration.access();
                let world_identity = access.world_id();
                Ok($state { world_identity, $handle: access.$get::<C>()? })
            }
            fn prepare_inner(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
                command_world(state.world_identity, UnsafeWorldCell::from_ref(world).id())?;
                Ok(ParamAccess::Together(Vec::new()))
            }
            fn declaration_inner(state: &Self::State, world: WorldRef<'_>) -> Result<AccessTree, EcsError> {
                command_world(state.world_identity, UnsafeWorldCell::from_ref(world).id())?;
                Ok(AccessTree::Together(Vec::new()))
            }
            fn ordering_inner(_: &Self::State, _: &OrderingArchetypes) -> Result<Vec<OrderingAccess>, EcsError> {
                Ok(Vec::new())
            }
            unit_pending!();
            unsafe fn fetch_inner<'r>(state: &'r mut Self::State, _: &'r mut (), context: SystemRunContext<'r>) -> Self::Item<'r> {
                command_world(state.world_identity, context.access.world_cell().id()).expect("invalid command parameter source");
                &$($mutable)? state.$handle
            }
            fn unregister_inner(state: Self::State, registration: &mut ParamRegistration<'_>) -> Result<(), EcsError> {
                let result = command_world(state.world_identity, registration.access().world_id());
                drop(state);
                result
            }
        }
    };
}

#[cfg(feature = "rc")]
command_impl!(
    &CommandSender<C>,
    CommandSenderParamState,
    sender,
    command_sender,
);
#[cfg(feature = "rc")]
command_impl!(
    &mut CommandReceiver<C>,
    CommandReceiverParamState,
    receiver,
    command_receiver,
    mut
);

#[cfg(feature = "rc")]
fn reader_leaf(state: &ReaderParamState) -> AccessDeclaration {
    AccessDeclaration {
        observations: Box::new([(state.component, state.kind)]),
        ..empty_leaf()
    }
}

#[cfg(feature = "rc")]
macro_rules! reader_impl {
    ($parameter:ident) => {
        impl<T: pi_ecs_core::component::Component> $parameter<'_, T> {
            pub(super) fn reader_declaration_inner(
                state: &ReaderParamState,
                world: WorldRef<'_>,
            ) -> Result<AccessTree, EcsError> {
                world.notifications().is_empty(&state.subscription)?;
                Ok(AccessTree::Leaf(reader_leaf(state)))
            }
            pub(super) fn reader_ordering_inner(
                state: &ReaderParamState,
                _: &OrderingArchetypes,
            ) -> Result<Vec<OrderingAccess>, EcsError> {
                Ok(non_entity_ordering(reader_leaf(state)))
            }
        }
        impl<T: pi_ecs_core::component::Component> RebindImpl for $parameter<'_, T> {
            unsafe fn rebind_inner<'r>(
                state: &mut ReaderParamState,
                context: SystemRunContext<'r>,
            ) -> Result<SystemRunContext<'r>, EcsError> {
                context.access.fetch_access()?;
                // SAFETY: the Reader's original observation leaf is used after the prior
                // short borrow has ended. is_empty checks the subscription without advancing it.
                unsafe { context.access.world_cell().notifications() }
                    .is_empty(&state.subscription)?;
                Ok(context)
            }
        }
    };
}
#[cfg(feature = "rc")]
reader_impl!(AddedReader);
#[cfg(feature = "rc")]
reader_impl!(ChangedReader);
#[cfg(feature = "rc")]
reader_impl!(RemovedReader);

#[cfg(not(feature = "rc"))]
const NON_RC_SPAWN: &str = "non-RC Spawn is unavailable";

impl<B: SystemBundle<Shape = B> + 'static> ParamImpl for Spawn<'_, B> {
    fn initialize_inner(
        config: SpawnConfig,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        #[cfg(feature = "rc")]
        {
            registration.access().spawn_state::<B>(config.partition)
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (config, registration);
            Err(unsupported(NON_RC_SPAWN))
        }
    }
    fn prepare_inner(
        state: &mut Self::State,
        world: WorldRef<'_>,
    ) -> Result<ParamAccess, EcsError> {
        #[cfg(feature = "rc")]
        {
            use pi_ecs_core::access::{ResolvedStructureAccess, StructuralAccess};
            let AccessTree::Leaf(declaration) = Self::declaration_inner(state, world)? else {
                unreachable!()
            };
            let StructuralAccess::Spawn { components, .. } = declaration.structure else {
                unreachable!()
            };
            let targets = world
                .archetypes()?
                .iter()
                .filter_map(|arch| {
                    arch.components()
                        .iter()
                        .map(|entry| entry.component())
                        .eq(components.iter().copied())
                        .then_some(arch.id())
                })
                .collect();
            Ok(ParamAccess::Leaf(vec![ResolvedAccess {
                resources: Box::new([]),
                multi_resources: Box::new([]),
                observations: Box::new([]),
                values: Box::new([]),
                tick_reads: Box::new([]),
                entities: Box::new([]),
                structure: Some(ResolvedStructureAccess {
                    sources: Box::new([]),
                    targets,
                    added: components,
                    removed: Box::new([]),
                }),
            }]))
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, world);
            Err(unsupported(NON_RC_SPAWN))
        }
    }
    fn declaration_inner(state: &Self::State, world: WorldRef<'_>) -> Result<AccessTree, EcsError> {
        #[cfg(feature = "rc")]
        {
            if state.world_id() != UnsafeWorldCell::from_ref(world).id() {
                return Err(classified_error(
                    ErrorKind::InvalidState,
                    "Spawn parameter belongs to a different World",
                ));
            }
            Ok(AccessTree::Leaf(state.declaration()))
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, world);
            Err(unsupported(NON_RC_SPAWN))
        }
    }
    fn ordering_inner(
        state: &Self::State,
        logical: &OrderingArchetypes,
    ) -> Result<Vec<OrderingAccess>, EcsError> {
        #[cfg(feature = "rc")]
        {
            let declaration = state.declaration();
            let pi_ecs_core::access::StructuralAccess::Spawn { components, .. } =
                &declaration.structure
            else {
                unreachable!()
            };
            let target = logical
                .components
                .iter()
                .position(|ids| ids == components)
                .ok_or_else(|| {
                    classified_error(
                        ErrorKind::InvalidState,
                        "Spawn target missing from stage ordering",
                    )
                })?;
            Ok(vec![OrderingAccess {
                declaration,
                sources: Box::new([]),
                entities: Box::new([]),
                targets: Box::new([target]),
            }])
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, logical);
            Err(unsupported(NON_RC_SPAWN))
        }
    }
    unit_pending!();
    unsafe fn fetch_inner<'r>(
        state: &'r mut Self::State,
        _: &'r mut (),
        context: SystemRunContext<'r>,
    ) -> Self::Item<'r> {
        #[cfg(feature = "rc")]
        {
            unsafe { context.access.bind_spawn(state, context.window.this_run) }
                .expect("invalid Spawn binding")
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, context);
            panic!("{NON_RC_SPAWN}")
        }
    }
    fn unregister_inner(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        if state.world_id() != registration.access().world_id() {
            return Err(classified_error(
                ErrorKind::InvalidState,
                "Spawn parameter belongs to a different World",
            ));
        }
        #[cfg(not(feature = "rc"))]
        let _ = (state, registration);
        Ok(())
    }
}

impl<B: SystemBundle<Shape = B> + 'static> RebindImpl for Spawn<'_, B> {
    unsafe fn rebind_inner<'r>(
        _: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        Ok(context)
    }
}

#[cfg(not(feature = "rc"))]
const NON_RC_ALTER: &str = "non-RC QueryAlter is unavailable";
impl<Q, F, Add, Remove> ParamImpl for QueryAlter<'_, Q, F, Add, Remove>
where
    Q: QueryData + 'static,
    F: QueryFilter + 'static,
    Add: SystemBundle<Shape = Add> + 'static,
    Remove: Bundle + 'static,
{
    fn initialize_inner(
        (): Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        #[cfg(feature = "rc")]
        {
            registration
                .access()
                .typed_query_alter_state::<Q, F, Add, Remove>()
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = registration;
            Err(unsupported(NON_RC_ALTER))
        }
    }
    fn prepare_inner(
        state: &mut Self::State,
        world: WorldRef<'_>,
    ) -> Result<ParamAccess, EcsError> {
        #[cfg(feature = "rc")]
        {
            let (query, declaration) = state.declaration()?;
            prepare_query_access(world, &query, declaration, |access| {
                // SAFETY: preparation has no live results and uses this state's own leaf.
                unsafe { access.refresh_alter(state) }.map(|_| ())
            })
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, world);
            Err(unsupported(NON_RC_ALTER))
        }
    }
    fn declaration_inner(state: &Self::State, _: WorldRef<'_>) -> Result<AccessTree, EcsError> {
        #[cfg(feature = "rc")]
        {
            Ok(AccessTree::Leaf(state.declaration()?.1))
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = state;
            Err(unsupported(NON_RC_ALTER))
        }
    }
    fn ordering_inner(
        state: &Self::State,
        logical: &OrderingArchetypes,
    ) -> Result<Vec<OrderingAccess>, EcsError> {
        #[cfg(feature = "rc")]
        {
            let (query, declaration) = state.declaration()?;
            Ok(order_query_access(&query, declaration, logical))
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, logical);
            Err(unsupported(NON_RC_ALTER))
        }
    }
    unit_pending!();
    unsafe fn fetch_inner<'r>(
        state: &'r mut Self::State,
        _: &'r mut (),
        context: SystemRunContext<'r>,
    ) -> Self::Item<'r> {
        #[cfg(feature = "rc")]
        {
            unsafe { context.access.bind_alter(state, context.window) }
                .expect("invalid QueryAlter binding")
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, context);
            panic!("{NON_RC_ALTER}")
        }
    }
    fn unregister_inner(
        mut state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        {
            registration.access().unregister_query_alter(&mut state)
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (&mut state, registration);
            Err(unsupported(NON_RC_ALTER))
        }
    }
}
impl<Q, F, Add, Remove> RebindImpl for QueryAlter<'_, Q, F, Add, Remove>
where
    Q: QueryData + 'static,
    F: QueryFilter + 'static,
    Add: SystemBundle<Shape = Add> + 'static,
    Remove: Bundle + 'static,
{
    unsafe fn rebind_inner<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        let (access, window) = context.into_parts();
        // SAFETY: the earlier member view has ended; Core checks the exact Alter leaf.
        SystemRunContext::new(unsafe { access.refresh_alter(state)? }, window)
    }
}

#[cfg(not(feature = "rc"))]
const NON_RC_PRODUCER: &str = "non-RC multi-resource producers are unavailable";

#[cfg(feature = "rc")]
fn producer_leaf(state: &MultiResourceProducerState) -> AccessDeclaration {
    let (resource, producer) = state.registration.slot_identity();
    AccessDeclaration {
        multi_resources: Box::new([(
            resource,
            pi_ecs_core::access::MultiResourceAccess::WriteSlot { producer },
        )]),
        ..empty_leaf()
    }
}

impl<T: Send + Sync + 'static> ParamImpl for MultiResMut<'_, T> {
    fn initialize_inner(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        #[cfg(feature = "rc")]
        {
            let (system, parameter) = registration.producer_position()?;
            Ok(MultiResourceProducerState {
                registration: registration.access().register_producer(config.initial)?,
                producer: crate::resource_param::ProducerState {
                    system,
                    parameter,
                    binding: None,
                },
                recovery: config.recovery,
            })
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (config, registration);
            Err(unsupported(NON_RC_PRODUCER))
        }
    }
    fn prepare_inner(
        state: &mut Self::State,
        world: WorldRef<'_>,
    ) -> Result<ParamAccess, EcsError> {
        #[cfg(feature = "rc")]
        {
            // Even with unchanged archetypes, this collection may have compacted.
            let binding = match world.producer_binding(&state.registration) {
                Ok(binding) => binding,
                Err(error) => {
                    state.producer.binding = None;
                    return Err(error);
                }
            };
            if state
                .producer
                .binding
                .as_ref()
                .is_none_or(|cached| cached.structure_version != binding.structure_version)
            {
                state.producer.binding = Some(binding);
            }
            Ok(non_entity_access(producer_leaf(state)))
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, world);
            Err(unsupported(NON_RC_PRODUCER))
        }
    }
    fn declaration_inner(state: &Self::State, _: WorldRef<'_>) -> Result<AccessTree, EcsError> {
        #[cfg(feature = "rc")]
        {
            Ok(AccessTree::Leaf(producer_leaf(state)))
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = state;
            Err(unsupported(NON_RC_PRODUCER))
        }
    }
    fn ordering_inner(
        state: &Self::State,
        _: &OrderingArchetypes,
    ) -> Result<Vec<OrderingAccess>, EcsError> {
        #[cfg(feature = "rc")]
        {
            Ok(non_entity_ordering(producer_leaf(state)))
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = state;
            Err(unsupported(NON_RC_PRODUCER))
        }
    }
    unit_pending!();
    unsafe fn fetch_inner<'r>(
        state: &'r mut Self::State,
        _: &'r mut (),
        context: SystemRunContext<'r>,
    ) -> Self::Item<'r> {
        #[cfg(feature = "rc")]
        {
            let binding = state.producer.binding.as_ref().unwrap_or_else(|| {
                panic!(
                    "missing prepared multi-resource binding for {:?} parameter {}",
                    state.producer.system, state.producer.parameter,
                )
            });
            // SAFETY: the caller excludes conflicting references; Core checks this leaf,
            // the registration and the binding's current collection version.
            let value = unsafe {
                context.access.producer_mut::<T>(
                    &state.registration,
                    binding,
                    context.window.this_run,
                )
            }
            .expect("invalid multi-resource producer binding");
            MultiResMut {
                value,
                window: context.window,
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, context);
            panic!("{NON_RC_PRODUCER}")
        }
    }
    fn unregister_inner(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        {
            registration
                .access()
                .unregister_producer::<T>(state.registration)
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, registration);
            Err(unsupported(NON_RC_PRODUCER))
        }
    }
}
impl<T: Send + Sync + 'static> RebindImpl for MultiResMut<'_, T> {
    unsafe fn rebind_inner<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        #[cfg(feature = "rc")]
        {
            // Slots cannot move during execution. No view is retained between borrows;
            // the immediately following fetch validates the version and projects the slot.
            if state.producer.binding.is_none() {
                return Err(classified_error(
                    ErrorKind::InvalidState,
                    "missing prepared producer binding",
                ));
            }
            Ok(context)
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, context);
            Err(unsupported(NON_RC_PRODUCER))
        }
    }
}

#[cfg(feature = "rc")]
fn multi_read_leaf(state: &MultiResourceParamState) -> AccessDeclaration {
    use pi_ecs_core::access::MultiResourceAccess;
    AccessDeclaration {
        multi_resources: Box::new([(state.resource, MultiResourceAccess::ReadAll)]),
        ..empty_leaf()
    }
}

impl<T: Send + Sync + 'static> ParamImpl for MultiRes<'_, T> {
    fn initialize_inner(
        _: (),
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        let resource = registration
            .access()
            .multi_resource_id::<T>()
            .ok_or_else(|| {
                classified_error(ErrorKind::NotFound, "multi-resource type is not registered")
            })?;
        Ok(MultiResourceParamState {
            resource,
            previous_version: None,
        })
    }
    fn prepare_inner(
        state: &mut Self::State,
        world: WorldRef<'_>,
    ) -> Result<ParamAccess, EcsError> {
        #[cfg(feature = "rc")]
        {
            let leaf = AccessTree::Leaf(multi_read_leaf(state));
            let revision = UnsafeWorldCell::from_ref(world).archetype_revision().0;
            // SAFETY: no live parameter results during prepare; only this read leaf is probed.
            unsafe {
                world.system_scope(&leaf, revision, |access| {
                    access.multi_resource_parts::<T>(state.resource).map(|_| ())
                })?
            }?;
            Ok(non_entity_access(multi_read_leaf(state)))
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, world);
            Err(unsupported("non-RC multi-resource binding is unavailable"))
        }
    }
    fn declaration_inner(state: &Self::State, _: WorldRef<'_>) -> Result<AccessTree, EcsError> {
        #[cfg(feature = "rc")]
        {
            Ok(AccessTree::Leaf(multi_read_leaf(state)))
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = state;
            Err(unsupported("non-RC multi-resource binding is unavailable"))
        }
    }
    fn ordering_inner(
        state: &Self::State,
        _: &OrderingArchetypes,
    ) -> Result<Vec<OrderingAccess>, EcsError> {
        #[cfg(feature = "rc")]
        {
            Ok(non_entity_ordering(multi_read_leaf(state)))
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = state;
            Err(unsupported("non-RC multi-resource binding is unavailable"))
        }
    }
    fn begin_inner(_: &mut Self::State) -> Self::Pending {
        MultiResourcePending {
            observed_version: None,
        }
    }
    unsafe fn fetch_inner<'r>(
        state: &'r mut Self::State,
        pending: &'r mut Self::Pending,
        context: SystemRunContext<'r>,
    ) -> Self::Item<'r> {
        #[cfg(feature = "rc")]
        {
            // SAFETY: the ReadAll leaf excludes every writer of this collection for this borrow.
            let view = unsafe { context.access.multi_resource_parts::<T>(state.resource) }
                .expect("invalid multi-resource read binding");
            pending.observed_version = Some(view.structure_version);
            MultiRes {
                values: view.values,
                ticks: view.ticks,
                structure_version: view.structure_version,
                previous_structure_version: state.previous_version,
                window: context.window,
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, pending, context);
            panic!("non-RC multi-resource binding is unavailable")
        }
    }
    fn finish_inner(state: &mut Self::State, pending: Self::Pending, _: &SystemRunContext<'_>) {
        if let Some(version) = pending.observed_version {
            state.previous_version = Some(version);
        }
    }
    fn unregister_inner(_: Self::State, _: &mut ParamRegistration<'_>) -> Result<(), EcsError> {
        Ok(())
    }
}
impl<T: Send + Sync + 'static> RebindImpl for MultiRes<'_, T> {
    unsafe fn rebind_inner<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        #[cfg(feature = "rc")]
        {
            // SAFETY: the previous short borrow has ended; the authorized view is not retained.
            unsafe { context.access.multi_resource_parts::<T>(state.resource)? };
            Ok(context)
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (state, context);
            Err(unsupported("non-RC multi-resource binding is unavailable"))
        }
    }
}
