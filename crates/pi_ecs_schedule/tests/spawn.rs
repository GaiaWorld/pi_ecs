#![cfg(feature = "rc")]

use std::{any::TypeId, cell::Cell, rc::Rc};

use pi_ecs_core::{
    alter::QueryAlter,
    bundle::{Bundle, BundleContext, BundleWriter, ComponentValue, SystemBundle},
    component::{Component, PlainAccess, StoragePolicy},
    entity::{domain_bits, Entity, PartitionId},
    error::{EcsError, ErrorKind},
    query::{Query, QueryMut},
    query_filter::{Added, With},
    world::World,
};
use pi_ecs_schedule::{
    executor::{ExecutionMode, Executor},
    notification_reader::{AddedReader, ChangedReader},
    param_composition::{ParamSet, ParamSetItem},
    schedule::Schedule,
    spawn::{Spawn, SpawnConfig},
    stage::{Stage, StageId},
    system::{SystemFunction, SystemId},
    system_builder::SystemBuilder,
};

struct Seed(u32);
struct Middle;
struct ResultValue(u32);
impl Component for Seed {
    type Access = PlainAccess;
}
impl Component for Middle {
    type Access = PlainAccess;
}
impl Component for ResultValue {
    type Access = PlainAccess;
}

fn schedule() -> (Schedule, Stage) {
    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    (schedule, Stage::new(StageId(0), executor))
}

#[test]
fn empty_spawn_precedes_member_read_and_uses_fixed_birth_and_initial_partitions() {
    let mut world = World::with_partitions::<3>().unwrap();
    world.finish_registration().unwrap();
    let (mut schedule, mut stage) = schedule();
    let observed = Rc::new(Cell::new(None));
    let out = observed.clone();
    stage
        .add_system(
            SystemBuilder::new(move |entities: Query<'_, Entity>, units: Query<'_, ()>| {
                assert_eq!(units.iter().unwrap().count(), 2);
                let found: Vec<_> = entities.iter().unwrap().collect();
                assert_eq!(found.len(), 2);
                assert_eq!(domain_bits(&found[0]), 0);
                assert_eq!(domain_bits(&found[1]), 2);
                out.set(Some([found[0], found[1]]));
            })
            .build(SystemId(0))
            .unwrap(),
        )
        .unwrap();
    stage
        .add_system(
            SystemBuilder::new(|mut spawn: Spawn<'_, ()>| {
                let _ = spawn.spawn(()).unwrap().forget();
            })
            .build(SystemId(1))
            .unwrap(),
        )
        .unwrap();
    stage
        .add_system(
            SystemBuilder::with_config(
                |mut spawn: Spawn<'_, ()>| {
                    let _ = spawn.spawn(()).unwrap().forget();
                },
                (SpawnConfig {
                    partition: PartitionId(2),
                },),
            )
            .build(SystemId(2))
            .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();
    let before = world.read_change_tick().get();
    schedule.run(&mut world).unwrap();
    assert_eq!(world.read_change_tick().get(), before + 3);
    for (entity, partition) in observed.get().unwrap().into_iter().zip([0, 2]) {
        assert_eq!(
            world.locate(entity).unwrap().partition(),
            PartitionId(partition)
        );
        // SAFETY: Spawn transferred the initial strong via forget.
        unsafe { world.despawn(entity).unwrap() };
    }
    schedule.unregister(&mut world).unwrap();
    world.close().unwrap();
}

#[test]
fn invalid_spawn_partition_is_rejected_at_registration() {
    let mut world = World::with_partitions::<2>().unwrap();
    world.finish_registration().unwrap();
    let before = world.read_change_tick();
    let (mut schedule, mut stage) = schedule();
    stage
        .add_system(
            SystemBuilder::with_config(
                |_: Spawn<'_, ()>| {
                    panic!("invalid configuration must not run");
                },
                (SpawnConfig {
                    partition: PartitionId(2),
                },),
            )
            .build(SystemId(0))
            .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();
    assert_eq!(
        *schedule.run(&mut world).unwrap_err().current_context(),
        ErrorKind::InvalidInput
    );
    assert_eq!(world.read_change_tick(), before);
    assert!(!world.is_stopped());
    world.close().unwrap();
}

