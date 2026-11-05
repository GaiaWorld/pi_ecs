//! World 内实体身份、分配来源与三字位置编码。
pub use pi_key::Key;
pi_key::new_key_type! {
    /// 完整代际身份；来源/类别编码待闭合，迁区不改身份。
    pub struct Entity;
}
/// 确定性身份分配来源，不等于当前分区。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct AllocationDomain(
    /// 来源编号及扩展位编码待定。
    pub u32,
);
/// 当前分区使用12位编码，并须小于已配置数量；最大编码是否保留另定。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct PartitionId(
    /// 当前 World 分区编号；12位编码，实际须小于已配置分区数量。
    pub u32,
);
/// 业务身份类别，不改变所属 World。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct EntityCategory(
    /// 类型编码待定。
    pub u32,
);
/// 本 World 的二维原型身份，恢复后不能直接复用旧布局计划。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ArchetypeId {
    /// World 内 Table 编号，16 位。
    pub table: u16,
    /// Table 内变体编号，0 为基础原型，16 位。
    pub variant: u16,
}
/// 三个 u32；分区位宽固定，不编码全局/本地恢复域。
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityLocation {
    /// 高12位分区，低20位原型成员行；无本地域哨兵。
    pub(crate) partition_and_archetype_row: u32,
    /// Table 物理行，32位；长度计数边界不得溢出。
    pub(crate) table_row: u32,
    /// 高16位 Table ID，低16位 variant ID，与当前 World 的布局代次对应。
    pub(crate) table_and_variant: u32,
}
/// 原型成员行，20位编码容量。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchetypeRow(
    /// 行号必须小于 2^20。
    pub(crate) u32,
);
// @to_design 校验构造/解码、身份无位置状态和长度计数极限待闭合，不能以全零位置表示不存在。
