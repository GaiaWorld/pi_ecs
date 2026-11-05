//! 批量点查；固定数组可并存，流式可变访问通过短借用避免别名。
use crate::{
    entity::Entity,
    query::{NoFilter, Query, QueryData, QueryFilter, QueryMut, ReadOnlyQueryData},
};
/// 点查业务错误，不把内部授权失效伪装为不匹配。
#[derive(Debug)]
pub enum QueryEntityError {
    /// 身份失效或已物理销毁；PendingDestroy 在本阶段仍有效。
    NoSuchEntity(Entity),
    /// 缺必需组件或未通过过滤。
    NotMatched(Entity),
    /// 同一批可变输出存在重复完整 Entity。
    AliasedEntity(Entity),
}
/// 固定批量失败，不返回部分借用，也不标记 Changed。
#[derive(Debug)]
pub struct BatchQueryError {
    /// 首个失败输入位置；可变查重优先于取值，确定具体索引规则待实现统一。
    pub index: usize,
    /// 失败身份及原因。
    pub cause: QueryEntityError,
}
/// 只读逐项结果，输入允许重复，不自动略过错误。
pub struct QueryMany<'a, 'w, Q: ReadOnlyQueryData, F: QueryFilter, I: Iterator<Item = Entity>> {
    /// 保活查询和 guard。
    pub(crate) query: &'a Query<'w, Q, F>,
    /// 未消费的身份输入，保持输入顺序。
    pub(crate) entities: I,
}
impl<'a, Q: ReadOnlyQueryData, F: QueryFilter, I: Iterator<Item = Entity>> Iterator
    for QueryMany<'a, '_, Q, F, I>
{
    type Item = Result<Q::Item<'a, 'a>, QueryEntityError>;
    /// 返回一项结果，错误仍占一个输入位置，耗尽后不再取值。
    fn next(&mut self) -> Option<Self::Item> {
        crate::query_batch_inner::next_inner(self)
    }
}
/// 可变输入允许重复，但每项结果仅借用本次 fetch_next，不能同时保留再访问。
pub struct QueryManyMut<
    'a,
    'w,
    Q: QueryData,
    F: QueryFilter = NoFilter,
    I: Iterator<Item = Entity> = std::vec::IntoIter<Entity>,
> {
    /// 独占借用当前可变查询，不扩大权限。
    pub(crate) query: &'a mut QueryMut<'w, Q, F>,
    /// 按原顺序访问，不做全量查重。
    pub(crate) entities: I,
}
impl<Q: QueryData, F: QueryFilter, I: Iterator<Item = Entity>> QueryManyMut<'_, '_, Q, F, I> {
    /// 结果与本次 &mut self 同寿命；先前可变结果使用结束后才能再调用。
    /// 错误不跳过后续项，不隐式标记 Changed；不是普通 Iterator。
    pub fn fetch_next(&mut self) -> Option<Result<Q::Item<'_, '_>, QueryEntityError>> {
        crate::query_batch_inner::fetch_next_inner(self)
    }
}
impl<Q: ReadOnlyQueryData, F: QueryFilter> Query<'_, Q, F> {
    /// entities 保序且允许重复；N=0 合法，全部身份/过滤检查后才返回引用，失败不交出部分结果。
    pub fn get_many<const N: usize>(
        &self,
        entities: [Entity; N],
    ) -> Result<[Q::Item<'_, '_>; N], BatchQueryError> {
        crate::query_batch_inner::get_many_inner(self, entities)
    }
}
impl<'w, Q: ReadOnlyQueryData, F: QueryFilter> Query<'w, Q, F> {
    /// entities 不要求唯一，迭代逐项返回错误，不隐式丢弃输入。
    pub fn iter_many<I: IntoIterator<Item = Entity>>(
        &self,
        entities: I,
    ) -> QueryMany<'_, 'w, Q, F, I::IntoIter> {
        crate::query_batch_inner::iter_many_inner(self, entities)
    }
}
impl<Q: QueryData, F: QueryFilter> QueryMut<'_, Q, F> {
    /// entities 中重复完整 Entity 一律拒绝，即使可选组件不存在；N=0 返回空数组。
    /// 先查重及验证所有身份/过滤，再构造全部结果；不重排输入，不返回部分借用。
    pub fn get_many_mut<const N: usize>(
        &mut self,
        entities: [Entity; N],
    ) -> Result<[Q::Item<'_, '_>; N], BatchQueryError> {
        crate::query_batch_inner::get_many_mut_inner(self, entities)
    }
}
impl<'w, Q: QueryData, F: QueryFilter> QueryMut<'w, Q, F> {
    /// entities 保序允许重复，通过 fetch_next 的短借用保证同一实体引用不并存。
    pub fn iter_many_mut<I: IntoIterator<Item = Entity>>(
        &mut self,
        entities: I,
    ) -> QueryManyMut<'_, 'w, Q, F, I::IntoIter> {
        crate::query_batch_inner::iter_many_mut_inner(self, entities)
    }
}
// 唯一输入包装及其普通可变 Iterator 后置；实际引用/Option/元组 fetch 接线仍待闭合。
