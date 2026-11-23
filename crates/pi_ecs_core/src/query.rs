//! RC 查询可见性以完整身份/非 Null 发布位置为准，strong=0 待收束仍可读写组件，不能据此重新保活。
//! 点查/批量/遍历一致；begin_destroy 隐藏后不可见，结果存活期间禁止销毁收束。
//! 紧凑查询状态及本次权限包装；无独立 View，点查与迭代复用内置 fetch 状态。
#[path = "query_inner.rs"]
mod query_inner;
use crate::{
    access::{AccessDeclaration, AccessMode},
    change_detection::TrackingDemand,
    component::ComponentId,
    entity::Entity,
    error::EcsError,
    query_filter::FilterExpression,
    world::World,
};
#[cfg(feature = "rc")]
pub(crate) use query_inner::{missing_entity, unmatched_entity};
/// 查询项在原型上的存在性；可选项缺失仍匹配，必需项缺失使原型不匹配。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryItemPresence {
    /// 原型必须包含该组件，缺失则不匹配。
    Required,
    /// 允许缺失，不约束原型匹配；fetch 时缺失返回空分支。
    Optional,
}
/// 查询数据声明的单项组件访问；保留重复项供组合前别名检查。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryComponentAccess {
    /// 已登记组件编号，不跨 World 解释。
    pub component: ComponentId,
    /// 读写模式，决定与其他参数/查询的冲突。
    pub mode: AccessMode,
    /// 是否要求原型包含该组件。
    pub presence: QueryItemPresence,
}
/// QueryData 的真实访问声明；由类型描述解析，不读取业务值或存储地址。
pub struct QueryDataDeclaration {
    /// 按描述顺序保留的组件访问，包括重复项；不得遗漏 fetch 实际访问的组件。
    pub components: Box<[QueryComponentAccess]>,
    /// 本描述需要启用的变化状态，未启用的项不得写 tick。
    pub tracking: Box<[crate::tracking::ComponentTickDemand]>,
    /// 实际读取活 tick 的组件，独立于 tracking 的分配需求；含重复可在汇总时去重。
    /// Ref/OrDefaultRef 或 Added/Changed 按所属描述贡献，不产生值权限或通知流能力。
    pub tick_reads: Box<[ComponentId]>,
}
/// QueryFilter 的真实声明；结构表达式保留 AND/OR 分支，变化项另记 tick 需求。
pub struct QueryFilterDeclaration {
    /// 供原型匹配和跨参数冲突检查的结构表达式，不扁平化 OR；组件身份均在其中。
    pub expression: FilterExpression,
    /// 过滤启用变化状态所需的 tick 需求；结构条件不读取值。
    pub tracking: Box<[crate::tracking::ComponentTickDemand]>,
    /// 实际读取活 tick 的组件，独立于 tracking 分配需求与结构表达式。
    /// 外部过滤器必须显式覆盖全部真实读取；组合保留所有子项并集，不按短路/命中删项。
    pub tick_reads: Box<[ComponentId]>,
}
/// 单个查询的数据与过滤声明汇总；供调用方先查别名再组合系统权限。
pub struct QueryAccessDeclaration {
    /// 数据描述的组件访问及变化需求。
    pub data: QueryDataDeclaration,
    /// 过滤的结构/变化声明。
    pub filter: QueryFilterDeclaration,
}
impl QueryAccessDeclaration {
    /// 合并为系统权限声明；不读取业务值、不取得运行权限、不缓存部分结果。
    /// 数据组件重复出现且任一项为 Write 时返回错误，不按后写覆盖；两个 Read 允许。
    /// 过滤的 With/Without/Added/Changed 只读结构或变化元数据，不并入值权限，也不与数据写构成别名。
    /// 只有 All、With、Without、Added、Changed 的 AND 组合能收窄 scope：With/Added/Changed 进入 with，Without 进入 without。
    /// 出现 Or 或空 OR 时清空 with/without，不把单分支要求提升为全局，也不据此证明跨系统互斥。
    /// 数据 Required 项并入 scope.with，Optional 项不约束匹配；实体范围不按分区收窄。
    /// structure 为 None，observations 为空；tracking 取数据与过滤的逐项并集，任一为真即为真。
    /// tick_reads 合并数据/过滤实际读项，保留与值权限的区分；内部含写 Q/F 的过滤先短读再取值。
    /// 组件值内部别名检查只比较真实值项；不能因同查询内部读 tick 就拒绝 &mut C + Changed<C>。
    /// 权限组合仍由调用方汇总多个参数后检查；本方法不做跨参数仲裁。
    pub fn to_access(&self) -> Result<AccessDeclaration, EcsError> {
        Self::to_access_inner(self)
    }
}
/// 查询数据描述，框架提供内置组合，允许外部 unsafe 实现并承担完整访问及生命周期责任。
/// RC 类型化 fetch 通过元组组合、值/tick 权限和单槽包装提供受限访问。
/// # Safety
/// 实现必须完整声明实际访问并保持结果生命周期，禁止内部可变别名和越权取值。
pub unsafe trait QueryData {
    /// 'a 是组件借用期，'w 是拥有结果有效期；宿主组合点查分离两者。
    /// 系统当前壳保守使用相同短寿命，后续 fetch 不得将其扩大至运行授权外，不引用临时缓冲。
    type Item<'a, 'w>;
    /// 解析类型编号与访问要求的长期状态。
    type State;
    /// 报告本描述的真实组件访问与变化需求；不读取业务值、不取得运行权限。
    /// 必须覆盖 plan/fetch 可能访问的全部组件；可选项标 Optional，变化读取写入 tracking。
    /// 重复项按描述顺序保留，由调用方在组合前检查可变别名；返回空声明掩盖真实访问违反 unsafe 契约。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError>;
    /// 可长期存于 QueryState 的准备状态；直接持有对应稳定容器裸地址。
    /// 不缓存值/tick 缓冲地址或借用。
    /// 缓存不保活 World、不授予权限；先核对绑定 World 身份，再使用容器地址。
    type FetchState;
    /// # 用途
    /// state/scope/binding 同源且布局冻结；一次完成范围匹配、来源解析和稳定容器地址绑定。
    /// Table 范围：Ready 须证明全部当前/未来合法变体结构适用且共享完整获取状态；
    /// NotMatched 须证明全表恒不匹配，不能统一则 NeedsVariants，不以当前样本或权限声明推断。
    /// Archetype 范围：只允许 Ready/NotMatched；返回 NeedsVariants 违反实现契约，调用方报错且不推进进度。
    /// 真正类型/来源/绑定错误走 Err，不降级或被其他字段排除吞掉；Ready 不含 Unbound 或本次视图。
    /// 元组/过滤按声明递归组合，保留动态过滤；不读业务值、不登记类型，失败不发布部分状态。
    /// 外部 unsafe 实现须显式实现或转发；无法证明全表可返回 NeedsVariants，不伪造 Ready。
    fn init_fetch_state(
        state: &Self::State,
        scope: crate::query_fetch::FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<FetchStateResult<Self::FetchState>, EcsError>;

    /// 本次访问器，从内置 FetchState 绑定实际权限和存储，不能跨阶段使用。
    type Fetch<'w>: crate::query_fetch::QueryFetch<'w, Self>
    where
        Self: Sized;
    /// world 已冻结；先拒绝内部可变别名，再与过滤组合检查静态矛盾，不登记类型。
    /// Option 首版仅 &T/&mut T，可选缺失返回 None，不决定原型匹配；存在时照常计入别名检查。
    /// 单一 &mut C 实现委托 ComponentWrite，&C 委托 ComponentRead；不另写重叠容器特化。
    /// 普通值返回 Mut，内置容器返回专用对象；Option/元组/Lookup/批量遵守同一映射。
    /// 重复只读允许。失败不改业务值；实际引用 QueryData/QueryFetch 实例尚未接线。
    fn init_state(world: &World) -> Result<Self::State, EcsError>;
}
/// 只产生共享结果，允许共享 World 宿主入口。
/// # Safety
/// 所有取值及其派生行为只读，不得隐藏修改访问。
pub unsafe trait ReadOnlyQueryData: QueryData {}
/// 过滤的结构匹配和实体判定分离，点查不可绕过过滤。
/// # Safety
/// 必须完整声明只读过滤访问，不能修改业务值；结构匹配与逐实体判断保持一致。
pub unsafe trait QueryFilter {
    /// 为 true 时只依赖原型结构，对所有已匹配实体的动态 matches 必须为 true。
    const IS_ARCHETYPAL: bool;
    /// 长期过滤状态，不含实际组件借用。
    /// 开放元组/OR 必须逐项保存原始关联 State，不根据内置表达式重建外部状态。
    type State;
    /// 每个紧凑原型条目中的长期过滤 fetch 状态。
    /// 开放组合逐项保存 Option<子 FetchState>，None 仅表示结构 NotMatched。
    type FetchState;
    /// # 用途
    /// state/scope/binding 同源且布局冻结；一次完成范围匹配、来源解析和稳定容器地址绑定。
    /// Table 范围：Ready 须证明全部当前/未来合法变体结构适用且共享完整获取状态；
    /// NotMatched 须证明全表恒不匹配，不能统一则 NeedsVariants，不以当前样本或权限声明推断。
    /// Archetype 范围：只允许 Ready/NotMatched；返回 NeedsVariants 违反实现契约，调用方报错且不推进进度。
    /// 真正类型/来源/绑定错误走 Err，不降级或被其他字段排除吞掉；Ready 不含 Unbound 或本次视图。
    /// 元组/过滤按声明递归组合，保留动态过滤；不读业务值、不登记类型，失败不发布部分状态。
    /// 外部 unsafe 实现须显式实现或转发；无法证明全表可返回 NeedsVariants，不伪造 Ready。
    fn init_fetch_state(
        state: &Self::State,
        scope: crate::query_fetch::FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<FetchStateResult<Self::FetchState>, EcsError>;

    /// 本次借用期的实体过滤器。
    /// 开放组合保存并调用真实子 Fetch，不以 FilterExpression 代替动态 matches。
    type Fetch<'w>: crate::query_fetch::FilterFetch<'w>;
    /// world 已冻结；解析过滤条件，不隐式登记类型。元组 AND，Or 分支 OR。
    /// 静态矛盾返回错误，包括必需 T 与 `Without<T>`；没有当前匹配原型不等于矛盾。
    /// OR 不把单分支必需组件提升到全局；变化条件不能证明互斥。
    fn init_state(world: &World) -> Result<Self::State, EcsError>;
    /// 报告结构表达式、只读组件及变化需求；不读取业务值、不取得运行权限。
    /// OR 保留分支不扁平化；Added/Changed 写入相应 tick 需求，不得声明值写访问。
    /// tick_reads 显式报告全部实际 tick 读取，不能仅从 tracking 或内置表达式猜测；
    /// 组合递归合并全部子声明，准备/动态短路不能删减权限。
    /// 不得遗漏 matches_archetype/matches 实际读取的组件；重复项由调用方组合时检查。
    fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError>;
    /// 将 fetch_state 绑定到本次 access，不扫描实体，也不涉及分区。
    /// # Safety
    /// 权限/World/计划匹配，guard 及访问范围覆盖全部结果，期间无冲突修改。
    unsafe fn prepare<'w>(
        access: &crate::query_fetch::FetchAccess<'w>,
        plan: crate::query_fetch::FetchScope,
        state: &Self::State,
        fetch_state: &Self::FetchState,
    ) -> Result<Self::Fetch<'w>, EcsError>;
}
/// 无额外过滤，必需组件仍由数据描述约束。
pub struct NoFilter;
/// 无额外实体判断的访问器。
pub struct NoFilterFetch;
impl private::Sealed for NoFilter {}
impl private::Sealed for NoFilterFetch {}
unsafe impl<'w> crate::query_fetch::FilterFetch<'w> for NoFilterFetch {
    /// # 用途
    /// 判断已授权目标的无过滤访问；尚未接入真实过滤取值。
    /// # Safety
    /// 调用方保证实体、计划与授权同源且有效，读取期排除冲突修改。
    unsafe fn matches(&mut self, _entity: &crate::query_fetch::FetchEntity) -> bool {
        Self::matches_inner(self, _entity)
    }
}
unsafe impl QueryFilter for NoFilter {
    const IS_ARCHETYPAL: bool = true;
    type State = ();
    type FetchState = ();
    type Fetch<'w> = NoFilterFetch;
    /// # 用途
    /// 无过滤在 Table/Archetype 范围均返回完整空状态，不绑定业务后端。
    fn init_fetch_state(
        state: &Self::State,
        scope: crate::query_fetch::FetchScope,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<FetchStateResult<Self::FetchState>, EcsError> {
        crate::query_filter::CompositeFilterState::init_fetch::<Self>(state, scope, binding)
    }
    /// # 用途
    /// 解析无过滤长期状态，不登记组件或访问业务值。
    fn init_state(_world: &World) -> Result<(), EcsError> {
        Self::init_state_inner(_world)
    }
    /// # 用途
    /// 描述无额外过滤的权限与需求，不授予运行权限。
    fn declare_access(_state: &()) -> Result<QueryFilterDeclaration, EcsError> {
        crate::query_filter::CompositeFilterState::declare_inner::<Self>(_state)
    }
    /// # 用途
    /// 绑定同源计划与本次授权，结果不得越过授权寿命。
    /// # Safety
    /// 调用方保证实体、计划与授权同源且有效，读取期排除冲突修改。
    unsafe fn prepare<'w>(
        _access: &crate::query_fetch::FetchAccess<'w>,
        _plan: crate::query_fetch::FetchScope,
        _state: &(),
        _fetch_state: &(),
    ) -> Result<NoFilterFetch, EcsError> {
        Self::prepare_inner(_access, _plan, _state, _fetch_state)
    }
}
pub(crate) mod private {
    /// 数据描述独立封闭，避免与过滤元组的条件实现重叠。
    pub trait DataSealed {}
    /// # 用途
    /// 框架过滤标记的封闭边界，与数据描述封闭边界分离。
    pub trait Sealed {}
}
/// 一个匹配范围的紧凑 fetch 计划；RC 归属 QueryTable 的统一分支或变体分支。
pub(crate) struct QueryArchetype<Q: QueryData, F: QueryFilter> {
    /// 数据状态直接保存稳定容器地址，准备时直接绑定当前缓冲视图。
    pub(crate) data: Q::FetchState,
    /// 内置过滤 fetch 状态，运行时条件仍逐实体检查。
    pub(crate) filter: F::FetchState,
}
/// # 用途
/// 同源查询缓存；按 Table 聚合匹配与准备状态，身份单源保存在 QueryState.bound_world。
pub(crate) struct WorldQueryCache<Q: QueryData, F: QueryFilter> {
    /// 潜在相关 Table 的最小编号；tables 为空时规范为零，没有可访问上界。
    /// 冻结布局下首次确定，不以当前非空或已有匹配变体裁剪未来候选。
    pub(crate) first_table: crate::entity::TableId,
    /// 连续 Table 区间，tables[i] 对应 first_table + i。
    /// 非空时最大编号为 usize::from(first_table) + tables.len() - 1，计算不得在 u16 上溢出。
    /// 区间内空洞用 QueryTable::Excluded；区间外直接拒绝，不使用无符号回绕索引。
    pub(crate) tables: Box<[QueryTable<Q, F>]>,
}
/// 长期状态内置紧凑原型/fetch 数组和压缩映射，不缓存实体结果或长期行号。
/// 重绑定协议：首次绑定记录 World 身份；此后身份必须相同，否则拒绝复用。
/// 系统启动前按系统总量记录决定是否同步；参数只比较相关 Table 变体数量并扫描新增后缀。
/// 宿主入口执行同样检查；布局替换代次变化才全量重建，新原型追加不推进该代次。
/// 不匹配变体也计入已检查数量，成员增删不刷新匹配；不缓存实体行号。
/// RC 的 FetchState 缓存冻结容器裸地址，随 World 身份核对；值/tick 缓冲视图仅在本次授权内绑定。
/// 当前 RC 不提供存储替换/恢复操作；缓存不含可搬移缓冲地址，不因缓冲增长重新搜索容器。
pub struct QueryState<Q: QueryData, F: QueryFilter = NoFilter> {
    /// 宿主完整叶子声明的唯一拥有者；初始化验证 Q/F 后生成，首次计划同步成功后提交。
    /// 布局代次变化时在无活包装/结果的独占准备期整体重建；新增变体只刷新解析范围。
    /// 失败不发布半声明或借用，注销后不得再绑定；系统路径仍借系统自己的 prepared_access。
    pub(crate) host_access: Option<crate::access::AccessTree>,
    /// 本查询在 World 全局 tick 表中的独立责任；无 tick 需求为 None，声明固定，OR 全分支并集。
    /// 必须显式 unregister_tracking 清偿，Drop 不提交；不能复制或仅按组件编号注销他人需求。
    pub(crate) tracking_registration: Option<crate::tracking::TickDemandRegistration>,
    /// 显式清偿后为 true，即使原本无 tick 需求也不可再次查询；不以 None 猜测是否已注销。
    pub(crate) tracking_unregistered: bool,
    /// 类型解析与访问声明。
    pub(crate) data_state: Q::State,
    /// 过滤声明。
    pub(crate) filter_state: F::State,
    /// 首次成功绑定后的 World 身份；None 表示尚未绑定，绑定后不得改指其他 World。
    pub(crate) bound_world: Option<u64>,
    /// 本 World 的两级索引及紧凑计划，恢复或布局变化后整体重建。
    pub(crate) cache: WorldQueryCache<Q, F>,
}
impl<Q: QueryData, F: QueryFilter> QueryState<Q, F> {
    /// # 用途
    /// RC 冻结后的 world 静止点创建并绑定状态，登记本查询数据/过滤逐组件 tick 并集。
    /// 类型/声明失败清理本次部分登记，不修改业务值；无 tick 需求也绑定 World 身份。
    /// 不立即应用 tick 槽；宿主调用 World::apply_tracking 后才使用，系统复用 Stage 登记屏障。
    /// 本入口仅宿主 Idle；系统初始化使用 ParameterRegistration::query_state，不能因根 Running 统一放行。
    #[cfg(feature = "rc")]
    pub fn new(world: &mut World) -> Result<Self, EcsError> {
        Self::new_registered(world)
    }
    /// # 用途
    /// 显式撤销本查询 tick 责任；world 必须同源，无活查询结果，错误保留登记以便正确清偿。
    /// 已解除/无需求幂等；成功后本状态禁止再查询，重新使用须创建新状态，不自动重新登记。
    /// 不影响其他查询/通知订阅；本次不运行维护，下一次 apply_tracking 更新有效开关。
    /// 本入口仅宿主 Idle/Stopped；系统失败清理/注销经 ParameterRegistration::unregister_query。
    #[cfg(feature = "rc")]
    pub fn unregister_tracking(&mut self, world: &mut World) -> Result<(), EcsError> {
        self.unregister_tracking_inner(world)
    }
    /// 汇总本查询数据与过滤器的声明，不读取业务值或取得运行权限。
    /// 组件项及 OR 分支原样保留，由调用方先检查重复/可变别名再与系统权限组合。
    /// 失败不缓存半声明；重复调用只依赖长期 state，不改变计划或追踪需求。
    pub fn declaration(&self) -> Result<QueryAccessDeclaration, EcsError> {
        Self::declaration_inner(self)
    }
    /// 宿主显式只读观察窗口；不分配写 tick，不修改或自动推进宿主观察点。
    /// window.last_run=None 表示首次，按左开右闭筛选；非法时间顺序拒绝，不能隐式替换全局基线。
    /// 观察点的 World/恢复分支正确性完全由调用者负责，不附加身份/epoch 或分支校验。
    /// # Safety
    /// 同 query：保证包装及全部结果期间无冲突写/结构失效，World/后端保护有效。
    /// tick 读取不是写者完成屏障，调用者须保证观察截点前相关写入已收束。
    pub unsafe fn query_with_window<'w>(
        &'w mut self,
        world: crate::world::WorldRef<'w>,
        window: crate::change_detection::ChangeWindow,
    ) -> Result<Query<'w, Q, F>, EcsError>
    where
        Q: ReadOnlyQueryData,
    {
        Self::query_with_window_inner(self, world, window)
    }
    /// 宿主可写观察：last_run 为调用者上次成功观察点，None 为首次；准备后取得本次写 tick 作为上界。
    /// 分配规则同 query_mut，不另收任意上界；非法窗口/耗尽在交出可写结果前报错，不推进调用者进度。
    /// 观察点的 World/恢复分支正确性完全由调用者负责，不附加身份/epoch 或分支校验。
    /// # Safety
    /// 同 query_mut：全使用期排除冲突访问/结构失效并保活存储；不申请门禁，截点需外部同步。
    pub unsafe fn query_mut_since<'w>(
        &'w mut self,
        world: crate::world::WorldRef<'w>,
        last_run: Option<crate::change_detection::ChangeTick>,
    ) -> Result<QueryMut<'w, Q, F>, EcsError> {
        Self::query_mut_since_inner(self, world, last_run)
    }

    /// world 兼容且已冻结，共享 &World 绑定本次可变访问，不要求 WorldPin/WorldWrite。
    /// 宿主无窗口入口若 F 含 Added/Changed（含组合分支）返回缺少观察窗口错误，不隐式选择基线。
    /// 共享签名不是可变授权：按 Q/F 的访问声明排除冲突，系统复用已有运行授权。
    /// 不申请/登记宿主门禁，不持动态访问租约；仍校验组合/计划并准备 fetch 和后端保护。
    /// 准备全部完成后借 self.host_access 生成完整叶子 RunAccess，包装只读借整个准备状态；
    /// host_access 与 state 均为 self 的共享投影，避免返回借状态内字段同时独占整个状态的自引用结构。
    /// 生命周期仍由入口独占 self 的借用限制，活包装/结果期间不能刷新状态或再次绑定。
    /// 宿主校验/准备成功后分配一次非零写 tick，整次 QueryMut 点查/遍历共用；无实际修改也允许消耗。
    /// 耗尽在交出可写结果前报错，不回绕；系统可信绑定使用已有 this_run，不调用此宿主分配路径。
    /// # Safety
    /// 调用者须保证本次包装及所有派生结果/迭代器有效期内，无重叠可变访问、冲突系统执行、
    /// 会使存储失效的结构修改/回收/settle/恢复/关闭；World/后端保护始终有效。
    /// 不能依赖 pin/write/维护入口发现本查询：本入口没有登记。
    pub unsafe fn query_mut<'w>(
        &'w mut self,
        world: crate::world::WorldRef<'w>,
    ) -> Result<QueryMut<'w, Q, F>, EcsError> {
        /* 设计已确认：
        校验 World/类型/计划并准备 fetch 与后端保护，不申请访问门禁。
        准备成功后分配一次非零写 tick；耗尽清理本次准备并报错，不交出可写结果。
        返回持固定 tick 的 QueryMut；实际写标记才更新组件时间/通知。
        系统由可信绑定提供 this_run，不重复分配；外部同步责任不变。
        */
        Self::query_mut_inner(self, world)
    }
    /// 仅只读 Q，共享绑定不申请门禁、不分配写 tick；错误不改业务值。
    /// 宿主无窗口入口若 F 含 Added/Changed（含组合分支）返回缺少观察窗口错误，不隐式选择基线。
    /// # Safety
    /// 调用者在本次包装及全部派生结果有效期内排除冲突写入、系统执行及失效存储的结构操作，
    /// 保证 World 与后端保护有效；只读不等于无需外部同步，系统由调度器履行此责任。
    pub unsafe fn query<'w>(
        &'w mut self,
        world: crate::world::WorldRef<'w>,
    ) -> Result<Query<'w, Q, F>, EcsError>
    where
        Q: ReadOnlyQueryData,
    {
        Self::query_inner(self, world)
    }
    /// 共享 World 单点入口，只补目标原型；缺实体/不匹配返回错误，不构造全局 View。
    /// 宿主无窗口入口若 F 含 Added/Changed（含组合分支）返回缺少观察窗口错误，不隐式选择基线。
    /// 不申请/保存宿主访问租约；返回值可越过调用，必需的后端 guard 仍须保活到结果结束。
    /// 校验匹配与准备成功后每次分配一个新写 tick，耗尽在返回可写结果前报错；拒绝不分配。
    /// 实际写入才标 Changed；批量优先绑定 QueryMut 复用 tick，系统点查使用已有 this_run。
    /// # Safety
    /// 调用者在返回结果有效期内保证 World/后端保护有效、无冲突读写或系统执行、无使结果
    /// 失效的结构修改/回收/settle/恢复/关闭；本入口不登记，其他安全入口不会自动发现该访问。
    pub unsafe fn get_mut<'w>(
        &'w mut self,
        world: crate::world::WorldRef<'w>,
        entity: Entity,
    ) -> Result<Q::Item<'w, 'w>, EcsError> {
        Self::get_mut_inner(self, world, entity)
    }
    /// 只读单点，完整 Entity 与过滤均需通过，不改业务值，不申请宿主门禁、不分配写 tick。
    /// 宿主无窗口入口若 F 含 Added/Changed（含组合分支）返回缺少观察窗口错误，不隐式选择基线。
    /// # Safety
    /// 调用者在返回结果有效期内排除冲突写入/系统执行和使存储失效的结构操作，
    /// 保证 World 与后端保护有效；不能以共享签名或只读结果替代外部同步。
    pub unsafe fn get<'w>(
        &'w mut self,
        world: crate::world::WorldRef<'w>,
        entity: Entity,
    ) -> Result<Q::Item<'w, 'w>, EcsError>
    where
        Q: ReadOnlyQueryData,
    {
        Self::get_inner(self, world, entity)
    }
}
/// 本次共享访问包装，随机点查和遍历共用 QueryState 内置 fetch，不另建 View。
/// 安全构造依赖 unsafe 入口调用方/调度器的访问证明，不代表已登记动态门禁。
pub struct Query<'w, Q: ReadOnlyQueryData, F: QueryFilter = NoFilter> {
    /// 本次显式宿主窗口或系统注入窗口；无变化过滤时可为 None，不保存在长期 QueryState。
    pub(crate) window: Option<crate::change_detection::ChangeWindow>,
    /// 按值完整叶子授权，宿主/系统均复用原授权寿命，不借临时运行描述，也不恢复整份 World。
    pub(crate) access: crate::access::RunAccess<'w>,
    /// 已完成阶段准备的紧凑状态。
    pub(crate) state: &'w QueryState<Q, F>,
}
impl<Q: ReadOnlyQueryData, F: QueryFilter> Query<'_, Q, F> {
    /// entity 完整有效；位置→压缩偏移→内置 fetch→实体过滤→取值，不扫描全局原型。
    /// 缺失或不匹配返回错误，引用受本次借用限制。
    pub fn get(&self, entity: Entity) -> Result<Q::Item<'_, '_>, EcsError> {
        Self::query_get_inner(self, entity)
    }
    /// 遍历紧凑匹配原型成员，跳过洞且每实体最多一次；不承诺稳定顺序。
    pub fn iter(&self) -> Result<crate::query_iter::QueryIter<'_, Q, F>, EcsError> {
        Self::iter_inner(self)
    }
}
/// 本次可变包装，直接绑定共享 World；组件可变性由调用方/调度器证明，不申请宿主门禁。
pub struct QueryMut<'w, Q: QueryData, F: QueryFilter = NoFilter> {
    /// 本次显式宿主窗口或系统注入窗口；无变化过滤时可为 None，不保存在长期 QueryState。
    pub(crate) window: Option<crate::change_detection::ChangeWindow>,
    /// 宿主绑定时分配、系统绑定时提供的 this_run；所有派生写包装共用，不逐 get/next 分配。
    pub(crate) write_tick: crate::change_detection::ChangeTick,
    /// 按值完整叶子授权，保留原声明及原型版本；宿主 unsafe 绑定亦构造同源叶子授权。
    /// get_mut/iter_mut 仅短借此授权，不能从裸句柄重建更宽权限或重复兑现同一可变位置。
    pub(crate) access: crate::access::RunAccess<'w>,
    /// 已准备紧凑原型/fetch 状态；写标记使用 write_tick，取得包装本身不标变化。
    pub(crate) state: &'w QueryState<Q, F>,
}
impl<Q: QueryData, F: QueryFilter> QueryMut<'_, Q, F> {
    /// 返回固定写 tick，也是 query_mut_since 的观察上界；不推进宿主观察点，不另分配。
    pub fn this_run(&self) -> crate::change_detection::ChangeTick {
        Self::this_run_inner(self)
    }
    /// entity 完整有效；通过压缩偏移选择已准备条目，不重复解析组合或建立临时 View。
    /// 返回短借用防止同实体重复可变取值并存，缺失/过滤失败返回错误。
    pub fn get_mut(&mut self, entity: Entity) -> Result<Q::Item<'_, '_>, EcsError> {
        Self::query_get_mut_inner(self, entity)
    }
    /// 每实体只产出一次，可同时保留不同实体结果；创建失败返回错误，next 内部失败 panic。
    pub fn iter_mut(&mut self) -> Result<crate::query_iter::QueryIter<'_, Q, F>, EcsError> {
        Self::iter_mut_inner(self)
    }
}
#[cfg(test)]
mod unchecked_binding_compile_checks {
    use super::*;
    // 只编译，不执行失败桩；本函数把完整使用期的排他责任继续交给调用者。
    unsafe fn bind_and_read<Q: ReadOnlyQueryData, F: QueryFilter>(
        state: &mut QueryState<Q, F>,
        world: &mut World,
        entity: Entity,
    ) -> Result<(), EcsError> {
        // SAFETY: 本函数调用者保证本次包装和派生结果期间没有冲突写入/结构操作。
        let query = unsafe { state.query(world)? };
        let value = query.get(entity)?;
        drop(value);
        drop(query);
        Ok(())
    }
    unsafe fn bind_and_write<Q: QueryData, F: QueryFilter>(
        state: &mut QueryState<Q, F>,
        world: &mut World,
        entity: Entity,
    ) -> Result<(), EcsError> {
        // SAFETY: 本函数调用者保证全使用期排他访问及存储有效。
        let mut query = unsafe { state.query_mut(world)? };
        let value = query.get_mut(entity)?;
        drop(value);
        drop(query);
        Ok(())
    }
}
// 长期 FetchState 与本次 Fetch 分开；RC 状态直接保存稳定容器地址，不保存可搬移值/tick 缓冲地址。
// 无额外后端 guard：非 rc 分段地址稳定，rc 增长需要独占且借用期内无法增长；
// 因此所有结果寿命由 FetchAccess/Query 的 'w 约束，不需要另建遗忘型 guard 表。
// 单点结果借用本次 Query/QueryMut；遍历每实体最多一次，结构修改前必须结束。
// QueryState 直接共享入口均 unsafe 且不申请门禁/租约，系统由调度器证明后在内部绑定。
// 重绑定、World 身份与布局代次检查见 QueryState；分区 fetch 的位宽组织仍在计划侧。
// 不将接口改为传入 WorldPin/WorldWrite，也不暗中给每个系统查询重新申请宿主全局门禁。

