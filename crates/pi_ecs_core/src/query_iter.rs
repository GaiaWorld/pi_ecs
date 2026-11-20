//! 原型成员迭代器；当前原型 fetch 复用，next 不重复产出实体。
#[path = "query_iter_inner.rs"]
mod query_iter_inner;
use crate::{
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
    /// 直接遍历 cache.tables 的偏移，不构造另一份 targets 数组。
    pub(crate) table_cursor: usize,
    /// 当前 QueryTable::Variants.entries 的下一偏移；Uniform 不使用。
    pub(crate) variant_cursor: usize,
    /// 当前目标的下一行：表级为物理 Table 行，原型级为成员行；均跳过洞。
    pub(crate) member_cursor: usize,
    /// 当前区/原型 fetch；切换时替换，不重新解析组件类型。
    pub(crate) current: Option<PreparedFetch<'w, Q, F>>,
}
impl<'w, Q: QueryData, F: QueryFilter> Iterator for QueryIter<'w, Q, F> {
    type Item = Q::Item<'w, 'w>;
    /// 返回下一个通过过滤的已发布成员（RC 含 strong=0、非 Null 的待收束实体）；耗尽返回 None，之后不能再产出实体。
    /// 不保证数量精确或稳定顺序；内部失配/panic 不能伪装成普通过滤或耗尽。
    fn next(&mut self) -> Option<Self::Item> {
        /* 设计已确认：
        直接遍历 cache.tables：编号为 first_table + table_cursor，Excluded 跳过，
        Uniform 只扫描一次 Table 已发布行，Variants 顺序遍历 entries 的原型成员。
        表级目标不再判断 variant 是否匹配；仍校验完整身份/发布位置/权限并执行动态过滤。
        全表证明须覆盖未来变体且 Q/F 计划统一，不能据当前匹配数量推断；准备时不伪造原型编号。
        当前目标结束则前进到下个唯一目标，复用紧凑条目内置状态绑定当前数据及过滤 fetch。
        先执行过滤：只复制所需 tick，全部短读结束后才 data.fetch_at 形成值/tick 写引用。
        扫描成员行，先前进游标再处理；跳过 None，使用成员 Entity/table_row 及原型行定位。
        RC 不以 strong=0 跳过非 Null 的待收束成员；按与点查相同的 Q/F 和完整身份位置规则取值。
        begin_destroy 隐藏发生于无业务借用的收束点，不与 next 交错；之后成员移出/留洞。
        每个实体只产出一次；已有结果可存活，后续 fetch 不得重借用整列覆盖这些结果。
        切换 fetch 不能析构组件或让结果引用本迭代器临时内存。
        */
        Self::next_inner(self)
    }
}

// 设计已确认：保留标准 Iterator，不改为逐次短借的 fetch_next；不同目标写结果允许并存。
// 构造前校验查询内部含写重复项、目标原型去重及成员唯一；不能靠临时 seen 集合补救错误存储。
// 存储访问器在该存储首次交出结果前绑定，值/tick 取址描述不拥有业务值，不提供安全 Clone/重置。
// 后续 fetch 只对目标值和 tick 单槽形成引用；不能再次 &mut World/Column/SparseSet 覆盖活结果。
// 切换原型时允许同组件跨原型存在未释放结果，不能重新独占绑定它们共享的 Table 列/稀疏值缓冲。
// 丢弃迭代器或替换 current 不得使已交出结果失效；存储地址稳定由原查询授权保护到全部结果结束。
// 共享索引/owner 元数据可读，但不得连带共享引用整个存放活写结果的值缓冲。
// RC 已落壳 ComponentView/ColumnView/SparseView：各 fetch 保存共享后端描述，值/tick 只在单槽形成引用。
// 芯仍未实现，不能以容器地址或声明可编译代替实际别名证明；非 RC 后端核查第二轮进行。
