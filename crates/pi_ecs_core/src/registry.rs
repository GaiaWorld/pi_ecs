//! 组件、Table 和原型的统一登记壳；所有编号由所属 World 的注册表解释。
#[path = "registry_inner.rs"]
mod registry_inner;
use crate::{
    archetype::Archetype,
    component::{ComponentId, ComponentInfo, StoragePolicy},
    entity::ArchetypeId,
    error::EcsError,
    table::TableLayout,
};
use std::{any::TypeId, collections::HashMap};
/// 冻结后禁止新增组件与 Table，但仍允许为已登记类型显式声明新原型组合。
/// 预计算后的待提交原型；只由 plan_archetype 构造，commit 前不修改 registry。
pub(crate) struct ArchetypePlan {
    /// 已存在或本次将分配的完整原型编号。
    pub(crate) id: ArchetypeId,
    /// 规范化、升序且无重复的完整组件集合。
    pub(crate) canonical: Box<[ComponentId]>,
    /// 新原型的声明；已有集合为 None，commit 不重复写。
    pub(crate) archetype: Option<Archetype>,
    /// 是否需要在 commit 时追加 registry 记录。
    pub(crate) is_new: bool,
}
/// # 用途
/// 单个 World 的类型、Table 与原型元数据；冻结后仅追加合法变体，不重排既有编号。
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
    // 本表只存声明元数据；Table/列/分区后备/共享稀疏/原型成员实例全部由 WorldStorage 持有。
    // 冻结本身不创建物理存储；World 先构建存储，再提交本 registry 的冻结状态。
}
impl TypeRegistry {
    /// 创建登记阶段并保留零号空 Table；基础原型留待 freeze 完成登记，不创建业务实体。
    pub(crate) fn new() -> Self {
        Self::new_inner()
    }

    /// 显式登记 T 的布局、析构与 storage，并登记其稀疏数据源；不复制或构造 T 的业务值。
    /// 同类型同策略重复登记返回原编号（冻结后亦幂等）；策略冲突、冻结后新增或编号耗尽返回错误且登记不变。
    /// 内置与普通类型允许交错登记，首次登记按 components 末尾分配稳定编号，不预留前段、不在冻结时重排。
    /// 不同 N/T 特化各按完整 TypeId 登记，插件无须提前收集全部内置特化。
    /// T 显式实现 Component，Access 同源生成具体类型的唯一策略释放能力；不构造值。
    /// T 的 Send+Sync 约束不证明 World/存储可并发；RC 不据此跨线程传递访问句柄。
    /// 类型/布局与 ComponentPolicy<T> 必须一致；登记生成擦除适配，不接受独立分类或用户拼装函数。
    /// 安装由 BundleWriter::write_input 单态入口负责；旧登记芯暂停为失败桩，待实现新元数据发布。
    pub(crate) fn register_component<T: crate::component::Component>(
        &mut self,
        storage: StoragePolicy,
    ) -> Result<ComponentId, EcsError> {
        Self::register_component_inner::<T>(self, storage)
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
        Self::register_table_inner(self, components)
    }

    /// 冻结布局并建立同 ArchetypeId 的基础原型；partition_count 为所属 World 的固定分区数。
    /// 必须在系统初始化与动态原型登记前调用；相同编号对应全 World 唯一物理 Table。
    /// 成功冻结后重复调用无副作用；容量或配置校验失败不冻结、不公开部分基础原型。
    /// 冻结只提交元数据：不创建物理存储；容量上限为 Table/变体 16 位、分区 MAX_PARTITIONS、组件 u32。
    /// 校验失败保持未冻结；存储构建失败发生在冻结之前，失败不改变本 registry。
    /// 分配失败/panic 由分配器或展开终止，不暴露半冻结状态。
    pub(crate) fn freeze(&mut self, partition_count: usize) -> Result<(), EcsError> {
        /* 设计流程：
        中文伪码：
          若已冻结则返回；校验组件、Table 和基础原型编号容量。
           准备同号基础原型元数据；不在本方法创建物理存储。
           World::finish_registration 已先完成物理存储准备，再调用本方法提交元数据冻结。
           校验或元数据准备失败不冻结，由 World 清理未安装的物理准备数据。
        */
        Self::freeze_inner(self, partition_count)
    }

    /// 内部预计算完整 components 声明，不修改 registry。
    /// 已存在集合返回 `is_new=false` 的计划；新集合选择最大覆盖 Table 并计算 variant。
    pub(crate) fn plan_archetype(
        &self,
        components: &[ComponentId],
    ) -> Result<ArchetypePlan, EcsError> {
        Self::plan_archetype_inner(self, components)
    }
    /// 显式选择基础 Table 的预计算；base 必须 variant=0 且 components 包含其全部列。
    pub(crate) fn plan_archetype_on(
        &self,
        base: ArchetypeId,
        components: &[ComponentId],
    ) -> Result<ArchetypePlan, EcsError> {
        Self::plan_archetype_on_inner(self, base, components)
    }
    /// 提交已通过 plan 校验的原型；调用前必须完成物理 variant 准备。
    /// 新计划写入 archetype/集合索引；已有计划幂等返回，不改变 registry。
    pub(crate) fn commit_archetype(&mut self, plan: ArchetypePlan) -> ArchetypeId {
        Self::commit_archetype_inner(self, plan)
    }

    /// 返回 id 对应组件元数据；未知编号返回 None，不隐式登记。
    pub(crate) fn component(&self, id: ComponentId) -> Option<&ComponentInfo> {
        Self::component_inner(self, id)
    }
    /// 返回 id 对应预设布局；未知或尚未登记的编号返回 None。
    pub(crate) fn table(&self, id: ArchetypeId) -> Option<&TableLayout> {
        Self::table_inner(self, id)
    }
    /// 返回 id 对应完整原型声明；未知编号返回 None。
    pub(crate) fn archetype(&self, id: ArchetypeId) -> Option<&Archetype> {
        Self::archetype_inner(self, id)
    }
}
