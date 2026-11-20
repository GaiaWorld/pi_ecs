//! 访问声明与本次授权；组件值权限和容器内存回收分离。
#[path = "access_inner.rs"]
mod access_inner;
use crate::{component::ComponentId, entity::ArchetypeId, error::EcsError};
/// 单个组件的值访问模式；结构过滤本身不读取组件值。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    /// 多个读者可共存。
    Read,
    /// 与重叠范围内的任何读写排斥。
    Write,
}
/// # 用途
/// 单个通知流的潜在访问，不含实体范围；不是运行令牌或订阅责任。
#[derive(Clone, Copy)]
pub struct NoticeAccess {
    /// 流所属组件，按本 World 冻结编号解释。
    pub component: ComponentId,
    /// Added/Changed/Removed 独立借用单元。
    pub kind: crate::notification::NoticeKind,
    /// Read 为观察，Write 为潜在发布；与实际开关无关。
    pub mode: AccessMode,
}
/// # 用途
/// 通知权限推导的借用输入；静态/展开适配只提供来源，规则统一由 visit 承载。
pub enum NoticeAccessSource<'a> {
    /// 完整固定参数声明，保留尚未命中的潜在写项。
    Declaration(&'a AccessDeclaration),
    /// prepare 的展开权限，values 保留空匹配组件条目，结构增删保留固定声明。
    Resolved(&'a ResolvedAccess),
}
impl NoticeAccessSource<'_> {
    /// # 用途
    /// visit 同步接收流访问，不分配集合、不修改输入、不查询 World；允许重复项，无顺序承诺。
    /// observations 每项产生对应流 Read；组件值 Write 产生 Changed Write；资源/多例不产流访问。
    /// Spawn 目标的每个组件与 Alter added 均产生 Added、Changed Write；Alter removed 产生 Removed Write。
    /// 保留值搬移不产生 Changed，过滤/只读值不产生发布。空匹配不删潜在能力。
    /// 此处仅推导：跨系统同流读写/写写排斥、发布先观察；同系统写令牌可轮流短追加，
    /// 不把它们当作同时存活的 &mut 流；同流 Reader 与潜在写参数仍拒绝。
    pub fn visit(&self, visit: impl FnMut(NoticeAccess)) {
        self.visit_inner(visit)
    }
}
/// 查询实体范围，所有条件取交集；不是已匹配实体的快照，不携带分区维度。
pub struct EntityScope {
    /// 必须存在的组件，不能据此推导读取其值。
    pub with: Box<[ComponentId]>,
    /// 必须不存在的组件；与 with 相交则范围为空。
    pub without: Box<[ComponentId]>,
}
/// 一个参数的值权限和结构权限；普通运行时条件不能用作互斥证明。
pub struct AccessDeclaration {
    /// 唯一资源访问不受实体范围过滤影响。
    pub resources: Box<[(crate::resource::ResourceId, AccessMode)]>,
    /// 多例访问；读全体与全部写槽冲突，不同生产者的写槽兼容。
    pub multi_resources: Box<[(crate::resource::ResourceId, MultiResourceAccess)]>,
    /// 组件编号与访问模式，规范化去重前必须检查查询内部可变别名。
    pub components: Box<[(ComponentId, AccessMode)]>,
    /// 实际读取活组件 tick 的组件集合，与值权限分开；追踪分配需求不等于读取能力。
    /// 与 components 共用 scope，按组件去重；不按当前追踪开关、动态真假或实体数量缩减。
    /// 独立参数中重叠的值 Write 与 tick 读冲突；同一 Q/F 的过滤先读后写在查询内部证明。
    pub tick_reads: Box<[ComponentId]>,
    /// 读取实体成员或定位身份；即使不读取值或 tick（如 Query<Entity>/Query<()>），也须声明。
    pub reads_entities: bool,
    /// 实体访问范围，点查与迭代均不得绕过。
    pub scope: EntityScope,
    /// 结构变更声明；源和目标的权限均需覆盖。
    pub structure: StructuralAccess,
    /// 通知观察对应组件/种类；必须与对应范围写入及结构变更排斥，不只是队列读。
    pub observations: Box<[(ComponentId, crate::notification::NoticeKind)]>,
}
/// # 用途
/// 完整参数声明树；保留位置/互斥边界，不由展开权限反推，不扁平合并范围。
pub enum AccessTree {
    /// 单参数完整声明，包括资源、范围、结构与观察权限。
    Leaf(AccessDeclaration),
    /// 同时可用的参数，按位置保存并逐对校验别名。
    Together(Vec<AccessTree>),
    /// 分时参数，成员内部校验，对外占全部成员权限；取值必须短借父授权。
    Exclusive(Vec<AccessTree>),
}
/// 多例参数的声明粒度，不把不同生产者误判成同一可变槽。
pub enum MultiResourceAccess {
    /// 读取本类型的全部紧凑实例。
    ReadAll,
    /// 写本参数唯一拥有的生产者槽。
    WriteSlot {
        /// 核心维护登记得到的稳定槽身份，不是物理偏移。
        producer: u64,
    },
}
/// 结构操作能力，不因并发容器存在就自动允许并发搬行。
pub enum StructuralAccess {
    /// 不搬移、不增删实体或组件。
    None,
    /// 固定创建声明；没有源，只写完整目标的列、成员和身份位置。
    Spawn {
        /// 完整目标组件编号，升序去重；空组合仍会新增实体成员。
        components: Box<[ComponentId]>,
        /// 已登记的固定分区，同时决定出生域和初始位置；排序仍沿现有无分区访问模型。
        partition: crate::entity::PartitionId,
    },
    /// 固定增删声明；执行前按匹配源/目标展开列、成员与身份位置访问。
    Alter {
        /// 严格新增组件。
        added: Box<[ComponentId]>,
        /// 严格删除组件。
        removed: Box<[ComponentId]>,
    },
}
impl AccessDeclaration {
    /// # 用途
    /// visit 接收本参数全部潜在流访问；规则单源于 NoticeAccessSource::visit，不读订阅或 tick 开关。
    /// 不按 scope 或当前匹配缩减流权限，不分配集合；重复项允许，调用方不能按次数解释责任。
    pub fn visit_notice_access(&self, visit: impl FnMut(NoticeAccess)) {
        self.visit_notice_access_inner(visit)
    }
    /// other 为同一注册配置下另一参数的声明；返回是否存在潜在冲突，不改变任一声明。
    /// 读读不冲突；写与读/写仅在分区或 With/Without 能证明互斥时放行。
    /// 结构访问还须展开实际源/目标计划；未展开时保守返回冲突，不能用源范围单独放行。
    /// 用于同系统参数组合等静态前置检查；跨系统并发性以 ResolvedAccess::conflicts 为准。
    /// 独立参数的 tick_reads 与同组件值 Write 也按静态范围排斥；内部 Q/F 不拆成两个独立参数再误判。
    /// 本方法不展开实际原型，也不产生执行顺序。
    /// 同系统 Lookup 不读取成员列表；可变 Lookup 按 Q 声明全部潜在值写范围，仍拒绝重叠值别名。
    /// RC 即时 Spawn/Alter 与任何独立组件 Query/Lookup、Reader 或其他结构写 首版保守冲突；资源/Local 可独立组合。
    /// 不用不同原型证明兼容：它们可能共享 Table，扩容会使旧组件引用失效。
    /// ParamSet 可轮流获取冲突参数，每次切换释放全部结果及物理 fetch 缓存，再从逻辑编号重新绑定。
    /// RC 节点池采用每方法内部短借 RefCell，容器对象不持整个池独占引用，不因同池就拒绝不同头。
    /// 组件头仍按值权限检查：相同头重叠读写拒绝，不同头/不同用途类型及单查询不同实体可同时返回。
    /// 通知以 (ComponentId, NoticeKind) 分流授权；只读目录不独占管理器，其他流追加可共存。
    /// 同流读写拒绝；同流写写在同系统可轮流短追加，不持流写 guard 跨用户调用，值别名仍独立拒绝。
    /// 跨系统同流写写排斥，同流独立订阅读读兼容；观察与对应组件值写的业务冲突另行检查。
    pub fn conflicts(&self, other: &Self) -> bool {
        Self::conflicts_inner(self, other)
    }
}
/// 阶段准备时按真实原型展开的组件值访问；只在本次阶段有效。
pub struct ResolvedValueAccess {
    /// 被访问的组件。
    pub component: ComponentId,
    /// 读写模式。
    pub mode: AccessMode,
    /// 可能命中的原型位置；升序去重，不含未匹配原型。
    pub archetypes: Box<[ArchetypeId]>,
}
/// 阶段准备时展开的结构写范围。
pub struct ResolvedStructureAccess {
    /// 结构修改前的源原型位置。
    pub sources: Box<[ArchetypeId]>,
    /// 本次可能发布的目标原型位置；同 Table 路径可为同号。
    pub targets: Box<[ArchetypeId]>,
    /// 新发布的组件；Spawn 为完整目标，Alter 为严格新增。
    pub added: Box<[ComponentId]>,
    /// 严格删除组件。
    pub removed: Box<[ComponentId]>,
}
/// 一个系统本次已展开的权限；跨系统比较不再依赖静态 scope 猜测。
pub struct ResolvedAccess {
    /// 唯一资源权限，沿用参数声明，不受实体范围过滤。
    pub resources: Box<[(crate::resource::ResourceId, AccessMode)]>,
    /// 多例权限，沿用参数声明。
    pub multi_resources: Box<[(crate::resource::ResourceId, MultiResourceAccess)]>,
    /// 通知观察，沿用参数声明。
    pub observations: Box<[(ComponentId, crate::notification::NoticeKind)]>,
    /// 组件值访问展开结果；空表示不读取组件值。
    pub values: Box<[ResolvedValueAccess]>,
    /// 活 tick 读取的真实原型范围；mode 固定 Read，不能与 values 合并后丢失访问种类。
    /// 不产生值读取或通知发布权限，Table::Uniform 须展开到实际覆盖范围而非只取基础 variant。
    pub tick_reads: Box<[ResolvedValueAccess]>,
    /// 仅遍历或定位成员、不读值的原型范围；结构写与之重叠即冲突。
    pub entities: Box<[ArchetypeId]>,
    /// 结构写；None 表示不搬移或增删。
    pub structure: Option<ResolvedStructureAccess>,
}
impl ResolvedAccess {
    /// # 用途
    /// visit 接收与静态声明同规则的潜在流访问，不因匹配原型为空而删除已声明组件写的能力。
    /// values 必须保留每个声明组件条目（允许 archetypes 为空），不凭当下命中猜测未来通知开关。
    pub fn visit_notice_access(&self, visit: impl FnMut(NoticeAccess)) {
        self.visit_notice_access_inner(visit)
    }
    /// other 为同一次阶段准备的另一系统权限；返回是否不能并发。
    /// 值访问：同组件、至少一方写、且分区+原型集合相交才冲突；实体成员访问之间不冲突。
    /// 首版任一方有结构写即视为跨系统冲突，结构系统独占运行；不因原型不相交而放行。
    /// 观察与对应组件的写冲突；无结构写时资源/多例及普通值/成员沿用范围规则。
    /// 通知物理访问按组件+种类展开；潜在同流写写排斥，即使值原型不相交，读读兼容、不同流独立。
    /// 潜在 Changed 发布由组件值写声明保守导出，不依赖订阅/tick 开关；结构发布导出 Added/Changed/Removed。
    /// 同组件值 Write 与 tick_reads 在原型范围重叠时排斥，两方向均检查；tick 读读兼容。
    /// 不按追踪开关或当前 tick 真假跳过；纯 tick 读不得经 visit_notice_access 推导为值写/通知发布。
    /// 本方法只给出可并发性，不产生执行顺序或错误。
    pub fn conflicts(&self, other: &Self) -> bool {
        Self::resolved_conflicts_inner(self, other)
    }
}
/// 本次运行的受限权限描述；不能 Clone，不能从裸共享 World 安全制造。
/// 不以对象唯一性证明值互斥：派生描述仍受同一根会话与 unsafe 取参协议约束。
/// 来源声明/World 和会话授权必须覆盖全部引用、迭代结果及延迟提交；临时描述可按值消费。
pub struct RunAccess<'w> {
    /// 统一底层句柄；声明证明具体权限，不持整个 World 引用，不授予任意可变访问。
    pub(crate) world: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
    /// 已校验的参数权限，初始化声明及阶段版本须一致。
    pub(crate) declaration: &'w AccessTree,
    /// 授权所基于的已同步原型版本；不是实体位置版本。
    pub(crate) archetype_version: u64,
}
impl<'w> RunAccess<'w> {
    /// # 用途
    /// ParamSet 独占成员短借内同步同源查询状态；state 未注销且类型声明与本叶子完全一致。
    /// 只经字段级句柄读取注册表与稳定存储根，增量补扫新变体；布局替换则整体重建。
    /// 不登记需求/扩大权限/读取业务值；成功返回同一叶子声明、World 及寿命，结构版本更新为当前值。
    /// 错误不交付新授权、不推进失败扫描记录；不回滚已发布 Alter，不允许继续使用旧物理视图。
    /// # Safety
    /// 调用方排除本成员及兄弟全部活结果和缓存视图，保持根会话及固定静态权限，不恢复 World 引用。
    pub unsafe fn refresh_query<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        self,
        state: &mut crate::query::QueryState<Q, F>,
    ) -> Result<Self, EcsError> {
        self.refresh_query_inner(state)
    }
    /// # 用途
    /// 按 refresh_query 同步 Alter 的源匹配及转换计划，固定增删声明不变；返回更新结构版本的原授权。
    /// 以全部当前已发布 Q/F 匹配原型重建 iteration_order，按固定转换目标优先排序，
    /// 重新证明并生成 table_order（无法合并则 None）；两者与查询计划一起准备成功后交付成员。
    /// 本次成员借用/游标内冻结，期间创建的新目标留待下次重绑；不重排系统或改变静态权限。
    /// 不创建新原型或延迟发布请求；错误不交付 Item，不扩权或分配 tick。
    /// # Safety
    /// 满足 refresh_query 的无活结果/视图及原根会话前提，且本叶子覆盖完整 Alter 源/目标结构写。
    pub unsafe fn refresh_alter<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        self,
        state: &mut crate::alter::QueryAlterState<Q, F>,
    ) -> Result<Self, EcsError> {
        self.refresh_alter_inner(state)
    }
    /// world/declaration/version 必须来自同一次已授权的阶段执行，不改业务值。
    /// # Safety
    /// 调用方证明唯一 World 运行会话内的同系统参数与并行任务无冲突，类型可在当前线程访问，
    /// 权限对应当前系统启动前的原型快照；本令牌和全部派生引用失效前，不撤销授权或执行冲突结构修改。
    /// 非 rc 调用者持布局授权：含 Alter 系统为独占写，其余为共享读，授权保活到所有派生引用结束。
    /// 同系统参数复用授权而不重入布局锁；结构写能力不豁免同系统组件别名校验。
    /// 不同阶段按序完全收束；不支持同一 World 的多个 Schedule 重叠运行，安全入口须验证此限制。
    pub unsafe fn from_executor(
        world: crate::world::WorldRef<'w>,
        declaration: &'w AccessTree,
        archetype_version: u64,
    ) -> Self {
        Self::from_executor_inner(world, declaration, archetype_version)
    }
    /// # 用途
    /// 按 position 取得直接子参数的短授权，范围只能缩小，不构造业务引用。
    /// Together 子项共享短借用；Exclusive 子项必须经 exclusive_child 的独占短借用。
    /// 越界/节点种类不符返回错误，不回退为父节点权限。
    pub fn child(&self, position: usize) -> Result<RunAccess<'_>, EcsError> {
        self.child_inner(position)
    }
    /// # 用途
    /// 框架取参时按 position 派生按值子描述；结果仅借原声明/World，不借临时父对象。
    /// 越界/叶子返回 Err；不构造业务引用，不扩大权限，父对象仍须由根会话保活。
    /// # Safety
    /// 调用者只在框架组合取参使用；Exclusive 兄弟可同时存储但不得同时 fetch，选择必须有 ParamSet 短借证明。
    /// 不得利用父节点或重复子节点制造额外可变权限，全部上下文限制在原授权期。
    pub unsafe fn context_child(&self, position: usize) -> Result<RunAccess<'w>, EcsError> {
        self.context_child_inner(position)
    }
    /// 生成按值 fetch 访问，引用原声明/World 而非本临时描述；调用者经 unsafe fetch 协议排除冲突引用。
    /// 仅 Leaf 可生成 FetchAccess，组合节点返回错误；不能把整个系统权限并集赋给一个参数。
    /// 重复生成描述不等于允许重复取可变值；实际绑定/取值入口负责保持同一位置的独占性。
    pub fn fetch_access(&self) -> Result<crate::query_fetch::FetchAccess<'w>, EcsError> {
        Self::fetch_access_inner(self)
    }
    /// # 用途
    /// 返回绑定原运行授权的按值统一句柄，不借住临时描述、不返回 &World；使用其 unsafe 方法仍须证明本参数声明与别名约束。
    pub fn world_cell(&self) -> crate::unsafe_world_cell::UnsafeWorldCell<'w> {
        self.world_cell_inner()
    }
}
// RC 唯一运行会话及阶段授权由 World::schedule_scope/system_scope 声明；上述 unsafe 入口只是跨包证明边界，
// 不能当作安全调度实现。不得给 World/OwningPtr/条目手写 unsafe Send/Sync 来绕过类型能力登记。

