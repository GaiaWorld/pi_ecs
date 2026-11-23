//! 查询迭代：每个缓存范围只绑定一次 fetch，逐行验证发布位置。
use crate::{
    error::EcsError,
    query::{Query, QueryData, QueryFilter, QueryMut, ReadOnlyQueryData},
    query_iter::QueryIter,
};
impl<Q: ReadOnlyQueryData, F: QueryFilter> Query<'_, Q, F> {
    pub(crate) fn iter_inner<'a>(query: &'a Self) -> Result<QueryIter<'a, Q, F>, EcsError> {
        let mut access = query.access.fetch_access()?;
        access.window = query.window;
        Ok(QueryIter::new_bound(query.state, access))
    }
}
impl<Q: QueryData, F: QueryFilter> QueryMut<'_, Q, F> {
    pub(crate) fn iter_mut_inner<'a>(query: &'a mut Self) -> Result<QueryIter<'a, Q, F>, EcsError> {
        let mut access = query.access.fetch_access()?;
        access.world = query.access.world.reborrow();
        access.window = query.window;
        access.write_tick = Some(query.write_tick);
        Ok(QueryIter::new_bound(query.state, access))
    }
}
impl<'w, Q: QueryData, F: QueryFilter> QueryIter<'w, Q, F> {
    fn new_bound(
        state: &'w crate::query::QueryState<Q, F>,
        access: crate::query_fetch::FetchAccess<'w>,
    ) -> Self {
        Self {
            access,
            state,
            table_cursor: 0,
            variant_cursor: 0,
            member_cursor: 0,
            current: None,
        }
    }

    pub(crate) fn next_inner(iter: &mut Self) -> Option<Q::Item<'w, 'w>> {
        #[cfg(feature = "rc")]
        {
            use crate::{
                entity::ArchetypeId,
                query::QueryTable,
                query_fetch::{FetchEntity, FetchScope, FilterFetch, PreparedFetch, QueryFetch},
            };
            let world = iter.access.world.pointer.as_ptr();
            // SAFETY: only read identity/member metadata under the original authorization.
            let (storage, entities) = unsafe {
                (
                    (&*std::ptr::addr_of!((*world).storage))
                        .as_ref()
                        .expect("查询存储缺失"),
                    &*std::ptr::addr_of!((*world).entities),
                )
            };
            loop {
                let entry = iter.state.cache.tables.get(iter.table_cursor)?;
                let table = crate::entity::TableId(
                    u16::try_from(usize::from(iter.state.cache.first_table) + iter.table_cursor)
                        .expect("查询 Table 编号越界"),
                );
                let (scope, plan, len) = match entry {
                    QueryTable::Uniform { fetch } => (
                        FetchScope::Table { table },
                        fetch,
                        storage.published_rows(table).expect("查询 Table 缺失"),
                    ),
                    QueryTable::Variants { entries, .. } if iter.variant_cursor < entries.len() => {
                        let (variant, fetch) = &entries[iter.variant_cursor];
                        let archetype = ArchetypeId {
                            table,
                            variant: *variant,
                        };
                        (
                            FetchScope::Archetype { archetype },
                            fetch,
                            storage.member_len(archetype).expect("查询原型缺失"),
                        )
                    }
                    _ => {
                        iter.table_cursor += 1;
                        iter.variant_cursor = 0;
                        iter.member_cursor = 0;
                        iter.current = None;
                        continue;
                    }
                };
                if iter.member_cursor >= len as usize {
                    if matches!(scope, FetchScope::Table { .. }) {
                        iter.table_cursor += 1;
                    } else {
                        iter.variant_cursor += 1;
                    }
                    iter.member_cursor = 0;
                    iter.current = None;
                    continue;
                }
                if iter.current.is_none() {
                    // SAFETY: these typed plans and the leaf authorization came from the same frozen cache.
                    iter.current = Some(unsafe {
                        PreparedFetch {
                            data: Q::Fetch::prepare(
                                &iter.access,
                                scope,
                                &iter.state.data_state,
                                &plan.data,
                            )
                            .expect("查询数据绑定失效"),
                            filter: F::prepare(
                                &iter.access,
                                scope,
                                &iter.state.filter_state,
                                &plan.filter,
                            )
                            .expect("查询过滤绑定失效"),
                        }
                    });
                }
                let row = iter.member_cursor as u32;
                iter.member_cursor += 1;
                let (entity, table_row) = match scope {
                    FetchScope::Table { .. } => {
                        let Some(entity) =
                            storage.row_entity(table, crate::table::TableRow::new(row))
                        else {
                            continue;
                        };
                        (entity, row)
                    }
                    FetchScope::Archetype { archetype } => {
                        let Some(member) = storage.member(archetype, row) else {
                            continue;
                        };
                        (member.entity, member.table_row.index())
                    }
                };
                let location = entities
                    .borrow()
                    .locate(entity)
                    .expect("发布行的实体位置缺失");
                assert_eq!(location.archetype().table, table);
                assert_eq!(location.table_row(), table_row);
                if let FetchScope::Archetype { archetype } = scope {
                    assert_eq!(location.archetype(), archetype);
                    assert_eq!(location.archetype_row().index(), row);
                }
                let fetch = iter.current.as_mut().unwrap();
                let target = FetchEntity { entity, location };
                // SAFETY: each live row belongs to exactly one plan range and is visited once;
                // filter reads complete before the data fetch can create a mutable result.
                if unsafe { fetch.filter.matches(&target) } {
                    return Some(unsafe { fetch.data.fetch_at(&target) });
                }
            }
        }
        #[cfg(not(feature = "rc"))]
        panic!("非 RC 查询迭代延期")
    }
}
