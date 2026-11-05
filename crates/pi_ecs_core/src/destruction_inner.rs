//! 阶段末销毁配对芯，设计期仅失败桩。
use crate::{destruction::PendingDestruction, entity::Entity, error::EcsError, world::World};
pub(crate) fn enqueue_inner(_pending: &PendingDestruction, _entity: Entity) {
    todo!("@to_design 原子归零发布与待销毁列表")
}
pub(crate) fn flush_inner(_world: &World) -> Result<(), EcsError> {
    todo!("@to_design 静止点授权、确定顺序及级联收束")
}
