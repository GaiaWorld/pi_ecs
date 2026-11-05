//! Alter 配对芯；仅失败桩，不引用其他芯。
pub(crate) fn query_get_inner<'a, Q: crate::query::QueryData, F: crate::query::QueryFilter>(
    _value: &'a mut crate::alter::QueryAlter<'_, Q, F>,
    _entity: Entity,
) -> Result<Q::Item<'a, 'a>, EcsError> {
    todo!("尚未实现查询式隐藏行变更")
}
pub(crate) fn iter_inner<'a, Q: crate::query::QueryData, F: crate::query::QueryFilter>(
    _value: &'a mut crate::alter::QueryAlter<'_, Q, F>,
) -> crate::alter::AlterIter<'a, Q, F> {
    todo!("尚未实现查询式隐藏行变更")
}
pub(crate) fn query_alter_inner<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
    _value: &mut crate::alter::QueryAlter<'_, Q, F>,
    _entity: Entity,
    _values: EntityValues<'_>,
) -> Result<(), EcsError> {
    todo!("尚未实现查询式隐藏行变更")
}
pub(crate) fn iter_next_inner<'a, Q: crate::query::QueryData, F: crate::query::QueryFilter>(
    _value: &'a mut crate::alter::AlterIter<'_, Q, F>,
) -> Option<Q::Item<'a, 'a>> {
    todo!("尚未实现查询式隐藏行变更")
}
pub(crate) fn iter_alter_inner<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
    _value: &mut crate::alter::AlterIter<'_, Q, F>,
    _values: EntityValues<'_>,
) -> Result<(), EcsError> {
    todo!("尚未实现查询式隐藏行变更")
}

use crate::{
    alter::{Alter, AlterState},
    component::ComponentId,
    entity::Entity,
    entity_values::EntityValues,
    error::EcsError,
    world::World,
};
pub(crate) fn new_inner(
    _world: &World,
    _added: &[ComponentId],
    _removed: &[ComponentId],
) -> Result<AlterState, EcsError> {
    todo!("@to_design 注册兼容检查与转换计划准备")
}
pub(crate) fn scope_inner<R>(
    _state: &mut AlterState,
    _world: &World,
    _run: impl for<'a> FnOnce(&mut Alter<'a>) -> R,
) -> Result<R, EcsError> {
    todo!("@to_design 批次受限访问通道和共同搬移壳")
}
pub(crate) fn alter_inner(
    _alter: &mut Alter<'_>,
    _entity: Entity,
    _values: EntityValues<'_>,
) -> Result<(), EcsError> {
    todo!("@to_design 单请求准备与共享转换壳")
}
