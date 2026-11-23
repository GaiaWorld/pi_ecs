//! 实体出生分区位段的配对芯；只做位段读写，不改变本地槽或代际。
use super::{Entity, TableId, VariantId, DOMAIN_MASK, DOMAIN_SHIFT, LOCAL_SLOT_MASK};
use pi_key::Key;
use pi_null::Null;

macro_rules! id_ops {
    ($id:ident) => {
        impl Null for $id {
            fn null() -> Self {
                Self(u16::MAX)
            }
            fn is_null(&self) -> bool {
                self.0 == u16::MAX
            }
        }

        impl From<$id> for usize {
            fn from(id: $id) -> Self {
                usize::from(id.0)
            }
        }
    };
}
id_ops!(TableId);
id_ops!(VariantId);

/// 返回实体索引高位的出生分区编号；只读，不改变身份。
pub fn domain_bits(entity: &Entity) -> u32 {
    (entity.data().index() >> DOMAIN_SHIFT) & DOMAIN_MASK
}

/// 返回出生分区内的本地槽索引；出生分区不参与槽内寻址。
pub fn slot_index(entity: &Entity) -> u32 {
    entity.data().index() & LOCAL_SLOT_MASK
}

/// 返回替换出生分区位后的完整键；本地槽与完整代际保持不变。
/// domain 必须不超过 DOMAIN_MASK；越界直接 panic，不产生半更新键。
pub fn with_domain_bits(entity: Entity, domain: u32) -> Entity {
    assert!(domain <= DOMAIN_MASK, "出生分区超出 DOMAIN_BITS");
    let data = entity.data();
    let index = (domain << DOMAIN_SHIFT) | (data.index() & LOCAL_SLOT_MASK);
    Entity::from(pi_key::KeyData::new(index, data.version()))
}

const ARCHETYPE_ROW_BITS: u32 = 20;
const ARCHETYPE_ROW_MASK: u32 = (1 << ARCHETYPE_ROW_BITS) - 1;
const PARTITION_MASK: u32 = (1 << super::PARTITION_BITS) - 1;

/// 当前分区；只供分区系统读取，查询/调度不解释。
impl super::EntityLocation {
    pub(super) fn partition_inner(&self) -> super::PartitionId {
        super::PartitionId(
            (self.partition_and_archetype_row >> ARCHETYPE_ROW_BITS) & PARTITION_MASK,
        )
    }

    /// 当前原型编号；由位置高位段与 Table/variant 字段解码。
    pub(super) fn archetype_inner(&self) -> super::ArchetypeId {
        super::ArchetypeId {
            table: TableId((self.table_and_variant >> 16) as u16),
            variant: VariantId(self.table_and_variant as u16),
        }
    }

    /// 原型成员行。
    pub(super) fn archetype_row_inner(&self) -> super::ArchetypeRow {
        super::ArchetypeRow(self.partition_and_archetype_row & ARCHETYPE_ROW_MASK)
    }

    /// Table 物理行。
    pub(super) fn table_row_inner(&self) -> u32 {
        self.table_row
    }

    /// 由已校验字段构造位置；越界 panic。
    pub(super) fn from_parts_inner(
        partition: u32,
        archetype_row: u32,
        table_row: u32,
        table: TableId,
        variant: VariantId,
    ) -> super::EntityLocation {
        assert!(partition <= PARTITION_MASK, "当前分区超出 PARTITION_BITS");
        assert!(archetype_row <= ARCHETYPE_ROW_MASK, "原型成员行超出 20 位");
        assert!(
            !table.is_null() && !variant.is_null(),
            "活位置不可使用空 Table 或变体编号"
        );
        super::EntityLocation {
            partition_and_archetype_row: (partition << ARCHETYPE_ROW_BITS) | archetype_row,
            table_row,
            table_and_variant: (u32::from(table.0) << 16) | u32::from(variant.0),
        }
    }

    pub(super) fn is_null_inner(&self) -> bool {
        *self == Self::NULL
    }
}