/// # 用途
/// 单 Table 的完整查询状态；编号为 cache.first_table 加数组偏移，不另存路由。
/// 统一计划与变体计划在此二选一，适用于 Query 遍历、Lookup 点查及 Alter 的匹配部分。
pub(crate) enum QueryTable<Q: QueryData, F: QueryFilter> {
    /// 区间内可静态排除的 Table，不准备 fetch、不扫描成员或后续变体。
    Excluded,
    /// 已证明任意合法变体都结构匹配且可共用获取计划；未来新增变体无需重新匹配。
    Uniform {
        /// 数据与过滤的长期获取状态；准备时从外层 Table 编号临时传入全表范围。
        fetch: QueryArchetype<Q, F>,
    },
    /// 无法证明全表统一；只扫描本 Table 新增变体后缀。
    Variants {
        /// variant 直接索引的匹配条目偏移；None 为已检查但不匹配。
        /// 数组长度就是已检查变体数量，不另存 scanned_variants；不从 entries 数量推断进度。
        map: Vec<Option<usize>>,
        /// 本 Table 匹配的紧凑变体计划；完整新条目成功后才追加 map 并发布。
        entries: Vec<(
            // 真实变体编号，用于成员遍历；不重复保存可由外层计算的 TableId。
            crate::entity::VariantId,
            // 本变体的数据与过滤获取状态，不再持有计划包装。
            QueryArchetype<Q, F>,
        )>,
    },
}

