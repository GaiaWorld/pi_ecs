//! Alter 专用紧凑 Table→variant 计划映射；查询使用独立 WorldQueryCache。
#[path = "plan_map_inner.rs"]
mod plan_map_inner;

/// 固定 Table 数的两级映射压缩为单数组：头部每 Table 一项相对偏移和数量，尾部为 plan 下标。
/// 计划下标 0..65535，u16::MAX 为已扫描但不合法源；未扫描由段数量判断。
/// u16 头部数量及相对偏移受压缩容量限制，超限报 ResourceExhausted，不截断或伪造计划。
pub(crate) struct ArchetypePlanMap {
    /// 头部长度固定为 table_count*2，尾部为各 Table 的连续 variant 段。
    pub(crate) entries: Vec<u16>,
    /// 冻结后的 Table 数量；新增 variant 不在头部增长。
    pub(crate) table_count: usize,
}

impl ArchetypePlanMap {
    /// table_count 来自冻结布局；头部一次分配，不随后续变体改变。
    pub(crate) fn new(table_count: usize) -> Self {
        Self::new_inner(table_count)
    }
    /// 返回 (table, variant) 的合法 plan 下标；未知、未扫描及已扫描不合法均 None。
    /// 扫描进度由 AlterCache.tables 单源维护，不用 get 的 None 推断是否扫描。
    pub(crate) fn get(
        &self,
        table: crate::entity::TableId,
        variant: crate::entity::VariantId,
    ) -> Option<u16> {
        Self::get_inner(self, table, variant)
    }
    /// 写入/追加 (table, variant)；Some(plan) 必须小于 u16::MAX，None 编码为不合法源哨兵。
    /// 每个已扫描变体均须写槽，variant 不跳号；覆盖不改变数量，clear 清全部扫描槽。
    /// 在临时区完成容量校验/压缩后发布；普通 Err 原映射不变，不能因废弃段占用误拒可表示的新映射。
    pub(crate) fn set(
        &mut self,
        table: crate::entity::TableId,
        variant: crate::entity::VariantId,
        plan: Option<u16>,
    ) -> Result<(), crate::error::EcsError> {
        Self::set_inner(self, table, variant, plan)
    }
    /// 清空所有命中但保留固定头部；布局代次变化时整体重建。
    pub(crate) fn clear(&mut self) {
        Self::clear_inner(self)
    }
    /// 返回头部 Table 数量。
    pub(crate) fn table_count(&self) -> usize {
        self.table_count
    }
}
