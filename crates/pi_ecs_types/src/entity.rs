//! World 内实体身份、出生分区与当前分区位置编码。
#[path = "entity_inner.rs"]
mod entity_inner;

pub use entity_inner::{domain_bits, slot_index, with_domain_bits};
pub use pi_key::{Key, KeyData};
pi_key::new_key_type! {
    /// 完整代际身份；索引高位为出生分区，低位为分区内槽，迁区不改身份。
    pub struct Entity;
}
/// 出生分区在实体索引中的位宽；与当前分区共用同一分区数量上限。
pub const DOMAIN_BITS: u32 = 12;
/// 出生分区位掩码；出生分区编号必须落在该范围内。
pub const DOMAIN_MASK: u32 = (1 << DOMAIN_BITS) - 1;
/// 出生分区数量上限；与 MAX_PARTITIONS 相等。
pub const MAX_DOMAINS: usize = 1 << DOMAIN_BITS;
/// 实体索引中本地槽的位宽；出生分区之外的全部索引位。
pub const LOCAL_SLOT_BITS: u32 = 20;
/// 出生分区在实体索引中的移位量。
pub const DOMAIN_SHIFT: u32 = LOCAL_SLOT_BITS;
/// 本地槽位掩码；只在所属出生分区内寻址。
pub const LOCAL_SLOT_MASK: u32 = (1u32 << LOCAL_SLOT_BITS) - 1;
/// 当前分区字段位宽；EntityLocation 使用该编号空间过滤/加速。
/// 与出生分区共用数量；EntityLocation 高位段仅使用低 PARTITION_BITS 位。
pub const PARTITION_BITS: u32 = DOMAIN_BITS;
/// 分区数量上限；构造后固定，合法编号为 0..partition_count。
pub const MAX_PARTITIONS: usize = 1 << PARTITION_BITS;
/// 出生分区；实体键分配时确定，迁区不改。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct AllocationDomain(
    /// 出生分区编号；写入实体索引高 DOMAIN_BITS 位，必须不超过 DOMAIN_MASK。
    pub u32,
);
/// 当前分区使用与出生分区相同的编号空间；不保留额外哨兵值。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct PartitionId(
    /// 当前 World 中的当前分区编号；须小于已配置分区数量。
    pub u32,
);
/// 业务身份类别，不改变所属 World；首版不写入 Entity 键，恒按 0 处理。
/// 保留未来按类别隔离查询/恢复的能力，当前不参与出生分区/槽/当前分区寻址。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct EntityCategory(
    /// 类别编号；首版唯一合法值为 0，其他值暂不产生实体键。
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
/// 三个 u32；高 12 位段保存当前分区与原型成员行，不编码出生分区。
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityLocation {
    /// 高位段中低 PARTITION_BITS 位为当前分区，低 20 位为原型成员行；其余高位保留 0。
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
impl ArchetypeRow {
    /// 返回成员行号。
    pub fn index(self) -> u32 {
        self.0
    }
}
impl EntityLocation {
    /// # 用途
    /// 规范空位置，三个字均为 u32::MAX；仅保留这一完整组合，不单独占用分区/原型/行编号。
    /// 不能用于索引存储，读取分区/行/原型前必须排除 Null；全零仍是合法位置。
    pub const NULL: Self = Self {
        partition_and_archetype_row: u32::MAX,
        table_row: u32::MAX,
        table_and_variant: u32::MAX,
    };
    /// # 用途
    /// 仅规范 NULL 返回 true，不访问 World，不以单个字段的最大值判断无位置。
    pub fn is_null(&self) -> bool {
        self.is_null_inner()
    }
    /// 由已校验的当前分区、成员行、Table 行与原型编号构造位置。
    /// 越界或全部字段组成规范 NULL 时直接 panic；空位置只能用 NULL。
    pub fn from_parts(
        partition: u32,
        archetype_row: u32,
        table_row: u32,
        table: u16,
        variant: u16,
    ) -> Self {
        Self::from_parts_inner(partition, archetype_row, table_row, table, variant)
    }
    /// 当前分区；仅供分区系统使用，Query/Stage 不读取。
    pub fn partition(&self) -> PartitionId {
        self.partition_inner()
    }
    /// 当前原型编号。
    pub fn archetype(&self) -> ArchetypeId {
        self.archetype_inner()
    }
    /// 原型成员行。
    pub fn archetype_row(&self) -> ArchetypeRow {
        self.archetype_row_inner()
    }
    /// Table 物理行。
    pub fn table_row(&self) -> u32 {
        self.table_row_inner()
    }
}
// 规范空位置使用 NULL，全零仍合法；解码方法调用前须排除空位置。
// 出生分区位解码/重标记逻辑在配对芯 entity_inner.rs，壳只重导出；不在此处做位运算。
