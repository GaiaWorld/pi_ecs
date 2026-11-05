//! 稀疏数组映射维护配对芯；设计桩，不提供伪无锁实现。
pub(crate) fn hash_map_inner(
    _component: pi_ecs_types::component::ComponentId,
    _layout: std::alloc::Layout,
) -> Result<crate::sparse_set::SparseSetStorage, pi_ecs_types::error::EcsError> {
    todo!("尚未实现空哈希后端构造")
}
pub(crate) fn compact_inner<const P: usize>(_owners: &mut crate::sparse_set::SparseOwners<P>) {
    todo!("尚未实现模式分派：非 rc 延迟映射整理，rc 空操作")
}
