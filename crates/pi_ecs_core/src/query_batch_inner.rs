//! 批量查询配对芯，仅失败桩。
use crate::{entity::Entity, query::*, query_batch::*};

#[cfg(feature = "rc")]
unsafe fn prepare_batch<'a, Q: QueryData, F: QueryFilter, const N: usize>(
    state: &QueryState<Q, F>,
    access: &crate::query_fetch::FetchAccess<'a>,
    entities: [Entity; N],
) -> Result<[Q::Item<'a, 'a>; N], BatchQueryError> {
    use crate::query_fetch::QueryFetch;
    let mut plans = Vec::with_capacity(N);
    for (index, entity) in entities.into_iter().enumerate() {
        let plan = unsafe { QueryState::<Q, F>::prepare_cached_target(state, access, entity) }
            .map_err(|error| {
                let cause = error
                    .downcast_ref::<QueryEntityContext>()
                    .map(|cause| match cause {
                        QueryEntityContext::NoSuchEntity(entity) => {
                            QueryEntityContext::NoSuchEntity(*entity)
                        }
                        QueryEntityContext::NotMatched(entity) => {
                            QueryEntityContext::NotMatched(*entity)
                        }
                        QueryEntityContext::AliasedEntity(entity) => {
                            QueryEntityContext::AliasedEntity(*entity)
                        }
                    });
                if let Some(cause) = cause {
                    error.attach(BatchQueryContext { index, cause })
                } else {
                    error
                }
            })?;
        plans.push(plan);
    }
    let mut plans = plans.into_iter();
    Ok(std::array::from_fn(|_| {
        let (target, mut fetch) = plans.next().expect("批量准备结果数量一致");
        // SAFETY: each target passed validation; the caller proves read-only or distinct writes.
        unsafe { fetch.fetch_at(&target) }
    }))
}
impl<'w, Q: ReadOnlyQueryData, F: QueryFilter> Query<'w, Q, F> {
    pub(crate) fn get_many_inner<'a, const N: usize>(
        _query: &'a Query<'_, Q, F>,
        _entities: [Entity; N],
    ) -> Result<[Q::Item<'a, 'a>; N], BatchQueryError> {
        #[cfg(feature = "rc")]
        {
            let mut access = _query.access.fetch_access()?;
            access.window = _query.window;
            // SAFETY: read-only data permits duplicate entity results.
            unsafe { prepare_batch(_query.state, &access, _entities) }
        }
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC 批量查询延期",
        ))
    }
}
impl<'w, Q: QueryData, F: QueryFilter> QueryMut<'w, Q, F> {
    pub(crate) fn get_many_mut_inner<'a, const N: usize>(
        _query: &'a mut QueryMut<'_, Q, F>,
        _entities: [Entity; N],
    ) -> Result<[Q::Item<'a, 'a>; N], BatchQueryError> {
        #[cfg(feature = "rc")]
        {
            let mut seen = std::collections::HashSet::new();
            for (index, entity) in _entities.into_iter().enumerate() {
                if !seen.insert(entity) {
                    return Err(crate::error::classified_error(
                        crate::error::ErrorKind::Conflict,
                        "可变批次包含重复实体",
                    )
                    .attach(BatchQueryContext {
                        index,
                        cause: QueryEntityContext::AliasedEntity(entity),
                    }));
                }
            }
            let mut access = _query.access.fetch_access()?;
            access.world = _query.access.world.reborrow();
            access.window = _query.window;
            access.write_tick = Some(_query.write_tick);
            // SAFETY: the exclusive short borrow protects this batch and full identities are unique.
            unsafe { prepare_batch(_query.state, &access, _entities) }
        }
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC 批量查询延期",
        ))
    }
}
impl<'w, Q: ReadOnlyQueryData, F: QueryFilter> Query<'w, Q, F> {
    pub(crate) fn iter_many_inner<'a, I: IntoIterator<Item = Entity>>(
        _query: &'a Query<'w, Q, F>,
        _entities: I,
    ) -> QueryMany<'a, 'w, Q, F, I::IntoIter> {
        QueryMany {
            query: _query,
            entities: _entities.into_iter(),
        }
    }
}
impl<'w, Q: QueryData, F: QueryFilter> QueryMut<'w, Q, F> {
    pub(crate) fn iter_many_mut_inner<'a, I: IntoIterator<Item = Entity>>(
        _query: &'a mut QueryMut<'w, Q, F>,
        _entities: I,
    ) -> QueryManyMut<'a, 'w, Q, F, I::IntoIter> {
        QueryManyMut {
            query: _query,
            entities: _entities.into_iter(),
        }
    }
}
impl<'a, Q: ReadOnlyQueryData, F: QueryFilter, I: Iterator<Item = Entity>>
    QueryMany<'a, '_, Q, F, I>
{
    pub(crate) fn next_inner(
        _iter: &mut QueryMany<'a, '_, Q, F, I>,
    ) -> Option<Result<Q::Item<'a, 'a>, QueryEntityError>> {
        let entity = _iter.entities.next()?;
        Some(_iter.query.get(entity))
    }
}
impl<Q: QueryData, F: QueryFilter, I: Iterator<Item = Entity>> QueryManyMut<'_, '_, Q, F, I> {
    pub(crate) fn fetch_next_inner<'a>(
        _iter: &'a mut QueryManyMut<'_, '_, Q, F, I>,
    ) -> Option<Result<Q::Item<'a, 'a>, QueryEntityError>> {
        let entity = _iter.entities.next()?;
        Some(_iter.query.get_mut(entity))
    }
}
