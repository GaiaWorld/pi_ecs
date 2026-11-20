//! Table 布局配对芯；只构造空物理布局与 Table。
#[cfg(all(test, feature = "rc"))]
mod capacity_tests {
    use crate::table::{Table, TableLayout};
    use pi_ecs_types::entity::ArchetypeId;
    #[test]
    fn claimed_column_slot_is_writable_before_row_publication() {
        use crate::{column::Column, table::ColumnId};
        use pi_ecs_types::{
            component::ComponentId,
            entity::{Entity, KeyData},
        };
        use std::{alloc::Layout, mem::MaybeUninit};
        let component = ComponentId(0);
        let layout = TableLayout::new(
            ArchetypeId {
                table: 0,
                variant: 0,
            },
            Box::new([component]),
        )
        .unwrap();
        let column = Column::new(component, Layout::new::<u64>()).unwrap();
        let mut table = Table::new(&layout, Box::new([column])).unwrap();
        let row = table.claim_row().unwrap();
        let column = ColumnId(0);
        let target = table.claimed_column_slot(row, column).unwrap();
        // SAFETY: the buffer was prepared for u64 and is not accessed elsewhere.
        unsafe {
            target.cast::<MaybeUninit<u64>>().as_mut().write(317);
        }
        let entity = Entity::from(KeyData::new(0, 1));
        table.publish_row(row, entity).unwrap();
        assert!(table.claimed_column_slot(row, column).is_none());
        // SAFETY: the row is published and its only column was initialized above.
        let mut storage_column = table.column_mut(column).unwrap();
        let pointer = unsafe { storage_column.get(row.index() as usize) };
        assert_eq!(
            unsafe { *pointer.as_non_null().cast::<u64>().as_ref() },
            317
        );
    }

    #[test]
    fn exhausted_claim_is_rejected_before_any_slot_change() {
        let layout = TableLayout::new(
            ArchetypeId {
                table: 0,
                variant: 0,
            },
            Box::new([]),
        )
        .unwrap();
        let mut table = Table::new(&layout, Box::new([])).unwrap();
        table.claimed_rows = u32::MAX;
        assert!(table.claim_row().is_err());
        assert_eq!(table.claimed_rows, u32::MAX);
        assert_eq!(table.rows.contiguous_len(), 0);
    }
}

use crate::table::{PendingTableRemovals, Table, TableLayout};
use pi_ecs_types::{component::ComponentId, entity::ArchetypeId, error::EcsError};
#[cfg(not(feature = "rc"))]
use std::sync::atomic::AtomicU32;
impl Table {
    pub(super) fn new_inner(
        layout: &TableLayout,
        columns: Box<[crate::column::Column]>,
    ) -> Result<Table, EcsError> {
        if columns.len() != layout.columns.len() {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "Table 列数量与布局不一致",
            ));
        }
        for (column, component) in columns.iter().zip(layout.columns.iter()) {
            if column.component != *component {
                return Err(pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::InvalidInput,
                    "Table 列组件与布局顺序不一致",
                ));
            }
        }
        #[cfg(feature = "rc")]
        let pending_removals = PendingTableRemovals { rows: Vec::new() };
        #[cfg(not(feature = "rc"))]
        let pending_removals = PendingTableRemovals {
            rows: Default::default(),
        };
        Ok(Table {
            id: layout.id,
            rows: pi_arr::Arr::new(),
            #[cfg(feature = "rc")]
            claimed_rows: 0,
            #[cfg(not(feature = "rc"))]
            claimed_rows: AtomicU32::new(0),
            #[cfg(feature = "rc")]
            published_rows: 0,
            #[cfg(not(feature = "rc"))]
            published_rows: AtomicU32::new(0),
            #[cfg(not(feature = "rc"))]
            columns,
            #[cfg(feature = "rc")]
            columns: columns
                .into_vec()
                .into_iter()
                .map(|value| std::rc::Rc::new(std::cell::UnsafeCell::new(value)))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            pending_removals,
        })
    }
}
impl TableLayout {
    pub(super) fn layout_new_inner(
        id: ArchetypeId,
        columns: Box<[ComponentId]>,
    ) -> Result<TableLayout, EcsError> {
        if id.variant != 0 {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "TableLayout 必须绑定基础原型 variant=0",
            ));
        }
        if !columns.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "TableLayout 列必须升序且无重复",
            ));
        }
        Ok(TableLayout { id, columns })
    }
}
impl TableLayout {
    pub(super) fn layout_id_inner(layout: &TableLayout) -> ArchetypeId {
        layout.id
    }
}
impl TableLayout {
    pub(super) fn layout_columns_inner(layout: &TableLayout) -> &[ComponentId] {
        &layout.columns
    }
}

