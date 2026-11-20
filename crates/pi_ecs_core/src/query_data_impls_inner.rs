//! Static dispatch for built-in query descriptors and open tuple members.
use super::*;
use crate::{
    access::AccessMode,
    archetype::ComponentSource,
    error::{classified_error, ErrorKind},
    query::{FetchStateResult, QueryComponentAccess, QueryItemPresence},
    tracking::ComponentTickDemand,
};
use FetchStateResult::{NeedsVariants, NotMatched, Ready};

pub(super) trait DataParts: QueryData {
    fn init(world: &World) -> Result<Self::State, EcsError>;
    fn declare(state: &Self::State) -> Result<QueryDataDeclaration, EcsError>;
    fn bind(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<FetchStateResult<Self::FetchState>, EcsError>;
}

impl<Q: DataParts> QueryDataBinding<Q> {
    pub(crate) fn init_inner(world: &World) -> Result<Q::State, EcsError> {
        Q::init(world)
    }
    pub(crate) fn declare_inner(state: &Q::State) -> Result<QueryDataDeclaration, EcsError> {
        Q::declare(state)
    }
    pub(crate) fn init_fetch_inner(
        state: &Q::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<FetchStateResult<Q::FetchState>, EcsError> {
        Q::bind(state, scope, binding)
    }
}

fn empty_declaration() -> QueryDataDeclaration {
    QueryDataDeclaration {
        components: Box::new([]),
        tracking: Box::new([]),
        tick_reads: Box::new([]),
    }
}

macro_rules! metadata {
    ($ty:ty, [$($gen:tt)*]) => {
        impl<$($gen)*> DataParts for $ty {
            fn init(_: &World) -> Result<(), EcsError> { Ok(()) }
            fn declare(_: &()) -> Result<QueryDataDeclaration, EcsError> { Ok(empty_declaration()) }
            fn bind(_: &(), _: FetchScope, _: &FetchBinding<'_>) -> Result<FetchStateResult<()>, EcsError> { Ok(Ready(())) }
        }
    };
}
metadata!(Entity, []);
metadata!(EntityLocation, []);
metadata!((), []);
metadata!(&'static Archetype, []);
metadata!(PhantomData<T>, [T: ?Sized]);

fn component_bind(
    state: &ComponentQueryState,
    scope: FetchScope,
    binding: &FetchBinding<'_>,
    required: bool,
    values: bool,
) -> Result<FetchStateResult<ComponentQueryFetchState>, EcsError> {
    let (archetype, uniform) = match scope {
        FetchScope::Table { table } => (crate::entity::ArchetypeId { table, variant: 0 }, true),
        FetchScope::Archetype { archetype } => (archetype, false),
    };
    let archetype = binding
        .archetypes
        .iter()
        .find(|a| a.id() == archetype)
        .ok_or_else(|| classified_error(ErrorKind::NotFound, "查询原型未登记"))?;
    let source = archetype
        .components()
        .iter()
        .find(|entry| entry.component() == state.component)
        .map(|entry| entry.source());
    if source.is_none() {
        if uniform {
            return Ok(NeedsVariants);
        }
        if required {
            return Ok(NotMatched);
        }
    }
    let storage = if values {
        match source {
            Some(ComponentSource::Table(column)) => ComponentStorageBinding::Column(
                binding.column_address(archetype.id().table, column)?,
            ),
            Some(ComponentSource::Sparse) => {
                ComponentStorageBinding::Sparse(binding.sparse_address(state.component)?)
            }
            None => ComponentStorageBinding::Missing,
        }
    } else {
        ComponentStorageBinding::Missing
    };
    Ok(Ready(ComponentQueryFetchState { source, storage }))
}

macro_rules! component_descriptor {
    ($ty:ty, [$($bounds:tt)*], $required:literal, $values:literal, $mode:ident, $default:literal, $ticks:literal) => {
        impl<C: Component $($bounds)*> DataParts for $ty {
            fn init(world: &World) -> Result<Self::State, EcsError> {
                let component = world.component_id::<C>().ok_or_else(|| classified_error(ErrorKind::NotFound, "查询组件未登记"))?;
                let default_type = if $default {
                    let id = std::any::TypeId::of::<C>();
                    if !world.query_defaults.values.contains_key(&id) { return Err(classified_error(ErrorKind::NotFound, "查询默认值未登记")); }
                    Some(id)
                } else { None };
                Ok(ComponentQueryState { component, default_type })
            }
            fn declare(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
                let mut declaration = empty_declaration();
                if $values {
                    declaration.components = Box::new([QueryComponentAccess {
                        component: state.component,
                        mode: AccessMode::$mode,
                        presence: if $required { QueryItemPresence::Required } else { QueryItemPresence::Optional },
                    }]);
                }
                if $ticks {
                    declaration.tracking = Box::new([ComponentTickDemand { component: state.component, added: true, changed: true }]);
                    declaration.tick_reads = Box::new([state.component]);
                }
                Ok(declaration)
            }
            fn bind(state: &Self::State, scope: FetchScope, binding: &FetchBinding<'_>) -> Result<FetchStateResult<Self::FetchState>, EcsError> {
                component_bind(state, scope, binding, $required, $values)
            }
        }
    };
}
component_descriptor!(&'static C, [], true, true, Read, false, false);
component_descriptor!(Option<&'static C>, [], false, true, Read, false, false);
component_descriptor!(&'static mut C, [], true, true, Write, false, false);
component_descriptor!(Option<&'static mut C>, [], false, true, Write, false, false);
component_descriptor!(Has<C>, [], false, false, Read, false, false);
component_descriptor!(Ref<C>, [<Access = PlainAccess>], true, true, Read, false, true);
component_descriptor!(OrDefault<C>, [<Access = PlainAccess>], false, true, Read, true, false);
component_descriptor!(OrDefaultRef<C>, [<Access = PlainAccess>], false, true, Read, true, true);

macro_rules! tuple_descriptor {
    ($($Q:ident:$i:tt),+) => {
        impl<$($Q: QueryData),+> DataParts for ($($Q,)+) {
            fn init(world: &World) -> Result<Self::State, EcsError> {
                let state = ($($Q::init_state(world)?,)+);
                Self::declare(&state)?;
                Ok(state)
            }
            fn declare(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
                let mut components: Vec<QueryComponentAccess> = Vec::new();
                let mut tracking = Vec::new();
                let mut tick_reads = Vec::new();
                $(let child = $Q::declare_access(&state.$i)?;
                for access in child.components {
                    if components.iter().any(|other| other.component == access.component && (other.mode == AccessMode::Write || access.mode == AccessMode::Write)) {
                        return Err(classified_error(ErrorKind::InvalidInput, "查询元组包含可变别名"));
                    }
                    components.push(access);
                }
                tracking.extend(child.tracking);
                tick_reads.extend(child.tick_reads);)+
                Ok(QueryDataDeclaration { components: components.into(), tracking: tracking.into(), tick_reads: tick_reads.into() })
            }
            fn bind(state: &Self::State, scope: FetchScope, binding: &FetchBinding<'_>) -> Result<FetchStateResult<Self::FetchState>, EcsError> {
                let mut excluded = false;
                let mut variants = false;
                let children = ($(match $Q::init_fetch_state(&state.$i, scope, binding)? {
                    Ready(plan) => Some(plan),
                    NotMatched => { excluded = true; None },
                    NeedsVariants => { variants = true; None },
                },)+);
                if variants && matches!(scope, FetchScope::Archetype { .. }) { return Err(classified_error(ErrorKind::InvalidState, "具体原型不允许请求变体")); }
                if excluded { return Ok(NotMatched); }
                if variants { return Ok(NeedsVariants); }
                Ok(Ready(($(children.$i.expect("完整子查询计划"),)+)))
            }
        }
        impl<$($Q: QueryData),+> QueryDataBinding<($($Q,)+)> {
            pub(super) fn prepare_tuple_inner<'w>(access: &FetchAccess<'w>, scope: FetchScope, state: &<($($Q,)+) as QueryData>::State, plan: &<($($Q,)+) as QueryData>::FetchState) -> Result<<($($Q,)+) as QueryData>::Fetch<'w>, EcsError> {
                Ok(($(unsafe { <$Q::Fetch<'w> as QueryFetch<'w, $Q>>::prepare(access, scope, &state.$i, &plan.$i)? },)+))
            }
            pub(super) fn fetch_tuple_inner<'w>(fetch: &mut <($($Q,)+) as QueryData>::Fetch<'w>, entity: &FetchEntity) -> <($($Q,)+) as QueryData>::Item<'w, 'w> {
                ($(unsafe { <$Q::Fetch<'w> as QueryFetch<'w, $Q>>::fetch_at(&mut fetch.$i, entity) },)+)
            }
        }
    };
}
tuple_descriptor!(Q0:0);
tuple_descriptor!(Q0:0, Q1:1);
tuple_descriptor!(Q0:0, Q1:1, Q2:2);
tuple_descriptor!(Q0:0, Q1:1, Q2:2, Q3:3);
tuple_descriptor!(Q0:0, Q1:1, Q2:2, Q3:3, Q4:4);
tuple_descriptor!(Q0:0, Q1:1, Q2:2, Q3:3, Q4:4, Q5:5);
tuple_descriptor!(Q0:0, Q1:1, Q2:2, Q3:3, Q4:4, Q5:5, Q6:6);
tuple_descriptor!(Q0:0, Q1:1, Q2:2, Q3:3, Q4:4, Q5:5, Q6:6, Q7:7);

// The plan must be bound to this exact World and scope, and the original
// authorization must pin the backing metadata and containers for 'w.
unsafe fn bind_live_component<'w, Q, C: Component>(
    access: &FetchAccess<'w>,
    scope: FetchScope,
    state: &ComponentQueryState,
    plan: &ComponentQueryFetchState,
    mode: AccessMode,
    observes_ticks: bool,
) -> Result<ComponentFetch<'w, Q, C>, EcsError> {
    let invalid = || classified_error(ErrorKind::InvalidState, "组件取值授权或追踪来源不匹配");
    let writable = mode == AccessMode::Write;
    if !access
        .declaration
        .components
        .iter()
        .any(|&(id, permission)| {
            id == state.component && (!writable || permission == AccessMode::Write)
        })
        || writable && (!access.world.mutable || access.write_tick.is_none())
        || observes_ticks
            && (access.window.is_none()
                || !access.declaration.tick_reads.contains(&state.component))
    {
        return Err(invalid());
    }
    let root = access.world.pointer.as_ptr();
    // SAFETY: the original execution authority pins the registry, default
    // directory and the bound storage containers for the whole result lifetime.
    let (info, storage, notifications, default) = unsafe {
        let registry = &*std::ptr::addr_of!((*root).registry);
        let info = registry
            .component(state.component)
            .filter(|info| info.type_id == std::any::TypeId::of::<C>())
            .ok_or_else(invalid)?;
        if observes_ticks
            && plan.source.is_some()
            && !(info.tracking.added_tick && info.tracking.changed_tick)
        {
            return Err(invalid());
        }
        let storage = match &plan.storage {
            ComponentStorageBinding::Missing => ComponentView::Missing,
            ComponentStorageBinding::Column(column) => {
                ComponentView::Column(column.as_ref().bind_view())
            }
            ComponentStorageBinding::Sparse(sparse) => {
                ComponentView::Sparse(sparse.as_ref().bind_view())
            }
        };
        let notifications = (writable && info.tracking.changed_notice)
            .then(|| &*std::ptr::addr_of!((*root).notifications));
        let default = if let Some(ty) = state.default_type.filter(|_| plan.source.is_none()) {
            Some(
                (&*std::ptr::addr_of!((*root).query_defaults))
                    .values
                    .get(&ty)
                    .and_then(|value| value.downcast_ref::<C>())
                    .ok_or_else(invalid)?,
            )
        } else {
            None
        };
        (info, storage, notifications, default)
    };
    Ok(ComponentFetch {
        access: access.retain(),
        scope,
        storage,
        demand: &info.tracking,
        notifications,
        default,
        marker: PhantomData,
    })
}

impl<'w, Q, C: Component> ComponentFetch<'w, Q, C> {
    // Only the validated current target may reach a bound view.
    unsafe fn read_address(&self, target: &FetchEntity) -> Option<std::ptr::NonNull<C>> {
        check_scope(self.scope, target);
        let erased = match &self.storage {
            ComponentView::Missing => return None,
            ComponentView::Column(view) => unsafe {
                view.read(target.location.table_row() as usize)
            },
            ComponentView::Sparse(view) => {
                unsafe { view.read(target.entity) }.expect("已匹配的稀疏值缺失")
            }
        };
        Some(erased.as_non_null().cast())
    }

    unsafe fn read_bound(&self, target: &FetchEntity) -> Option<ComponentRead<'w, 'w, C>> {
        let pointer = unsafe { self.read_address(target)? };
        // SAFETY: this view is a live C and 'w is the original authorization, not a borrow of self.
        Some(unsafe { crate::component::read_component(pointer.as_ref(), self.access.world) })
    }
}

// These concrete implementations keep the GAT result tied to its descriptor;
// no layout, name or lifetime cast participates in dispatch.
pub(super) trait ComponentResult<C: Component>:
    QueryData<State = ComponentQueryState, FetchState = ComponentQueryFetchState> + Sized
{
    const MODE: AccessMode;
    const OBSERVES_TICKS: bool = false;
    unsafe fn fetch<'w>(
        bound: &mut ComponentFetch<'w, Self, C>,
        target: &FetchEntity,
    ) -> Self::Item<'w, 'w>;
}

macro_rules! component_result {
    ($desc:ty, [$($bounds:tt)*], $mode:ident, $observes:literal, |$bound:ident, $target:ident| $expr:expr) => {
        impl<C: Component $($bounds)*> ComponentResult<C> for $desc {
            const MODE: AccessMode = AccessMode::$mode;
            const OBSERVES_TICKS: bool = $observes;
            unsafe fn fetch<'w>(
                $bound: &mut ComponentFetch<'w, Self, C>,
                $target: &FetchEntity,
            ) -> Self::Item<'w, 'w> {
                // SAFETY: QueryFetch supplies the validated target and full result authorization.
                unsafe { $expr }
            }
        }
    };
}
component_result!(&'static C, [], Read, false, |bound, target| bound
    .read_bound(target)
    .expect("必需组件缺失"));
component_result!(Option<&'static C>, [], Read, false, |bound, target| bound
    .read_bound(target));
component_result!(&'static mut C, [], Write, false, |bound, target| bound
    .write_bound(target)
    .expect("必需组件缺失"));
component_result!(Option<&'static mut C>, [], Write, false, |bound, target| {
    bound.write_bound(target)
});
component_result!(Ref<C>, [<Access = PlainAccess>], Read, true, |bound, target| bound.tracked_bound(target, false));
component_result!(OrDefault<C>, [<Access = PlainAccess>], Read, false, |bound, target| bound.read_bound(target).or(bound.default).expect("默认值缺失"));
component_result!(OrDefaultRef<C>, [<Access = PlainAccess>], Read, true, |bound, target| bound.tracked_bound(target, true));

