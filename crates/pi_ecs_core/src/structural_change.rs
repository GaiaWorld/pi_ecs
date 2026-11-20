//! 结构变更的组件互转计划；不是事务容器，不持有 World 或部分执行状态。
//! World 直接修改与 Alter 声明化批量修改共用组件差异、OwningPtr 搬移及位置发布能力。
//! World 与 Alter 均在单次调用返回前发布；源留洞，settle 后续整理。
//! 两种入口均先检查准备，再移动；执行失败直接 panic，不回滚。
//! Alter 的 A/D 首版为 ComponentId 集合；长期映射缓存在 AlterState.cache，受限权限覆盖完整即时转换，
//! 类型化新增值直接写目标，调用内发布，不保存批次或 Drop 发布路径。
use crate::{archetype::ComponentSource, component::ComponentId, entity::ArchetypeId};

/// 共有组件的存储映射；由已登记原型生成，不复制组件实例。
#[derive(Clone)]
pub(crate) struct TransferComponent {
    /// 新旧原型都包含的组件。
    pub(crate) component: ComponentId,
    /// 源原型中的唯一存储来源。
    pub(crate) source: ComponentSource,
    /// 目标原型中的唯一存储来源；同一物理列或同一全局稀疏实例不移动。
    pub(crate) target: ComponentSource,
}

/// 改原型前生成的静态组件差异；不认领实体、不持有组件值、不提供取消或回滚。
#[derive(Clone)]
pub(crate) struct ArchetypeChange {
    /// 已校验的当前完整原型。
    pub(crate) source: ArchetypeId,
    /// 已校验的目标完整原型。
    pub(crate) target: ArchetypeId,
    /// 共有组件及存储互转映射。
    pub(crate) retained: Vec<TransferComponent>,
    /// 仅目标包含，输入必须恰好提供的组件。
    pub(crate) added: Vec<ComponentId>,
    /// 仅源包含，执行时移除并析构的组件。
    pub(crate) removed: Vec<ComponentId>,
}

// 移动前检查冻结、完整身份、区/域、原型、值类型/集合及容量上限，并准备列槽、
// SparseSet 后端容量、原型成员、退休记录及变化记录空间；不得移动源值或公开半实体。
// RC 稀疏路径单源：World::spawn/change_archetype 与 Alter 均调用 WorldStorage::prepare_sparse_tracked。
// 新增按已启用项构造结构 tick；共有值经 Column::remove_raw_with_ticks 同步移出并复制原时间，或先由 WorldStorage::sparse_ticks 复制再移出。
// 目标稀疏经 insert_sparse_tracked 安装值/时间并最后发布 owner；目标列沿 set_ticks 保留同一快照。
// 不从空 SparseView 取 tick 写权限，不以 apply_tracking 代替安装；同一物理来源保留值与 tick 不动。
// 开始移动后不再返回普通错误：隐藏源行，按映射搬值/增删并修复索引，最后更新位置和发布。
// 内部不变量失败、分配异常或用户析构 panic 直接崩溃，不回滚、不补发退休行或继续使用 World。
// 不以 catch_unwind 后继续模拟作为支持场景；panic 展开期间的内存安全仍是实现必须满足的约束，
// 不得二次析构或读取已移出值；这不意味着承诺完整业务资源清理，也不强制全局 panic=abort。
