//! 登记配对芯；显式失败桩，不实现新业务。
use crate::{
    archetype::Archetype,
    component::{ComponentId, ComponentInfo, StoragePolicy},
    entity::ArchetypeId,
    error::EcsError,
    registry::TypeRegistry,
    table::TableLayout,
};
pub(crate) fn new_inner() -> TypeRegistry {
    todo!("尚未实现")
}
pub(crate) fn register_component_inner<T: crate::component::Component>(
    _registry: &mut TypeRegistry,
    _storage: StoragePolicy,
) -> Result<ComponentId, EcsError> {
    todo!("尚未实现")
}
pub(crate) fn register_table_inner(
    _registry: &mut TypeRegistry,
    _components: &[ComponentId],
) -> Result<ArchetypeId, EcsError> {
    todo!("尚未实现")
}
pub(crate) fn freeze_inner(
    _registry: &mut TypeRegistry,
    _partition_count: usize,
) -> Result<(), EcsError> {
    todo!("@to_design 存储初始化与冻结提交协调")
}
pub(crate) fn register_archetype_inner(
    _registry: &mut TypeRegistry,
    _components: &[ComponentId],
    _partition_count: usize,
) -> Result<ArchetypeId, EcsError> {
    todo!("尚未实现")
}
pub(crate) fn component_inner(
    _registry: &TypeRegistry,
    _id: ComponentId,
) -> Option<&ComponentInfo> {
    todo!("尚未实现")
}
pub(crate) fn table_inner(_registry: &TypeRegistry, _id: ArchetypeId) -> Option<&TableLayout> {
    todo!("尚未实现")
}
pub(crate) fn archetype_inner(_registry: &TypeRegistry, _id: ArchetypeId) -> Option<&Archetype> {
    todo!("尚未实现")
}
