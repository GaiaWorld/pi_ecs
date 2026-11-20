#![cfg(feature = "rc")]
use pi_ecs_storage::{ptr::OwningPtr, sparse_set::SparseSetStorage, storage::WorldStorage};
use pi_ecs_types::{
    change_detection::ComponentTicks,
    component::ComponentId,
    entity::{Entity, KeyData},
};
use std::{alloc::Layout, cell::Cell, mem::ManuallyDrop, ptr::NonNull, rc::Rc};

#[test]
fn world_tracked_delegates_without_duplicating_drop_responsibility() {
    struct Value(Rc<Cell<usize>>);
    impl Drop for Value {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let count = Rc::new(Cell::new(0));
    let set = SparseSetStorage::sparse_array(ComponentId(0), Layout::new::<Value>()).unwrap();
    let mut world = WorldStorage::new(
        vec![].into_boxed_slice(),
        vec![Some(set), None].into_boxed_slice(),
    )
    .unwrap();
    let key = Entity::from(KeyData::new(7, 1));
    assert!(world.prepare_sparse_tracked(ComponentId(1), key).is_err());
    assert!(world.sparse_ticks(ComponentId(1), key).is_none());
    world.prepare_sparse_tracked(ComponentId(0), key).unwrap();
    assert!(world.sparse_ticks(ComponentId(0), key).is_none());
    let mut value = ManuallyDrop::new(Value(count.clone()));
    unsafe {
        world.insert_sparse_tracked(
            ComponentId(0),
            key,
            OwningPtr::from_raw(NonNull::from(&mut *value).cast()),
            ComponentTicks {
                added: None,
                changed: None,
            },
        );
    }
    let ticks = world.sparse_ticks(ComponentId(0), key).unwrap();
    assert!(ticks.added.is_none() && ticks.changed.is_none());
    let removed = world.remove_sparse(ComponentId(0), key).unwrap();
    unsafe {
        std::ptr::drop_in_place(removed.into_raw().cast::<Value>());
    }
    assert!(world.remove_sparse(ComponentId(0), key).is_none());
    drop(world);
    assert_eq!(count.get(), 1);
}
