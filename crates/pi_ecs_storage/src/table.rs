//! 预设布局与分区独占的物理 Table；元信息共享，行列数据独立。
use crate::column::Column;
use pi_ecs_types::component::ComponentId;
use pi_ecs_types::entity::{ArchetypeId, Entity};
use std::sync::atomic::AtomicU32;
/// 一个 Table 中的列序号；相同 ArchetypeId 在各分区具有相同列顺序。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColumnId(
    /// 从零开始的列序号，必须在对应 Table 的 columns 范围内。
    pub u32,
);
/// 固定 Table 布局，由显式登记生成；只包含允许列存储的组件。
/// columns 按 ComponentId 升序且无重复；ArchetypeId 同时为其基础原型编号。
pub struct TableLayout {
    /// 各分区共用的布局编号；空 Table 固定为零。
    pub(crate) id: ArchetypeId,
    /// 确定的列顺序，列号等于其下标；不含显式稀疏组件。
    pub(crate) columns: Box<[ComponentId]>,
}
impl TableLayout {
    /// id 须为本 World 的基础原型（variant=0），columns 已由 core 校验允许入表且升序无重复。
    /// 本入口只建立物理布局，不登记组件或选择原型；失败不改变调用方注册表。
    pub fn new(
        id: ArchetypeId,
        columns: Box<[ComponentId]>,
    ) -> Result<Self, pi_ecs_types::error::EcsError> {
        crate::table_inner::layout_new_inner(id, columns)
    }
    /// 返回所属 World 的基础布局编号，不授予修改权限。
    pub fn id(&self) -> ArchetypeId {
        crate::table_inner::layout_id_inner(self)
    }
    /// 返回固定列顺序，调用者不能借此修改布局或推导组件权限。
    pub fn columns(&self) -> &[ComponentId] {
        crate::table_inner::layout_columns_inner(self)
    }
}
/// Table 内物理行号，32位编码入 EntityLocation，完整容量与长度计数边界须一致。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TableRow(
    /// 从零开始的物理行；未 settle 的洞同样占行号容量。
    pub(crate) u32,
);
/// 行的发布状态；不占用 Entity 的保留位，也不新增 archetype_row。
pub(crate) enum RowState {
    /// 可被查询访问的完整实体；原型从其 EntityLocation 读取。
    Live(
        /// 包含代际及来源扩展位的完整键。
        Entity,
    ),
    /// 查询跳过的无效行；执行中可暂时隐藏源行，正常执行完成后的退休行不再拥有组件值。
    Retired,
}
/// 单个分区的实际 Table；行数含洞，列顺序与公共 TableLayout 一致。
/// 只有 Live 行允许查询；原型绑定和当前位置必须与本 Table 一致。
pub struct Table {
    /// 各分区一致的公共布局编号。
    pub(crate) id: ArchetypeId,
    /// 本实例所在的当前分区。
    pub(crate) partition: pi_ecs_types::entity::PartitionId,
    /// 行标记采用稳定扩展桶；槽默认值由内部工厂初始化为 Retired，不代表业务值存在。
    pub(crate) rows: pi_arr::Arr<std::cell::UnsafeCell<RowState>>,
    /// CAS 每次加一认领唯一物理行；容量检查在认领前，不以该计数作为查询上界。
    pub(crate) claimed_rows: AtomicU32,
    /// 连续已完成前缀，CAS 每次推进一行；查询 Acquire 读取，仅访问此界限内的 Live 行。
    /// 不能越过未完成的早期行；执行失败直接 panic，不承诺失败行补发。
    pub(crate) published_rows: AtomicU32,
    /// 按布局列序保存的独占列；已发布 Live 行的每列均已初始化。
    pub(crate) columns: Box<[Column]>,
    /// 已退休且尚未整理的行，每行至多登记一次；settle 后清空。
    pub(crate) pending_removals: Vec<TableRow>,
    // 参考 pi_world::safe_vec::insert，完成本行后等待前缀等于行号，再 CAS 加一发布，不越过前行。
    // 成功发布使用 AcqRel、失败/等待使用 Acquire，使最后前缀读取者获得整个前缀的初始化。
    // 所有普通检查/准备错误在认领行前返回；认领后的执行失败直接 panic。
    // 不承诺 lock-free 进展：前行暂停时后行等待；调度不得安排依赖后行才能完成的前行。
    // 不提供 panic 后续跑，不在 Drop 中等待发布；执行器不得依赖失败行补发才能退出。
    // CAS 只保证认领和发布协议，不授权别名写入；源行退休仍由访问冲突规则保护。
    // @to_design pending_removals 并发写入的容器随执行访问模型补齐；不承诺共享 Vec 并发修改。
}
impl Table {
    /// layout 属于本 World，partition 为实例当前分区，columns 为按布局顺序构造的空列。
    /// 调用方已校验编号/单值布局一致；不传 ComponentInfo 或析构能力，不创建业务活行。
    /// 返回独占物理表；准备失败不安装到调用方分区。
    pub fn new(
        layout: &TableLayout,
        partition: pi_ecs_types::entity::PartitionId,
        columns: Box<[Column]>,
    ) -> Result<Self, pi_ecs_types::error::EcsError> {
        crate::table_inner::new_inner(layout, partition, columns)
    }
}