/// # 用途
/// 排序用逻辑原型全集，含已发布组合与 Spawn 种子，以及本阶段全部 Alter 转换的保守闭包；不创建 variant。
pub struct OrderingArchetypes {
    /// 规范化去重的完整组件组合，每组合有冻结时已登记 Table；逻辑序号不是真实 ArchetypeId。
    pub components: Box<[Box<[ComponentId]>]>,
}
/// # 用途
/// 相对同一逻辑全集展开的排序权限，不用于运行授权或 Query 匹配缓存。
pub struct OrderingAccess {
    /// 完整声明，保留资源、多例、范围、tick 读取、通知观察和结构权限。
    /// tick_reads 与组件值权限共用 sources 逻辑范围，不依赖当前创建的 variant 或订阅开关。
    pub declaration: AccessDeclaration,
    /// 可能命中的逻辑原型序号，含未创建组合，排序去重。
    pub sources: Box<[usize]>,
    /// 实际读取成员列表或定位身份的逻辑原型序号；不能从 components 是否非空推断。
    /// 纯资源参数为空；包含仅遍历实体、Option 全不命中但仍遍历成员的查询。
    pub entities: Box<[usize]>,
    /// Spawn 或 Alter 的逻辑目标序号；Spawn 只写目标，sources 和 entities 均为空。
    pub targets: Box<[usize]>,
}

