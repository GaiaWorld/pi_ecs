//! 每个 World 一套布局和分区存储；游戏与渲染使用独立 World。
//! 非 rc 普通添加/删除不搬移其他存活值地址；rc 为单线程模式，容量增长允许搬移旧值。
//! rc 增长前结束受影响存储的引用/迭代器/地址使用，增长后重绑地址缓存；单线程不豁免别名约束。
//! 两种模式均不在普通增删中夹带填洞压缩。
//! 显式迁移、settle/压缩/恢复等搬移操作必须受相应排他授权约束；地址稳定不延长已删除值寿命。
use crate::{sparse_set::SparseSetStorage, table::Table};
use pi_ecs_types::entity::PartitionId;

/// 一个分区的物理数据，布局声明在本 World 内共享。
pub struct PartitionStorage {
    /// 当前 World 的分区编号。
    pub(crate) partition: PartitionId,
    /// 按 table 编号寻址的物理表。
    pub(crate) tables: Box<[Table]>,
    /// 普通组件未被 Table 覆盖时的本分区后备。
    pub(crate) fallback_sparse: Box<[Option<SparseSetStorage>]>,
    /// Table/变体成员，二维存储表示待闭合。
    pub(crate) archetypes: Vec<crate::archetype::ArchetypeStorage>,
}
/// 不包含恢复域；游戏恢复只操作游戏 World，不触碰渲染 World。
pub struct WorldStorage {
    /// 默认一个分区，按需固定配置多个分区。
    pub(crate) partitions: Box<[PartitionStorage]>,
    /// 显式稀疏跨本 World 分区共享。
    pub(crate) shared_sparse: Box<[Option<SparseSetStorage>]>,
}
impl WorldStorage {
    /// partitions 为按分区编号排列且已完整准备的物理分区，shared_sparse 仅显式共享组件有实例。
    /// 各数组使用同一 World 的编号空间；本入口只组装物理所有权，不读取逻辑注册或业务析构能力。
    /// 失败清理未安装的空存储，调用方已有 World 不变；不得传入仍含业务活值的半状态。
    pub fn new(
        partitions: Box<[PartitionStorage]>,
        shared_sparse: Box<[Option<SparseSetStorage>]>,
    ) -> Result<Self, pi_ecs_types::error::EcsError> {
        crate::storage_inner::world_new_inner(partitions, shared_sparse)
    }
}
impl PartitionStorage {
    /// partition 是当前区编号；tables 已按布局编号排列，fallback_sparse 仅普通组件有分区后备。
    /// 输入均是无业务活值的准备态；本入口不选择组件存储策略、不登记逻辑原型，失败不安装半分区。
    pub fn new(
        partition: PartitionId,
        tables: Box<[Table]>,
        fallback_sparse: Box<[Option<SparseSetStorage>]>,
    ) -> Result<Self, pi_ecs_types::error::EcsError> {
        crate::storage_inner::partition_new_inner(partition, tables, fallback_sparse)
    }
    /// id 已由 core 完成逻辑登记，且绑定本区既有 Table；只准备空物理成员列表。
    /// 重复同号请求幂等，容量失败保持已有成员，不推导或修改组件集合。
    pub fn prepare_archetype(
        &mut self,
        id: pi_ecs_types::entity::ArchetypeId,
    ) -> Result<(), pi_ecs_types::error::EcsError> {
        crate::storage_inner::prepare_archetype_inner(self, id)
    }
}
