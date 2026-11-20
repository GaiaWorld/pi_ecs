//! 立即发布的严格结构修改；查询选择与增删合法性独立，固定原型列表按目标优先遍历。
#[path = "alter_inner.rs"]
mod alter_inner;
use crate::{
    bundle::{Bundle, SystemBundle},
    component::ComponentId,
    entity::{ArchetypeId, Entity},
    error::EcsError,
    world::{World, WorldRef},
};
/// # 用途
/// 固定新增/删除声明与源转换逻辑缓存，不保存组件值或批次请求。
pub struct AlterState {
    /// 严格新增，已登记、无重复。
    pub(crate) added: Box<[ComponentId]>,
    /// 严格删除，已登记、无重复且与 added 不相交。
    pub(crate) removed: Box<[ComponentId]>,
    /// 当前 World 的逻辑计划缓存，不创建目标原型。
    pub(crate) cache: AlterCache,
}
/// # 用途
/// 源到逻辑计划的缓存；不因查询匹配而假定增删合法。
pub(crate) struct AlterCache {
    /// 冻结后确定的潜在相关源 Table 及扫描进度，不能只保留当前已命中的源。
    pub(crate) tables: Box<[crate::archetype_scan::TableScan]>,
    /// 所属唯一 World。
    pub(crate) world_identity: u64,
    /// 生成时布局代次，失配重新计算；不得改变本次成员借用/游标内固定的匹配列表与遍历顺序。
    pub(crate) layout_epoch: u64,
    /// Table/variant 到逻辑计划编号的映射。
    pub(crate) map: crate::plan_map::ArchetypePlanMap,
    /// 只包含严格可转换源，查询遍历不据此过滤实体。
    pub(crate) plans: Vec<AlterPlan>,
}
/// # 用途
/// 固定增删对应一个源的逻辑计划，不持组件值/物理地址，不以目标存在为前提。
#[derive(Clone)]
pub(crate) struct AlterPlan {
    /// 已登记的源原型，计划只适用于此源。
    pub(crate) source: ArchetypeId,
    /// 升序完整目标组件集合，首次执行时据此查找或创建 variant。
    pub(crate) target_components: Box<[ComponentId]>,
    /// 已存在的基础 Table（variant=0），prepare 按最大覆盖规则选择，不新增 Table。
    pub(crate) target_table: ArchetypeId,
    /// 可选已存在目标；None 不表示错误，也不得回退源原型，执行时重新解析。
    pub(crate) resolved_target: Option<ArchetypeId>,
    /// 共有组件的来源映射；目标列由基础 Table 确定，不依赖目标 variant 创建。
    pub(crate) retained: Vec<crate::structural_change::TransferComponent>,
    /// 严格新增组件，输入必须精确提供这些值。
    pub(crate) added: Box<[ComponentId]>,
    /// 严格删除组件，源必须全部包含。
    pub(crate) removed: Box<[ComponentId]>,
}
impl AlterState {
    /// # 用途
    /// world 已冻结；added/removed 必须已登记、无重复且互不相交，空集合合法。
    /// 失败不改 World，成功绑定声明；不登记目标原型。
    pub fn new(
        world: &World,
        added: &[ComponentId],
        removed: &[ComponentId],
    ) -> Result<Self, EcsError> {
        Self::new_inner(world, added, removed)
    }
    /// # 用途
    /// 为同一 world 的严格可转换源生成逻辑计划，缺目标记 None，不创建 variant/成员存储或推进代次。
    /// 布局替换时全量重建，其余仅检查相关 Table 新增后缀；成功才推进扫描记录。
    /// 失败不发布半缓存、不修改实体；Q/F 独立匹配，新增匹配后重排目标优先顺序。
    pub fn prepare(&mut self, world: WorldRef<'_>) -> Result<(), EcsError> {
        self.prepare_inner(world)
    }
    /// # 用途
    /// world 无冲突组件引用，取得结构排他作用域并运行 run，宿主共用一次 tick，系统复用 this_run。
    /// 每次 alter 返回前已发布，不在退出/Drop/finish 提交；普通错误保留此前成功，panic 标停不回滚。
    /// 返回 R 不得携带作用域借用；退出不执行 settle 或移动其他活成员。
    pub fn scope<R>(
        &mut self,
        world: WorldRef<'_>,
        run: impl for<'a> FnOnce(&mut Alter<'a>) -> R,
    ) -> Result<R, EcsError> {
        self.scope_inner(world, run)
    }
}
/// # 用途
/// 受限即时结构修改会话，不保存延迟批次、去重集合或 World 裸指针。
pub struct Alter<'a> {
    /// 可刷新逻辑缓存，实际转换首次创建目标。
    pub(crate) state: &'a mut AlterState,
    /// 统一底层句柄，来源与会话同一 World；每次操作仅短借所需字段，不还原整个 &mut World。
    pub(crate) world: crate::unsafe_world_cell::UnsafeWorldCell<'a>,
    /// 本次写入/通知共用的变化 tick。
    pub(crate) tick: crate::change_detection::ChangeTick,
}
impl Alter<'_> {
    /// # 用途
    /// entity 为当前存活完整键；values 类型精确等于固定 added，直接按值写存储，不经过 EntityValues。
    /// 仅调用时检查 added 全缺、removed 全在；不通过批次 HashSet 拒绝重复，重复调用按当前组合校验。
    /// 首次修改前错误释放尚未移入的值；首次修改后失败标停，局部进度负责唯一清理，不回滚。
    /// 成功返回时 retained/removed/added、位置/成员和通知已完成，源成员留洞，目标仅尾部追加。
    /// RC 前置要求 strong>0 且位置非 Null；归零待销毁即拒绝，不修改结构/成员或移交输入。
    pub fn alter<I: Bundle>(&mut self, entity: Entity, values: I) -> Result<(), EcsError> {
        /* 设计已确认：校验当前实体/严格增删/类型和来源，按需创建目标并准备槽。
        按值直接写新增，栈上进度保护部分初始化；搬 retained、转交/释放 removed。
        发布位置/成员及通知，源留洞不交换尾成员；不保存请求，不在 Drop 再发布。 */
        self.alter_inner(entity, values)
    }
}
/// 查询式 Alter 的长期状态，参考 pi_world::QueryAlterState 的查询/变更组合，不持 World 裸指针。
/// 同一参数位置独立拥有；初始化解析固定增删集合，prepare 刷新查询和源/目标访问声明。
pub struct QueryAlterState<
    Q: crate::query::QueryData,
    F: crate::query::QueryFilter = crate::query::NoFilter,
