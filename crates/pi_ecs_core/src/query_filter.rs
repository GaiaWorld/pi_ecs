//! 结构及变化过滤；元组 AND、Or 为分支 OR，不消费通知。
use crate::{
    archetype::Archetype,
    change_detection::ChangeWindow,
    component::ComponentId,
    error::EcsError,
    query::{QueryFilter, QueryFilterDeclaration},
    query_fetch::{FetchAccess, FetchEntity, FetchScope, FilterFetch},
    world::World,
};
use std::marker::PhantomData;
/// 仅要求存在，不读取 T 值。
pub struct With<T: 'static>(PhantomData<fn() -> T>);
/// 仅要求缺失，不读取 T 值。
pub struct Without<T: 'static>(PhantomData<fn() -> T>);
/// 至少一个分支通过；空分支集合永不匹配，不提升单分支要求为全局要求。
pub struct Or<F>(PhantomData<fn() -> F>);
/// 活组件新增时间在窗口内；启用 added tick，不订阅通知。
pub struct Added<T: 'static>(PhantomData<fn() -> T>);
/// 活组件修改时间在窗口内；启用 changed tick，不能证明写查询互斥。
pub struct Changed<T: 'static>(PhantomData<fn() -> T>);
/// 结构条件树，保持分支供匹配和访问冲突检查使用。
pub enum FilterExpression {
    /// 无条件通过。
    All,
    /// 无条件拒绝，空 OR。
    None,
    /// 要求存在的已登记组件。
    With(ComponentId),
    /// 要求缺失的已登记组件。
    Without(ComponentId),
    /// 所有子项同时满足，顺序与声明一致。
    And(Vec<FilterExpression>),
    /// 任一子项满足，不丢失分支边界。
    Or(Vec<FilterExpression>),
    /// added 状态读取，不以其动态真假证明静态互斥。
    Added(ComponentId),
    /// changed 状态读取，不以其动态真假证明静态互斥。
    Changed(ComponentId),
}
/// 组合长期状态，不含业务值地址；变化读取需求从表达式汇总。
pub struct CompositeFilterState {
    /// 已解析且检查矛盾的结构/变化表达式。
    pub(crate) expression: FilterExpression,
    /// 变化叶子按表达式深度优先声明顺序直接保存字段级容器来源；外层 None 为未绑定。
    /// Missing 表示本原型缺该分支组件（如 OR 的另一分支），叶子判 false；纯结构项为 Some 空数组。
    /// 重复叶子可保存相同裸地址，不创建重复组件引用，不增加写授权。
    pub(crate) tick_storages: Option<Box<[crate::query_data::ComponentStorageBinding]>>,
}
/// 组合本次访问，变化项必须有显式窗口，纯结构项不要求窗口。
pub struct CompositeFilterFetch<'a> {
    /// 按值保留原授权，不借用 prepare 的临时 access 对象，不新增权限。
    pub(crate) access: FetchAccess<'a>,
    /// prepare 从变化叶子的缓存地址绑定视图；与表达式叶子顺序一致，不逐实体查找目录。
    /// 每次 matches 只复制所需 tick，返回前结束全部单槽读取，不保留到写 data fetch。
    pub(crate) ticks: Box<[crate::query_data::ComponentView<'a>]>,
    /// 保留分支表达式，不按原型匹配结果删除其他访问声明。
    pub(crate) expression: FilterExpression,
    /// 系统或宿主窗口，变化分支缺失时绑定报错。
    pub(crate) window: Option<ChangeWindow>,
}
/// # 用途
/// 开放过滤组合的长期状态；S 是按声明顺序排列的子过滤器真实 State 元组。
/// 不擦除为内置表达式，不要求外部状态 Clone/Any/'static；声明逐项调用子项 declare_access。
pub struct FilterTupleState<S> {
    /// 子状态元组，位置与过滤类型及 FetchState 一一对应，不缓存业务引用。
    pub(crate) children: S,
}
/// # 用途
/// 开放组合在一个范围的完整子计划；S 是 Option<子 FetchState> 元组。
/// None 仅代表该子项结构 NotMatched，不能表示未扫描或吞掉错误。
pub struct FilterTupleFetchState<S> {
    /// 按声明位置保存真实计划；AND Ready 时全部 Some，OR Ready 时至少一个 Some。
    /// 所有子项先完成检查：真实 Err 优先；Table 任一 NeedsVariants 则整体 NeedsVariants，
    /// 否则 AND 任一 None 为 NotMatched，OR 全 None 为 NotMatched，不按当前 tick 求结构匹配。
    pub(crate) children: S,
}
/// # 用途
/// 本次开放过滤组合；T 为 Option<子 Fetch> 元组，ANY=true 为 OR，false 为 AND。
/// 各子 fetch 保留原始类型/状态及授权寿命，不把自定义动态 matches 替换为表达式解释。
pub struct FilterTupleFetch<T, const ANY: bool> {
    /// 原顺序的本次访问器；仅绑定 Some 子计划，None 在 OR 判 false。
    /// matches 可按声明顺序短路，但全部声明/tick 需求必须保留；返回前结束所有单槽读取。
    pub(crate) children: T,
}
/// 单变化组件的已登记身份。
pub struct ChangeFilterState {
    /// added/changed 所属组件。
    pub(crate) component: ComponentId,
}
/// 单变化过滤本次状态；实际存储读取仍待实现。
pub struct ChangeFilterFetch<'a> {
    /// 本次窗口，不推进任何游标。
    pub(crate) window: &'a ChangeWindow,
}
#[path = "query_filter_inner.rs"]
mod query_filter_inner;
unsafe impl FilterFetch<'_> for CompositeFilterFetch<'_> {
    /// 判断已匹配实体；不修改业务值，纯结构组合返回 true。
    /// # Safety
    /// 目标、授权与表达式属于同 World，相关读取受保护。
    unsafe fn matches(&mut self, entity: &FetchEntity) -> bool {
        Self::matches_inner(self, entity)
    }
}
unsafe impl QueryFilter for () {
    const IS_ARCHETYPAL: bool = true;
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    /// 解析表达式，汇总跟踪需求并检查静态矛盾，不隐式登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// 绑定声明与窗口，不申请新门禁；含变化分支而无窗口报错。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    /// Added/Changed 全部分支贡献 tick_reads，不按短路、当前命中或窗口真假删项；With/Without 不贡献。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl QueryFilter for Or<()> {
    const IS_ARCHETYPAL: bool = true;
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    /// 解析表达式，汇总跟踪需求并检查静态矛盾，不隐式登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// 绑定声明与窗口，不申请新门禁；含变化分支而无窗口报错。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<T: 'static> QueryFilter for With<T> {
    const IS_ARCHETYPAL: bool = true;
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    /// 解析表达式，汇总跟踪需求并检查静态矛盾，不隐式登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// 绑定声明与窗口，不申请新门禁；含变化分支而无窗口报错。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<T: 'static> QueryFilter for Without<T> {
    const IS_ARCHETYPAL: bool = true;
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    /// 解析表达式，汇总跟踪需求并检查静态矛盾，不隐式登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// 绑定声明与窗口，不申请新门禁；含变化分支而无窗口报错。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<T: 'static> QueryFilter for Added<T> {
    const IS_ARCHETYPAL: bool = false;
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    /// 解析表达式，汇总跟踪需求并检查静态矛盾，不隐式登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// 绑定声明与窗口，不申请新门禁；含变化分支而无窗口报错。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<T: 'static> QueryFilter for Changed<T> {
    const IS_ARCHETYPAL: bool = false;
    type State = CompositeFilterState;
    type FetchState = CompositeFilterState;
    type Fetch<'a> = CompositeFilterFetch<'a>;
    /// 解析表达式，汇总跟踪需求并检查静态矛盾，不隐式登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// 绑定声明与窗口，不申请新门禁；含变化分支而无窗口报错。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<F0: QueryFilter> QueryFilter for (F0,) {
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL;
    type State = FilterTupleState<(F0::State,)>;
    type FetchState = FilterTupleFetchState<(Option<F0::FetchState>,)>;
    type Fetch<'a> = FilterTupleFetch<(Option<F0::Fetch<'a>>,), false>;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<F0: QueryFilter> QueryFilter for Or<(F0,)> {
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL;
    type State = FilterTupleState<(F0::State,)>;
    type FetchState = FilterTupleFetchState<(Option<F0::FetchState>,)>;
    type Fetch<'a> = FilterTupleFetch<(Option<F0::Fetch<'a>>,), true>;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<F0: QueryFilter, F1: QueryFilter> QueryFilter for (F0, F1) {
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL && F1::IS_ARCHETYPAL;
    type State = FilterTupleState<(F0::State, F1::State)>;
    type FetchState = FilterTupleFetchState<(Option<F0::FetchState>, Option<F1::FetchState>)>;
    type Fetch<'a> = FilterTupleFetch<(Option<F0::Fetch<'a>>, Option<F1::Fetch<'a>>), false>;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<F0: QueryFilter, F1: QueryFilter> QueryFilter for Or<(F0, F1)> {
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL && F1::IS_ARCHETYPAL;
    type State = FilterTupleState<(F0::State, F1::State)>;
    type FetchState = FilterTupleFetchState<(Option<F0::FetchState>, Option<F1::FetchState>)>;
    type Fetch<'a> = FilterTupleFetch<(Option<F0::Fetch<'a>>, Option<F1::Fetch<'a>>), true>;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<F0: QueryFilter, F1: QueryFilter, F2: QueryFilter> QueryFilter for (F0, F1, F2) {
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL && F1::IS_ARCHETYPAL && F2::IS_ARCHETYPAL;
    type State = FilterTupleState<(F0::State, F1::State, F2::State)>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
        ),
        false,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<F0: QueryFilter, F1: QueryFilter, F2: QueryFilter> QueryFilter for Or<(F0, F1, F2)> {
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL && F1::IS_ARCHETYPAL && F2::IS_ARCHETYPAL;
    type State = FilterTupleState<(F0::State, F1::State, F2::State)>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
        ),
        true,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<F0: QueryFilter, F1: QueryFilter, F2: QueryFilter, F3: QueryFilter> QueryFilter
    for (F0, F1, F2, F3)
{
    const IS_ARCHETYPAL: bool =
        F0::IS_ARCHETYPAL && F1::IS_ARCHETYPAL && F2::IS_ARCHETYPAL && F3::IS_ARCHETYPAL;
    type State = FilterTupleState<(F0::State, F1::State, F2::State, F3::State)>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
        Option<F3::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
            Option<F3::Fetch<'a>>,
        ),
        false,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<F0: QueryFilter, F1: QueryFilter, F2: QueryFilter, F3: QueryFilter> QueryFilter
    for Or<(F0, F1, F2, F3)>
{
    const IS_ARCHETYPAL: bool =
        F0::IS_ARCHETYPAL && F1::IS_ARCHETYPAL && F2::IS_ARCHETYPAL && F3::IS_ARCHETYPAL;
    type State = FilterTupleState<(F0::State, F1::State, F2::State, F3::State)>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
        Option<F3::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
            Option<F3::Fetch<'a>>,
        ),
        true,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<F0: QueryFilter, F1: QueryFilter, F2: QueryFilter, F3: QueryFilter, F4: QueryFilter>
    QueryFilter for (F0, F1, F2, F3, F4)
{
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL
        && F1::IS_ARCHETYPAL
        && F2::IS_ARCHETYPAL
        && F3::IS_ARCHETYPAL
        && F4::IS_ARCHETYPAL;
    type State = FilterTupleState<(F0::State, F1::State, F2::State, F3::State, F4::State)>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
        Option<F3::FetchState>,
        Option<F4::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
            Option<F3::Fetch<'a>>,
            Option<F4::Fetch<'a>>,
        ),
        false,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<F0: QueryFilter, F1: QueryFilter, F2: QueryFilter, F3: QueryFilter, F4: QueryFilter>
    QueryFilter for Or<(F0, F1, F2, F3, F4)>
{
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL
        && F1::IS_ARCHETYPAL
        && F2::IS_ARCHETYPAL
        && F3::IS_ARCHETYPAL
        && F4::IS_ARCHETYPAL;
    type State = FilterTupleState<(F0::State, F1::State, F2::State, F3::State, F4::State)>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
        Option<F3::FetchState>,
        Option<F4::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
            Option<F3::Fetch<'a>>,
            Option<F4::Fetch<'a>>,
        ),
        true,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<
        F0: QueryFilter,
        F1: QueryFilter,
        F2: QueryFilter,
        F3: QueryFilter,
        F4: QueryFilter,
        F5: QueryFilter,
    > QueryFilter for (F0, F1, F2, F3, F4, F5)
{
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL
        && F1::IS_ARCHETYPAL
        && F2::IS_ARCHETYPAL
        && F3::IS_ARCHETYPAL
        && F4::IS_ARCHETYPAL
        && F5::IS_ARCHETYPAL;
    type State = FilterTupleState<(
        F0::State,
        F1::State,
        F2::State,
        F3::State,
        F4::State,
        F5::State,
    )>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
        Option<F3::FetchState>,
        Option<F4::FetchState>,
        Option<F5::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
            Option<F3::Fetch<'a>>,
            Option<F4::Fetch<'a>>,
            Option<F5::Fetch<'a>>,
        ),
        false,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<
        F0: QueryFilter,
        F1: QueryFilter,
        F2: QueryFilter,
        F3: QueryFilter,
        F4: QueryFilter,
        F5: QueryFilter,
    > QueryFilter for Or<(F0, F1, F2, F3, F4, F5)>
{
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL
        && F1::IS_ARCHETYPAL
        && F2::IS_ARCHETYPAL
        && F3::IS_ARCHETYPAL
        && F4::IS_ARCHETYPAL
        && F5::IS_ARCHETYPAL;
    type State = FilterTupleState<(
        F0::State,
        F1::State,
        F2::State,
        F3::State,
        F4::State,
        F5::State,
    )>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
        Option<F3::FetchState>,
        Option<F4::FetchState>,
        Option<F5::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
            Option<F3::Fetch<'a>>,
            Option<F4::Fetch<'a>>,
            Option<F5::Fetch<'a>>,
        ),
        true,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<
        F0: QueryFilter,
        F1: QueryFilter,
        F2: QueryFilter,
        F3: QueryFilter,
        F4: QueryFilter,
        F5: QueryFilter,
        F6: QueryFilter,
    > QueryFilter for (F0, F1, F2, F3, F4, F5, F6)
{
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL
        && F1::IS_ARCHETYPAL
        && F2::IS_ARCHETYPAL
        && F3::IS_ARCHETYPAL
        && F4::IS_ARCHETYPAL
        && F5::IS_ARCHETYPAL
        && F6::IS_ARCHETYPAL;
    type State = FilterTupleState<(
        F0::State,
        F1::State,
        F2::State,
        F3::State,
        F4::State,
        F5::State,
        F6::State,
    )>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
        Option<F3::FetchState>,
        Option<F4::FetchState>,
        Option<F5::FetchState>,
        Option<F6::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
            Option<F3::Fetch<'a>>,
            Option<F4::Fetch<'a>>,
            Option<F5::Fetch<'a>>,
            Option<F6::Fetch<'a>>,
        ),
        false,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<
        F0: QueryFilter,
        F1: QueryFilter,
        F2: QueryFilter,
        F3: QueryFilter,
        F4: QueryFilter,
        F5: QueryFilter,
        F6: QueryFilter,
    > QueryFilter for Or<(F0, F1, F2, F3, F4, F5, F6)>
{
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL
        && F1::IS_ARCHETYPAL
        && F2::IS_ARCHETYPAL
        && F3::IS_ARCHETYPAL
        && F4::IS_ARCHETYPAL
        && F5::IS_ARCHETYPAL
        && F6::IS_ARCHETYPAL;
    type State = FilterTupleState<(
        F0::State,
        F1::State,
        F2::State,
        F3::State,
        F4::State,
        F5::State,
        F6::State,
    )>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
        Option<F3::FetchState>,
        Option<F4::FetchState>,
        Option<F5::FetchState>,
        Option<F6::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
            Option<F3::Fetch<'a>>,
            Option<F4::Fetch<'a>>,
            Option<F5::Fetch<'a>>,
            Option<F6::Fetch<'a>>,
        ),
        true,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<
        F0: QueryFilter,
        F1: QueryFilter,
        F2: QueryFilter,
        F3: QueryFilter,
        F4: QueryFilter,
        F5: QueryFilter,
        F6: QueryFilter,
        F7: QueryFilter,
    > QueryFilter for (F0, F1, F2, F3, F4, F5, F6, F7)
{
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL
        && F1::IS_ARCHETYPAL
        && F2::IS_ARCHETYPAL
        && F3::IS_ARCHETYPAL
        && F4::IS_ARCHETYPAL
        && F5::IS_ARCHETYPAL
        && F6::IS_ARCHETYPAL
        && F7::IS_ARCHETYPAL;
    type State = FilterTupleState<(
        F0::State,
        F1::State,
        F2::State,
        F3::State,
        F4::State,
        F5::State,
        F6::State,
        F7::State,
    )>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
        Option<F3::FetchState>,
        Option<F4::FetchState>,
        Option<F5::FetchState>,
        Option<F6::FetchState>,
        Option<F7::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
            Option<F3::Fetch<'a>>,
            Option<F4::Fetch<'a>>,
            Option<F5::Fetch<'a>>,
            Option<F6::Fetch<'a>>,
            Option<F7::Fetch<'a>>,
        ),
        false,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}
