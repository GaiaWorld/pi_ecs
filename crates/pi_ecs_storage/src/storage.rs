//! 每个 World 一套全局组件存储；游戏与渲染使用独立 World。
//! Table、列、稀疏后端和原型成员不再按分区复制；分区只用于实体分配与当前位置过滤。
//! 非 rc 普通添加/删除不搬移其他存活值地址；rc 为单线程模式，容量增长允许搬移旧值。
//! rc 增长前结束受影响存储的引用/迭代器/地址使用，增长后重绑地址缓存；单线程不豁免别名约束。
//! 两种模式均不在普通增删中夹带填洞压缩。
//! 显式迁移、settle/压缩/恢复等搬移操作必须受相应排他授权约束；地址稳定不延长已删除值寿命。
#[path = "storage_inner.rs"]
mod storage_inner;
use crate::{
    sparse_set::SparseSetStorage,
    table::{Table, TableRow},
};
use pi_ecs_types::component::ComponentId;

/// # 用途
/// 物理成员只读副本，不包含组件值或引用；只能在所属存储解释。
pub struct MemberLocation {
    /// 完整实体身份，不能只用槽索引解释。
    pub entity: pi_ecs_types::entity::Entity,
    /// 成员当前 Table 行，与原型成员行不同。
    pub table_row: TableRow,
}
/// # 用途
/// 一个组件的目标活值 tick 开关；通知订阅不在此结构表达。
pub struct TrackingTarget {
    /// 同源冻结组件编号；应用覆盖该组件全部列及稀疏活槽。
    pub component: ComponentId,
    /// Added 活值 tick 是否启用。
    pub added: bool,
    /// Changed 活值 tick 是否启用。
    pub changed: bool,
}
/// # 用途
/// 整理后交回 core 的最终位置修复记录；不拥有业务值。
pub struct SettledLocation {
    /// 完整实体身份；core 保留其当前分区不变。
    pub entity: pi_ecs_types::entity::Entity,
    /// 最终完整原型编号，包含所属 Table。
    pub archetype: pi_ecs_types::entity::ArchetypeId,
    /// 最终 Table 物理行。
    pub table_row: TableRow,
    /// 最终原型成员行。
    pub archetype_row: u32,
}
/// 一个 Table 的全部变体成员；variant=0 固定为基础原型。
pub struct TableArchetypes {
    /// 按 variant 直接索引；基础原型创建后可追加变体，不回填空洞。
    pub(crate) variants: Vec<crate::archetype::ArchetypeStorage>,
}
/// 当前 RC 冻结后固定容器位置；长期查询缓存仅指向容器，不指向可增长缓冲。
/// RC 列/稀疏容器由私有 Rc<UnsafeCell<_>> 分配体拥有；Box 目录仅保存句柄，移动句柄不搬动容器。
/// 禁止 Rc::get_mut/make_mut/try_unwrap 与整对象替换；结构操作只经 UnsafeCell 临时借用内部对象。
/// 缓存从 Rc::as_ptr 指向的 UnsafeCell::get 导出，不从临时 &mut 容器派生。
/// 当前不引入恢复或存储替换能力。
pub struct WorldStorage {
    /// 按 table 编号寻址的全局物理表。
    pub(crate) tables: Box<[Table]>,
    /// 普通组件的全局稀疏后端；不再有分区后备或显式共享之分。
    #[cfg(not(feature = "rc"))]
    pub(crate) sparse: Box<[Option<SparseSetStorage>]>,
    /// RC 私有稳定拥有句柄；不公开 Rc 克隆/解包，不从临时可变引用导出缓存。
    #[cfg(feature = "rc")]
    pub(crate) sparse: Box<[Option<std::rc::Rc<std::cell::UnsafeCell<SparseSetStorage>>>]>,
    /// 按 Table 编号寻址的全局变体成员；每个条目内按 variant 直接索引。
    pub(crate) archetypes: Box<[TableArchetypes]>,
}
impl WorldStorage {
    /// # 用途
    /// RC 结构入口为 component/entity 联合准备值、已启用 tick 和 owner，沿 SparseSetStorage::prepare_tracked。
    /// 未登记返回错误；不发布值，不改变旧活值，调用者仍拥有输入。
    #[cfg(feature = "rc")]
    pub fn prepare_sparse_tracked(
        &mut self,
        component: ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) -> Result<(), pi_ecs_types::error::EcsError> {
        self.prepare_sparse_tracked_inner(component, entity)
    }
    /// # 用途
    /// 撤销本次独占维护中该组件最新、仍未写入的空稀疏预留；保留已分配容量。
    #[cfg(feature = "rc")]
    pub fn discard_sparse_reservation(
        &mut self,
        component: ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) {
        self.discard_sparse_reservation_inner(component, entity)
    }
    /// # 用途
    /// 移出前复制同源活槽 tick；缺组件/实体 None，启用项必须有初始化时间，不取得业务值权限。
    #[cfg(feature = "rc")]
    pub fn sparse_ticks(
        &self,
        component: ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) -> Option<pi_ecs_types::change_detection::ComponentTicks> {
        self.sparse_ticks_inner(component, entity)
    }
    /// # 用途
    /// 结构入口安装已准备空槽，沿 SparseSetStorage::insert_tracked 的值→tick→owner 协议。
    /// 新增传结构 tick，共有值传移出前快照；调用方先结束所有冲突视图并保持源分配存活。
    #[cfg(feature = "rc")]
    pub fn insert_sparse_tracked(
        &mut self,
        component: ComponentId,
        entity: pi_ecs_types::entity::Entity,
        value: crate::ptr::OwningPtr<'_>,
        ticks: pi_ecs_types::change_detection::ComponentTicks,
    ) {
        self.insert_sparse_tracked_inner(component, entity, value, ticks)
    }
    /// # 用途
    /// targets 无重复且编号同源；未列组件保持原状，tick 为 core 本轮已取得的维护时间。
    /// RC 独占维护且无活视图/结果；首次或重启分支初始化全部现存 Live 值，已启用不重置。
    /// 关闭分支撤销 tick 数组（仅元数据），重启重新初始化；不改业务值、不补通知。
    /// 先校验并准备全部列/稀疏数组，普通 Err 不改变开关或旧 tick；提交无分配，panic 由 core 标停。
    #[cfg(feature = "rc")]
    pub fn apply_tracking(
        &mut self,
        targets: &[TrackingTarget],
        tick: pi_ecs_types::change_detection::ChangeTick,
    ) -> Result<(), pi_ecs_types::error::EcsError> {
        self.apply_tracking_inner(targets, tick)
    }
    /// # 用途
    /// RC 独占静止点，core 已撤销准备且确认无引用/视图/批次；先准备公共动作表和返回空间。
    /// 按 Table/variant 顺序，尾 Live 行填前洞，各列及 tick 同移；同步成员 table_row，再紧缩成员。
    /// 移动只转移活值不析构；合并缓冲，提交认领/发布长度并清待整理项；稀疏不参与。
    /// 返回每个位置发生改变实体的最终记录（去重，含 Table 行和成员行），core 在任何业务访问前全部应用。
    /// core 应用记录不分配，不改变当前分区；未完成交接或任意搬移后 panic 必须标停，不重试旧动作。
    /// 普通 Err 仅发生在首次搬移前，物理和逻辑均保持不变；幂等调用允许只合并缓冲，返回空记录。
    #[cfg(feature = "rc")]
    pub fn settle(&mut self) -> Result<Vec<SettledLocation>, pi_ecs_types::error::EcsError> {
        self.settle_inner()
    }
    /// # 用途
    /// table 为同源编号；返回连续已发布前缀尾后界，不含认领未发布行；未知为 NotFound。
    /// 只提供元数据，不授予组件访问；同一次无结构冲突的读取期内使用。
    pub fn published_rows(&self, table: u16) -> Result<u32, pi_ecs_types::error::EcsError> {
        self.published_rows_inner(table)
    }
    /// # 用途
    /// table/row 的 Live 身份副本；未知、未发布、越界或洞 None，不读组件字节。
    /// 结构修改后必须重新定位，不以此副本授予逻辑权限。
    pub fn row_entity(&self, table: u16, row: TableRow) -> Option<pi_ecs_types::entity::Entity> {
        self.row_entity_inner(table, row)
    }
    /// # 用途
    /// id 为同源原型，返回成员尾后界（含洞），未知为 NotFound，不用容量代替有效范围。
    pub fn member_len(
        &self,
        id: pi_ecs_types::entity::ArchetypeId,
    ) -> Result<u32, pi_ecs_types::error::EcsError> {
        self.member_len_inner(id)
    }
    /// # 用途
    /// id/row 的完整成员与 Table 行副本；未知、越界或洞 None，不授予值权限。
    /// 调用方排除冲突成员修改；结构操作后旧副本失效，必须重新核验位置。
    pub fn member(
        &self,
        id: pi_ecs_types::entity::ArchetypeId,
        row: u32,
    ) -> Option<MemberLocation> {
        self.member_inner(id, row)
    }
    /// tables 已按布局编号排列，sparse 仅普通稀疏组件有实例，archetypes 与 tables 同长。
    /// 各数组使用同一 World 的编号空间；本入口只组装物理所有权，不读取逻辑注册或业务析构能力。
    /// 失败清理未安装的空存储，调用方已有 World 不变；不得传入仍含业务活值的半状态。
    pub fn new(
        tables: Box<[Table]>,
        sparse: Box<[Option<SparseSetStorage>]>,
    ) -> Result<Self, pi_ecs_types::error::EcsError> {
        Self::world_new_inner(tables, sparse)
    }
    /// 按 Table 编号取得只读物理表；越界返回 None。
    pub fn table(&self, table: u16) -> Option<&Table> {
        self.tables.get(table as usize)
    }
    /// 按 Table 编号取得可变物理表；越界返回 None。
    #[cfg(not(feature = "rc"))]
    pub fn table_mut(&mut self, table: u16) -> Option<&mut Table> {
        self.tables.get_mut(table as usize)
    }
    /// 按组件编号取得只读稀疏后端；未登记或编号越界返回 None。
    pub fn sparse(&self, component: ComponentId) -> Option<&SparseSetStorage> {
        Self::sparse_inner(self, component)
    }
    /// 按组件编号取得可变稀疏后端；未登记或编号越界返回 None。
    #[cfg(not(feature = "rc"))]
    pub fn sparse_mut(&mut self, component: ComponentId) -> Option<&mut SparseSetStorage> {
        Self::sparse_mut_inner(self, component)
    }
    /// 在目标原型尾部追加成员并返回成员行号；调用前该行组件值已写入。
    /// 成员行严格小于 2^20，与 EntityLocation 编码一致；尾后界可等于 2^20，此时拒绝继续追加。
    /// 容量/编号错误在成员发布前返回且成员目录不变；调用方须在移交组件值前预检相同容量，
    /// 若进入不可回滚的搬值阶段后仍出现该错误，沿结构失败标停，不伪造位置或丢失值责任。
    /// 不复用成员洞；Alter 先遍历已有目标再遍历源，新增目标不加入当前成员借用/游标的固定原型列表，下次 ParamSet 重绑按已发布结构重建。
    pub fn append_member(
        &mut self,
        id: pi_ecs_types::entity::ArchetypeId,
        entity: pi_ecs_types::entity::Entity,
        table_row: TableRow,
    ) -> Result<u32, pi_ecs_types::error::EcsError> {
        Self::append_member_inner(self, id, entity, table_row)
    }
    /// 移除目标原型的成员行；洞由后续整理处理。
    pub fn remove_member(&mut self, id: pi_ecs_types::entity::ArchetypeId, row: u32) {
        Self::remove_member_inner(self, id, row)
    }
    /// id 已由 core 完成逻辑登记，且绑定本 World 既有 Table；只准备空物理成员列表。
    /// 重复同号请求幂等，容量失败保持已有成员，不推导或修改组件集合。
    pub fn prepare_archetype(
        &mut self,
        id: pi_ecs_types::entity::ArchetypeId,
    ) -> Result<(), pi_ecs_types::error::EcsError> {
        Self::prepare_archetype_inner(self, id)
    }
}

