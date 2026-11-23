//! 类型化过滤状态与组合分派。
use super::*;
use crate::{
    error::{classified_error, ErrorKind},
    query_data::{ComponentStorageBinding, ComponentView},
    tracking::ComponentTickDemand,
};

pub(crate) trait FilterParts: QueryFilter {
    fn init(world: &World) -> Result<Self::State, EcsError>;
    fn declaration(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError>;
    fn bind(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError>;
    unsafe fn run<'a>(
        access: &FetchAccess<'a>,
        scope: FetchScope,
        state: &Self::State,
        plan: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError>;
}

#[cfg(all(test, feature = "rc"))]
#[test]
fn typed_filter_binding_keeps_sparse_membership_variant_specific() {
    use crate::{
        component::{Component, PlainAccess, StoragePolicy},
        query::FetchStateResult::*,
    };
    struct Tagged;
    impl Component for Tagged {
        type Access = PlainAccess;
    }
    let mut world = World::new().unwrap();
    world
        .register_component::<Tagged>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let binding = crate::query_fetch::FetchBinding {
        storage: world.storage.as_ref().unwrap(),
        world_identity: world.identity,
        archetypes: &world.registry.archetypes,
    };
    let state = With::<Tagged>::init_state(&world).unwrap();
    assert!(matches!(
        <With<Tagged> as FilterParts>::bind(
            &state,
            FetchScope::Table {
                table: crate::entity::TableId(0)
            },
            &binding
        )
        .unwrap(),
        NeedsVariants
    ));
    assert!(matches!(
        <With<Tagged> as FilterParts>::bind(
            &state,
            FetchScope::new(&binding.archetypes[0]),
            &binding
        )
        .unwrap(),
        NotMatched
    ));
    world.close().unwrap();
}

fn no_demands(expression: FilterExpression) -> QueryFilterDeclaration {
    QueryFilterDeclaration {
        expression,
        tracking: Box::new([]),
        tick_reads: Box::new([]),
    }
}

impl FilterParts for crate::query::NoFilter {
    fn init(_: &World) -> Result<(), EcsError> {
        Ok(())
    }
    fn bind(
        _: &(),
        _: FetchScope,
        _: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<()>, EcsError> {
        Ok(crate::query::FetchStateResult::Ready(()))
    }
    fn declaration(_: &()) -> Result<QueryFilterDeclaration, EcsError> {
        Ok(no_demands(FilterExpression::All))
    }
    unsafe fn run<'a>(
        _: &FetchAccess<'a>,
        _: FetchScope,
        _: &(),
        _: &(),
    ) -> Result<Self::Fetch<'a>, EcsError> {
        Ok(crate::query::NoFilterFetch)
    }
}

fn resolve<T: 'static>(world: &World) -> Result<ComponentId, EcsError> {
    world
        .registry
        .component_ids
        .get(&std::any::TypeId::of::<T>())
        .copied()
        .ok_or_else(|| classified_error(ErrorKind::NotFound, "过滤组件未登记"))
}

fn leaf_expression(state: &CompositeFilterState) -> FilterExpression {
    use FilterExpression::*;
    match state.expression {
        All => All,
        None => None,
        With(id) => With(id),
        Without(id) => Without(id),
        Added(id) => Added(id),
        Changed(id) => Changed(id),
        And(_) | Or(_) => unreachable!("组合过滤不使用叶子表达式"),
    }
}

fn bind_leaf(
    state: &CompositeFilterState,
    scope: FetchScope,
    binding: &crate::query_fetch::FetchBinding<'_>,
) -> Result<crate::query::FetchStateResult<CompositeFilterState>, EcsError> {
    use crate::{archetype::ComponentSource, query::FetchStateResult::*};
    use FilterExpression::*;
    let id = match state.expression {
        None => return Ok(NotMatched),
        All => {
            return Ok(Ready(CompositeFilterState {
                expression: All,
                tick_storages: Some(Box::new([])),
            }))
        }
        With(id) | Without(id) | Added(id) | Changed(id) => id,
        And(_) | Or(_) => unreachable!("组合过滤不使用叶子绑定"),
    };
    let (archetype_id, uniform) = match scope {
        FetchScope::Table { table } => (
            crate::entity::ArchetypeId {
                table,
                variant: crate::entity::VariantId(0),
            },
            true,
        ),
        FetchScope::Archetype { archetype } => (archetype, false),
    };
    let archetype = binding
        .archetypes
        .iter()
        .find(|entry| entry.id() == archetype_id)
        .ok_or_else(|| classified_error(ErrorKind::NotFound, "过滤范围不存在"))?;
    let entries = archetype.components();
    let source = entries
        .binary_search_by_key(&id, |entry| entry.component())
        .ok()
        .map(|index| entries[index].source());
    if uniform && !matches!(source, Some(ComponentSource::Table(_))) {
        return Ok(NeedsVariants);
    }
    if source.is_some() == matches!(state.expression, Without(_)) {
        return Ok(NotMatched);
    }
    let mut tick_storages = Vec::new();
    if matches!(state.expression, Added(_) | Changed(_)) {
        let storage = match source.expect("变化过滤只匹配含该组件的原型") {
            ComponentSource::Table(column) => {
                ComponentStorageBinding::Column(binding.column_address(archetype_id.table, column)?)
            }
            ComponentSource::Sparse => ComponentStorageBinding::Sparse(binding.sparse_address(id)?),
        };
        tick_storages.push(storage);
    }
    Ok(Ready(CompositeFilterState {
        expression: leaf_expression(state),
        tick_storages: Some(tick_storages.into_boxed_slice()),
    }))
}

