//! 存在性/变化状态和组合过滤；元组 AND，Or 为 OR；静态矛盾报错，不消费通知。
use crate::{
    archetype::Archetype,
    change_detection::ChangeWindow,
    component::ComponentId,
    entity::PartitionId,
    error::EcsError,
    query::{private, QueryFilter},
    query_fetch::{ArchetypeFetchPlan, FetchAccess, FetchEntity, FilterFetch},
    world::World,
};
use std::marker::PhantomData;

/// 结构存在性过滤，不读取 T 值。
pub struct With<T: 'static>(PhantomData<fn() -> T>);
/// 结构缺失过滤，不读取 T 值。
pub struct Without<T: 'static>(PhantomData<fn() -> T>);
/// 分支 OR，结构和动态判断必须保留分支，不能把某分支所需组件提升为整体必需。
pub struct Or<F>(PhantomData<fn() -> F>);
/// 结构/组合过滤的长期描述，不能持实际组件引用。
pub struct CompositeFilterState {
    // @to_design 结构约束表达式及分支 fetch 表示待闭合；初始化必须拒绝静态矛盾。
    pub(crate) _pending: (),
}
/// 本次组合过滤器，实际分支及状态 tick 地址绑定在授权期。
pub struct CompositeFilterFetch<'a> {
    /// 绑定本次访问期，不通过伪造 static 持有状态。
    pub(crate) marker: PhantomData<&'a ()>,
}
impl private::Sealed for CompositeFilterFetch<'_> {}
impl<'a> FilterFetch<'a> for CompositeFilterFetch<'a> {
    unsafe fn matches(&mut self, _entity: &FetchEntity) -> bool {
        todo!("尚未实现")
    }
}
impl<T: 'static> private::Sealed for With<T> {}
impl<T: 'static> QueryFilter for With<T> {
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 结构表达式及静态矛盾校验")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 分支准备与授权")
    }
}
impl<T: 'static> private::Sealed for Without<T> {}
impl<T: 'static> QueryFilter for Without<T> {
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 结构表达式及静态矛盾校验")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 分支准备与授权")
    }
}
impl<F: QueryFilter> private::Sealed for Or<(F,)> {}
impl<F: QueryFilter> QueryFilter for Or<(F,)> {
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 结构表达式及静态矛盾校验")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 分支准备与授权")
    }
}
impl<A: QueryFilter> private::Sealed for (A,) {}
impl<A: QueryFilter> QueryFilter for (A,) {
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 结构表达式及静态矛盾校验")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 分支准备与授权")
    }
}
impl<A: QueryFilter, B: QueryFilter> private::Sealed for (A, B) {}
impl<A: QueryFilter, B: QueryFilter> QueryFilter for (A, B) {
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 结构表达式及静态矛盾校验")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 分支准备与授权")
    }
}
impl<A: QueryFilter, B: QueryFilter> private::Sealed for Or<(A, B)> {}
impl<A: QueryFilter, B: QueryFilter> QueryFilter for Or<(A, B)> {
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 结构表达式及静态矛盾校验")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 分支准备与授权")
    }
}
impl<A: QueryFilter, B: QueryFilter, C: QueryFilter> private::Sealed for (A, B, C) {}
impl<A: QueryFilter, B: QueryFilter, C: QueryFilter> QueryFilter for (A, B, C) {
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 结构表达式及静态矛盾校验")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 分支准备与授权")
    }
}
impl<A: QueryFilter, B: QueryFilter, C: QueryFilter> private::Sealed for Or<(A, B, C)> {}
impl<A: QueryFilter, B: QueryFilter, C: QueryFilter> QueryFilter for Or<(A, B, C)> {
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 结构表达式及静态矛盾校验")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 分支准备与授权")
    }
}
impl<A: QueryFilter, B: QueryFilter, C: QueryFilter, D: QueryFilter> private::Sealed
    for (A, B, C, D)
{
}
impl<A: QueryFilter, B: QueryFilter, C: QueryFilter, D: QueryFilter> QueryFilter for (A, B, C, D) {
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 结构表达式及静态矛盾校验")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 分支准备与授权")
    }
}
impl<A: QueryFilter, B: QueryFilter, C: QueryFilter, D: QueryFilter> private::Sealed
    for Or<(A, B, C, D)>
{
}
impl<A: QueryFilter, B: QueryFilter, C: QueryFilter, D: QueryFilter> QueryFilter
    for Or<(A, B, C, D)>
{
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 结构表达式及静态矛盾校验")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 分支准备与授权")
    }
}

/// 已解析的变化过滤组件，不持有业务值。
pub struct ChangeFilterState {
    /// 对应 T 的登记编号，不允许初始化时隐式登记。
    pub(crate) component: ComponentId,
}
/// 本次变化状态读取，权限及 tick 地址由阶段准备绑定。
pub struct ChangeFilterFetch<'a> {
    /// 本系统观察窗口，不使用通知游标。
    pub(crate) window: &'a ChangeWindow,
    // @to_design 实际 tick 槽访问器与窗口从 FetchAccess 注入的表示待闭合。
}
impl private::Sealed for ChangeFilterFetch<'_> {}
impl<'a> FilterFetch<'a> for ChangeFilterFetch<'a> {
    unsafe fn matches(&mut self, _entity: &FetchEntity) -> bool {
        todo!("@to_design tick 槽及窗口判断")
    }
}
/// Added 活组件状态过滤：注册对应 tick 需求，不订阅通知；组件删除后不再匹配。
pub struct Added<T: 'static>(
    /// 类型描述，不持有 T 值。
    PhantomData<fn() -> T>,
);
impl<T: 'static> private::Sealed for Added<T> {}
impl<T: 'static> QueryFilter for Added<T> {
    type State = ChangeFilterState;
    type FetchState = ChangeFilterState;
    type Fetch<'a> = ChangeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 汇总 Added tick 需求的登记通道")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 系统观察窗口与 tick 访问授权")
    }
}
/// Changed 活组件状态过滤：注册对应 tick 需求，不订阅通知；组件删除后不再匹配。
pub struct Changed<T: 'static>(
    /// 类型描述，不持有 T 值。
    PhantomData<fn() -> T>,
);
impl<T: 'static> private::Sealed for Changed<T> {}
impl<T: 'static> QueryFilter for Changed<T> {
    type State = ChangeFilterState;
    type FetchState = ChangeFilterState;
    type Fetch<'a> = ChangeFilterFetch<'a>;
    fn init_state(_world: &World) -> Result<Self::State, EcsError> {
        todo!("@to_design 汇总 Changed tick 需求的登记通道")
    }
    fn matches_archetype(_state: &Self::State, _archetype: &Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &Self::State,
        _plan: &ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'a>(
        _access: &FetchAccess<'a>,
        _partition: PartitionId,
        _plan: &ArchetypeFetchPlan,
        _state: &Self::State,
        _fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        todo!("@to_design 系统观察窗口与 tick 访问授权")
    }
}
