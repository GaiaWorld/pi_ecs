#![cfg(feature = "rc")]

use std::{
    any::TypeId,
    cell::Cell,
    panic::{catch_unwind, AssertUnwindSafe},
};

use pi_ecs_core::{
    alter::AlterState,
    bundle::{Bundle, BundleContext, BundleWriter, ComponentValue, SystemBundle},
    component::{Component, PlainAccess, StoragePolicy},
    entity::AllocationDomain,
    error::EcsError,
    notification::NoticeKind,
    world::World,
};

struct Seed;
impl Component for Seed {
    type Access = PlainAccess;
}

struct ForgedEmpty;
impl Bundle for ForgedEmpty {
    fn component_types() -> Vec<TypeId> {
        Vec::new()
    }

    fn validate(&self, _: &BundleContext<'_>) -> Result<(), EcsError> {
        Ok(())
    }

    fn write(self, writer: &mut BundleWriter<'_>) {
        writer.write(Seed);
    }
}
impl SystemBundle for ForgedEmpty {
    type Shape = ();
}

#[test]
fn empty_alter_rejects_a_bundle_that_writes_a_component() {
    let mut world = World::new().unwrap();
    world
        .register_component::<Seed>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let entity = world.spawn(AllocationDomain(0), ()).unwrap().forget();
    let mut state = AlterState::new(&world, &[], &[]).unwrap();

    let result = catch_unwind(AssertUnwindSafe(|| {
        state.scope(&mut world, |alter| alter.alter(entity, ForgedEmpty))
    }));
    let stopped = world.is_stopped();
    // SAFETY: The test owns the initial strong transferred by forget.
    unsafe { world.despawn(entity).unwrap() };
    world.close().unwrap();

    assert!(
        result.is_err(),
        "zero-slot writer must reject the actual write"
    );
    assert!(stopped);
}

struct Empty<'a>(&'a Cell<u32>);
impl Bundle for Empty<'_> {
    fn component_types() -> Vec<TypeId> {
        Vec::new()
    }

    fn validate(&self, _: &BundleContext<'_>) -> Result<(), EcsError> {
        Ok(())
    }

    fn write(self, _: &mut BundleWriter<'_>) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn valid_empty_alter_consumes_bundle_without_republishing_or_advancing_tick() {
    let mut world = World::new().unwrap();
    let seed = world
        .register_component::<Seed>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let entity = world
        .spawn(AllocationDomain(0), ComponentValue(Seed))
        .unwrap()
        .forget();
    let mut added = world.subscribe_notice(seed, NoticeKind::Added).unwrap();
    let mut changed = world.subscribe_notice(seed, NoticeKind::Changed).unwrap();
    let mut removed = world.subscribe_notice(seed, NoticeKind::Removed).unwrap();
    let location = world.locate(entity).unwrap();
    let before = world.read_change_tick();
    let written = Cell::new(0);
    let mut state = AlterState::new(&world, &[], &[]).unwrap();

    state
        .scope(&mut world, |alter| alter.alter(entity, Empty(&written)))
        .unwrap()
        .unwrap();

    assert_eq!(written.get(), 1);
    assert_eq!(world.locate(entity).unwrap(), location);
    assert_eq!(world.read_change_tick().get(), before.get() + 1);
    assert!(world
        .notifications()
        .read(&mut added)
        .unwrap()
        .next()
        .is_none());
    assert!(world
        .notifications()
        .read(&mut changed)
        .unwrap()
        .next()
        .is_none());
    assert!(world
        .notifications()
        .read_removed::<Seed>(&world, &mut removed)
        .unwrap()
        .next()
        .is_none());
    world.unsubscribe_notice(added).unwrap();
    world.unsubscribe_notice(changed).unwrap();
    world.unsubscribe_notice(removed).unwrap();
    // SAFETY: The test owns the initial strong transferred by forget.
    unsafe { world.despawn(entity).unwrap() };
    world.close().unwrap();
}