unsafe fn run_leaf<'a>(
    access: &FetchAccess<'a>,
    state: &CompositeFilterState,
    plan: &CompositeFilterState,
) -> Result<CompositeFilterFetch<'a>, EcsError> {
    let sources = plan
        .tick_storages
        .as_ref()
        .ok_or_else(|| classified_error(ErrorKind::InvalidState, "过滤未绑定"))?;
    let window = access.window;
    if !sources.is_empty() && window.is_none() {
        return Err(classified_error(
            ErrorKind::InvalidState,
            "变化过滤需要观察窗口",
        ));
    }
    if let FilterExpression::Added(id) | FilterExpression::Changed(id) = state.expression {
        if !access.declaration.tick_reads.contains(&id) {
            return Err(classified_error(
                ErrorKind::InvalidState,
                "变化过滤缺少 tick 权限",
            ));
        }
    }
    let ticks = sources
        .iter()
        .map(|source| match source {
            ComponentStorageBinding::Missing => ComponentView::Missing,
            // SAFETY: bound roots belong to the original authorization, which excludes maintenance.
            ComponentStorageBinding::Column(pointer) => {
                ComponentView::Column(unsafe { pointer.as_ref().bind_view() })
            }
            ComponentStorageBinding::Sparse(pointer) => {
                ComponentView::Sparse(unsafe { pointer.as_ref().bind_view() })
            }
        })
        .collect();
    Ok(CompositeFilterFetch {
        access: access.retain(),
        ticks,
        expression: leaf_expression(state),
        window,
    })
}