> {
    /// 仅 Q/F 的类型状态和匹配缓存，不混入严格增删条件；允许 Option 结果有/无两分支。
    /// RC 复用表级覆盖路由；点查可省 variant 匹配，但严格增删与目标计划仍依赖实际完整源原型。
    pub(crate) query: crate::query::QueryState<Q, F>,
    /// 固定新增/删除集合与源到最终原型映射；与 query 属于同一 World。
    pub(crate) alteration: AlterState,
    /// 登记时 Add 的展平顺序，供运行输入核对；与 alteration.added 的集合一致。
    pub(crate) added_order: Box<[ComponentId]>,
    /// 系统启动前、或 ParamSet 每次成员重绑时按当前已发布原型完整生成，只保留 Q/F 匹配。
    /// 快照在本次成员借用及其游标内冻结；切换结束后再次取参可纳入此前新建原型。
    /// 若固定严格增删可将 A 转为匹配目标 B，B 必须在 A 前；不以可转换性过滤查询。
    /// 非空增删下目标不再是合法源，故可将不可转换原型排前、合法源排后，无需通用拓扑图。
    /// 两组内保留原匹配顺序；空增删不搬成员、沿用查询顺序，不形成自环排序约束。
    pub(crate) iteration_order: Box<[ArchetypeId]>,
    /// RC 已证明安全的表级执行顺序；None 表示按 iteration_order 直接遍历原型。
    /// 表顺序和每表变体顺序均须满足目标先于源，不能为了排序改动按 TableId 索引的 query.cache.tables。
    /// 全部可合并且无回流才记录 Some；不能证明则整次回退原型，不引入混合目标或尾界。
    /// 每次重建 iteration_order 都须重新证明并生成本字段，禁止沿用先前快照的表顺序。
    pub(crate) table_order: Option<Box<[u16]>>,
}
#[cfg(feature = "rc")]
impl<Q: crate::query::QueryData, F: crate::query::QueryFilter> QueryAlterState<Q, F> {
    /// # 用途
    /// 返回 Q/F 的精确匹配声明和同源的运行权限（含固定增删）。
    /// 只读声明，不交付内部查询状态或运行授权；运行准备使用 RunAccess::refresh_alter。
    pub fn declaration(
        &self,
    ) -> Result<
        (
            crate::query::QueryAccessDeclaration,
            crate::access::AccessDeclaration,
        ),
        EcsError,
    > {
        self.declaration_inner()
    }
}
/// # 用途
/// 即时结构修改系统参数；Q/F 声明可操作源，Add/Remove 静态声明增删集合，() 为空。
/// Add 接收同形 Bundle，包括短借输入；Remove 只查类型清单，从不构造或接收值。
/// 源及对应目标均为结构写，另计 Q 的值权限；宿主 AlterState 仍按编号配置。
pub struct QueryAlter<
    'a,
    Q: crate::query::QueryData,
    F: crate::query::QueryFilter = crate::query::NoFilter,
    Add: SystemBundle<Shape = Add> + 'static = (),
    Remove: Bundle = (),
