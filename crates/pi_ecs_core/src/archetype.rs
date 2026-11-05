//! 原型组件声明与分区内成员行；查询先匹配原型，再遍历其实体。
use crate::{component::ComponentId, entity::ArchetypeId, table::ColumnId};
/// 组件在特定原型中的唯一数据来源；不能只按 Rust 类型固定列或稀疏。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentSource {
    /// 从原型绑定 Table 的列取得；实际实例由实体当前分区确定。
    Table(
        /// 对应 TableLayout 内的列号。
        ColumnId,
    ),
    /// 显式 SparseOnly 组件，在本 World 跨分区共享后端按完整 Entity 取得。
    SharedSparse,
    /// 普通组件未被 Table 覆盖，从当前分区的后备取得。
    PartitionSparse,
}
/// 原型组件及其固定获取路径；按组件编号排序，可用于查询准备。
pub struct ArchetypeComponent {
    /// 已显式登记的组件编号。
    pub(crate) component: ComponentId,
    /// 在该原型内唯一有效的存储路径。
    pub(crate) source: ComponentSource,
}
/// 完整组件集合的声明；相同规范集合复用同一编号，各分区共用。
/// 组件集合和 Table 绑定创建后不可改变；实际成员列表归属各分区的 ArchetypeStorage。
pub struct Archetype {
    /// World 内 table 固定，variant=0 为基础，新增变体在所属 Table 内递增。
    pub(crate) id: ArchetypeId,
    /// 固定绑定的基础原型编号，与 id 同 World 同 table 且 variant=0；基础原型绑定自身。
    pub(crate) table: ArchetypeId,
    /// 按 ComponentId 升序且无重复的完整集合，包含每项来源。
    pub(crate) components: Box<[ArchetypeComponent]>,
    /// 未被 Table 覆盖的组件集合，包括显式稀疏与稀疏后备。
    pub(crate) sparse_components: Box<[ComponentId]>,
}
// @to_design 参考 Bevy Edges 缓存已发生的组合增删目标；待 Bundle/结构操作契约确定键和失败结果后补声明。
// 不预先构造所有组合，也不因缓存未命中认定目标原型不存在。