#[test]
fn together_rejects_spawn_with_pure_entity_access() {
    let mut world = World::new().unwrap();
    world.finish_registration().unwrap();
    let (mut schedule, mut stage) = schedule();
    stage
        .add_system(
            SystemBuilder::new(|_: Spawn<'_, ()>, _: Query<'_, ()>| {
                panic!("conflicting parameters must not run");
            })
            .build(SystemId(0))
            .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();
    let error = schedule.run(&mut world).unwrap_err();
    assert_eq!(*error.current_context(), ErrorKind::InvalidState);
    assert!(format!("{error:?}").contains("同时使用的参数访问冲突"));
    schedule.unregister(&mut world).unwrap();
    world.close().unwrap();
}

struct WrongOrder;
impl Bundle for WrongOrder {
    fn component_types() -> Vec<TypeId> {
        vec![TypeId::of::<Middle>(), TypeId::of::<Seed>()]
    }
    fn validate(&self, _: &BundleContext<'_>) -> Result<(), EcsError> {
        panic!("a mismatched frozen list must be rejected before validation");
    }
    fn write(self, _: &mut BundleWriter<'_>) {
        panic!("mismatched input must not write");
    }
}
impl SystemBundle for WrongOrder {
    type Shape = (ComponentValue<Seed>, ComponentValue<Middle>);
}

#[test]
fn param_set_rebinds_after_spawn_and_rejects_a_forged_shape() {
    type Create = Spawn<'static, (ComponentValue<Seed>, ComponentValue<Middle>)>;
    type Read = Query<'static, (Entity, &'static Seed), With<Middle>>;
    struct Step(Rc<Cell<Option<Entity>>>);
    impl SystemFunction<()> for Step {
        type Param = ParamSet<(Create, Read)>;
        fn call<'r>(&mut self, mut set: ParamSetItem<'r, 'r, Create, Read>) {
            assert!(set.p1().iter().unwrap().next().is_none());
            let rejected = set
                .p0()
                .spawn(WrongOrder)
                .err()
                .expect("same Shape is not authorization");
            assert_eq!(*rejected.current_context(), ErrorKind::InvalidInput);
            assert!(set.p1().iter().unwrap().next().is_none());
            let entity = set
                .p0()
                .spawn((ComponentValue(Seed(19)), ComponentValue(Middle)))
                .unwrap()
                .forget();
            let query = set.p1();
            let mut items = query.iter().unwrap();
            let (found, seed) = items.next().unwrap();
            assert_eq!((found, seed.0), (entity, 19));
            assert!(items.next().is_none());
            self.0.set(Some(entity));
        }
    }
    let mut world = World::new().unwrap();
    let seed = world
        .register_component::<Seed>(StoragePolicy::TableEligible)
        .unwrap();
    world.register_table(&[seed]).unwrap();
    world
        .register_component::<Middle>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let (mut schedule, mut stage) = schedule();
    let created = Rc::new(Cell::new(None));
    stage
        .add_system(
            SystemBuilder::new(Step(created.clone()))
                .build(SystemId(0))
                .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();
    let before = world.archetypes().unwrap().len();
    schedule.run(&mut world).unwrap();
    assert_eq!(world.archetypes().unwrap().len(), before + 1);
    schedule.unregister(&mut world).unwrap();
    // SAFETY: Spawn transferred the initial strong via forget.
    unsafe { world.despawn(created.get().unwrap()).unwrap() };
    world.close().unwrap();
}

struct BorrowedResult<'a>(&'a u32);
impl Bundle for BorrowedResult<'_> {
    fn component_types() -> Vec<TypeId> {
        vec![TypeId::of::<ResultValue>()]
    }
    fn validate(&self, _: &BundleContext<'_>) -> Result<(), EcsError> {
        Ok(())
    }
    fn write(self, writer: &mut BundleWriter<'_>) {
        writer.write(ResultValue(*self.0));
    }
}
impl SystemBundle for BorrowedResult<'_> {
    type Shape = ComponentValue<ResultValue>;
}