> {
    /// 仅标记静态增删类型，不拥有值或借用运行输入。
    pub(crate) shape: std::marker::PhantomData<fn() -> (Add, Remove)>,
    /// 本参数长期查询/变更状态的本次独占借用，不克隆计划或缓存组件地址。
    pub(crate) state: &'a mut QueryAlterState<Q, F>,
    /// 复用本次运行授权，覆盖完整源/目标结构写，冲突结果先结束，不重新申请 World 门禁。
    pub(crate) access: crate::access::RunAccess<'a>,
    /// 系统固定观察窗口与统一写 tick；不借临时 context，不逐点查/结构操作重新分配。
    pub(crate) window: crate::change_detection::ChangeWindow,
}
/// # 用途
/// QueryAlter 本次短借游标，保留同一组 Q/F/Add/Remove；结束结果借用后才能前进或修改。
pub struct AlterIter<
    'a,
    'run,
    Q: crate::query::QueryData,
    F: crate::query::QueryFilter = crate::query::NoFilter,
    Add: SystemBundle<Shape = Add> + 'static = (),
    Remove: Bundle = (),
> {
    /// 短借用期为 'a，内部状态/运行授权期为 'run；二者独立，禁止另一游标同时访问同批状态。
    pub(crate) source: &'a mut QueryAlter<'run, Q, F, Add, Remove>,
    /// 最近成功返回且可操作的实体；耗尽后清空，失败不得沿用旧当前项。
    pub(crate) current: Option<Entity>,
    /// 本次所选表顺序或原型顺序的游标，前进后不回退；不分配每游标列表或成员尾界。
    pub(crate) archetype_cursor: usize,
    /// 当前目标下一行；原型级为成员行，RC 表级为物理行，先前进再 fetch。
    /// 遇洞跳过，读取当前已发布长度，无起始尾界；结构操作后重新短借元数据，不持旧缓冲引用。
    pub(crate) member_cursor: usize,
}
impl<'run, Q, F, Add, Remove> QueryAlter<'run, Q, F, Add, Remove>
where
    Q: crate::query::QueryData,
    F: crate::query::QueryFilter,
    Add: SystemBundle<Shape = Add> + 'static,
    Remove: Bundle,
{
    /// # 用途
    /// entity 须通过源声明及过滤；短借用返回 Q 结果，内部别名检查不可豁免。
    /// RC 表级路由可直接定位统一 Q/F 计划，仍核对当前位置与动态过滤；不以全表匹配放行严格增删。
    pub fn get_mut(&mut self, entity: Entity) -> Result<Q::Item<'_, '_>, EcsError> {
        self.query_get_inner(entity)
    }
    /// # 用途
    /// 仅按 Q/F 创建游标（Option 组件不附加 With/Without），使用本次成员借用固定的目标优先顺序。
    /// 已有匹配目标先遍历，转换仅向已结束的原型追加；本次借用中新目标不加入列表，不记录成员尾界。
    /// ParamSet 下次取该成员前重建快照并纳入已发布新目标，同步重证 table_order；不改变系统排序。
    /// 固定非空严格增删不形成 A<->B；空增删成功为结构无操作，不搬成员、不追加或制造通知。
    /// 删除留洞不复用，遍历内禁止 settle；不得交错其他 Alter 声明、spawn 或迁移向未遍历原型注入实体。
    /// 每项不保存跨操作组件地址，前次结果结束后才能修改；其他冲突结构修改由授权排除。
    pub fn iter_mut(&mut self) -> AlterIter<'_, 'run, Q, F, Add, Remove> {
        self.iter_inner()
    }
    /// # 用途
    /// entity 须通过本次 Q/F；values 为 Add 的同形输入，可短借，来源及有序组件清单须一致。
    /// Remove 无运行值；严格增删按当前位置检查，待增全缺、待删全在。
    /// 成功使用 this_run 直接写入并立即发布；空增删不搬移、不发布通知。
    /// 前置拒绝不修改实体；写入后失败沿 Alter::alter 的停止与局部清理。
    pub fn alter<I: SystemBundle<Shape = Add>>(
        &mut self,
        entity: Entity,
        values: I,
    ) -> Result<(), EcsError> {
        self.query_alter_inner(entity, values)
    }
}
impl<Q, F, Add, Remove> AlterIter<'_, '_, Q, F, Add, Remove>
where
    Q: crate::query::QueryData,
    F: crate::query::QueryFilter,
    Add: SystemBundle<Shape = Add> + 'static,
    Remove: Bundle,
{
    /// # 用途
    /// 返回下一项短借用结果；旧结果释放后可 alter 当前实体或继续前进，耗尽为 None。
    /// 首版不实现长寿命 Iterator，避免迭代器同时修改已交出的组件结果。
    pub fn fetch_next(&mut self) -> Option<Q::Item<'_, '_>> {
        self.iter_next_inner()
    }
    /// # 用途
    /// 修改最近成功取出的实体；values 必须为 Add 同形同源的输入，可短借。
    /// 未取项/已耗尽返回错误；重复按当前位置严格校验，须先结束当前结果借用。
    /// 返回前立即发布，不改变本次固定原型列表和目标优先顺序。
    pub fn alter<I: SystemBundle<Shape = Add>>(&mut self, values: I) -> Result<(), EcsError> {
        self.iter_alter_inner(values)
    }
}

