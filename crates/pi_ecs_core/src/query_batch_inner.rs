//! 批量查询配对芯，仅失败桩。
use crate::{entity::Entity, query::*, query_batch::*};
pub(crate) fn get_many_inner<'a, Q: ReadOnlyQueryData, F: QueryFilter, const N: usize>(
    _query: &'a Query<'_, Q, F>,
    _entities: [Entity; N],
) -> Result<[Q::Item<'a, 'a>; N], BatchQueryError> {
    todo!("尚未实现")
}
pub(crate) fn get_many_mut_inner<'a, Q: QueryData, F: QueryFilter, const N: usize>(
    _query: &'a mut QueryMut<'_, Q, F>,
    _entities: [Entity; N],
) -> Result<[Q::Item<'a, 'a>; N], BatchQueryError> {
    todo!("尚未实现")
}
pub(crate) fn iter_many_inner<
    'a,
    'w,
    Q: ReadOnlyQueryData,
    F: QueryFilter,
    I: IntoIterator<Item = Entity>,
>(
    _query: &'a Query<'w, Q, F>,
    _entities: I,
) -> QueryMany<'a, 'w, Q, F, I::IntoIter> {
    todo!("尚未实现")
}
pub(crate) fn iter_many_mut_inner<
    'a,
    'w,
    Q: QueryData,
    F: QueryFilter,
    I: IntoIterator<Item = Entity>,
>(
    _query: &'a mut QueryMut<'w, Q, F>,
    _entities: I,
) -> QueryManyMut<'a, 'w, Q, F, I::IntoIter> {
    todo!("尚未实现")
}
pub(crate) fn next_inner<'a, Q: ReadOnlyQueryData, F: QueryFilter, I: Iterator<Item = Entity>>(
    _iter: &mut QueryMany<'a, '_, Q, F, I>,
) -> Option<Result<Q::Item<'a, 'a>, QueryEntityError>> {
    todo!("尚未实现")
}
pub(crate) fn fetch_next_inner<'a, Q: QueryData, F: QueryFilter, I: Iterator<Item = Entity>>(
    _iter: &'a mut QueryManyMut<'_, '_, Q, F, I>,
) -> Option<Result<Q::Item<'a, 'a>, QueryEntityError>> {
    todo!("尚未实现")
}