#[cfg(feature = "rc")]
impl WorldStorage {
    /// # 用途
    /// 维护 table 的行和列；返回受限借用而非可替换 Table 的 &mut，不提供 DerefMut/解包。
    /// 越界 None；self 独占期排除所有运行视图和组件结果，缓存裸地址不构成活引用。
    pub fn table_mut(&mut self, table: u16) -> Option<TableMaintenance<'_>> {
        self.table_maintenance_inner(table)
    }
}
/// # 用途
/// RC 固定 Table 的独占维护能力；只提供行/列操作，不提供替换 Table 或其列数组的能力。
#[cfg(feature = "rc")]
pub struct TableMaintenance<'a> {
    /// 固定 Table 私有借用；不能穿过公开签名逃逸，调用结束归还原 WorldStorage。
    pub(crate) table: &'a mut Table,
}
#[cfg(feature = "rc")]
impl TableMaintenance<'_> {
    /// # 用途
    /// 认领空行并准备各列，失败沿 Table::claim_row，不返回业务组件引用。
    pub fn claim_row(&mut self) -> Result<TableRow, pi_ecs_types::error::EcsError> {
        self.table.claim_row()
    }
    /// # 用途
    /// 发布已完整初始化行，entity 和 row 满足 Table 发布顺序；错误不补发未完成行。
    pub fn publish_row(
        &mut self,
        row: TableRow,
        entity: pi_ecs_types::entity::Entity,
    ) -> Result<(), pi_ecs_types::error::EcsError> {
        self.table.publish_row(row, entity)
    }
    /// # 用途
    /// 退休 row，不移动其他行；调用方已交接业务值责任并结束该行引用。
    pub fn retire_row(&mut self, row: TableRow) {
        self.table.retire_row(row)
    }
    /// # 用途
    /// column 为冻结列号；返回不可替换列的维护能力，越界 None。
    pub fn column_mut(
        &mut self,
        column: crate::table::ColumnId,
    ) -> Option<crate::column::ColumnMaintenance<'_>> {
        self.table.column_mut(column)
    }
}

