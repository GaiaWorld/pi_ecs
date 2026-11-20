//! 阶段内系统访问冲突的确定性分组；只计算顺序，不执行或派发任务。
#[path = "stage_plan_inner.rs"]
mod stage_plan_inner;
use pi_ecs_core::access::{OrderingArchetypes, ResolvedAccess};
/// 一个波次：成员之间不存在访问冲突，可按执行器能力并行或串行。
pub struct StageWave {
    /// 本波系统的登记下标，升序且不重复；下标对应 Stage 系统槽位。
    pub systems: Box<[usize]>,
}
/// 阶段执行计划：波次之间严格先后，波内无冲突。
pub struct StagePlan {
    /// 按执行顺序排列的波次；阶段没有系统时为空。
    pub waves: Box<[StageWave]>,
}
/// # 用途
/// 两系统的排序关系，与内存访问排斥分开；顺序参数为登记较早者、登记较晚者。
pub(crate) enum OrderingRelation {
    /// 无排斥也无可见性依赖，不加边。
    Independent,
    /// 早登记者先执行，含写写、双向读写、两个结构写系统或仅物理排斥。
    EarlierFirst,
    /// 后登记者单向写入早登记者读取范围，后者先执行。
    LaterFirst,
}
impl StagePlan {
    /// # 用途
    /// earlier/later 是同一逻辑全集上两系统的全部参数权限，按系统登记先后传入。
    /// 跨系统对 ParamSet 取全部成员权限；按实际重叠范围判方向，不按组件编号全局合并。
    /// 组件、唯一资源、多例同槽/读全体按写先读，两个写者按登记顺序。
    /// 组件值写仅在含该组件的共享逻辑原型上先于值读或 tick 读；纯 tick 读不伪装成值读或通知观察。
    /// 双向依赖沿登记顺序/成环串行回退，不因 Changed 当前为 false 或追踪尚未启用删除边。
    /// Spawn 目标或 Alter 源/目标的成员写先于受影响成员读取；新增组件的值/tick 及通知发布先于对应观察。
    /// 保留值物理搬移只排斥，不当作业务值写；两个结构写系统无论潜在生产消费关系均按登记顺序。
    /// 同一对存在双向写先读要求按登记顺序；纯物理排斥也按登记顺序；无关系返回 Independent。
    /// 通知以组件+种类判同流物理排斥，不同流不因共享目录串行；只发布对应流才形成写先观察方向。
    /// 同流潜在写写按登记顺序，不用值范围不相交或暂时无订阅放行；排序不随追踪开关变化。
    /// 流访问统一调用 OrderingAccess.declaration.visit_notice_access，不在本模块另写发布推导规则。
    /// 不创建原型、不执行系统，不承诺成环时每个读者均能看见本轮所有写入。
    pub(crate) fn relation(
        logical: &OrderingArchetypes,
        earlier: &[pi_ecs_core::access::OrderingAccess],
        later: &[pi_ecs_core::access::OrderingAccess],
    ) -> OrderingRelation {
        Self::relation_inner(logical, earlier, later)
    }
    /// # 用途
    /// world 提供冻结 Table/已发布组合；trees 含本阶段全部位置（含 ParamSet 成员）。
    /// 先将所有 Spawn 完整目标作为种子去重，再从这些种子和已发布组合按全部 Alter 推导至固定点。
    /// 元数据只经 World::table_layouts/archetypes、TableLayout::columns 及 Archetype::components 取得；
    /// 组件编号/来源经 ArchetypeComponent 只读访问，不访问 core 私有字段，不借物理存储。
    /// 保守忽略运行数据条件，遵守结构范围和严格增删，去重；不创建原型/推进版本/污染运行匹配。
    /// 返回潜在逻辑闭包；无目标 Table/非法组合/容量错误返回 Err，不使用半计划，World 保持不变。
    pub(crate) fn ordering_archetypes(
        world: pi_ecs_core::world::WorldRef<'_>,
        trees: &[&pi_ecs_core::access::AccessTree],
    ) -> Result<pi_ecs_core::access::OrderingArchetypes, pi_ecs_core::error::EcsError> {
        Self::ordering_archetypes_inner(world, trees)
    }
    /// # 用途
    /// access 按活动系统登记顺序排列，slots 与其等长且严格递增，保存原 Stage 永久槽号。
    /// Stage 已验证映射唯一有效；内部图使用紧凑索引，输出 waves.systems 必须转换为 slots 中的槽号。
    /// 空活动列表输出空计划；零参数活动系统仍有条目，不以空权限丢弃。
    /// 全部逻辑原型序号来自同一全集；潜在目标参与值/成员/结构冲突。
    /// 对系统对调用 relation 定向；两个结构写系统按登记顺序，不因目标能成为其他 Alter 的源而交换。
    /// 多系统成环则整阶段回退登记串行，允许读者本轮看不到后写者更新；不丢边并发或重复执行。
    /// 只在入口重建，不重复派发或创建原型。
    pub(crate) fn build_ordering(
        logical: &OrderingArchetypes,
        access: &[Vec<pi_ecs_core::access::OrderingAccess>],
        slots: &[usize],
    ) -> Self {
        Self::build_ordering_inner(logical, access, slots)
    }

    /// access 按系统登记下标排列，每项是该系统本次按真实原型展开的权限。
    /// 本公开低层入口仅排序给定的已发布范围，不能用于含未创建 Alter 目标的正式阶段；
    /// 正式 Stage 使用 ordering_archetypes/relation/build_ordering，不通过本入口重建运行中顺序。
    /// 必须按重叠范围判定值/tick、资源及多例写先读方向；物理排斥不冒充业务写方向。
    /// 当前旧实现已暂停，现有验收仅编译，不能把该接口存在称为排序行为可用。
    /// 空权限不表示已注销：无参活动系统仍须执行。存在空槽时由 Stage 按活动槽构造输入并将结果显式映射回稳定槽位。
    /// 同一系统内部视为已校验；本方法只计算系统之间的顺序，不重新校验参数别名。
    /// 冲突用 ResolvedAccess::conflicts 判定：值访问需同组件、含写且原型集合相交；
    /// 首版含结构写的系统与所有其他系统冲突，独占一个运行波次；普通系统按值/成员范围比较。
    /// 有冲突时按写先于读、写者按登记顺序定向；读读不加边。
    /// 加边若会形成环，放弃整张图并回退为逐系统、按登记顺序执行；不跳过冲突边让系统同波并发。
    /// 波次取最长路径层级加一，波内保持登记顺序；不追求最少波数或最大并行度。
    /// 冲突图与排序复用本地 pi_graph 0.6（graph-storage 的 SparseGraph、graph-algo 的
    /// 可达性与 toposort），不自定义图结构或 Kahn；节点为系统登记下标。
    /// 本方法只生成阶段初始计划，不用于执行中对全体系统重新排序；计算无错误分支。
    /// 运行中仅刷新未启动系统匹配；入口含潜在目标的排序使用 build_ordering，下一次入口才重排。
    pub fn build(access: &[Vec<ResolvedAccess>]) -> Self {
        Self::build_inner(access)
    }
}
