//! 查询计划、访问令牌与取值协议；不实现原始指针读取。
use crate::{
    component::{Component, ComponentId, ComponentRead, ComponentWrite},
    entity::{ArchetypeId, Entity, EntityLocation},
    error::EcsError,
    query::QueryData,
    world::World,
};

/// 本次访问的内部权限令牌；仅可信查询入口构造，不提供安全的裸 World 逃逸。
/// 生命周期只约束存储存活；可变权限必须由独占入口或系统访问证明授予，不能从共享 World 制造。
/// 可变查询只授权已校验的组件集合；共享指针不自动授权可写访问。
pub struct FetchAccess<'w> {
    /// 统一底层句柄，复制不新增权限；实际组件引用期内禁止冲突结构修改或整理。
    pub(crate) world: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
    /// 已校验的权限范围，含实际 tick_reads；系统入口由 RunAccess 的短借用提供，不复制授权。
    /// 写组件可经自身 ChangeMarker 读 tick；只读变化过滤必须有对应 tick_reads，tracking 开关不授予读取权。
    pub(crate) declaration: &'w crate::access::AccessDeclaration,
    /// 系统或宿主显式提供的变化窗口；无窗口查询为 None，不由 fetch 自行推断。
    pub(crate) window: Option<crate::change_detection::ChangeWindow>,
    /// 已授权写上下文的统一 tick；只读授权为 None，write 不另行分配。
    pub(crate) write_tick: Option<crate::change_detection::ChangeTick>,
    // 本访问不持有哈希/节点 guard：稀疏存储已改为两级 SparseArray，papaya 已移除。
    // 地址稳定性由 storage 契约保证：非 rc 段增长不搬移旧槽；rc 增长需独占，借用期内不可增长。
    // 结果寿命统一由 'w 约束，禁止在授权结束后保留引用；不据 PhantomData 推导 Send/Sync。
}
/// 本次目标实体及其已校验位置，供过滤和取值共用，不跨结构修改保存。
pub struct FetchEntity {
    /// 完整身份；稀疏访问须同时校验域位、槽索引与代际，不能只用 index。
    pub(crate) entity: Entity,
    /// 同时包含 Table 行及 archetype_row 的当前位置。
    pub(crate) location: EntityLocation,
}
#[path = "query_fetch_inner.rs"]
mod query_fetch_inner;
impl FetchEntity {
    /// 返回完整目标身份，不增持实体，不授予其他实体访问权限。
    pub fn entity(&self) -> Entity {
        Self::entity_inner(self)
    }
    /// 返回本次已验证位置的副本；不能跨迁移、整理或恢复作为有效位置复用。
    pub fn location(&self) -> EntityLocation {
        Self::location_inner(self)
    }
}
impl<'w> FetchAccess<'w> {
    /// 保留同一授权期的受限访问描述，供外部 Fetch 保存；不新增授权、门禁或存储保护责任。
    /// 返回值可晚于当前描述对象，但不能晚于原始授权；内部后端 guard 必须由原授权作用域保活。
    /// 多个描述不允许重复可变借用，实际取值仍须满足 read/write 的 unsafe 前提。
    pub fn retain(&self) -> Self {
        Self::retain_inner(self)
    }
    /// entity 为框架传入的本次目标；只读取得 T，缺失返回 None，不分配、不标变化、不隐式注册。
    /// 返回 ComponentRead：普通组件为共享引用，内置类型为只读专用对象，不暴露原始容器头。
    /// 真实槽解析后统一调用 crate 内 component::read_component；Removed 独立调用 read_removed，不按旧实体查询当前槽。
    /// 类型、声明读权限（或写权限）、分区/原型范围及当前身份位置须在交出结果前校验；违反协议 panic。
    /// # Safety
    /// entity 必须属于本 access 的 World，位置仍有效且已通过本查询过滤；不得伪造跨 World 的同号身份。
    /// 调用者保证整个 'w 内没有冲突写入；所有引用和后端 guard 由原授权保活，不能靠本描述的 Drop 延长保护。
    pub unsafe fn read<T: Component>(
        &self,
        entity: &FetchEntity,
    ) -> Option<ComponentRead<'w, 'w, T>> {
        Self::read_inner::<T>(self, entity)
    }
    /// entity 为框架传入的本次目标；取得 T 的授权写对象，缺失返回 None，不隐式插入或改结构。
    /// 返回 ComponentWrite：普通组件为 Mut，内置类型为专用写对象；按已有规则标 tick/通知，不开放绕过标记的写指针。
    /// RC 容器写结果独占单个头但共享 EntityNodePools，每操作短借后端；不得逐项构造整个池的 &mut。
    /// 真实槽及 ChangeMarker 解析后统一调用 crate 内 component::write_component，不在每个 QueryData 重复构造内置对象。
    /// 类型、声明写权限、分区/原型范围及当前身份位置须在交出结果前校验；无写授权、无写 tick 或协议错误 panic。
    /// # Safety
    /// entity 的 World、位置、过滤和保活要求同 read；调用者额外保证整个结果期无重叠读写引用，
    /// 包括本查询的重复实体、其他 retain 描述及其他访问器；共享 self 本身不证明独占性。
    pub unsafe fn write<T: Component>(
        &self,
        entity: &FetchEntity,
    ) -> Option<ComponentWrite<'w, 'w, T>> {
        Self::write_inner::<T>(self, entity)
    }
    // 设计已确认：先验证权限/身份/类型/位置，再只对真实单槽形成引用，委托封闭策略包装。
    // 绝不为定位后续行重建整个 World/列/稀疏集合的可变引用；元数据共享访问不得覆盖活槽。
    // None 仅表示可选组件缺失，身份/权限/计划失配不能伪装 None；写包装不先标记。
    // 已确定：无后端 guard 需要保活；写 tick/ChangeMarker 注入与运行窗口仍需接入真实 fetch。
    // RC read/write 通过配对芯验证原授权；不提供裸 World 或伪造声明入口。
}
impl World {
    /// 查询本 World 已登记 T 的组件编号；未登记为 None，不访问业务值、不隐式注册或取得值权限。
    pub fn component_id<T: Component>(&self) -> Option<ComponentId> {
        Self::component_id_inner::<T>(self)
    }
}
/// 单个分区原型的可执行 fetch；内部持有列/稀疏访问器，不持有实体结果缓存。
/// 可由外部 QueryData 关联实现，unsafe 实现必须证明访问不越权且返回引用生命周期正确。
/// # Safety
/// 实现的真实读写不得超过 Q 声明，返回值由授权保活且无冲突别名。
pub unsafe trait QueryFetch<'w, Q: QueryData> {
    /// 从已校验逻辑计划准备本次访问器，不扫描实体，不重新解析类型。
    /// plan 仅是临时范围参数；组件身份/地址从 state/fetch_state 取得，不保存另一份路径数组。
    /// plan 为 Table 时直接绑定字段缓存来源，不要求等于某个具体原型；
    /// fetch_at 校验实体 Table 归属而非 variant 相等。Archetype 范围才要求完整原型匹配。
    /// access 必须属于 plan 所属 World；绑定后逐实体不再分派计划或重新搜索容器，错误不修改业务值。
    /// # Safety
    /// 调用方已校验 Q 内部冲突、World/原型兼容及权限，访问期间排除结构修改。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        plan: FetchScope,
        state: &Q::State,
        fetch_state: &Q::FetchState,
    ) -> Result<Self, EcsError>
    where
        Self: Sized;
    /// 按 entity 的当前行取值，Column/Sparse OwningPtr 在此边界才转换为 Q 的引用组合。
    /// # Safety
    /// entity 属于已准备区/原型、完整键/发布位置有效且匹配；RC 允许 strong=0 但非 Null。
    /// 不把查询有效误作强增持许可；过滤已通过。返回可变值时，调用方必须保证
    /// 在所有仍存活结果及其他访问器间没有冲突；同一实体重复 fetch 不能自动成立。
    /// 结果不得借用 self 内部临时缓冲，应指向由 access 保活的存储。
    unsafe fn fetch_at(&mut self, entity: &FetchEntity) -> Q::Item<'w, 'w>;
}
/// 过滤与数据取值分离；状态不允许偷偷修改业务组件或产生未声明写访问。
/// # Safety
/// 只读取已声明且受授权保护的过滤状态，不构造逃逸引用或修改业务值。
pub unsafe trait FilterFetch<'w> {
    /// entity 为已匹配原型内的有效目标，true 表示通过；点查和遍历调用相同语义。
    /// # Safety
    /// 访问器已为目标区/原型准备，访问令牌和过滤上下文有效，不产生冲突引用。
    unsafe fn matches(&mut self, entity: &FetchEntity) -> bool;
}
/// 同一视图内每个区/原型的已准备访问器；成员行和组件存储不可在此期间搬动。
pub(crate) struct PreparedFetch<'w, Q: QueryData, F: crate::query::QueryFilter> {
    /// 当前原型数据访问器。
    pub(crate) data: Q::Fetch<'w>,
    /// 同原型的实体过滤器。
    pub(crate) filter: F::Fetch<'w>,
}

