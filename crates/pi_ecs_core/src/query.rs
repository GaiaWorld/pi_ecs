//! 紧凑查询状态及本次权限包装；无独立 View，点查与迭代复用内置 fetch 状态。
use crate::{entity::Entity, error::EcsError, world::World};
/// 核心提供的引用、Entity、可选项与组合描述，不开放外部实现。
/// @to_design 实际引用/元组实现、访问集合和 OwningPtr 转换待闭合。
pub trait QueryData: private::Sealed {
    /// 'a 是组件借用期，'w 是拥有结果有效期；宿主组合点查分离两者。
    /// 系统当前壳保守使用相同短寿命，后续 fetch 不得将其扩大至运行授权外，不引用临时缓冲。
    type Item<'a, 'w>;
    /// 解析类型编号与访问要求的长期状态。
    type State;
    /// 可长期存于 QueryState 的 fetch 准备状态；不能携带 papaya guard 或未经重绑的存储指针。
    type FetchState;
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
    /// 为同 World 的 archetype 生成来源计划，缺必需组件返回 None，不扫描其他原型。
    fn plan(
        state: &Self::State,
        archetype: &crate::archetype::Archetype,
    ) -> Option<crate::query_fetch::ArchetypeFetchPlan>;
    /// 按已解析 state 和 plan 准备可长期保存的 fetch 状态，不读取业务值。
    fn init_fetch_state(
        state: &Self::State,
        plan: &crate::query_fetch::ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError>;
}
/// 只产生共享结果，允许共享 World 宿主入口。
pub trait ReadOnlyQueryData: QueryData {}
/// 过滤的结构匹配和实体判定分离，点查不可绕过过滤。
pub trait QueryFilter: private::Sealed {
    /// 长期过滤状态，不含实际组件借用。
    type State;
    /// 每个紧凑原型条目中的长期过滤 fetch 状态。
    type FetchState;
    /// 本次借用期的实体过滤器。
    type Fetch<'w>: crate::query_fetch::FilterFetch<'w>;
    /// world 已冻结；解析过滤条件，不隐式登记类型。元组 AND，Or 分支 OR。
    /// 静态矛盾返回错误，包括必需 T 与 `Without<T>`；没有当前匹配原型不等于矛盾。
    /// OR 不把单分支必需组件提升到全局；变化条件不能证明互斥。
    fn init_state(world: &World) -> Result<Self::State, EcsError>;
    /// state 与 archetype 属于同一 World，仅判断结构条件。
    fn matches_archetype(state: &Self::State, archetype: &crate::archetype::Archetype) -> bool;
    /// 为匹配 plan 构造长期状态，不读取业务组件。
    fn init_fetch_state(
        state: &Self::State,
        plan: &crate::query_fetch::ArchetypeFetchPlan,
    ) -> Result<Self::FetchState, EcsError>;
    /// 将 fetch_state 绑定到本次 access 和 partition，不扫描实体。
    /// # Safety
    /// 权限/World/计划匹配，guard 及访问范围覆盖全部结果，期间无冲突修改。
    unsafe fn prepare<'w>(
        access: &crate::query_fetch::FetchAccess<'w>,
        partition: crate::entity::PartitionId,
        plan: &crate::query_fetch::ArchetypeFetchPlan,
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
impl<'w> crate::query_fetch::FilterFetch<'w> for NoFilterFetch {
    unsafe fn matches(&mut self, _entity: &crate::query_fetch::FetchEntity) -> bool {
        todo!("尚未实现")
    }
}
impl QueryFilter for NoFilter {
    type State = ();
    type FetchState = ();
    type Fetch<'w> = NoFilterFetch;
    fn init_state(_world: &World) -> Result<(), EcsError> {
        todo!("尚未实现")
    }
    fn matches_archetype(_state: &(), _archetype: &crate::archetype::Archetype) -> bool {
        todo!("尚未实现")
    }
    fn init_fetch_state(
        _state: &(),
        _plan: &crate::query_fetch::ArchetypeFetchPlan,
    ) -> Result<(), EcsError> {
        todo!("尚未实现")
    }
    unsafe fn prepare<'w>(
        _access: &crate::query_fetch::FetchAccess<'w>,
        _partition: crate::entity::PartitionId,
        _plan: &crate::query_fetch::ArchetypeFetchPlan,
        _state: &(),
        _fetch_state: &(),
    ) -> Result<NoFilterFetch, EcsError> {
        todo!("尚未实现")
    }
}
pub(crate) mod private {
    pub trait Sealed {}
}
/// 一个匹配原型的紧凑条目；不为未匹配原型分配完整 fetch 槽。
pub(crate) struct QueryArchetype<Q: QueryData, F: QueryFilter> {
    /// 实际原型编号、组件来源和基础 Table 绑定的准备信息。
    pub(crate) plan: crate::query_fetch::ArchetypeFetchPlan,
    /// 内置数据 fetch 状态，各区的实际地址在本次授权时绑定，不由临时 View 所有。
    pub(crate) data: Q::FetchState,
    /// 内置过滤 fetch 状态，运行时条件仍逐实体检查。
    pub(crate) filter: F::FetchState,
}
/// 每 World 连续 u16 两级索引，前段 Table→变体段偏移，后段 variant→fetch+1，0 不匹配。
pub(crate) struct TableVariantMap {
    /// 两段共用数组；偏移/整体长度容量必须校验，不因 ID16位就假设索引能容纳。
    pub(crate) indices: Vec<u16>,
    /// 每个 Table 已同步的变体数，用于拒绝跨段越界。
    pub(crate) variant_counts: Vec<u32>,
}
/// 本 World 布局派生缓存，恢复后整体失效；不影响其他 World 的查询计划。
pub(crate) struct WorldQueryCache<Q: QueryData, F: QueryFilter> {
    /// 本 World两级映射。
    pub(crate) map: TableVariantMap,
    /// 只保存匹配的紧凑 fetch 条目。
    pub(crate) archetypes: Vec<QueryArchetype<Q, F>>,
    /// 本 World计划所属布局代次。
    pub(crate) layout_epoch: u64,
}
/// 长期状态内置紧凑原型/fetch 数组和压缩映射，不缓存实体结果或长期行号。
/// @to_design 原型版本和 World 兼容身份字段、本阶段重绑定/失效协议仍待闭合。
pub struct QueryState<Q: QueryData, F: QueryFilter = NoFilter> {
    /// 类型解析与访问声明。
    pub(crate) data_state: Q::State,
    /// 过滤声明。
    pub(crate) filter_state: F::State,
    /// 本 World 的两级索引及紧凑计划，恢复后整体重建。
    pub(crate) cache: WorldQueryCache<Q, F>,
}
impl<Q: QueryData, F: QueryFilter> QueryState<Q, F> {
    /// 宿主显式只读观察窗口；不分配写 tick，不修改或自动推进宿主观察点。
    /// window.last_run=None 表示首次，按左开右闭筛选；非法时间顺序拒绝，不能隐式替换全局基线。
    /// 观察点的 World/恢复分支正确性完全由调用者负责，不附加身份/epoch 或分支校验。
    /// # Safety
    /// 同 query：保证包装及全部结果期间无冲突写/结构失效，World/后端保护有效。
    /// tick 读取不是写者完成屏障，调用者须保证观察截点前相关写入已收束。
    pub unsafe fn query_with_window<'w>(
        &'w mut self,
        world: &'w World,
        window: crate::change_detection::ChangeWindow,
    ) -> Result<Query<'w, Q, F>, EcsError>
    where
        Q: ReadOnlyQueryData,
    {
        crate::query_inner::query_with_window_inner(self, world, window)
    }
    /// 宿主可写观察：last_run 为调用者上次成功观察点，None 为首次；准备后取得本次写 tick 作为上界。
    /// 分配规则同 query_mut，不另收任意上界；非法窗口/耗尽在交出可写结果前报错，不推进调用者进度。
    /// 观察点的 World/恢复分支正确性完全由调用者负责，不附加身份/epoch 或分支校验。
    /// # Safety
    /// 同 query_mut：全使用期排除冲突访问/结构失效并保活存储；不申请门禁，截点需外部同步。
    pub unsafe fn query_mut_since<'w>(
        &'w mut self,
        world: &'w World,
        last_run: Option<crate::change_detection::ChangeTick>,
    ) -> Result<QueryMut<'w, Q, F>, EcsError> {
        crate::query_inner::query_mut_since_inner(self, world, last_run)
    }

    /// world 兼容且已冻结，共享 &World 绑定本次可变访问，不要求 WorldPin/WorldWrite。
    /// 宿主无窗口入口若 F 含 Added/Changed（含组合分支）返回缺少观察窗口错误，不隐式选择基线。
    /// 共享签名不是可变授权：按 Q/F 的访问声明排除冲突，系统复用已有运行授权。
    /// 不申请/登记宿主门禁，不持动态访问租约；仍校验组合/计划并准备 fetch 和后端保护。
    /// 宿主校验/准备成功后分配一次非零写 tick，整次 QueryMut 点查/遍历共用；无实际修改也允许消耗。
    /// 耗尽在交出可写结果前报错，不回绕；系统可信绑定使用已有 this_run，不调用此宿主分配路径。
    /// # Safety
    /// 调用者须保证本次包装及所有派生结果/迭代器有效期内，无重叠可变访问、冲突系统执行、
    /// 会使存储失效的结构修改/回收/settle/恢复/关闭；World/后端保护始终有效。
    /// 不能依赖 pin/write/维护入口发现本查询：本入口没有登记。
    pub unsafe fn query_mut<'w>(
        &'w mut self,
        world: &'w World,
    ) -> Result<QueryMut<'w, Q, F>, EcsError> {
        /* 设计已确认：
        校验 World/类型/计划并准备 fetch 与后端保护，不申请访问门禁。
        准备成功后分配一次非零写 tick；耗尽清理本次准备并报错，不交出可写结果。
        返回持固定 tick 的 QueryMut；实际写标记才更新组件时间/通知。
        系统由可信绑定提供 this_run，不重复分配；外部同步责任不变。
        */
        crate::query_inner::query_mut_inner(self, world)
    }
    /// 仅只读 Q，共享绑定不申请门禁、不分配写 tick；错误不改业务值。
    /// 宿主无窗口入口若 F 含 Added/Changed（含组合分支）返回缺少观察窗口错误，不隐式选择基线。
    /// # Safety
    /// 调用者在本次包装及全部派生结果有效期内排除冲突写入、系统执行及失效存储的结构操作，
    /// 保证 World 与后端保护有效；只读不等于无需外部同步，系统由调度器履行此责任。
    pub unsafe fn query<'w>(&'w mut self, world: &'w World) -> Result<Query<'w, Q, F>, EcsError>
    where
        Q: ReadOnlyQueryData,
    {
        crate::query_inner::query_inner(self, world)
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
        world: &'w World,
        entity: Entity,
    ) -> Result<Q::Item<'w, 'w>, EcsError> {
        crate::query_inner::get_mut_inner(self, world, entity)
    }
    /// 只读单点，完整 Entity 与过滤均需通过，不改业务值，不申请宿主门禁、不分配写 tick。
    /// 宿主无窗口入口若 F 含 Added/Changed（含组合分支）返回缺少观察窗口错误，不隐式选择基线。
    /// # Safety
    /// 调用者在返回结果有效期内排除冲突写入/系统执行和使存储失效的结构操作，
    /// 保证 World 与后端保护有效；不能以共享签名或只读结果替代外部同步。
    pub unsafe fn get<'w>(
        &'w mut self,
        world: &'w World,
        entity: Entity,
    ) -> Result<Q::Item<'w, 'w>, EcsError>
    where
        Q: ReadOnlyQueryData,
    {
        crate::query_inner::get_inner(self, world, entity)
    }
}
/// 本次共享访问包装，随机点查和遍历共用 QueryState 内置 fetch，不另建 View。
/// 安全构造依赖 unsafe 入口调用方/调度器的访问证明，不代表已登记动态门禁。
pub struct Query<'w, Q: ReadOnlyQueryData, F: QueryFilter = NoFilter> {
    /// 本次显式宿主窗口或系统注入窗口；无变化过滤时可为 None，不保存在长期 QueryState。
    pub(crate) window: Option<crate::change_detection::ChangeWindow>,
    /// 宿主只读存储借用，系统受限入口仍待接线。
    pub(crate) world: &'w World,
    /// 已完成阶段准备的紧凑状态。
    pub(crate) state: &'w mut QueryState<Q, F>,
}
impl<Q: ReadOnlyQueryData, F: QueryFilter> Query<'_, Q, F> {
    /// entity 完整有效；位置→压缩偏移→内置 fetch→实体过滤→取值，不扫描全局原型。
    /// 缺失或不匹配返回错误，引用受本次借用限制。
    pub fn get(&self, entity: Entity) -> Result<Q::Item<'_, '_>, EcsError> {
        crate::query_inner::query_get_inner(self, entity)
    }
    /// 遍历紧凑匹配原型成员，跳过洞且每实体最多一次；不承诺稳定顺序。
    pub fn iter(&self) -> Result<crate::query_iter::QueryIter<'_, Q, F>, EcsError> {
        crate::query_iter_inner::iter_inner(self)
    }
}
/// 本次可变包装，直接绑定共享 World；组件可变性由调用方/调度器证明，不申请宿主门禁。
pub struct QueryMut<'w, Q: QueryData, F: QueryFilter = NoFilter> {
    /// 本次显式宿主窗口或系统注入窗口；无变化过滤时可为 None，不保存在长期 QueryState。
    pub(crate) window: Option<crate::change_detection::ChangeWindow>,
    /// 宿主绑定时分配、系统绑定时提供的 this_run；所有派生写包装共用，不逐 get/next 分配。
    pub(crate) write_tick: crate::change_detection::ChangeTick,
    /// World 存活绑定；本字段单独不能排除组件冲突或授予可变访问。
    pub(crate) world: &'w World,
    /// 已准备紧凑原型/fetch 状态；写标记使用 write_tick，取得包装本身不标变化。
    pub(crate) state: &'w mut QueryState<Q, F>,
}
impl<Q: QueryData, F: QueryFilter> QueryMut<'_, Q, F> {
    /// 返回固定写 tick，也是 query_mut_since 的观察上界；不推进宿主观察点，不另分配。
    pub fn this_run(&self) -> crate::change_detection::ChangeTick {
        crate::query_inner::this_run_inner(self)
    }
    /// entity 完整有效；通过压缩偏移选择已准备条目，不重复解析组合或建立临时 View。
    /// 返回短借用防止同实体重复可变取值并存，缺失/过滤失败返回错误。
    pub fn get_mut(&mut self, entity: Entity) -> Result<Q::Item<'_, '_>, EcsError> {
        crate::query_inner::query_get_mut_inner(self, entity)
    }
    /// 每实体只产出一次，可同时保留不同实体结果；创建失败返回错误，next 内部失败 panic。
    pub fn iter_mut(&mut self) -> Result<crate::query_iter::QueryIter<'_, Q, F>, EcsError> {
        crate::query_iter_inner::iter_mut_inner(self)
    }
}
#[cfg(test)]
mod unchecked_binding_compile_checks {
    use super::*;
    // 只编译，不执行失败桩；本函数把完整使用期的排他责任继续交给调用者。
    unsafe fn bind_and_read<Q: ReadOnlyQueryData, F: QueryFilter>(
        state: &mut QueryState<Q, F>,
        world: &World,
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
        world: &World,
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
// 长期 FetchState 和本次 Fetch 分开，仅为内存/guard 生命周期约束，不恢复独立 View API。
// QueryState 直接共享入口均 unsafe 且不申请门禁/租约，系统由调度器证明后在内部绑定。
// @to_design Query/QueryMut/QueryState 的必需后端 guard 持有与遗忘安全、
// 单点结果保护期、分区 fetch 紧凑组织、位宽编码及系统权限/变化窗口待闭合。
// 不将接口改为传入 WorldPin/WorldWrite，也不暗中给每个系统查询重新申请宿主全局门禁。
