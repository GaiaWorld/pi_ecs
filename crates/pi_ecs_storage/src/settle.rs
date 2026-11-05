//! 参考 pi_world 的批量填洞计划；不同列与实体行应用同一动作表。
use crate::table::TableRow;
/// 把一个尾部存活行移到前方退休洞；源、目标不重叠且不得重复使用。
pub(crate) struct RowMove {
    /// 拥有全列活值的源行，完成后无值。
    pub(crate) source: TableRow,
    /// 无业务值的退休洞，完成后拥有整行。
    pub(crate) target: TableRow,
}
/// 独占 settle 时构造并消费的临时整理计划，不改变组件逻辑变化状态。
pub(crate) struct SettlePlan {
    /// 对全部组件列、行标记和位置修复共用的确定动作。
    pub(crate) moves: Vec<RowMove>,
    /// 填洞后存活前缀长度，截去区域不得仍有业务值。
    pub(crate) len: usize,
}
// 少量洞可排序洞位置，大量洞可扫描行；参照 pi_world::Table::removes_action。
// 两种算法必须遵守相同的源尾行→前洞规则，不能由 HashMap 迭代顺序决定结果。
