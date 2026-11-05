//! 固定身份映射配对芯；尚未实现。
use crate::{
    entity::{AllocationDomain, Entity, EntityLocation},
    entity_store::EntityStore,
    error::EcsError,
};
pub(crate) fn allocate_inner(
    _store: &mut EntityStore,
    _domain: AllocationDomain,
    _location: EntityLocation,
) -> Result<Entity, EcsError> {
    todo!("@to_design 分配来源编码与分配失败协议")
}
pub(crate) fn locate_inner(_store: &EntityStore, _entity: Entity) -> Option<EntityLocation> {
    todo!("尚未实现")
}
pub(crate) fn relocate_inner(
    _store: &mut EntityStore,
    _entity: Entity,
    _location: EntityLocation,
) -> Result<(), EcsError> {
    todo!("尚未实现")
}
pub(crate) fn remove_inner(_store: &mut EntityStore, _entity: Entity) -> Option<EntityLocation> {
    todo!("尚未实现")
}