impl<'w, Q, C: Component> ComponentFetch<'w, Q, C> {
    unsafe fn write_bound(&self, target: &FetchEntity) -> Option<ComponentWrite<'w, 'w, C>> {
        check_scope(self.scope, target);
        let (pointer, ticks) = match &self.storage {
            ComponentView::Missing => return None,
            ComponentView::Column(view) => {
                let row = target.location.table_row() as usize;
                let address = unsafe { view.read(row) }.as_non_null();
                let (_, ticks) = unsafe { view.write(row) };
                (address, ticks)
            }
            ComponentView::Sparse(view) => {
                let address = unsafe { view.read(target.entity) }
                    .expect("已匹配的稀疏值缺失")
                    .as_non_null();
                let (_, ticks) = unsafe { view.write(target.entity) }.expect("已匹配的稀疏值缺失");
                (address, ticks)
            }
        };
        // SAFETY: only this target's value and tick slots are exclusively held.
        unsafe {
            let marker = self.change_marker(target, ticks);
            Some(crate::component::write_component(
                &mut *pointer.cast::<C>().as_ptr(),
                self.access.world,
                marker,
            ))
        }
    }
}

impl<'w, Q, C: Component<Access = PlainAccess>> ComponentFetch<'w, Q, C> {
    unsafe fn tracked_bound(&self, target: &FetchEntity, allow_default: bool) -> ValueRef<'w, C> {
        let window = self.access.window.expect("变化取值需要窗口");
        let Some(value) = (unsafe { self.read_bound(target) }) else {
            assert!(allow_default);
            return ValueRef {
                value: self.default.expect("默认值缺失"),
                ticks: None,
                window,
            };
        };
        let ticks = match &self.storage {
            ComponentView::Missing => unreachable!("已取得值但没有来源"),
            ComponentView::Column(view) => unsafe {
                view.ticks(target.location.table_row() as usize)
            },
            ComponentView::Sparse(view) => {
                unsafe { view.ticks(target.entity) }.expect("已匹配的稀疏 tick 缺失")
            }
        };
        ValueRef {
            value,
            ticks: Some((
                ticks.added.expect("缺少 Added tick"),
                ticks.changed.expect("缺少 Changed tick"),
            )),
            window,
        }
    }
}

