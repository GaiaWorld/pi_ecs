//! 结构变更的组件互转计划；不是事务容器，不持有 World 或部分执行状态。
//! World 直接修改与 Alter 声明化批量修改共用组件差异、OwningPtr 搬移及位置发布能力。
//! World 返回前发布；Alter 在确定的作用域边界按转换分组发布，不是任意命令队列或事务。
//! 两种入口均先检查准备，再移动；执行失败直接 panic，不回滚。
//! @to_design Alter 的 A/D 描述、长期映射缓存、受限权限及作用域提交壳尚待定义；不因注释视为已有能力。
use crate::{archetype::ComponentSource, component::ComponentId, entity::ArchetypeId};

/// 共有组件的存储映射；由已登记原型生成，不复制组件实例。
pub(crate) struct TransferComponent {
    /// 新旧原型都包含的组件。
    pub(crate) component: ComponentId,
    /// 源原型中的唯一存储来源。
    pub(crate) source: ComponentSource,
    /// 目标原型中的唯一存储来源；同一物理列、同一个共享/分区后备实例不移动；跨分区后备需要搬值。
    pub(crate) target: ComponentSource,
}

/// 改原型前生成的静态组件差异；不认领实体、不持有组件值、不提供取消或回滚。
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
// 开始移动后不再返回普通错误：隐藏源行，按映射搬值/增删并修复索引，最后更新位置和发布。
// 内部不变量失败、分配异常或用户析构 panic 直接崩溃，不回滚、不补发退休行或继续使用 World。
// 不以 catch_unwind 后继续模拟作为支持场景；panic 展开期间的内存安全仍是实现必须满足的约束，
// 不得二次析构或读取已移出值；这不意味着承诺完整业务资源清理，也不强制全局 panic=abort。
