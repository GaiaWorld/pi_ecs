//! Query 与 Alter 的相关 Table 增量扫描进度。
/// # 用途
/// 冻结后首次绑定时确定的潜在相关 Table，不因新变体改变列表；布局替换后重建。
pub(crate) struct TableScan {
    /// Table 编号，即使当前无匹配变体也记录潜在相关 Table。
    pub(crate) table: crate::entity::TableId,
    /// 已检查变体数量（含不匹配及基础变体）；这是数量，不是 VariantId。
    /// 仅扫描 [scanned_variants,current_count)，成功后推进，失败不能跳过。
    pub(crate) scanned_variants: usize,
}
