//! 原型成员迭代器；当前原型 fetch 复用，next 不重复产出实体。
use crate::{
    entity::PartitionId,
    query::{QueryData, QueryFilter, QueryState},
    query_fetch::{FetchAccess, PreparedFetch},
};
/// 本次遍历游标，可产生同一借用期的不同实体引用；不可 Clone，不支持重置。
/// mutable Item 安全依赖核心构造器证明区/原型互斥、成员唯一、查询内部访问无冲突。
pub struct QueryIter<'w, Q: QueryData, F: QueryFilter = crate::query::NoFilter> {
    /// 保活本次存储访问能力；不是未受限的公开 World 指针。
    pub(crate) access: FetchAccess<'w>,
    /// 长期计划的共享读取，不允许遍历期间更新匹配或注册原型。
    pub(crate) state: &'w QueryState<Q, F>,
    /// 不重复的（分区、紧凑原型偏移）目标，直接索引 state.archetypes，不按稀疏原型铺表。
    pub(crate) targets: Vec<(PartitionId, usize)>,
    /// targets 的下一个目标索引。
    pub(crate) target_cursor: usize,
    /// 当前原型内下一个待检查成员行，含需要跳过的洞。
    pub(crate) member_cursor: usize,
    /// 当前区/原型 fetch；切换时替换，不重新解析组件类型。
    pub(crate) current: Option<PreparedFetch<'w, Q, F>>,
}
impl<'w, Q: QueryData, F: QueryFilter> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w, 'w>;
    /// 返回下一个通过过滤的活成员；耗尽返回 None，之后不能再产出实体。
    /// 不保证数量精确或稳定顺序；内部失配/panic 不能伪装成普通过滤或耗尽。
    fn next(&mut self) -> Option<Self::Item> {
        /* 设计已确认：
        当前原型结束则前进到下个唯一区/原型，复用紧凑条目内置状态绑定当前数据及过滤 fetch。
        扫描成员行，先前进游标再处理；跳过 None，使用成员 Entity/table_row 及原型行定位。
        通过与点查相同的实体过滤后调用 QueryFetch::fetch_at。
        每个实体只产出一次；已有结果可存活，后续 fetch 不得重借用整列覆盖这些结果。
        切换 fetch 不能析构组件或让结果引用本迭代器临时内存。
        */
        crate::query_iter_inner::next_inner(self)
    }
}
