//! 点查参数配对芯，不调用其他芯。
use crate::{
    entity::Entity,
    error::EcsError,
    lookup::Lookup,
    query::{QueryData, QueryFilter, ReadOnlyQueryData},
};
pub(crate) fn get_mut_inner<'a, Q: QueryData, F: QueryFilter>(
    _lookup: &'a mut Lookup<'_, Q, F>,
    _entity: Entity,
) -> Result<Q::Item<'a, 'a>, EcsError> {
    todo!("尚未实现受限可变点查")
}
pub(crate) fn get_inner<'a, Q: ReadOnlyQueryData, F: QueryFilter>(
    _lookup: &'a Lookup<'_, Q, F>,
    _entity: Entity,
) -> Result<Q::Item<'a, 'a>, EcsError> {
    todo!("尚未实现已发布实体点查")
}