macro_rules! filter_leaf {
    ($filter:ty, [$($generic:tt)*], $init:expr) => {
        impl<$($generic)*> FilterParts for $filter {
            fn init(world: &World) -> Result<Self::State, EcsError> {
                Ok(CompositeFilterState { expression: ($init)(world)?, tick_storages: None })
            }
            fn declaration(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
                let mut declaration = no_demands(leaf_expression(state));
                if let FilterExpression::Added(id) | FilterExpression::Changed(id) = state.expression {
                    declaration.tracking = Box::new([ComponentTickDemand { component: id, added: matches!(state.expression, FilterExpression::Added(_)), changed: matches!(state.expression, FilterExpression::Changed(_)) }]);
                    declaration.tick_reads = Box::new([id]);
                }
                Ok(declaration)
            }
            fn bind(state: &Self::State, scope: FetchScope, binding: &crate::query_fetch::FetchBinding<'_>) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
                bind_leaf(state, scope, binding)
            }
            unsafe fn run<'a>(access: &FetchAccess<'a>, _: FetchScope, state: &Self::State, plan: &Self::FetchState) -> Result<Self::Fetch<'a>, EcsError> { unsafe { run_leaf(access, state, plan) } }
        }
    };
}
filter_leaf!((), [], |_: &World| Ok::<_, EcsError>(FilterExpression::All));
filter_leaf!(Or<()>, [], |_: &World| Ok::<_, EcsError>(
    FilterExpression::None
));
filter_leaf!(With<T>, [T: 'static], |world: &World| resolve::<T>(world).map(FilterExpression::With));
filter_leaf!(Without<T>, [T: 'static], |world: &World| resolve::<T>(world).map(FilterExpression::Without));
filter_leaf!(Added<T>, [T: 'static], |world: &World| resolve::<T>(world).map(FilterExpression::Added));
filter_leaf!(Changed<T>, [T: 'static], |world: &World| resolve::<T>(world).map(FilterExpression::Changed));

macro_rules! filter_tuple {
    ($($F:ident:$i:tt),+) => {
        filter_tuple!(@impl false, And, ($($F,)+), $($F:$i),+);
        filter_tuple!(@impl true, Or, Or<($($F,)+)>, $($F:$i),+);
        impl<'a, $($F: FilterFetch<'a>,)+ const ANY: bool> FilterTupleFetch<($(Option<$F>,)+), ANY> {
            pub(super) fn matches_inner(fetch: &mut Self, entity: &FetchEntity) -> bool {
                let mut result = !ANY;
                $(if result != ANY {
                    let matched = fetch.children.$i.as_mut().is_some_and(|child| unsafe { child.matches(entity) });
                    result = matched;
                })+
                result
            }
        }
    };
    (@impl $any:literal, $expr:ident, $ty:ty, $($F:ident:$i:tt),+) => {
        impl<$($F: QueryFilter),+> FilterParts for $ty {
            fn init(world: &World) -> Result<Self::State, EcsError> {
                Ok(FilterTupleState { children: ($($F::init_state(world)?,)+) })
            }
            fn declaration(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
                let mut expressions = Vec::new();
                let mut ticks = Vec::new();
                let mut reads = Vec::new();
                $(let child = $F::declare_access(&state.children.$i)?;
                  expressions.push(child.expression);
                  ticks.extend(child.tracking);
                  reads.extend(child.tick_reads);)+
                Ok(QueryFilterDeclaration { expression: FilterExpression::$expr(expressions), tracking: ticks.into(), tick_reads: reads.into() })
            }
            fn bind(state: &Self::State, scope: FetchScope, binding: &crate::query_fetch::FetchBinding<'_>) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
                use crate::query::FetchStateResult::*;
                let mut variants = false;
                let mut hits = 0usize;
                let mut count = 0usize;
                let children = ($( {
                    count += 1;
                    match $F::init_fetch_state(&state.children.$i, scope, binding)? {
                        Ready(child) => { hits += 1; Some(child) }
                        NotMatched => None,
                        NeedsVariants => { variants = true; None },
                    }
                }, )+);
                if variants {
                    return match scope {
                        FetchScope::Table { .. } => Ok(NeedsVariants),
                        FetchScope::Archetype { .. } => Err(classified_error(ErrorKind::InvalidState, "具体原型不能请求过滤变体")),
                    };
                }
                if if $any { hits == 0 } else { hits != count } {
                    return Ok(NotMatched);
                }
                Ok(Ready(FilterTupleFetchState { children }))
            }
            unsafe fn run<'a>(access: &FetchAccess<'a>, scope: FetchScope, state: &Self::State, plan: &Self::FetchState) -> Result<Self::Fetch<'a>, EcsError> {
                let children = ($(plan.children.$i.as_ref().map(|child| unsafe { $F::prepare(access, scope, &state.children.$i, child) }).transpose()?,)+);
                Ok(FilterTupleFetch { children })
            }
        }
    };
}
filter_tuple!(F0:0);
filter_tuple!(F0:0, F1:1);
filter_tuple!(F0:0, F1:1, F2:2);
filter_tuple!(F0:0, F1:1, F2:2, F3:3);
filter_tuple!(F0:0, F1:1, F2:2, F3:3, F4:4);
filter_tuple!(F0:0, F1:1, F2:2, F3:3, F4:4, F5:5);
filter_tuple!(F0:0, F1:1, F2:2, F3:3, F4:4, F5:5, F6:6);
filter_tuple!(F0:0, F1:1, F2:2, F3:3, F4:4, F5:5, F6:6, F7:7);

impl CompositeFilterState {
    pub(crate) fn init_inner<F: FilterParts>(_world: &World) -> Result<F::State, EcsError> {
        F::init(_world)
    }
}
impl CompositeFilterState {
    pub(crate) fn declare_inner<F: FilterParts>(
        _state: &F::State,
    ) -> Result<QueryFilterDeclaration, EcsError> {
        F::declaration(_state)
    }
}
impl CompositeFilterState {
    pub(crate) fn prepare_inner<'a, F: FilterParts>(
        _access: &FetchAccess<'a>,
        _plan: FetchScope,
        _state: &F::State,
        _fetch: &F::FetchState,
    ) -> Result<F::Fetch<'a>, EcsError> {
        unsafe { F::run(_access, _plan, _state, _fetch) }
    }
}
impl CompositeFilterFetch<'_> {
    pub(crate) fn matches_inner(
        _fetch: &mut CompositeFilterFetch<'_>,
        _entity: &FetchEntity,
    ) -> bool {
        let added = match _fetch.expression {
            FilterExpression::Added(_) => true,
            FilterExpression::Changed(_) => false,
            FilterExpression::None => return false,
            _ => return true,
        };
        let ticks = match &_fetch.ticks[0] {
            ComponentView::Missing => return false,
            ComponentView::Column(view) => unsafe {
                view.ticks(_entity.location.table_row() as usize)
            },
            ComponentView::Sparse(view) => {
                unsafe { view.ticks(_entity.entity) }.expect("已匹配的稀疏 tick 缺失")
            }
        };
        let window = _fetch.window.expect("变化过滤未绑定窗口");
        let tick =
            if added { ticks.added } else { ticks.changed }.expect("变化过滤所需 tick 未启用");
        tick <= window.this_run && window.last_run.is_none_or(|last| last < tick)
    }
}

impl CompositeFilterState {
    pub(super) fn init_fetch_inner<F: FilterParts>(
        state: &F::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<F::FetchState>, EcsError> {
        F::bind(state, scope, binding)
    }
}
