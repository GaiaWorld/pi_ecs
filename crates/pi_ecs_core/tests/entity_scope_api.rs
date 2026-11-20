#![cfg(feature = "rc")]

use pi_ecs_core::{
    bundle::ComponentValue,
    component::{Component, PlainAccess, StoragePolicy},
    entity::AllocationDomain,
    world::World,
};

struct TableValue(u32);
impl Component for TableValue {
    type Access = PlainAccess;
}
struct SparseValue(u32);
impl Component for SparseValue {
    type Access = PlainAccess;
}
struct Unregistered;
impl Component for Unregistered {
    type Access = PlainAccess;
}

#[test]
fn scope_reads_and_writes_table_and_sparse_components() {
    let mut world = World::new().unwrap();
    let table = world
        .register_component::<TableValue>(StoragePolicy::TableEligible)
        .unwrap();
    world
        .register_component::<SparseValue>(StoragePolicy::SparseOnly)
        .unwrap();
    world.register_table(&[table]).unwrap();
    world.finish_registration().unwrap();
    let entity = world
        .spawn(
            AllocationDomain(0),
            (
                ComponentValue(TableValue(12)),
                ComponentValue(SparseValue(23)),
            ),
        )
        .unwrap()
        .forget();
    let before = world.read_change_tick();
    world
        .entity_scope(|scope| {
            assert_eq!(scope.get::<TableValue>(entity)?.unwrap().0, 12);
            assert_eq!(scope.get::<SparseValue>(entity)?.unwrap().0, 23);
            assert!(scope.get::<Unregistered>(entity)?.is_none());
            assert!(scope.get_mut::<Unregistered>(entity)?.is_none());
            scope.get_mut::<TableValue>(entity)?.unwrap().0 += 4;
            scope.get_mut::<SparseValue>(entity)?.unwrap().0 += 9;
            Ok(())
        })
        .unwrap();
    assert_eq!(world.read_change_tick().get(), before.get() + 2);
    world
        .read_scope(|pin| {
            assert_eq!(pin.get::<TableValue>(entity).unwrap().0, 16);
            assert_eq!(pin.get::<SparseValue>(entity).unwrap().0, 32);
        })
        .unwrap();
    world.close().unwrap();
}

#[test]
fn scope_transfers_queue_ownership_across_a_short_borrow_into_spawn() {
    use pi_ecs_core::{
        bundle_input::{EmptyQueue, StrongArrayInput},
        entity_container::EntityQueue,
        owned_entity::StrongEntities,
    };
    let mut world = World::new().unwrap();
    world
        .register_component::<EntityQueue<()>>(StoragePolicy::SparseOnly)
        .unwrap();
    let strong_array = world
        .register_component::<StrongEntities<1>>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let (target, queue, owner) = world
        .entity_scope(|scope| {
            let target = scope.spawn(AllocationDomain(0), ())?;
            let id = target.entity();
            let queue = scope.spawn(AllocationDomain(0), EmptyQueue::<()>::new())?;
            let queue_id = queue.entity();
            scope
                .get_mut::<EntityQueue<()>>(queue_id)?
                .unwrap()
                .push_back(target)
                .map_err(|rejected| rejected.error)?;
            let popped = scope
                .get_mut::<EntityQueue<()>>(queue_id)?
                .unwrap()
                .pop_front()?
                .unwrap();
            assert_eq!(popped.entity(), id);
            assert!(scope
                .get_mut::<EntityQueue<()>>(queue_id)?
                .unwrap()
                .pop_front()?
                .is_none());
            let bundle =
                StrongArrayInput::<1, ()>::new([popped]).map_err(|rejected| rejected.error)?;
            let owner = scope.spawn(AllocationDomain(0), bundle)?;
            Ok((id, queue.forget(), owner.forget()))
        })
        .unwrap();
    // No transient handle to target remains: the installed array owns the only strong.
    world.flush_destruction().unwrap();
    assert!(world.locate(target).is_some());
    assert!(world.locate(queue).is_some());
    world
        .read_scope(|pin| assert!(pin.get::<StrongEntities<1>>(owner).is_some()))
        .unwrap();
    world
        .entity_scope(|scope| scope.change_archetype(owner, (), &[strong_array]))
        .unwrap();
    world.flush_destruction().unwrap();
    assert!(world.locate(target).is_none());
    world.close().unwrap();
}

#[test]
fn scope_changes_archetype_without_leaving_maintenance_on_success_or_error() {
    use pi_ecs_core::error::{classified_error, EcsError, ErrorKind};
    let mut world = World::new().unwrap();
    let table = world
        .register_component::<TableValue>(StoragePolicy::TableEligible)
        .unwrap();
    world
        .register_component::<SparseValue>(StoragePolicy::SparseOnly)
        .unwrap();
    world.register_table(&[table]).unwrap();
    world.finish_registration().unwrap();
    let entity = world
        .spawn(AllocationDomain(0), ComponentValue(TableValue(8)))
        .unwrap()
        .forget();
    let before = world.read_change_tick();
    let result: Result<(), EcsError> = world.entity_scope(|scope| {
        let held = scope.try_acquire(entity)?;
        assert!(scope
            .change_archetype(entity, ComponentValue(TableValue(99)), &[])
            .is_err());
        scope.change_archetype(entity, (), &[])?;
        scope.change_archetype(entity, ComponentValue(SparseValue(31)), &[table])?;
        assert!(scope.get::<TableValue>(entity)?.is_none());
        assert_eq!(scope.get::<SparseValue>(entity)?.unwrap().0, 31);
        drop(held);
        Err(classified_error(
            ErrorKind::InvalidInput,
            "caller returns normally",
        ))
    });
    let error = result.unwrap_err();
    assert_eq!(
        error.downcast_ref::<String>().map(String::as_str),
        Some("caller returns normally")
    );
    assert_eq!(world.read_change_tick().get(), before.get() + 1);
    world
        .read_scope(|pin| assert_eq!(pin.get::<SparseValue>(entity).unwrap().0, 31))
        .unwrap();
    world.close().unwrap();
}