impl<'w, Q: ComponentResult<C>, C: Component> ComponentFetch<'w, Q, C> {
    pub(super) fn prepare_live_inner(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &Q::State,
        fetch: &Q::FetchState,
    ) -> Result<Self, EcsError> {
        // SAFETY: 原 QueryFetch 授权覆盖此绑定及全部结果。
        unsafe { bind_live_component(access, scope, state, fetch, Q::MODE, Q::OBSERVES_TICKS) }
    }
    pub(super) fn fetch_live_inner(&mut self, entity: &FetchEntity) -> Q::Item<'w, 'w> {
        // SAFETY: 调用方已校验目标并排除冲突结果。
        unsafe { Q::fetch(self, entity) }
    }
}

pub(super) trait MetadataResult: QueryData + Sized {
    const ARCHETYPE: bool = false;
    fn result<'w>(bound: &MetadataFetch<'w, Self>, target: &FetchEntity) -> Self::Item<'w, 'w>;

    fn fetch<'w>(bound: &MetadataFetch<'w, Self>, target: &FetchEntity) -> Self::Item<'w, 'w> {
        check_scope(bound.scope, target);
        Self::result(bound, target)
    }
}

impl MetadataResult for Entity {
    fn result<'w>(_: &MetadataFetch<'w, Self>, target: &FetchEntity) -> Entity {
        target.entity
    }
}
impl MetadataResult for EntityLocation {
    fn result<'w>(_: &MetadataFetch<'w, Self>, target: &FetchEntity) -> EntityLocation {
        target.location
    }
}
impl MetadataResult for () {
    fn result<'w>(_: &MetadataFetch<'w, Self>, _: &FetchEntity) {}
}
impl<T: ?Sized> MetadataResult for PhantomData<T> {
    fn result<'w>(_: &MetadataFetch<'w, Self>, _: &FetchEntity) {}
}
impl MetadataResult for &'static Archetype {
    const ARCHETYPE: bool = true;

    fn result<'w>(bound: &MetadataFetch<'w, Self>, target: &FetchEntity) -> &'w Archetype {
        let id = target.location.archetype();
        bound
            .archetypes
            .expect("原型元数据未绑定")
            .iter()
            .find(|archetype| archetype.id() == id)
            .expect("发布实体的原型缺失")
    }
}