#[test]
fn spawn_seeds_multi_hop_alter_and_publishes_live_ticks_and_notices() {
    let mut world = World::new().unwrap();
    world
        .register_component::<Seed>(StoragePolicy::SparseOnly)
        .unwrap();
    world
        .register_component::<Middle>(StoragePolicy::SparseOnly)
        .unwrap();
    world
        .register_component::<ResultValue>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    assert_eq!(world.archetypes().unwrap().len(), 1);
    let (mut schedule, mut stage) = schedule();
    let observed = Rc::new(Cell::new(None));
    let out = observed.clone();
    stage.add_system(SystemBuilder::new(move |members: Query<'_, Entity, With<Seed>>, query: Query<'_, (Entity, &'static ResultValue), Added<ResultValue>>| {
        // Matching every source/target establishes write-before-read edges for
        // each structural system, avoiding the documented physical-order cycle.
        assert!(members.iter().unwrap().next().is_some());
        let mut items = query.iter().unwrap();
        let (entity, value) = items.next().expect("reader sees the new multi-hop target");
        assert_eq!(value.0, 73);
        let (other, value) = items.next().unwrap();
        assert_eq!(value.0, 8);
        assert!(items.next().is_none());
        out.set(Some([entity, other]));
    }).build(SystemId(0)).unwrap()).unwrap();
    stage
        .add_system(
            SystemBuilder::new(
                |mut added: AddedReader<'_, Seed>, mut changed: ChangedReader<'_, Seed>| {
                    let a: Vec<_> = added.read().unwrap().collect();
                    let c: Vec<_> = changed.read().unwrap().collect();
                    assert_eq!(a.len(), 2);
                    assert_eq!(c.len(), 2);
                    assert_eq!(a[0].tick, a[1].tick);
                    assert_eq!(a[0].tick, c[0].tick);
                    assert_eq!(a[0].entity, c[0].entity);
                    assert_eq!(a[1].entity, c[1].entity);
                },
            )
            .build(SystemId(1))
            .unwrap(),
        )
        .unwrap();
    stage
        .add_system(
            SystemBuilder::new(|mut spawn: Spawn<'_, ComponentValue<Seed>>| {
                let _ = spawn.spawn(ComponentValue(Seed(70))).unwrap().forget();
                let _ = spawn.spawn(ComponentValue(Seed(5))).unwrap().forget();
            })
            .build(SystemId(2))
            .unwrap(),
        )
        .unwrap();
    stage
        .add_system(
            SystemBuilder::new(
                |mut alter: QueryAlter<'_, &'static Seed, Added<Seed>, ComponentValue<Middle>>| {
                    let mut cursor = alter.iter_mut();
                    assert_eq!(cursor.fetch_next().unwrap().0, 70);
                    cursor.alter(ComponentValue(Middle)).unwrap();
                    assert_eq!(cursor.fetch_next().unwrap().0, 5);
                    cursor.alter(ComponentValue(Middle)).unwrap();
                    assert!(cursor.fetch_next().is_none());
                },
            )
            .build(SystemId(3))
            .unwrap(),
        )
        .unwrap();
    stage
        .add_system(
            SystemBuilder::new(
                |mut alter: QueryAlter<
                    '_,
                    &'static Seed,
                    With<Middle>,
                    ComponentValue<ResultValue>,
                    ComponentValue<Middle>,
                >| {
                    let mut cursor = alter.iter_mut();
                    let value = cursor.fetch_next().unwrap().0 + 3;
                    cursor.alter(BorrowedResult(&value)).unwrap();
                    let other = cursor.fetch_next().unwrap().0 + 3;
                    cursor.alter(BorrowedResult(&other)).unwrap();
                    assert!(cursor.fetch_next().is_none());
                },
            )
            .build(SystemId(4))
            .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();
    schedule.run(&mut world).unwrap();
    assert_eq!(world.archetypes().unwrap().len(), 4);
    schedule.unregister(&mut world).unwrap();
    // SAFETY: Spawn transferred the initial strong via forget.
    for entity in observed.get().unwrap() {
        unsafe { world.despawn(entity).unwrap() };
    }
    world.close().unwrap();
}