/// # 用途
/// Q/F 对匹配范围的完整状态构造结果；与 Err 分离，不把无法优化当作故障。
/// 外层 Result 的错误单源为 pi_result 报告；本枚举无错误字段，三个正常分支不创建报告。
pub enum FetchStateResult<T> {
    /// 给定范围不匹配；Table 表示所有合法变体均不匹配，Archetype 表示该具体原型不匹配。
    NotMatched,
    /// 无法证明全表统一，改用变体匹配；不代表存在任何当前匹配变体。
    NeedsVariants,
    /// 已完成范围匹配、来源与地址绑定的长期状态；不含本次借用。
    Ready(
        /// 已匹配范围的数据或过滤长期状态，不含未绑定来源。
        T,
    ),
}
impl<Q: QueryData, F: QueryFilter> QueryTable<Q, F> {
    /// # 用途
    /// data/filter 是同源已初始化声明，layout 为本固定表，binding 仅提供元数据与稳定地址。
    /// 以 Table 范围完成两侧 init_fetch_state 检查，任一 Err 丢弃临时状态且不发布缓存；不能以另一侧 Excluded 吞掉错误。
    /// 任一 NotMatched 则排除此表；两侧 Ready 才构造 Uniform；其余构造空 Variants 后扫描真实变体。
    /// Uniform 不再重复构造；Variants 对具体原型调用同一入口，完整成功后才发布。
    /// 缓存发布后普通运行只调用 prepare，新增变体仅更新 Variants，不重新证明已冻结表。
    pub(crate) fn bind_table(
        data: &Q::State,
        filter: &F::State,
        layout: &pi_ecs_storage::table::TableLayout,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<Self, EcsError> {
        Self::bind_table_inner(data, filter, layout, binding)
    }
}

impl<Q: QueryData, F: QueryFilter> QueryTable<Q, F> {
    /// # 用途
    /// data/filter/binding/archetype 同源，archetype 属于本表；仅在无活 fetch/结果的匹配同步点调用。
    /// Excluded 保持排除，Uniform 忽略新增变体，两者不调用 Q/F、不分配条目或记录无用逐表扫描进度。
    /// Variants 按 variant 编号顺序处理，map.len() 是下一未处理编号；已处理为幂等，跳号不允许跨过后缀。
    /// 以 Archetype 范围调用 Q/F::init_fetch_state；任一 Err 或 NeedsVariants 为失败，不推进 map。
    /// 无错误且任一 NotMatched 追加 None；两者 Ready 则先准备容量，再共同发布 entries 与 Some 偏移。
    /// 原型编号从 archetype 读取，绑定错误不变成过滤失败；先前已成功后缀保留，重试从 map.len() 继续。
    pub(crate) fn bind_variant(
        &mut self,
        data: &Q::State,
        filter: &F::State,
        archetype: &crate::archetype::Archetype,
        binding: &crate::query_fetch::FetchBinding<'_>,
    ) -> Result<(), EcsError> {
        self.bind_variant_inner(data, filter, archetype, binding)
    }
}
