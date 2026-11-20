//! 预设布局与全局物理 Table；布局元信息与行列数据均不再按分区复制。
#[path = "table_inner.rs"]
mod table_inner;
use crate::column::Column;
use pi_ecs_types::component::ComponentId;
use pi_ecs_types::entity::{ArchetypeId, Entity};
#[cfg(not(feature = "rc"))]
use std::sync::atomic::{AtomicU32, AtomicU64};
/// 一个 Table 中的列序号；相同 ArchetypeId 使用同一列顺序。
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
        Self::layout_new_inner(id, columns)
    }
    /// 返回所属 World 的基础布局编号，不授予修改权限。
    pub fn id(&self) -> ArchetypeId {
        Self::layout_id_inner(self)
    }
    /// 返回固定列顺序，调用者不能借此修改布局或推导组件权限。
    pub fn columns(&self) -> &[ComponentId] {
        Self::layout_columns_inner(self)
    }
}
/// Table 内物理行号，32位编码入 EntityLocation，完整容量与长度计数边界须一致。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TableRow(
    /// 从零开始的物理行；未 settle 的洞同样占行号容量。
    pub(crate) u32,
);
/// 行的发布状态；不占用 Entity 的保留位，也不新增 archetype_row。
#[derive(Default)]
pub(crate) enum RowState {
    /// 可被查询访问的完整实体；原型从其 EntityLocation 读取。
    Live(
        /// 包含代际及出生分区位的完整键。
        Entity,
    ),
    /// 查询跳过的无效行；Alter 未发布目标行使用 Entity::null()，查询跳过且发布前不拥有可见身份。
    /// 执行中可暂时隐藏源行，正常执行完成后的退休行不再拥有组件值。
    #[default]
    Retired,
}
/// 非 rc 待整理退休行；执行期唯一认领者追加，settle 等待全部追加者收束。
/// 不使用共享 Vec 并发修改；len 只是已发布前缀，不能替代收束证明。
#[cfg(not(feature = "rc"))]
pub(crate) struct PendingTableRemovals {
    /// 每行至多登记一次；settle 读取后清空，不搬移仍存活的行。
    pub(crate) rows: pi_append_vec::AppendVec<TableRow>,
}
/// rc 单线程待整理退休行；删除已即时缩短有效范围，本列表只服务同次 settle。
#[cfg(feature = "rc")]
pub(crate) struct PendingTableRemovals {
    /// 按退休顺序保存；settle 后清空。
    pub(crate) rows: Vec<TableRow>,
}
/// rc 行标记：单线程独占访问，直接保存状态，不需要内部可变性或原子。
#[cfg(feature = "rc")]
#[derive(Default)]
pub(crate) struct RowCell(
    /// 行状态；Table 以 &mut 访问，读者不与其并存。
    pub(crate) RowState,
);
/// 非 rc 行标记：原子 u64 打包完整键，Retired 使用 u64::MAX，不使用 UnsafeCell。
/// 写入只由唯一认领者进行，读取只访问已发布前缀内的 Live 行。
#[cfg(not(feature = "rc"))]
#[derive(Default)]
pub(crate) struct RowCell(
    /// Live 为 KeyData::as_ffi 的完整键，Retired 为 u64::MAX；由 Table 认领/发布协议访问。
    pub(crate) AtomicU64,
);
/// 全局实际 Table；行数含洞，列顺序与公共 TableLayout 一致。
/// 只有 Live 行允许查询；原型绑定和当前位置必须与本 Table 一致。
pub struct Table {
    /// 全局公共布局编号。
    pub(crate) id: ArchetypeId,
    /// 行标记采用稳定扩展桶；槽默认值由内部工厂初始化为 Retired，不代表业务值存在。
    pub(crate) rows: pi_arr::Arr<RowCell>,
    /// 每次加一认领唯一物理行；容量检查在认领前，不以该计数作为查询上界。
    #[cfg(feature = "rc")]
    pub(crate) claimed_rows: u32,
    /// 非 rc 原子认领计数，供并发写者共享。
    #[cfg(not(feature = "rc"))]
    pub(crate) claimed_rows: AtomicU32,
    /// 连续已完成前缀，每次推进一行；仅访问此界限内的 Live 行。
    /// 不能越过未完成的早期行；执行失败直接 panic，不承诺失败行补发。
    #[cfg(feature = "rc")]
    pub(crate) published_rows: u32,
    /// 非 rc 原子发布前缀，查询以 Acquire 读取。
    #[cfg(not(feature = "rc"))]
    pub(crate) published_rows: AtomicU32,
    /// 按布局列序保存的独占列；已发布 Live 行的每列均已初始化。
    #[cfg(not(feature = "rc"))]
    pub(crate) columns: Box<[Column]>,
    /// RC 列容器位于固定内部可变槽；行增长只改变列内部缓冲，不移动或替换 Column。
    #[cfg(feature = "rc")]
    pub(crate) columns: Box<[std::rc::Rc<std::cell::UnsafeCell<Column>>]>,
    /// 已退休且尚未整理的行，每行至多登记一次；settle 后清空。
    /// 非 rc 用 AppendVec 承接并发追加，rc 用本地 Vec。
    pub(crate) pending_removals: PendingTableRemovals,
    // 参考 pi_world::safe_vec::insert，完成本行后等待前缀等于行号，再 CAS 加一发布，不越过前行。
    // 成功发布使用 AcqRel、失败/等待使用 Acquire，使最后前缀读取者获得整个前缀的初始化。
    // 所有普通检查/准备错误在认领行前返回；认领后的执行失败直接 panic。
    // 不承诺 lock-free 进展：前行暂停时后行等待；调度不得安排依赖后行才能完成的前行。
    // 不提供 panic 后续跑，不在 Drop 中等待发布；执行器不得依赖失败行补发才能退出。
    // CAS 只保证认领和发布协议，不授权别名写入；源行退休仍由访问冲突规则保护。
    // pending_removals 非 rc 采用 AppendVec：唯一认领者追加，settle 等待全部追加者收束后清空；
    // rc 采用 Vec，由单线程独占写入。两者都不在普通增删中夹带填洞压缩。
}
impl TableRow {
    /// 由已校验行号构造；越界调用方负责。
    pub fn new(index: u32) -> Self {
        Self(index)
    }
    /// 返回物理行号；不授予存储访问。
    pub fn index(self) -> u32 {
        self.0
    }
}
impl Table {
    /// 认领一个空行并准备所有列槽；rc 单线程路径。
    /// u32 认领/发布计数表示尾后界；u32::MAX 保留为耗尽尾界，不分配该编号行。
    /// checked 容量检查先于任何槽修改，耗尽返回 ResourceExhausted，计数永久不回绕。
    /// 各列准备普通 Err 不推进认领计数，已准备空容量可保留；不构造或析构业务值。
    pub fn claim_row(&mut self) -> Result<TableRow, pi_ecs_types::error::EcsError> {
        Self::claim_row_inner(self)
    }
    /// 发布已准备好并写入的行；必须按 claimed 顺序连续发布。
    pub fn publish_row(
        &mut self,
        row: TableRow,
        entity: Entity,
    ) -> Result<(), pi_ecs_types::error::EcsError> {
        Self::publish_row_inner(self, row, entity)
    }
    /// 退休已发布行；不立即搬移其他行，settle 再整理。
    pub fn retire_row(&mut self, row: TableRow) {
        Self::retire_row_inner(self, row)
    }
    /// 按列号取得列；越界返回 None。
    /// 非 RC 保留原独占列接口；RC 在下方返回不可替换列的维护能力。
    #[cfg(not(feature = "rc"))]
    pub fn column_mut(&mut self, column: ColumnId) -> Option<&mut Column> {
        self.columns.get_mut(column.0 as usize)
    }
    /// 返回列数量。
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
    /// layout 属于本 World，columns 为按布局顺序构造的空列。
    /// 调用方已校验编号/单值布局一致；不传 ComponentInfo 或析构能力，不创建业务活行。
    /// 返回独占物理表；准备失败不安装到调用方存储。
    pub fn new(
        layout: &TableLayout,
        columns: Box<[Column]>,
    ) -> Result<Self, pi_ecs_types::error::EcsError> {
        Self::new_inner(layout, columns)
    }
}

#[cfg(feature = "rc")]
impl Table {
    /// # 用途
    /// column 为冻结列号；只交付槽操作，不允许替换列或取出底层 &mut Column，越界 None。
    /// 当前独占 Table 授权排除运行结果；长期缓存必须从固定 UnsafeCell 导出，而非此维护借用。
    pub fn column_mut(&mut self, column: ColumnId) -> Option<crate::column::ColumnMaintenance<'_>> {
        self.column_maintenance_inner(column)
    }
}

#[cfg(feature = "rc")]
impl Table {
    /// # 用途
    /// column 为固定列号；导出 Rc 分配体内的非拥有 Column 地址，越界 None，不授予读写权限。
    /// 地址来自 Rc::as_ptr→UnsafeCell::get，非临时 &mut Column；移动 Table 或目录句柄不改变该分配体。
    pub fn column_address(&self, column: ColumnId) -> Option<std::ptr::NonNull<Column>> {
        self.column_address_inner(column)
    }
}