#[test]
fn absent_optional_value_does_not_mask_spawn_before_reader() {
    type Create = Spawn<'static, ComponentValue<Seed>>;
    type Read = Query<'static, Option<&'static Middle>>;
    struct Step(Rc<Cell<Option<Entity>>>);
    impl SystemFunction<()> for Step {
        type Param = ParamSet<(Create, Read)>;
        fn call<'r>(&mut self, mut set: ParamSetItem<'r, 'r, Create, Read>) {
            let entity = set.p0().spawn(ComponentValue(Seed(41))).unwrap().forget();
            assert!(set.p1().iter().unwrap().all(|value| value.is_none()));
            self.0.set(Some(entity));
        }
    }

    let mut world = World::new().unwrap();
    world
        .register_component::<Seed>(StoragePolicy::SparseOnly)
        .unwrap();
    world
        .register_component::<Middle>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let (mut schedule, mut stage) = schedule();
    let observed = Rc::new(Cell::new(None));
    let out = observed.clone();
    stage
        .add_system(
            SystemBuilder::new(
                move |_: QueryMut<'_, Option<&'static mut Middle>>,
                      seeds: Query<'_, &'static Seed>| {
                    out.set(seeds.iter().unwrap().next().map(|seed| seed.0));
                },
            )
            .build(SystemId(0))
            .unwrap(),
        )
        .unwrap();
    let created = Rc::new(Cell::new(None));
    stage
        .add_system(
            SystemBuilder::new(Step(created.clone()))
                .build(SystemId(1))
                .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();
    schedule.run(&mut world).unwrap();
    schedule.unregister(&mut world).unwrap();
    // SAFETY: Spawn transferred the initial strong via forget.
    unsafe { world.despawn(created.get().unwrap()).unwrap() };
    world.close().unwrap();
    assert_eq!(observed.get(), Some(41));
}

#[test]
fn earlier_reader_sees_spawns_first_sparse_variant_and_same_run_tick() {
    let mut world = World::new().unwrap();
    world
        .register_component::<Seed>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let (mut schedule, mut stage) = schedule();
    let observed = Rc::new(Cell::new(None));
    let out = observed.clone();
    stage
        .add_system(
            SystemBuilder::new(
                move |query: Query<'_, (Entity, &'static Seed), Added<Seed>>| {
                    let (entity, value) = query.iter().unwrap().next().unwrap();
                    assert_eq!(value.0, 26);
                    out.set(Some(entity));
                },
            )
            .build(SystemId(0))
            .unwrap(),
        )
        .unwrap();
    let notice = Rc::new(Cell::new(None));
    let out = notice.clone();
    stage
        .add_system(
            SystemBuilder::new(move |mut notices: AddedReader<'_, Seed>| {
                let header = notices.read().unwrap().next().unwrap();
                out.set(Some((header.entity, header.tick)));
            })
            .build(SystemId(2))
            .unwrap(),
        )
        .unwrap();
    stage
        .add_system(
            SystemBuilder::new(|mut spawn: Spawn<'_, ComponentValue<Seed>>| {
                let value = 26;
                let _ = spawn.spawn(ComponentValue(Seed(value))).unwrap().forget();
            })
            .build(SystemId(1))
            .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();
    let before = world.archetypes().unwrap().len();
    schedule.run(&mut world).unwrap();
    assert_eq!(world.archetypes().unwrap().len(), before + 1);
    schedule.unregister(&mut world).unwrap();
    let (entity, tick) = notice.get().unwrap();
    assert_eq!(observed.get(), Some(entity));
    assert_eq!(tick.get() + 2, world.read_change_tick().get());
    // SAFETY: Spawn transferred the initial strong via forget.
    unsafe { world.despawn(entity).unwrap() };
    world.close().unwrap();
}
