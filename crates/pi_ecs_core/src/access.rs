//! 访问声明与本次授权；组件值权限和容器内存回收分离。
use crate::{component::ComponentId, entity::PartitionId, error::EcsError, world::World};
/// 单个组件的值访问模式；结构过滤本身不读取组件值。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    /// 多个读者可共存。
    Read,
    /// 与重叠范围内的任何读写排斥。
    Write,
}
/// 查询实体范围，所有条件取交集；不是已匹配实体的快照。
pub struct EntityScope {
    /// None 为全部分区，Some 为空则不访问实体。
    pub partitions: Option<Box<[PartitionId]>>,
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
    /// 值访问实体范围，点查与迭代均不得绕过。
    pub scope: EntityScope,
    /// 结构变更声明；源和目标的权限均需覆盖。
    pub structure: StructuralAccess,
    /// 通知观察对应组件/种类；必须与对应范围写入及结构变更排斥，不只是队列读。
    pub observations: Box<[(ComponentId, crate::notification::NoticeKind)]>,
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
    /// 固定增删声明；执行前按匹配源/目标展开列、成员与身份位置访问。
    Alter {
        /// 严格新增组件。
        added: Box<[ComponentId]>,
        /// 严格删除组件。
        removed: Box<[ComponentId]>,
    },
}
impl AccessDeclaration {
    /// other 为同一注册配置下另一参数的声明；返回是否存在潜在冲突，不改变任一声明。
    /// 读读不冲突；写与读/写仅在分区或 With/Without 能证明互斥时放行。
    /// 结构访问还须展开实际源/目标计划；未展开时保守返回冲突，不能用源范围单独放行。
    /// 同系统冲突拒绝参数组合，不同系统冲突由执行器串行/排序处理。
    /// 同系统 Lookup 不读取成员列表；可变 Lookup 按 Q 声明全部潜在值写范围，仍拒绝重叠值别名。
    /// 若仅与目标隐藏行追加重叠，可按稳定地址/未发布隔离证明兼容；提交前全部结果必须结束。
    /// 此兼容不是通用结构写豁免，跨系统源/目标结构写首版仍保守排斥。
    pub fn conflicts(&self, other: &Self) -> bool {
        crate::access_inner::conflicts_inner(self, other)
    }
}
/// 本次运行的权限凭据；不能 Clone，不能从裸共享 World 安全制造。
/// 令牌生命周期必须覆盖其产生的全部引用、迭代结果及延迟提交，不允许提前释放。
pub struct RunAccess<'w> {
    /// 所属 World，不公开任意可变访问。
    pub(crate) world: &'w World,
    /// 已校验的参数权限，初始化声明及阶段版本须一致。
    pub(crate) declaration: &'w AccessDeclaration,
    /// 授权所基于的已同步原型版本；不是实体位置版本。
    pub(crate) archetype_version: u64,
}
impl<'w> RunAccess<'w> {
    /// world/declaration/version 必须来自同一次已授权的阶段执行，不改业务值。
    /// # Safety
    /// 调用方证明唯一 World 运行会话内的同系统参数与并行任务无冲突，类型可在当前线程访问，
    /// 权限对应当前原型快照；本令牌和全部派生引用失效前，不撤销授权或执行冲突结构修改。
    /// 不同阶段按序完全收束；不支持同一 World 的多个 Schedule 重叠运行，安全入口须验证此限制。
    pub unsafe fn from_executor(
        world: &'w World,
        declaration: &'w AccessDeclaration,
        archetype_version: u64,
    ) -> Self {
        crate::access_inner::from_executor_inner(world, declaration, archetype_version)
    }
    /// 借用 self 生成只覆盖本次权限的 fetch 访问；返回寿命不超过本次令牌借用。
    /// 本入口不扩大权限，也不取得新的写权限或放开结构修改。
    pub fn fetch_access(&self) -> Result<crate::query_fetch::FetchAccess<'_>, EcsError> {
        crate::access_inner::fetch_access_inner(self)
    }
}
// @to_design 唯一 World 运行会话及阶段内授权生命周期尚未闭合；上述 unsafe 入口只是跨包证明边界，
// 不能当作安全调度实现。不得给 World/OwningPtr/条目手写 unsafe Send/Sync 来绕过类型能力登记。