#[cfg(feature = "rc")]
impl Table {
    /// Returns a raw, untyped destination in a claimed, unpublished row.
    /// The address is invalidated by table maintenance. It must only be written
    /// by the unique initializer and never read before initialization.
    pub fn claimed_column_slot(
        &mut self,
        row: crate::table::TableRow,
        column: crate::table::ColumnId,
    ) -> Option<std::ptr::NonNull<u8>> {
        if !(self.published_rows..self.claimed_rows).contains(&row.index()) {
            return None;
        }
        let column = self.columns.get(column.0 as usize)?;
        // SAFETY: the exclusive Table borrow excludes column maintenance and
        // live views. Only a raw destination address is returned.
        let column = unsafe { &*column.get() };
        std::ptr::NonNull::new(column.buffer.get(row.index() as usize)?)
    }

    pub(super) fn claim_row_inner(table: &mut Table) -> Result<crate::table::TableRow, EcsError> {
        let row = table.claimed_rows;
        let next = row.checked_add(1).ok_or_else(|| {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "Table 行容量耗尽",
            )
        })?;
        for slot in &table.columns {
            // SAFETY: 独占 Table 维护，无运行视图；仅增长列内部缓冲。
            unsafe { &mut *slot.get() }.prepare(row as usize)?;
        }
        table.rows.alloc(row as usize);
        table.claimed_rows = next;
        Ok(crate::table::TableRow::new(row))
    }
}
#[cfg(not(feature = "rc"))]
impl Table {
    pub(super) fn claim_row_inner(_table: &mut Table) -> Result<crate::table::TableRow, EcsError> {
        todo!("编号：T126，尚未实现， 非 rc 行认领")
    }
}

#[cfg(feature = "rc")]
impl Table {
    pub(super) fn publish_row_inner(
        table: &mut Table,
        row: crate::table::TableRow,
        entity: pi_ecs_types::entity::Entity,
    ) -> Result<(), EcsError> {
        let index = row.index() as usize;
        if table.published_rows as usize != index {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::Internal,
                "Table 行必须按认领顺序连续发布",
            ));
        }
        if row.index() >= table.claimed_rows {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::Internal,
                "发布行超出已认领范围",
            ));
        }
        let Some(cell) = table.rows.get_mut(index) else {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::Internal,
                "发布行超出已准备范围",
            ));
        };
        *cell = crate::table::RowCell(crate::table::RowState::Live(entity));
        table.published_rows += 1;
        Ok(())
    }
}
#[cfg(not(feature = "rc"))]
impl Table {
    pub(super) fn publish_row_inner(
        _table: &mut Table,
        _row: crate::table::TableRow,
        _entity: pi_ecs_types::entity::Entity,
    ) -> Result<(), EcsError> {
        todo!("编号：T127，尚未实现， 非 rc 行发布")
    }
}

#[cfg(feature = "rc")]
impl Table {
    pub(super) fn retire_row_inner(table: &mut Table, row: crate::table::TableRow) {
        let index = row.index() as usize;
        if let Some(cell) = table.rows.get_mut(index) {
            *cell = Default::default();
        }
        table.pending_removals.rows.push(row);
    }
}
#[cfg(not(feature = "rc"))]
impl Table {
    pub(super) fn retire_row_inner(_table: &mut Table, _row: crate::table::TableRow) {
        todo!("编号：T128，尚未实现， 非 rc 行退休")
    }
}

#[cfg(feature = "rc")]
impl Table {
    pub(super) fn column_maintenance_inner(
        &mut self,
        column: crate::table::ColumnId,
    ) -> Option<crate::column::ColumnMaintenance<'_>> {
        self.columns
            .get(column.0 as usize)
            .map(|slot| crate::column::ColumnMaintenance {
                slot: slot.as_ref(),
            })
    }
}

#[cfg(feature = "rc")]
impl Table {
    pub(super) fn column_address_inner(
        &self,
        column: crate::table::ColumnId,
    ) -> Option<std::ptr::NonNull<crate::column::Column>> {
        let slot = self.columns.get(column.0 as usize)?;
        // SAFETY: Rc 保活固定分配体，UnsafeCell 根指针不经临时可变引用派生。
        Some(unsafe { std::ptr::NonNull::new_unchecked((*std::rc::Rc::as_ptr(slot)).get()) })
    }
}