impl<'w, Q: MetadataResult> MetadataFetch<'w, Q> {
    pub(super) fn prepare_live_inner(
        _access: &FetchAccess<'w>,
        _scope: FetchScope,
        _state: &Q::State,
        _fetch: &Q::FetchState,
    ) -> Result<Self, EcsError> {
        // SAFETY: QueryFetch::prepare guarantees the source World and its frozen
        // archetype declarations remain live for the original authorization.
        let archetypes = Q::ARCHETYPE.then(|| unsafe {
            let root = _access.world.pointer.as_ptr();
            (&*std::ptr::addr_of!((*root).registry))
                .archetypes
                .as_slice()
        });
        Ok(Self {
            access: _access.retain(),
            scope: _scope,
            archetypes,
            marker: PhantomData,
        })
    }
    pub(super) fn fetch_live_inner(&mut self, entity: &FetchEntity) -> Q::Item<'w, 'w> {
        Q::fetch(self, entity)
    }
}

impl<'w, C: crate::component::Component> crate::query_data::PresenceFetch<'w, C> {
    pub(super) fn prepare_live_inner(
        _access: &FetchAccess<'w>,
        _scope: FetchScope,
        _state: &<crate::query_data::Has<C> as QueryData>::State,
        _fetch: &<crate::query_data::Has<C> as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        Ok(Self {
            access: _access.retain(),
            scope: _scope,
            present: _fetch.source.is_some(),
            marker: PhantomData,
        })
    }
    pub(super) fn fetch_live_inner(
        &mut self,
        _entity: &FetchEntity,
    ) -> <crate::query_data::Has<C> as QueryData>::Item<'w, 'w> {
        check_scope(self.scope, _entity);
        self.present
    }
}

