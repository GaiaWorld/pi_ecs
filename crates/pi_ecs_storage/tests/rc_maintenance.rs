#![cfg(feature = "rc")]
use pi_ecs_storage::{
    storage::WorldStorage,
    table::{Table, TableLayout, TableRow},
};
use pi_ecs_types::entity::{ArchetypeId, Entity, KeyData};
// Seam: 空表、多洞、全退休及成员独立于 Table 的最终交接。
#[test]
fn settle_covers_every_hole_shape_and_member_only_moves() {
    for mask in 0u32..32 {
        let mut world = empty();
        // 反序成员使成员移动与 Table 移动不能相互替代。
        for index in 0..5 {
            let row = world
                .table_mut(pi_ecs_types::entity::TableId(0))
                .unwrap()
                .claim_row()
                .unwrap();
            world
                .table_mut(pi_ecs_types::entity::TableId(0))
                .unwrap()
                .publish_row(row, Entity::from(KeyData::new(index, 1)))
                .unwrap();
        }
        for index in (0..5).rev() {
            world
                .append_member(
                    id(),
                    Entity::from(KeyData::new(index, 1)),
                    TableRow::new(index),
                )
                .unwrap();
        }
        for index in 0..5 {
            if mask & (1 << index) != 0 {
                world
                    .table_mut(pi_ecs_types::entity::TableId(0))
                    .unwrap()
                    .retire_row(TableRow::new(index));
                world.remove_member(id(), 4 - index);
            }
        }
        let records = world.settle().unwrap();
        let len = 5 - mask.count_ones();
        assert_eq!(
            world
                .published_rows(pi_ecs_types::entity::TableId(0))
                .unwrap(),
            len
        );
        assert_eq!(world.member_len(id()).unwrap(), len);
        let mut seen = Vec::new();
        for row in 0..len {
            let member = world.member(id(), row).unwrap();
            assert_eq!(
                world.row_entity(pi_ecs_types::entity::TableId(0), member.table_row),
                Some(member.entity)
            );
            assert!(!seen.contains(&member.entity));
            seen.push(member.entity);
            let changed = records
                .iter()
                .filter(|record| record.entity == member.entity)
                .collect::<Vec<_>>();
            assert!(changed.len() <= 1);
            if let Some(record) = changed.first() {
                assert_eq!(
                    (record.archetype_row, record.table_row),
                    (row, member.table_row)
                );
            }
        }
        assert!(world.settle().unwrap().is_empty());
    }
}
// Seam: 发布只接受已认领行；拒绝未认领邻行不改变已发布范围和身份。
#[test]
fn publishing_unclaimed_adjacent_row_preserves_metadata_and_allows_later_claim() {
    let mut world = empty();
    let first_entity = Entity::from(KeyData::new(7, 3));
    let first = world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .claim_row()
        .unwrap();
    world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .publish_row(first, first_entity)
        .unwrap();
    let next = TableRow::new(1);
    let rejected_entity = Entity::from(KeyData::new(8, 3));
    assert!(world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .publish_row(next, rejected_entity)
        .is_err());
    assert_eq!(
        world
            .published_rows(pi_ecs_types::entity::TableId(0))
            .unwrap(),
        1
    );
    assert_eq!(
        world.row_entity(pi_ecs_types::entity::TableId(0), first),
        Some(first_entity)
    );
    assert_eq!(
        world.row_entity(pi_ecs_types::entity::TableId(0), next),
        None
    );

    let claimed = world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .claim_row()
        .unwrap();
    assert_eq!(claimed, next);
    let next_entity = Entity::from(KeyData::new(9, 3));
    world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .publish_row(claimed, next_entity)
        .unwrap();
    assert_eq!(
        world
            .published_rows(pi_ecs_types::entity::TableId(0))
            .unwrap(),
        2
    );
    assert_eq!(
        world.row_entity(pi_ecs_types::entity::TableId(0), first),
        Some(first_entity)
    );
    assert_eq!(
        world.row_entity(pi_ecs_types::entity::TableId(0), next),
        Some(next_entity)
    );
}
// Seam: 认领准备失败不发布；整理拒绝未发布行时不修改已有成员。
#[test]
fn failed_claim_and_unpublished_settle_leave_metadata_unchanged() {
    use pi_ecs_storage::column::Column;
    use pi_ecs_types::component::ComponentId;
    use std::alloc::Layout;
    let layout = TableLayout::new(id(), vec![ComponentId(0)].into_boxed_slice()).unwrap();
    let column = Column::new(
        ComponentId(0),
        Layout::from_size_align(isize::MAX as usize, 1).unwrap(),
    )
    .unwrap();
    let table = Table::new(&layout, vec![column].into_boxed_slice()).unwrap();
    let mut world = WorldStorage::new(vec![table].into_boxed_slice(), Box::new([])).unwrap();
    assert!(world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .claim_row()
        .is_err());
    assert_eq!(
        world
            .published_rows(pi_ecs_types::entity::TableId(0))
            .unwrap(),
        0
    );
    assert!(world.settle().unwrap().is_empty());
    let mut world = empty();
    let key = Entity::from(KeyData::new(9, 1));
    let first = world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .claim_row()
        .unwrap();
    world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .publish_row(first, key)
        .unwrap();
    world.append_member(id(), key, first).unwrap();
    let second = world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .claim_row()
        .unwrap();
    assert!(world.settle().is_err());
    assert_eq!(
        world
            .published_rows(pi_ecs_types::entity::TableId(0))
            .unwrap(),
        1
    );
    assert_eq!(world.member(id(), 0).unwrap().entity, key);
    world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .publish_row(second, Entity::from(KeyData::new(10, 1)))
        .unwrap();
}
// Seam: WorldStorage::apply_tracking 在所有存储准备成功后才切换开关。
#[test]
fn tracking_initializes_live_values_preserves_old_ticks_and_rolls_back_invalid_targets() {
    use pi_ecs_storage::{
        column::Column, sparse_set::SparseSetStorage, storage::TrackingTarget, table::ColumnId,
    };
    use pi_ecs_types::{change_detection::ChangeTick, component::ComponentId};
    use std::{alloc::Layout, num::NonZeroU64};
    let tick = |n| ChangeTick::new(NonZeroU64::new(n).unwrap());
    let layout = TableLayout::new(id(), vec![ComponentId(0)].into_boxed_slice()).unwrap();
    let table = Table::new(
        &layout,
        vec![Column::new(ComponentId(0), Layout::new::<u32>()).unwrap()].into_boxed_slice(),
    )
    .unwrap();
    let sparse = SparseSetStorage::sparse_array(ComponentId(0), Layout::new::<u32>()).unwrap();
    let mut world = WorldStorage::new(
        vec![table].into_boxed_slice(),
        vec![Some(sparse)].into_boxed_slice(),
    )
    .unwrap();
    let key = Entity::from(KeyData::new(4, 2));
    let row = world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .claim_row()
        .unwrap();
    let mut value = 31u32;
    unsafe {
        world
            .table_mut(pi_ecs_types::entity::TableId(0))
            .unwrap()
            .column_mut(ColumnId(0))
            .unwrap()
            .insert_raw(0, (&mut value as *mut u32).cast(), Layout::new::<u32>());
    }
    world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .publish_row(row, key)
        .unwrap();
    world.prepare_sparse(ComponentId(0), key).unwrap();
    unsafe {
        world.insert_sparse_raw(
            ComponentId(0),
            key,
            (&mut value as *mut u32).cast(),
            Layout::new::<u32>(),
        );
    }
    let target = |added, changed| TrackingTarget {
        component: ComponentId(0),
        added,
        changed,
    };
    world
        .apply_tracking(&[target(true, false)], tick(3))
        .unwrap();
    assert_eq!(
        world.sparse_ticks(ComponentId(0), key).unwrap().added,
        Some(tick(3))
    );
    assert!(world
        .apply_tracking(
            &[
                target(false, true),
                TrackingTarget {
                    component: ComponentId(7),
                    added: true,
                    changed: true
                }
            ],
            tick(5)
        )
        .is_err());
    assert_eq!(
        world.sparse_ticks(ComponentId(0), key).unwrap().added,
        Some(tick(3))
    );
    assert_eq!(
        world.sparse_ticks(ComponentId(0), key).unwrap().changed,
        None
    );
    world
        .apply_tracking(&[target(true, true)], tick(7))
        .unwrap();
    let pointer = world
        .table(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .column_address(ColumnId(0))
        .unwrap();
    let column_ticks = unsafe { pointer.as_ref().bind_view().ticks(0) };
    assert_eq!(
        (column_ticks.added, column_ticks.changed),
        (Some(tick(3)), Some(tick(7)))
    );
    let sparse_ticks = world.sparse_ticks(ComponentId(0), key).unwrap();
    assert_eq!(
        (sparse_ticks.added, sparse_ticks.changed),
        (Some(tick(3)), Some(tick(7)))
    );
    world
        .apply_tracking(&[target(false, false)], tick(9))
        .unwrap();
    assert!(world
        .sparse_ticks(ComponentId(0), key)
        .unwrap()
        .added
        .is_none());
    world
        .apply_tracking(&[target(true, false)], tick(11))
        .unwrap();
    assert_eq!(
        world.sparse_ticks(ComponentId(0), key).unwrap().added,
        Some(tick(11))
    );
}
// Seam: settle 交付唯一最终位置，并保留多列值/tick 的配对与单次析构责任。
#[test]
fn settle_moves_values_ticks_and_members_once_and_is_idempotent() {
    use pi_ecs_storage::{column::Column, storage::TrackingTarget, table::ColumnId};
    use pi_ecs_types::{change_detection::ChangeTick, component::ComponentId};
    use std::{alloc::Layout, cell::Cell, mem::ManuallyDrop, num::NonZeroU64, rc::Rc};
    struct Value(u32, Rc<Cell<usize>>);
    impl Drop for Value {
        fn drop(&mut self) {
            self.1.set(self.1.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let layout = TableLayout::new(
        id(),
        vec![ComponentId(0), ComponentId(1)].into_boxed_slice(),
    )
    .unwrap();
    let table = Table::new(
        &layout,
        vec![
            Column::new(ComponentId(0), Layout::new::<Value>()).unwrap(),
            Column::new(ComponentId(1), Layout::new::<u32>()).unwrap(),
        ]
        .into_boxed_slice(),
    )
    .unwrap();
    let mut world = WorldStorage::new(vec![table].into_boxed_slice(), Box::new([])).unwrap();
    assert!(world.settle().unwrap().is_empty());
    let variant = ArchetypeId {
        table: pi_ecs_types::entity::TableId(0),
        variant: pi_ecs_types::entity::VariantId(1),
    };
    world.prepare_archetype(variant).unwrap();
    let tick = |n| ChangeTick::new(NonZeroU64::new(n).unwrap());
    world
        .apply_tracking(
            &[TrackingTarget {
                component: ComponentId(0),
                added: true,
                changed: true,
            }],
            tick(1),
        )
        .unwrap();
    for index in 0..4 {
        let key = Entity::from(KeyData::new(index, 2));
        let row = world
            .table_mut(pi_ecs_types::entity::TableId(0))
            .unwrap()
            .claim_row()
            .unwrap();
        let mut value = ManuallyDrop::new(Value(10 + index, drops.clone()));
        let mut other = 100 + index;
        let mut table = world.table_mut(pi_ecs_types::entity::TableId(0)).unwrap();
        unsafe {
            table.column_mut(ColumnId(0)).unwrap().insert_raw(
                index as usize,
                (&mut *value as *mut Value).cast(),
                Layout::new::<Value>(),
            );
        }
        table.column_mut(ColumnId(0)).unwrap().set_ticks(
            index as usize,
            Some(tick(2 + index as u64)),
            Some(tick(8 + index as u64)),
        );
        unsafe {
            table.column_mut(ColumnId(1)).unwrap().insert_raw(
                index as usize,
                (&mut other as *mut u32).cast(),
                Layout::new::<u32>(),
            );
        }
        table.publish_row(row, key).unwrap();
        world
            .append_member(if index == 0 { id() } else { variant }, key, row)
            .unwrap();
    }
    // 移出中间行并清偿一次，成员洞和 Table 洞并不处于相同行号。
    unsafe {
        let mut table = world.table_mut(pi_ecs_types::entity::TableId(0)).unwrap();
        let pointer = table.column_mut(ColumnId(0)).unwrap().remove_raw(1);
        std::ptr::drop_in_place(pointer.cast::<Value>());
    }
    world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .retire_row(TableRow::new(1));
    world.remove_member(variant, 0);
    let records = world.settle().unwrap();
    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.entity, Entity::from(KeyData::new(3, 2)));
    assert_eq!(
        (
            record.archetype,
            record.table_row.index(),
            record.archetype_row
        ),
        (variant, 1, 0)
    );
    assert_eq!(world.member(variant, 0).unwrap().table_row.index(), 1);
    assert_eq!(world.member_len(variant).unwrap(), 2);
    assert_eq!(
        world
            .published_rows(pi_ecs_types::entity::TableId(0))
            .unwrap(),
        3
    );
    assert!(world
        .row_entity(pi_ecs_types::entity::TableId(0), TableRow::new(3))
        .is_none());
    let pointer = world
        .table(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .column_address(ColumnId(0))
        .unwrap();
    let snapshot = unsafe { pointer.as_ref().bind_view().ticks(1) };
    assert_eq!(
        (snapshot.added, snapshot.changed),
        (Some(tick(5)), Some(tick(11)))
    );
    assert!(world.settle().unwrap().is_empty());
    assert_eq!(drops.get(), 1);
    for row in 0..3 {
        unsafe {
            let mut table = world.table_mut(pi_ecs_types::entity::TableId(0)).unwrap();
            let pointer = table.column_mut(ColumnId(0)).unwrap().remove_raw(row);
            assert_eq!((*pointer.cast::<Value>()).0, [10, 13, 12][row]);
            std::ptr::drop_in_place(pointer.cast::<Value>());
            let other = table
                .column_mut(ColumnId(1))
                .unwrap()
                .remove_raw(row)
                .cast::<u32>()
                .read();
            assert_eq!(other, [100, 103, 102][row]);
        }
        world
            .table_mut(pi_ecs_types::entity::TableId(0))
            .unwrap()
            .retire_row(TableRow::new(row as u32));
    }
    drop(world);
    assert_eq!(drops.get(), 4);
}
fn id() -> ArchetypeId {
    ArchetypeId {
        table: pi_ecs_types::entity::TableId(0),
        variant: pi_ecs_types::entity::VariantId(0),
    }
}
fn empty() -> WorldStorage {
    let layout = TableLayout::new(id(), vec![].into_boxed_slice()).unwrap();
    WorldStorage::new(
        vec![Table::new(&layout, vec![].into_boxed_slice()).unwrap()].into_boxed_slice(),
        vec![].into_boxed_slice(),
    )
    .unwrap()
}
// Seam: 固定容器地址跨维护增长和目录移动仍同源。
#[test]
fn stable_addresses_survive_growth_and_handle_moves() {
    use pi_ecs_storage::{column::Column, sparse_set::SparseSetStorage, table::ColumnId};
    use pi_ecs_types::component::ComponentId;
    use std::alloc::Layout;
    let layout = TableLayout::new(id(), vec![ComponentId(0)].into_boxed_slice()).unwrap();
    let table = Table::new(
        &layout,
        vec![Column::new(ComponentId(0), Layout::new::<u32>()).unwrap()].into_boxed_slice(),
    )
    .unwrap();
    let column = table.column_address(ColumnId(0)).unwrap();
    assert!(table.column_address(ColumnId(1)).is_none());
    let sparse = SparseSetStorage::sparse_array(ComponentId(1), Layout::new::<u32>()).unwrap();
    let mut world = WorldStorage::new(
        vec![table].into_boxed_slice(),
        vec![None, Some(sparse)].into_boxed_slice(),
    )
    .unwrap();
    let address = world.sparse_address(ComponentId(1)).unwrap();
    assert!(world.sparse_address(ComponentId(0)).is_none());
    for index in 0..10 {
        world
            .table_mut(pi_ecs_types::entity::TableId(0))
            .unwrap()
            .claim_row()
            .unwrap();
        world
            .prepare_sparse(ComponentId(1), Entity::from(KeyData::new(index, 1)))
            .unwrap();
    }
    let moved = Box::new(world);
    assert_eq!(
        moved
            .table(pi_ecs_types::entity::TableId(0))
            .unwrap()
            .column_address(ColumnId(0)),
        Some(column)
    );
    assert_eq!(moved.sparse_address(ComponentId(1)), Some(address));
}
// Seam: TableMaintenance 行认领/发布和 WorldStorage 元数据副本。
#[test]
fn metadata_distinguishes_claimed_published_retired_and_member_holes() {
    let mut world = empty();
    assert_eq!(
        world
            .published_rows(pi_ecs_types::entity::TableId(0))
            .unwrap(),
        0
    );
    assert!(world
        .published_rows(pi_ecs_types::entity::TableId(1))
        .is_err());
    assert!(world
        .member_len(ArchetypeId {
            table: pi_ecs_types::entity::TableId(0),
            variant: pi_ecs_types::entity::VariantId(1)
        })
        .is_err());
    let key = Entity::from(KeyData::new(7, 3));
    let row = world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .claim_row()
        .unwrap();
    assert_eq!(row.index(), 0);
    assert_eq!(
        world
            .published_rows(pi_ecs_types::entity::TableId(0))
            .unwrap(),
        0
    );
    assert!(world
        .row_entity(pi_ecs_types::entity::TableId(0), row)
        .is_none());
    world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .publish_row(row, key)
        .unwrap();
    assert_eq!(
        world.row_entity(pi_ecs_types::entity::TableId(0), row),
        Some(key)
    );
    assert_eq!(
        world
            .published_rows(pi_ecs_types::entity::TableId(0))
            .unwrap(),
        1
    );
    let member = world.append_member(id(), key, row).unwrap();
    assert_eq!(member, 0);
    assert_eq!(world.member_len(id()).unwrap(), 1);
    let copy = world.member(id(), member).unwrap();
    assert_eq!((copy.entity, copy.table_row), (key, row));
    world.remove_member(id(), member);
    world
        .table_mut(pi_ecs_types::entity::TableId(0))
        .unwrap()
        .retire_row(row);
    assert!(world.member(id(), member).is_none());
    assert!(world
        .row_entity(pi_ecs_types::entity::TableId(0), row)
        .is_none());
    assert!(world
        .row_entity(pi_ecs_types::entity::TableId(0), TableRow::new(1))
        .is_none());
    assert_eq!(world.member_len(id()).unwrap(), 1);
    assert_eq!(
        world
            .published_rows(pi_ecs_types::entity::TableId(0))
            .unwrap(),
        1
    );
}