unsafe impl<
        F0: QueryFilter,
        F1: QueryFilter,
        F2: QueryFilter,
        F3: QueryFilter,
        F4: QueryFilter,
        F5: QueryFilter,
        F6: QueryFilter,
        F7: QueryFilter,
    > QueryFilter for Or<(F0, F1, F2, F3, F4, F5, F6, F7)>
{
    const IS_ARCHETYPAL: bool = F0::IS_ARCHETYPAL
        && F1::IS_ARCHETYPAL
        && F2::IS_ARCHETYPAL
        && F3::IS_ARCHETYPAL
        && F4::IS_ARCHETYPAL
        && F5::IS_ARCHETYPAL
        && F6::IS_ARCHETYPAL
        && F7::IS_ARCHETYPAL;
    type State = FilterTupleState<(
        F0::State,
        F1::State,
        F2::State,
        F3::State,
        F4::State,
        F5::State,
        F6::State,
        F7::State,
    )>;
    type FetchState = FilterTupleFetchState<(
        Option<F0::FetchState>,
        Option<F1::FetchState>,
        Option<F2::FetchState>,
        Option<F3::FetchState>,
        Option<F4::FetchState>,
        Option<F5::FetchState>,
        Option<F6::FetchState>,
        Option<F7::FetchState>,
    )>;
    type Fetch<'a> = FilterTupleFetch<
        (
            Option<F0::Fetch<'a>>,
            Option<F1::Fetch<'a>>,
            Option<F2::Fetch<'a>>,
            Option<F3::Fetch<'a>>,
            Option<F4::Fetch<'a>>,
            Option<F5::Fetch<'a>>,
            Option<F6::Fetch<'a>>,
            Option<F7::Fetch<'a>>,
        ),
        true,
    >;
    /// # 用途
    /// 逐项调用子过滤器 init_state，按位置保存真实 State，不从类型名或内置表达式反推。
    /// 子项错误直接返回，已建状态正常释放，不登记业务类型。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        CompositeFilterState::init_inner::<Self>(world)
    }
    /// # 用途
    /// 对每个 Some 子计划调用其原始 prepare，保存真实 Fetch；None 不调用，OR 判 false。
    /// 不借临时 access，不扩权；任一错误释放已准备项，不交出半组合。
    /// # Safety
    /// World、计划、权限和窗口有效，读取期间排除冲突修改。
    unsafe fn prepare<'a>(
        access: &FetchAccess<'a>,
        plan: FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'a>, EcsError> {
        CompositeFilterState::prepare_inner::<Self>(access, plan, state, fetch_state)
    }
    /// 报告过滤的结构表达式、只读组件及变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，结构匹配和 tick 容器绑定一次完成；动态过滤保留到 matches。
    /// 仅 Table 可返回 NeedsVariants；具体原型 Ready/NotMatched，真实错误为 Err，失败不发布状态。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
        CompositeFilterState::declare_inner::<Self>(state)
    }
}

