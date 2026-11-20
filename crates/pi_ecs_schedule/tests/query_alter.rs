#![cfg(feature = "rc")]

use pi_ecs_core::{
    alter::QueryAlter,
    bundle::ComponentValue,
    component::{Component, PlainAccess, StoragePolicy},
    entity::{AllocationDomain, Entity},
    query::{Query, QueryState},
    query_filter::{Changed, With},
    world::World,
};
use pi_ecs_schedule::{
    executor::{ExecutionMode, Executor},
    schedule::Schedule,
    stage::{Stage, StageId},
    system::SystemId,
    system_builder::SystemBuilder,
};

#[test]
fn typed_empty_alter_rejects_a_write_at_the_zero_slot_writer() {
    use pi_ecs_core::{
        bundle::{Bundle, BundleContext, BundleWriter, SystemBundle},
        error::EcsError,
    };
    use std::{
        any::TypeId,
        cell::Cell,
        panic::{catch_unwind, AssertUnwindSafe},
        rc::Rc,
    };

    struct ForgedEmpty<'a>(&'a Cell<bool>);
    impl Bundle for ForgedEmpty<'_> {
        fn component_types() -> Vec<TypeId> {
            Vec::new()
        }

        fn validate(&self, _: &BundleContext<'_>) -> Result<(), EcsError> {
            Ok(())
        }

        fn write(self, writer: &mut BundleWriter<'_>) {
            self.0.set(true);
            writer.write(Tag(99));
        }
    }
    impl SystemBundle for ForgedEmpty<'_> {
        type Shape = ();
    }

    let mut world = World::new().unwrap();
    world
        .register_component::<Tag>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let entity = world.spawn(AllocationDomain(0), ()).unwrap().forget();
    let location = world.locate(entity).unwrap();
    let reached_writer = Rc::new(Cell::new(false));
    let marker = Rc::clone(&reached_writer);
    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    let mut stage = Stage::new(StageId(1), executor);
    stage
        .add_system(
            SystemBuilder::new(move |mut alter: QueryAlter<'_, Entity>| {
                let _ = alter.alter(entity, ForgedEmpty(&marker));
            })
            .build(SystemId(1))
            .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();

    let outcome = catch_unwind(AssertUnwindSafe(|| schedule.run(&mut world)));
    assert!(
        reached_writer.get(),
        "the typed input must reach Bundle::write"
    );
    assert!(
        outcome.is_err(),
        "the zero-slot writer must reject the write"
    );
    assert!(world.is_stopped());
    assert_eq!(world.locate(entity).unwrap(), location);
    schedule.unregister(&mut world).unwrap();
    // SAFETY: the test owns the initial strong transferred by forget.
    unsafe { world.despawn(entity).unwrap() };
    world.close().unwrap();
}

struct Anchor;
impl Component for Anchor {
    type Access = PlainAccess;
}
struct Tag(u32);
impl Component for Tag {
    type Access = PlainAccess;
}

#[test]
fn alter_publishes_a_new_archetype_before_an_earlier_registered_reader_and_unregisters_tracking() {
    let mut world = World::new().unwrap();
    world
        .register_component::<Anchor>(StoragePolicy::SparseOnly)
        .unwrap();
    world
        .register_component::<Tag>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let owned = world
        .spawn(AllocationDomain(0), ComponentValue(Anchor))
        .unwrap();
    let entity = owned.forget();
    let before = world.archetypes().unwrap().len();
    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    let mut stage = Stage::new(StageId(1), executor);
    stage
        .add_system(
            SystemBuilder::new(
                move |query: Query<'_, (Entity, &'static Tag), With<Anchor>>| {
                    let mut items = query.iter().unwrap();
                    let (found, value) = items.next().expect("reader sees newly published target");
                    assert_eq!(found, entity);
                    assert_eq!(value.0, 37);
                    assert!(items.next().is_none());
                },
            )
            .build(SystemId(1))
            .unwrap(),
        )
        .unwrap();
    stage
        .add_system(
            SystemBuilder::new(
                move |mut query: QueryAlter<'_, Entity, Changed<Anchor>, ComponentValue<Tag>>| {
                    let mut cursor = query.iter_mut();
                    assert_eq!(cursor.fetch_next(), Some(entity));
                    cursor.alter(ComponentValue(Tag(37))).unwrap();
                    assert!(cursor.fetch_next().is_none());
                },
            )
            .build(SystemId(2))
            .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();
    schedule.run(&mut world).unwrap();
    assert_eq!(world.archetypes().unwrap().len(), before + 1);
    let mut other = World::new().unwrap();
    other.finish_registration().unwrap();
    assert!(schedule.unregister(&mut other).is_err());
    other.close().unwrap();
    schedule.unregister(&mut world).unwrap();
    world.apply_tracking().unwrap();

    // A new demand must re-enable Changed<Anchor> and initialize its live ticks.
    let before_reenable = world.read_change_tick();
    let mut probe = QueryState::<Entity, Changed<Anchor>>::new(&mut world).unwrap();
    world.apply_tracking().unwrap();
    assert!(world.read_change_tick() > before_reenable);
    probe.unregister_tracking(&mut world).unwrap();
    // SAFETY: the successful spawn transferred one release responsibility.
    unsafe { world.despawn(entity).unwrap() };
    world.close().unwrap();
}

#[test]
fn param_set_retake_sees_the_new_target_even_when_it_cannot_be_altered_again() {
    use pi_ecs_schedule::{
        param_composition::{ParamSet, ParamSetItem},
        system::SystemFunction,
    };

    type Alter =
        QueryAlter<'static, (Entity, Option<&'static Tag>), With<Anchor>, ComponentValue<Tag>>;
    type Read = Query<'static, &'static Tag, With<Anchor>>;
    struct Step(Entity);
    impl SystemFunction<()> for Step {
        type Param = ParamSet<(Alter, Read)>;

        fn call<'r>(&mut self, mut set: ParamSetItem<'r, 'r, Alter, Read>) {
            assert!(set.p1().iter().unwrap().next().is_none());
            {
                let mut alter = set.p0();
                let mut cursor = alter.iter_mut();
                assert!(matches!(cursor.fetch_next(), Some((id, None)) if id == self.0));
                cursor.alter(ComponentValue(Tag(41))).unwrap();
                assert!(cursor.fetch_next().is_none());
            }
            assert_eq!(set.p1().iter().unwrap().next().unwrap().0, 41);
            let mut alter = set.p0();
            let mut cursor = alter.iter_mut();
            let (id, tag) = cursor.fetch_next().unwrap();
            assert_eq!(id, self.0);
            assert_eq!(tag.unwrap().0, 41);
            assert!(cursor.alter(ComponentValue(Tag(42))).is_err());
            assert!(cursor.fetch_next().is_none());
        }
    }

    let mut world = World::new().unwrap();
    world
        .register_component::<Anchor>(StoragePolicy::SparseOnly)
        .unwrap();
    world
        .register_component::<Tag>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let entity = world
        .spawn(AllocationDomain(0), ComponentValue(Anchor))
        .unwrap()
        .forget();
    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    let mut stage = Stage::new(StageId(1), executor);
    stage
        .add_system(SystemBuilder::new(Step(entity)).build(SystemId(1)).unwrap())
        .unwrap();
    schedule.add_stage(stage).unwrap();
    schedule.run(&mut world).unwrap();
    schedule.unregister(&mut world).unwrap();
    // SAFETY: the successful spawn transferred one release responsibility.
    unsafe { world.despawn(entity).unwrap() };
    world.close().unwrap();
}