/// # 用途
/// 冻结容器缓存的受限绑定来源；只解析固定容器，不授予组件取值/结构修改或运行权限。
/// 仅 core 在匹配条目初始化时构造，不向扩展提供整个 World；不用于每实体寻址。
pub struct FetchBinding<'a> {
    /// 已冻结同源物理目录；只能取得非拥有容器缓存，不得从这里读取业务值。
    pub(crate) storage: &'a crate::storage::WorldStorage,
    /// 来源 World 的非复用身份；缓存不能用于其他 World。
    pub(crate) world_identity: u64,
    /// 冻结的原型元数据，供变体绑定按范围解析来源；只共享声明，不暴露业务值。
    pub(crate) archetypes: &'a [crate::archetype::Archetype],
}
impl FetchBinding<'_> {
    /// # 用途
    /// table/column 为已解析同源固定列；按编号导出根地址，直接交给字段 FetchState 保存。
    /// 缺失/来源不一致返回错误，不构造组件引用，不登记全局槽号，不修改任何缓存或业务值。
    /// 地址从存储后端的稳定根导出，普通运行不重复调用。
    pub fn column_address(
        &self,
        table: crate::entity::TableId,
        column: pi_ecs_storage::table::ColumnId,
    ) -> Result<std::ptr::NonNull<pi_ecs_storage::column::Column>, EcsError> {
        self.column_address_inner(table, column)
    }
    /// # 用途
    /// component 是已解析同源稀疏组件；直接导出稳定根地址供对应 FetchState 保存。
    /// 未登记/来源不符为错误，不创建后端、不读取业务值，地址不拥有或延长 World 寿命。
    pub fn sparse_address(
        &self,
        component: ComponentId,
    ) -> Result<std::ptr::NonNull<pi_ecs_storage::sparse_set::SparseSetStorage>, EcsError> {
        self.sparse_address_inner(component)
    }
}

/// # 用途
/// 计划的真实匹配范围；全表与单原型不能互相冒充。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FetchScope {
    /// 任意当前/未来合法变体均适用；逐实体仅校验 Table 归属，不匹配 variant。
    Table {
        /// 冻结的物理 Table 编号。
        table: crate::entity::TableId,
    },
    /// 只适用于一个已匹配原型。
    Archetype {
        /// 完整原型身份。
        archetype: ArchetypeId,
    },
}
impl FetchScope {
    /// # 用途
    /// archetype 是同源已登记完整原型；只记录其身份，不重复解析或分配组件路径数组。
    /// 是否匹配由 Q/F 判断，组件来源在长期状态绑定时由真实元数据计算并写入 FetchState。
    pub fn new(archetype: &crate::archetype::Archetype) -> Self {
        Self::identity_inner(archetype)
    }
}
