//! 全局空存储构造验收；不创建业务值。
use pi_ecs_storage::column::Column;
use pi_ecs_storage::sparse_set::SparseSetStorage;
use pi_ecs_storage::storage::WorldStorage;
use pi_ecs_storage::table::{Table, TableLayout};
use pi_ecs_types::component::ComponentId;
use pi_ecs_types::entity::ArchetypeId;
use std::alloc::Layout;

#[test]
fn empty_global_storage_constructs() {
    let layout = TableLayout::new(
        ArchetypeId {
            table: 0,
            variant: 0,
        },
        vec![ComponentId(0), ComponentId(1)].into_boxed_slice(),
    )
    .unwrap();
    let first = Column::new(ComponentId(0), Layout::new::<u32>()).unwrap();
    let second = Column::new(ComponentId(1), Layout::new::<u64>()).unwrap();
    let table = Table::new(&layout, vec![first, second].into_boxed_slice()).unwrap();
    let sparse = vec![
        Some(SparseSetStorage::sparse_array(ComponentId(0), Layout::new::<u32>()).unwrap()),
        Some(SparseSetStorage::sparse_array(ComponentId(1), Layout::new::<u64>()).unwrap()),
    ]
    .into_boxed_slice();

    let mut storage = WorldStorage::new(vec![table].into_boxed_slice(), sparse).unwrap();
    storage
        .prepare_archetype(ArchetypeId {
            table: 0,
            variant: 0,
        })
        .unwrap();
    storage
        .prepare_archetype(ArchetypeId {
            table: 0,
            variant: 1,
        })
        .unwrap();
}

#[test]
fn layout_rejects_unsorted_columns() {
    assert!(TableLayout::new(
        ArchetypeId {
            table: 0,
            variant: 0,
        },
        vec![ComponentId(1), ComponentId(0)].into_boxed_slice(),
    )
    .is_err());
}

#[test]
fn table_rejects_column_layout_mismatch() {
    let layout = TableLayout::new(
        ArchetypeId {
            table: 0,
            variant: 0,
        },
        vec![ComponentId(0)].into_boxed_slice(),
    )
    .unwrap();
    let wrong = Column::new(ComponentId(1), Layout::new::<u32>()).unwrap();
    assert!(Table::new(&layout, vec![wrong].into_boxed_slice()).is_err());
}
