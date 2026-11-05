//! 组件、Table 和原型的统一登记壳；所有编号由所属 World 的注册表解释。
use crate::{
    archetype::Archetype,
    component::{ComponentId, ComponentInfo, StoragePolicy},
    entity::ArchetypeId,
    error::EcsError,
    table::TableLayout,
};
use std::{any::TypeId, collections::HashMap};
/// 冻结后禁止新增组件与 Table，但仍允许为已登记类型显式声明新原型组合。
pub struct TypeRegistry {
    /// 按编号保存的组件元数据；同时为每个类型登记固定稀疏数据源入口。
    pub(crate) components: Vec<ComponentInfo>,
    /// Rust 类型到当前注册表局部组件编号的映射。
    pub(crate) component_ids: HashMap<TypeId, ComponentId>,
    /// 包含零号空 Table 的预设布局，初始化冻结后不增长。
    pub(crate) tables: Vec<TableLayout>,
    /// 规范的组件集合到 Table 的去重索引。
    pub(crate) table_ids: HashMap<Box<[ComponentId]>, ArchetypeId>,
    /// 统一编号的基础及增补原型；身份为本 World 的 Table/变体，variant=0 表示基础。
    pub(crate) archetypes: Vec<Archetype>,
    /// 完整组件集合到唯一原型编号的去重索引。
    pub(crate) archetype_ids: HashMap<Box<[ComponentId]>, ArchetypeId>,
    /// 是否已冻结类型和布局；成功冻结不可回退。
    pub(crate) frozen: bool,
    // @to_design 稀疏数据源实际持有在 World 存储层；冻结需完成各分区 Table 与稀疏入口创建。
}
impl TypeRegistry {
    /// 创建登记阶段并保留零号空 Table；基础原型留待 freeze 完成登记，不创建业务实体。
    pub(crate) fn new() -> Self {
        crate::registry_inner::new_inner()
    }

    /// 显式登记 T 的布局、析构与 storage，并登记其稀疏数据源；不复制或构造 T 的业务值。
    /// 同类型同策略重复登记返回原编号；策略冲突、冻结后登记或编号耗尽返回错误且登记不变。
    /// T 显式实现 Component，Access 同源生成内置分类和释放能力；不构造值。
    /// 线程能力尚未决定，不能据此向其他线程传递其值。
    pub(crate) fn register_component<T: crate::component::Component>(
        &mut self,
        storage: StoragePolicy,
    ) -> Result<ComponentId, EcsError> {
        crate::registry_inner::register_component_inner::<T>(self, storage)
    }

    /// 显式登记 components 所列的 Table，同覆盖数量时首次成功登记者优先。
    /// 输入顺序不影响布局，重复组件、未登记编号、显式稀疏组件均返回错误且登记不变。
    /// 相同集合复用首次编号，不改变登记顺序。
    /// 空集合固定 table=0/variant=0，Table 编号在当前 World 内按首次登记分配。
    /// 冻结后任何 Table 登记均错误；不分配运行期动态 Table。
    pub(crate) fn register_table(
        &mut self,
        components: &[ComponentId],
    ) -> Result<ArchetypeId, EcsError> {
        crate::registry_inner::register_table_inner(self, components)
    }

    /// 冻结布局并建立同 ArchetypeId 的基础原型；partition_count 为所属 World 的固定分区数。
    /// 必须在系统初始化与动态原型登记前调用；各分区相同编号对应独立物理 Table。
    /// 成功冻结后重复调用无副作用；容量或配置校验失败不冻结、不公开部分基础原型。
    /// @to_design 存储构造失败与内存分配 panic 边界、N 上限随 World 初始化契约闭合。
    pub(crate) fn freeze(&mut self, partition_count: usize) -> Result<(), EcsError> {
        /* 设计流程：
        中文伪码：
          若已冻结则返回；校验组件、Table 和基础原型编号容量。
          准备空 Table 和同号基础原型，以及每个分区的存储和每组件稀疏数据源。
          准备成功才发布固定注册集合并冻结；失败清理准备数据。
          存储准备由 World 统一协调，具体分工待设计。
        */
        crate::registry_inner::freeze_inner(self, partition_count)
    }

    /// 显式声明完整 components 集合；仅可在冻结后调用，输入顺序不影响原型身份。
    /// 重复组件或未知编号错误；已知集合复用声明，包括同号基础原型。
    /// 自动选择最大子集 Table；同分首次登记优先，唯一空 Table 兜底。
    /// 成功只新增元数据，不创建实体或存储类型；失败不公开半声明，编号容量受位置位宽限制。
    pub(crate) fn register_archetype(
        &mut self,
        components: &[ComponentId],
        partition_count: usize,
    ) -> Result<ArchetypeId, EcsError> {
        /* 设计已确认：
        中文伪码：
          校验冻结状态、组件编号并规范化集合；已有声明则返回原编号。
          在当前 World 选覆盖最多的子集 Table；同分首次登记优先。
          按 Table 列和集合差构造列/稀疏获取路径，校验原型编号容量。
          一次发布完整声明和集合索引；失败不发布。
        */
        crate::registry_inner::register_archetype_inner(self, components, partition_count)
    }

    /// 返回 id 对应组件元数据；未知编号返回 None，不隐式登记。
    pub(crate) fn component(&self, id: ComponentId) -> Option<&ComponentInfo> {
        crate::registry_inner::component_inner(self, id)
    }
    /// 返回 id 对应预设布局；未知或尚未登记的编号返回 None。
    pub(crate) fn table(&self, id: ArchetypeId) -> Option<&TableLayout> {
        crate::registry_inner::table_inner(self, id)
    }
    /// 返回 id 对应完整原型声明；未知编号返回 None。
    pub(crate) fn archetype(&self, id: ArchetypeId) -> Option<&Archetype> {
        crate::registry_inner::archetype_inner(self, id)
    }
}