// 设计已确认：保持无尾界，Alter 整表合并必须另证防重访，不因 QueryTable::Uniform 直接放行。
#[cfg(test)]
mod cursor_lifetime_compile_checks {
    use super::*;
    fn construct<'a, 'run, Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        source: &'a mut QueryAlter<'run, Q, F>,
    ) -> AlterIter<'a, 'run, Q, F> {
        AlterIter {
            source,
            current: None,
            archetype_cursor: 0,
            member_cursor: 0,
        }
    }
    fn reuse<'run, Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        source: &mut QueryAlter<'run, Q, F>,
    ) {
        {
            let cursor = construct(source);
            drop(cursor);
        }
        {
            let cursor = construct(source);
            drop(cursor);
        }
    }
}
// prepare 在已有目标优先原型顺序之上检查候选表顺序及表内变体顺序：非空严格转换的跨目标边要求目标先于源，
// 覆盖所有可能源/目标变体（包括本次尚未存在但转换可创建的目标），不得只看当前匹配样本。
// 同目标的同表迁移必须证明保留当前物理行，且只能修改最近 fetch 成功、游标已越过的实体；
// 该次游标内任意 entity 的 QueryAlter::alter 不能交错，独占借用 source 须封住其他入口。
// 对 Variants 表，采用 iteration_order 在该表上的投影作为表内顺序，不改变直接 variant 映射；
// 表内投影需要穿插其他表、不能证明同表保行、出现聚合环或任何跨目标回流时，整次回退原 iteration_order，重新建立所需原型 fetch，
// 即使通用 Query 使用表路由也须为 Alter 回退保留/派生原型执行描述；不将表条目编号冒充原型编号。
// 表级遍历禁止重复覆盖；不引入 Entity 去重集或成员尾界，严格合法性仍在调用时按真实原型核验。
// 全表 Q/F 覆盖也不代表 AlterPlan 可按表共享：未查询的稀疏组件仍影响完整目标组件集合与保留映射。