#[cfg(feature = "rc")]
impl<'w> RunAccess<'w> {
    /// # 用途
    /// 消费当前参数叶子的 self，state 为同源已准备且未注销状态，window 为本系统固定窗口。
    /// 先检查 World 身份、准备有效和完整 Q/F 值/tick 权限覆盖；Alter 还须覆盖完整源/目标结构写。
    /// 错误不交付 Item、不新增登记或刷新匹配，不分配 tick/门禁；成功保留原叶子描述及按值窗口。
    /// 结果引用原声明/World 和 state，不借用临时上下文；写标记只使用 window.this_run。
    /// # Safety
    /// 调用方是已校验 SystemParam::fetch；原根会话覆盖全部 Item/结果且排除冲突参数。
    /// 同一位置仅兑现一次，ParamSet 必须先缩短 State/授权寿命，不能同时使用父权限及派生权限。
    pub unsafe fn bind_query<Q: crate::query::ReadOnlyQueryData, F: crate::query::QueryFilter>(
        self,
        state: &'w mut crate::query::QueryState<Q, F>,
        window: crate::change_detection::ChangeWindow,
    ) -> Result<crate::query::Query<'w, Q, F>, EcsError> {
        self.bind_query_inner(state, window)
    }
    /// # 用途
    /// 消费当前参数叶子的 self，state 为同源已准备且未注销状态，window 为本系统固定窗口。
    /// 先检查 World 身份、准备有效和完整 Q/F 值/tick 权限覆盖；Alter 还须覆盖完整源/目标结构写。
    /// 错误不交付 Item、不新增登记或刷新匹配，不分配 tick/门禁；成功保留原叶子描述及按值窗口。
    /// 结果引用原声明/World 和 state，不借用临时上下文；写标记只使用 window.this_run。
    /// # Safety
    /// 调用方是已校验 SystemParam::fetch；原根会话覆盖全部 Item/结果且排除冲突参数。
    /// 同一位置仅兑现一次，ParamSet 必须先缩短 State/授权寿命，不能同时使用父权限及派生权限。
    pub unsafe fn bind_query_mut<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        self,
        state: &'w mut crate::query::QueryState<Q, F>,
        window: crate::change_detection::ChangeWindow,
    ) -> Result<crate::query::QueryMut<'w, Q, F>, EcsError> {
        self.bind_query_mut_inner(state, window)
    }
    /// # 用途
    /// 消费当前参数叶子的 self，state 为同源已准备且未注销状态，window 为本系统固定窗口。
    /// 先检查 World 身份、准备有效和完整 Q/F 值/tick 权限覆盖；Alter 还须覆盖完整源/目标结构写。
    /// 错误不交付 Item、不新增登记或刷新匹配，不分配 tick/门禁；成功保留原叶子描述及按值窗口。
    /// 结果引用原声明/World 和 state，不借用临时上下文；写标记只使用 window.this_run。
    /// # Safety
    /// 调用方是已校验 SystemParam::fetch；原根会话覆盖全部 Item/结果且排除冲突参数。
    /// 同一位置仅兑现一次，ParamSet 必须先缩短 State/授权寿命，不能同时使用父权限及派生权限。
    pub unsafe fn bind_lookup<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        self,
        state: &'w mut crate::query::QueryState<Q, F>,
        window: crate::change_detection::ChangeWindow,
    ) -> Result<crate::lookup::Lookup<'w, Q, F>, EcsError> {
        self.bind_lookup_inner(state, window)
    }
    /// # 用途
    /// 消费当前参数叶子的 self，state 为同源已准备且未注销状态，window 为本系统固定窗口。
    /// 先检查 World 身份、准备有效和完整 Q/F 值/tick 权限覆盖；Alter 还须覆盖完整源/目标结构写。
    /// 错误不交付 Item、不新增登记或刷新匹配，不分配 tick/门禁；成功保留原叶子描述及按值窗口。
    /// 结果引用原声明/World 和 state，不借用临时上下文；写标记只使用 window.this_run。
    /// # Safety
    /// 调用方是已校验 SystemParam::fetch；原根会话覆盖全部 Item/结果且排除冲突参数。
    /// 同一位置仅兑现一次；Add/Remove 必须为登记 state 时采用的静态清单类型。
    /// ParamSet 必须先缩短 State/授权寿命，不能同时使用父权限及派生权限。
    pub unsafe fn bind_alter<
        Q: crate::query::QueryData,
        F: crate::query::QueryFilter,
        Add: crate::bundle::SystemBundle<Shape = Add> + 'static,
        Remove: crate::bundle::Bundle,
    >(
        self,
        state: &'w mut crate::alter::QueryAlterState<Q, F>,
        window: crate::change_detection::ChangeWindow,
    ) -> Result<crate::alter::QueryAlter<'w, Q, F, Add, Remove>, EcsError> {
        self.bind_alter_inner(state, window)
    }

    /// # 用途
    /// 消费本位置的结构写叶子；state 必须同源且完整目标/固定分区与叶子一致，tick 为本系统 this_run。
    /// 成功返回当前授权内的 Spawn，失败不交付参数、不触碰实体或分配 tick。
    /// # Safety
    /// 调用方保证当前系统及并行任务无冲突，覆盖派生句柄的完整寿命；同一参数位置不得重复兑现。
    pub unsafe fn bind_spawn<B: crate::bundle::SystemBundle<Shape = B> + 'static>(
        self,
        state: &'w crate::spawn::SpawnState<B>,
        tick: crate::change_detection::ChangeTick,
    ) -> Result<crate::spawn::Spawn<'w, B>, EcsError> {
        self.bind_spawn_inner(state, tick)
    }
}
