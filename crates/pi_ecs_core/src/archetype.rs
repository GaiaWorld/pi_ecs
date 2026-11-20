//! 原型组件声明与全局成员行；查询先匹配原型，再遍历其实体。
use crate::{component::ComponentId, entity::ArchetypeId, table::ColumnId};
#[path = "archetype_inner.rs"]
mod archetype_inner;
/// 组件在特定原型中的唯一数据来源；不能只按 Rust 类型固定列或稀疏。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComponentSource {
    /// 从原型绑定的全 World Table 列取得；当前分区不选择物理实例。
    Table(
        /// 对应 TableLayout 内的列号。
        ColumnId,
    ),
    /// 未被 Table 覆盖的组件，从本 World 的全局稀疏后端按完整 Entity 取得。
    Sparse,
}
/// 原型组件及其固定获取路径；按组件编号排序，可用于查询准备。
pub struct ArchetypeComponent {
    /// 已显式登记的组件编号。
    pub(crate) component: ComponentId,
    /// 在该原型内唯一有效的存储路径。
    pub(crate) source: ComponentSource,
}
/// 完整组件集合的声明；相同规范集合复用同一编号，全 World 共用。
/// 组件集合和 Table 绑定创建后不可改变；实际成员列表归属全局 ArchetypeStorage。
pub struct Archetype {
    /// World 内 table 固定，variant=0 为基础，新增变体在所属 Table 内递增。
    pub(crate) id: ArchetypeId,
    /// 固定绑定的基础原型编号，与 id 同 World 同 table 且 variant=0；基础原型绑定自身。
    pub(crate) table: ArchetypeId,
    /// 按 ComponentId 升序且无重复的完整集合，包含每项来源。
    pub(crate) components: Box<[ArchetypeComponent]>,
    /// 未被 Table 覆盖的组件集合，统一走全局稀疏后端。
    pub(crate) sparse_components: Box<[ComponentId]>,
}
impl Archetype {
    /// # 用途
    /// 返回本 World 原型编号，不提供成员或组件值访问权限。
    pub fn id(&self) -> ArchetypeId {
        self.id_inner()
    }
    /// # 用途
    /// 返回固定基础 Table 原型编号，variant 为零，不修改绑定。
    pub fn table(&self) -> ArchetypeId {
        self.table_inner()
    }
    /// # 用途
    /// 返回完整有序组件及来源声明，只读借用 self，不包含活值或实体成员。
    pub fn components(&self) -> &[ArchetypeComponent] {
        self.components_inner()
    }
}
impl ArchetypeComponent {
    /// # 用途
    /// 返回当前声明组件编号，只能在所属 World 解释。
    pub fn component(&self) -> ComponentId {
        self.component_inner()
    }
    /// # 用途
    /// 返回本原型固定存储来源的副本，不授予值访问权限。
    pub fn source(&self) -> ComponentSource {
        self.source_inner()
    }
}
// @to_design 后续性能优化（不属首轮 RC）：参考 Bevy Edges 设计组合增删目标缓存的键、失效及失败规则。
// 首轮沿现有目标解析/Alter 计划执行，不新增缓存字段，不依赖边缓存完成结构修改。
// 不预先构造所有组合，也不因缓存未命中认定目标原型不存在。