#[cfg(feature = "rc")]
impl WorldStorage {
    /// # 用途
    /// 仅关闭时扫描真实非 null dense owner，核验双向映射并撤销一份责任；prepare 空槽跳过。
    /// 返回组件编号及唯一拥有值地址；取尽 None，损坏 panic，不分配、不析构、不缩容或替换固定 Rc。
    /// # Safety
    /// World 已 Closing/Stopped，全部借用/视图结束且局部移出责任已收束，之后不恢复普通存储操作。
    /// 调用者逐份接管返回值（ZST 不按地址去重），不得覆盖/增长来源；源 storage 分配保活至 WholeState 释放完成。
    pub unsafe fn take_one_sparse_for_shutdown_raw(
        &mut self,
    ) -> Option<(ComponentId, std::ptr::NonNull<u8>)> {
        unsafe { self.take_one_sparse_for_shutdown_raw_inner() }
    }
    /// # 用途
    /// component 为已冻结编号；导出同一稳定稀疏分配体的非拥有地址，缺失 None，不授予值访问。
    /// 地址仅在原 WorldStorage 存活期有效，不从临时对象引用派生、不缓存值缓冲地址。
    pub fn sparse_address(
        &self,
        component: ComponentId,
    ) -> Option<std::ptr::NonNull<SparseSetStorage>> {
        self.sparse_address_inner(component)
    }
    /// # 用途
    /// 为 entity 在 component 后端准备槽；未登记返回错误，不隐式创建后端，失败语义沿 SparseSetStorage::prepare。
    /// 独占结构操作排除所有组件结果和运行视图，不替换容器，不新增独立维护阶段。
    pub fn prepare_sparse(
        &mut self,
        component: ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) -> Result<(), pi_ecs_types::error::EcsError> {
        self.prepare_sparse_inner(component, entity)
    }
    /// # 用途
    /// 将 value 安装到 component 的已准备空槽；后端必须存在，拥有责任与失败语义沿 insert。
    /// 调用方已结束相关借用，完整结构发布由 core 承担，不替换整个稀疏容器。
    pub fn insert_sparse(
        &mut self,
        component: ComponentId,
        entity: pi_ecs_types::entity::Entity,
        value: crate::ptr::OwningPtr<'_>,
    ) {
        self.insert_sparse_inner(component, entity, value)
    }
    /// # 用途
    /// 移出 component/entity 的值责任，未登记或缺值 None；结果借用结构操作期，不析构移出值。
    pub fn remove_sparse(
        &mut self,
        component: ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) -> Option<crate::ptr::OwningPtr<'_>> {
        self.remove_sparse_inner(component, entity)
    }
    /// # 用途
    /// 移入已准备空槽；component 必须登记且 source/layout 匹配，不释放源分配，不替换容器。
    /// # Safety
    /// 满足 SparseSetStorage::insert_raw 的全部类型、唯一责任和源失效前提，无活结果或视图。
    pub unsafe fn insert_sparse_raw(
        &mut self,
        component: ComponentId,
        entity: pi_ecs_types::entity::Entity,
        source: *mut u8,
        layout: std::alloc::Layout,
    ) {
        self.insert_sparse_raw_inner(component, entity, source, layout)
    }
    /// # 用途
    /// 移出 component/entity 的拥有值地址；缺失 None，消费前不得覆盖或增长源存储，不构造维护对象。
    /// # Safety
    /// 满足 SparseSetStorage::remove_raw 的唯一责任和地址消费前提，所有相关引用与视图已结束。
    pub unsafe fn remove_sparse_raw(
        &mut self,
        component: ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) -> Option<*mut u8> {
        self.remove_sparse_raw_inner(component, entity)
    }
}