#[cfg(all(test, feature = "rc"))]
mod rc_metadata_tests {
    use super::*;
    use crate::{access::AccessTree, query::NoFilter, unsafe_world_cell::UnsafeWorldCell};

    #[test]
    fn rc_typed_read_fetch_keeps_bound_slot_after_descriptor_drop() {
        use crate::{bundle::ComponentValue, component::StoragePolicy, entity::AllocationDomain};
        struct Value(u64);
        impl Component for Value {
            type Access = PlainAccess;
        }
        let mut world = World::new().unwrap();
        world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let entity = world
            .spawn(AllocationDomain(0), ComponentValue(Value(317)))
            .unwrap()
            .entity();
        let state = <&Value as QueryData>::init_state(&world).unwrap();
        let query = world.query::<&Value>().unwrap();
        let tree = AccessTree::Leaf(query.declaration().unwrap().to_access().unwrap());
        let location = world.locate(entity).unwrap();
        let scope = FetchScope::Archetype {
            archetype: location.archetype(),
        };
        let Ready(plan) = <&Value as QueryData>::init_fetch_state(
            &state,
            scope,
            &FetchBinding {
                storage: world.storage.as_ref().unwrap(),
                world_identity: world.identity,
                archetypes: &world.registry.archetypes,
            },
        )
        .unwrap() else {
            panic!("value must match")
        };
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                // SAFETY: same-source read-only authority and the current published target.
                unsafe {
                    world.system_scope(&tree, version, |access| {
                        let descriptor = access.fetch_access()?;
                        let bound = bind_live_component::<&Value, Value>(
                            &descriptor,
                            scope,
                            &state,
                            &plan,
                            AccessMode::Read,
                            false,
                        )?;
                        drop(descriptor);
                        let value = bound.read_bound(&FetchEntity { entity, location }).unwrap();
                        drop(bound);
                        assert_eq!(value.0, 317);
                        let descriptor = access.fetch_access()?;
                        let mut bound = bind_live_component::<&Value, Value>(
                            &descriptor,
                            scope,
                            &state,
                            &plan,
                            AccessMode::Read,
                            false,
                        )?;
                        let value = <&Value as ComponentResult<Value>>::fetch(
                            &mut bound,
                            &FetchEntity { entity, location },
                        );
                        drop(bound);
                        assert_eq!(value.0, 317);
                        Ok::<_, EcsError>(())
                    })??;
                }
                Ok(())
            })
            .unwrap();
        world.close().unwrap();
    }

    #[test]
    fn rc_archetype_metadata_fetch_prepares_with_the_original_authority() {
        let mut world = World::new().unwrap();
        world.finish_registration().unwrap();
        let entity = world
            .spawn(crate::entity::AllocationDomain(0), ())
            .unwrap()
            .entity();
        let location = world.locate(entity).unwrap();
        let query = world.query::<&Archetype>().unwrap();
        let tree = AccessTree::Leaf(query.declaration().unwrap().to_access().unwrap());
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                // SAFETY: the metadata descriptor accesses no business values;
                // the real same-World leaf remains live throughout preparation.
                unsafe {
                    world.system_scope(&tree, version, |access| {
                        let fetch = access.fetch_access()?;
                        let scope = FetchScope::Table { table: 0 };
                        let prepared = <<&Archetype as QueryData>::Fetch<'_> as QueryFetch<
                            '_,
                            &Archetype,
                        >>::prepare(&fetch, scope, &(), &())?;
                        // Retained authorization must not borrow the descriptor object.
                        drop(fetch);
                        assert_eq!(prepared.scope, scope);
                        assert_eq!(prepared.access.world.id(), access.world_cell().id());
                        assert_eq!(prepared.archetypes.unwrap()[0].id().table, 0);
                        let target = FetchEntity { entity, location };
                        let result = <&Archetype as MetadataResult>::fetch(&prepared, &target);
                        drop(prepared);
                        assert_eq!(result.id(), target.location.archetype());
                        Ok::<(), EcsError>(())
                    })??;
                }
                Ok(())
            })
            .unwrap();
        let mut query: crate::query::QueryState<&Archetype, NoFilter> = query;
        query.unregister_tracking(&mut world).unwrap();
        world.close().unwrap();
    }
}

fn check_scope(scope: FetchScope, entity: &FetchEntity) {
    let actual = entity.location.archetype();
    assert!(
        match scope {
            FetchScope::Table { table } => actual.table == table,
            FetchScope::Archetype { archetype } => actual == archetype,
        },
        "Fetch 目标超出绑定范围"
    );
}
