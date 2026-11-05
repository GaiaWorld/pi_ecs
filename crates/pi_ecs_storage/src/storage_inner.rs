//! 物理所有权组装配对芯；不依赖逻辑注册表或其他配对芯。
use crate::{
    sparse_set::SparseSetStorage,
    storage::{PartitionStorage, WorldStorage},
    table::Table,
};
use pi_ecs_types::{
    entity::{ArchetypeId, PartitionId},
    error::EcsError,
};
pub(crate) fn world_new_inner(
    _partitions: Box<[PartitionStorage]>,
    _shared_sparse: Box<[Option<SparseSetStorage>]>,
) -> Result<WorldStorage, EcsError> {
    todo!("尚未实现空物理存储组装")
}
pub(crate) fn partition_new_inner(
    _partition: PartitionId,
    _tables: Box<[Table]>,
    _fallback_sparse: Box<[Option<SparseSetStorage>]>,
) -> Result<PartitionStorage, EcsError> {
    todo!("尚未实现空物理分区构造")
}
pub(crate) fn prepare_archetype_inner(
    _partition: &mut PartitionStorage,
    _id: ArchetypeId,
) -> Result<(), EcsError> {
    todo!("尚未实现空原型成员准备")
}