impl CompositeFilterState {}

impl CompositeFilterState {}

impl CompositeFilterState {
    /// # 用途
    /// scope 与 state/binding 同源：Table 按固定布局证明，Archetype 按完整声明求结构匹配并直接绑定 tick 来源。
    /// Table 表外组件存在性为未知而非缺失；AND/OR 保留影响结果的分支，无法统一则 NeedsVariants。
    /// Ready 保留逐实体动态变化语义；NotMatched 只代表结构不匹配，不根据当前 tick/成员数量决定。
    /// 先检查各分支真实错误，不被恒假/恒真分支掩盖；失败不修改已发布缓存。
    pub(crate) fn init_fetch<F: query_filter_inner::FilterParts>(
        state: &F::State,
        scope: FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<F::FetchState>, EcsError> {
        Self::init_fetch_inner::<F>(state, scope, binding)
    }
}

unsafe impl<'a, T0: FilterFetch<'a>, const ANY: bool> FilterFetch<'a>
    for FilterTupleFetch<(Option<T0>,), ANY>
{
    /// # 用途
    /// 按 ANY 执行真实子 matches，不读已排除分支，不以短路裁剪权限。
    /// # Safety
    /// entity 与全部子访问器同源且在授权内；逐子调用前满足各自过滤前提。
    unsafe fn matches(&mut self, entity: &FetchEntity) -> bool {
        Self::matches_inner(self, entity)
    }
}

unsafe impl<'a, T0: FilterFetch<'a>, T1: FilterFetch<'a>, const ANY: bool> FilterFetch<'a>
    for FilterTupleFetch<(Option<T0>, Option<T1>), ANY>
{
    /// # 用途
    /// 按 ANY 执行真实子 matches，不读已排除分支，不以短路裁剪权限。
    /// # Safety
    /// entity 与全部子访问器同源且在授权内；逐子调用前满足各自过滤前提。
    unsafe fn matches(&mut self, entity: &FetchEntity) -> bool {
        Self::matches_inner(self, entity)
    }
}

unsafe impl<'a, T0: FilterFetch<'a>, T1: FilterFetch<'a>, T2: FilterFetch<'a>, const ANY: bool>
    FilterFetch<'a> for FilterTupleFetch<(Option<T0>, Option<T1>, Option<T2>), ANY>
{
    /// # 用途
    /// 按 ANY 执行真实子 matches，不读已排除分支，不以短路裁剪权限。
    /// # Safety
    /// entity 与全部子访问器同源且在授权内；逐子调用前满足各自过滤前提。
    unsafe fn matches(&mut self, entity: &FetchEntity) -> bool {
        Self::matches_inner(self, entity)
    }
}

unsafe impl<
        'a,
        T0: FilterFetch<'a>,
        T1: FilterFetch<'a>,
        T2: FilterFetch<'a>,
        T3: FilterFetch<'a>,
        const ANY: bool,
    > FilterFetch<'a> for FilterTupleFetch<(Option<T0>, Option<T1>, Option<T2>, Option<T3>), ANY>
{
    /// # 用途
    /// 按 ANY 执行真实子 matches，不读已排除分支，不以短路裁剪权限。
    /// # Safety
    /// entity 与全部子访问器同源且在授权内；逐子调用前满足各自过滤前提。
    unsafe fn matches(&mut self, entity: &FetchEntity) -> bool {
        Self::matches_inner(self, entity)
    }
}

unsafe impl<
        'a,
        T0: FilterFetch<'a>,
        T1: FilterFetch<'a>,
        T2: FilterFetch<'a>,
        T3: FilterFetch<'a>,
        T4: FilterFetch<'a>,
        const ANY: bool,
    > FilterFetch<'a>
    for FilterTupleFetch<(Option<T0>, Option<T1>, Option<T2>, Option<T3>, Option<T4>), ANY>
{
    /// # 用途
    /// 按 ANY 执行真实子 matches，不读已排除分支，不以短路裁剪权限。
    /// # Safety
    /// entity 与全部子访问器同源且在授权内；逐子调用前满足各自过滤前提。
    unsafe fn matches(&mut self, entity: &FetchEntity) -> bool {
        Self::matches_inner(self, entity)
    }
}

unsafe impl<
        'a,
        T0: FilterFetch<'a>,
        T1: FilterFetch<'a>,
        T2: FilterFetch<'a>,
        T3: FilterFetch<'a>,
        T4: FilterFetch<'a>,
        T5: FilterFetch<'a>,
        const ANY: bool,
    > FilterFetch<'a>
    for FilterTupleFetch<
        (
            Option<T0>,
            Option<T1>,
            Option<T2>,
            Option<T3>,
            Option<T4>,
            Option<T5>,
        ),
        ANY,
    >
{
    /// # 用途
    /// 按 ANY 执行真实子 matches，不读已排除分支，不以短路裁剪权限。
    /// # Safety
    /// entity 与全部子访问器同源且在授权内；逐子调用前满足各自过滤前提。
    unsafe fn matches(&mut self, entity: &FetchEntity) -> bool {
        Self::matches_inner(self, entity)
    }
}

unsafe impl<
        'a,
        T0: FilterFetch<'a>,
        T1: FilterFetch<'a>,
        T2: FilterFetch<'a>,
        T3: FilterFetch<'a>,
        T4: FilterFetch<'a>,
        T5: FilterFetch<'a>,
        T6: FilterFetch<'a>,
        const ANY: bool,
    > FilterFetch<'a>
    for FilterTupleFetch<
        (
            Option<T0>,
            Option<T1>,
            Option<T2>,
            Option<T3>,
            Option<T4>,
            Option<T5>,
            Option<T6>,
        ),
        ANY,
    >
{
    /// # 用途
    /// 按 ANY 执行真实子 matches，不读已排除分支，不以短路裁剪权限。
    /// # Safety
    /// entity 与全部子访问器同源且在授权内；逐子调用前满足各自过滤前提。
    unsafe fn matches(&mut self, entity: &FetchEntity) -> bool {
        Self::matches_inner(self, entity)
    }
}

unsafe impl<
        'a,
        T0: FilterFetch<'a>,
        T1: FilterFetch<'a>,
        T2: FilterFetch<'a>,
        T3: FilterFetch<'a>,
        T4: FilterFetch<'a>,
        T5: FilterFetch<'a>,
        T6: FilterFetch<'a>,
        T7: FilterFetch<'a>,
        const ANY: bool,
    > FilterFetch<'a>
    for FilterTupleFetch<
        (
            Option<T0>,
            Option<T1>,
            Option<T2>,
            Option<T3>,
            Option<T4>,
            Option<T5>,
            Option<T6>,
            Option<T7>,
        ),
        ANY,
    >
{
    /// # 用途
    /// 按 ANY 执行真实子 matches，不读已排除分支，不以短路裁剪权限。
    /// # Safety
    /// entity 与全部子访问器同源且在授权内；逐子调用前满足各自过滤前提。
    unsafe fn matches(&mut self, entity: &FetchEntity) -> bool {
        Self::matches_inner(self, entity)
    }
}
