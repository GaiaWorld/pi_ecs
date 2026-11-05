//! 迭代配对芯；不实现查询算法，不访问其他芯。
use crate::{
    error::EcsError,
    query::{Query, QueryData, QueryFilter, QueryMut, ReadOnlyQueryData},
    query_iter::QueryIter,
};
pub(crate) fn iter_inner<'a, Q: ReadOnlyQueryData, F: QueryFilter>(
    _query: &'a Query<'_, Q, F>,
) -> Result<QueryIter<'a, Q, F>, EcsError> {
    todo!("@to_design 访问令牌构造及目标唯一性证明")
}
pub(crate) fn iter_mut_inner<'a, Q: QueryData, F: QueryFilter>(
    _query: &'a mut QueryMut<'_, Q, F>,
) -> Result<QueryIter<'a, Q, F>, EcsError> {
    todo!("@to_design 可变存储重借用与 fetch 构造")
}
pub(crate) fn next_inner<'w, Q: QueryData, F: QueryFilter>(
    _iter: &mut QueryIter<'w, Q, F>,
) -> Option<Q::Item<'w, 'w>> {
    todo!("尚未实现")
}
