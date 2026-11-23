#![cfg(feature = "rc")]

use pi_ecs_core::world::World;
use pi_ecs_schedule::{
    executor::{ExecutionMode, Executor},
    local::{Local, LocalConfig},
    schedule::Schedule,
    stage::{Stage, StageId},
    system::SystemId,
    system_builder::SystemBuilder,
};
use std::{cell::RefCell, rc::Rc};

#[test]
fn same_type_locals_keep_independent_positions_across_system_runs() {
    let mut world = World::new().unwrap();
    world.finish_registration().unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    let system = SystemBuilder::with_config(
        move |mut first: Local<'_, u32>, mut second: Local<'_, u32>| {
            output.borrow_mut().push((*first, *second));
            *first += 1;
            *second *= 2;
        },
        (LocalConfig::new(3), LocalConfig::new(10)),
    )
    .build(SystemId(1))
    .unwrap();
    let mut schedule = schedule_for(system);
    schedule.run(&mut world).unwrap();
    schedule.run(&mut world).unwrap();
    schedule.unregister(&mut world).unwrap();
    assert_eq!(*seen.borrow(), [(3, 10), (4, 20)]);
    world.close().unwrap();
}

#[test]
fn param_set_repeated_short_borrows_preserve_member_positions() {
    use pi_ecs_schedule::{
        param_composition::{ParamSet, ParamSetItem},
        system::SystemFunction,
    };
    type Counter = Local<'static, u32>;
    struct Step(Rc<RefCell<Vec<(u32, u32, u32)>>>);
    impl SystemFunction<()> for Step {
        type Param = (Local<'static, u8>, ParamSet<(Counter, Counter)>);
        fn call<'r>(
            &mut self,
            (mut run, mut set): (Local<'r, u8>, ParamSetItem<'r, 'r, Counter, Counter>),
        ) {
            *run += 1;
            let first = *set.p0();
            *set.p0() += u32::from(*run);
            let second = *set.p1();
            *set.p1() *= 3;
            self.0.borrow_mut().push((first, second, *set.p0()));
        }
    }
    let mut world = World::new().unwrap();
    world.finish_registration().unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut schedule = schedule_for(
        SystemBuilder::with_config(
            Step(seen.clone()),
            (
                LocalConfig::new(0),
                (LocalConfig::new(4), LocalConfig::new(7)),
            ),
        )
        .build(SystemId(1))
        .unwrap(),
    );
    schedule.run(&mut world).unwrap();
    schedule.run(&mut world).unwrap();
    schedule.unregister(&mut world).unwrap();
    assert_eq!(*seen.borrow(), [(4, 7, 5), (5, 21, 7)]);
    world.close().unwrap();
}

#[test]
fn param_set_switches_between_two_writes_to_one_resource() {
    use pi_ecs_schedule::{
        param_composition::{ParamSet, ParamSetItem},
        resource_param::ResMut,
        system::SystemFunction,
    };
    type Slot = ResMut<'static, u32>;
    struct Step;
    impl SystemFunction<()> for Step {
        type Param = ParamSet<(Slot, Slot)>;
        fn call<'r>(&mut self, mut set: ParamSetItem<'r, 'r, Slot, Slot>) {
            *set.p0() += 2;
            assert_eq!(*set.p1(), 9);
            *set.p1() *= 2;
            assert_eq!(*set.p0(), 18);
        }
    }
    let mut world = World::new().unwrap();
    world.register_resource::<u32>().unwrap();
    world.finish_registration().unwrap();
    world.insert_resource(7u32).unwrap();
    run_once(
        &mut world,
        SystemBuilder::new(Step).build(SystemId(1)).unwrap(),
    );
    assert_eq!(
        world
            .read_scope(|pin| pin.resource::<u32>().copied())
            .unwrap(),
        Some(18)
    );
    world.close().unwrap();
}

fn schedule_for(system: impl pi_ecs_schedule::stage::StageSystemBound + 'static) -> Schedule {
    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    let mut stage = Stage::new(StageId(1), executor);
    stage.add_system(system).unwrap();
    schedule.add_stage(stage).unwrap();
    schedule
}

fn run_once(world: &mut World, system: impl pi_ecs_schedule::stage::StageSystemBound + 'static) {
    let mut schedule = schedule_for(system);
    schedule.run(world).unwrap();
    schedule.unregister(world).unwrap();
}

#[test]
fn query_tuple_runs_with_optional_data_and_or_filter() {
    use pi_ecs_core::{
        bundle::ComponentValue,
        component::{Component, PlainAccess, StoragePolicy},
        entity::AllocationDomain,
        query::Query,
        query_filter::{Or, With},
    };
    struct Tag;
    impl Component for Tag {
        type Access = PlainAccess;
    }
    struct Other(u32);
    impl Component for Other {
        type Access = PlainAccess;
    }
    let mut world = World::new().unwrap();
    world
        .register_component::<Tag>(StoragePolicy::SparseOnly)
        .unwrap();
    world
        .register_component::<Other>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let tagged = world
        .spawn(AllocationDomain(0), ComponentValue(Tag))
        .unwrap()
        .forget();
    let valued = world
        .spawn(AllocationDomain(0), ComponentValue(Other(19)))
        .unwrap()
        .forget();
    let excluded = world.spawn(AllocationDomain(0), ()).unwrap().forget();
    let called = Rc::new(RefCell::new(0));
    let seen = called.clone();
    let system = SystemBuilder::new(
        move |query: Query<'_, Option<&'static Other>, Or<(With<Tag>, With<Other>)>>,
              mut local: Local<'_, u32>| {
            let mut values: Vec<_> = query
                .iter()
                .unwrap()
                .map(|item| item.map(|value| value.0))
                .collect();
            values.sort_unstable();
            assert_eq!(values, [None, Some(19)]);
            *local += 1;
            *seen.borrow_mut() = *local;
        },
    )
    .build(SystemId(1))
    .unwrap();
    run_once(&mut world, system);
    assert_eq!(*called.borrow(), 1);
    // SAFETY: each successful spawn explicitly transferred one release responsibility.
    unsafe {
        for entity in [tagged, valued, excluded] {
            world.despawn(entity).unwrap();
        }
    }
    world.close().unwrap();
}

#[test]
fn failed_tuple_registration_returns_the_receiver_without_consuming_commands() {
    use pi_ecs_core::command::{CommandCapacity, CommandRead};
    let mut world = World::new().unwrap();
    world
        .register_command::<u32>(CommandCapacity::Unbounded)
        .unwrap();
    world.finish_registration().unwrap();
    assert!(world.command_sender::<u32>().unwrap().send(37).is_ok());
    let mut schedule = schedule_for(
        SystemBuilder::new(|_: CommandRead<'_, u32>, _: CommandRead<'_, u32>| {
            panic!("a system with two consumers of one queue must not run")
        })
        .build(SystemId(1))
        .unwrap(),
    );
    assert!(schedule.run(&mut world).is_err());
    assert!(!world.is_stopped());
    let mut receiver = world.command_receiver::<u32>().unwrap();
    assert_eq!(receiver.read().unwrap().next(), Some(37));
    drop(receiver);
    schedule.unregister(&mut world).unwrap();
    world.close().unwrap();
}

#[test]
fn named_param_set_commits_only_the_members_that_were_fetched() {
    use pi_ecs_schedule::{
        param_composition::{NamedParam, ParamSet, ParamSetItem, ParamView},
        resource_param::MultiRes,
        system::SystemFunction,
    };
    type Items = MultiRes<'static, u32>;
    type Set = ParamSet<(Items, Items)>;
    struct View;
    impl ParamView<Set> for View {
        type Item<'r> = ParamSetItem<'r, 'r, Items, Items>;
        fn from_param<'r>(item: Self::Item<'r>) -> Self::Item<'r> {
            item
        }
    }
    struct Step {
        run: u8,
        seen: Rc<RefCell<Vec<bool>>>,
    }
    impl SystemFunction<()> for Step {
        type Param = NamedParam<Set, View>;
        fn call<'r>(&mut self, mut set: ParamSetItem<'r, 'r, Items, Items>) {
            let mut seen = self.seen.borrow_mut();
            seen.push(set.p0().is_structure_changed());
            if self.run > 0 {
                seen.push(set.p1().is_structure_changed());
            }
            self.run += 1;
        }
    }
    let mut world = World::new().unwrap();
    world.register_multi_resource::<u32>().unwrap();
    world.finish_registration().unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut schedule = schedule_for(
        SystemBuilder::new(Step {
            run: 0,
            seen: seen.clone(),
        })
        .build(SystemId(1))
        .unwrap(),
    );
    for _ in 0..3 {
        schedule.run(&mut world).unwrap();
    }
    schedule.unregister(&mut world).unwrap();
    assert_eq!(*seen.borrow(), [true, false, true, false, false]);
    world.close().unwrap();
}

#[test]
fn param_set_query_and_lookup_share_a_value_without_sharing_a_live_borrow() {
    use pi_ecs_core::{
        bundle::ComponentValue,
        component::{Component, PlainAccess, StoragePolicy},
        entity::{AllocationDomain, Entity},
        lookup::Lookup,
        query::QueryMut,
    };
    use pi_ecs_schedule::{
        param_composition::{ParamSet, ParamSetItem},
        system::SystemFunction,
    };
    struct Value(u32);
    impl Component for Value {
        type Access = PlainAccess;
    }
    type Write = QueryMut<'static, &'static mut Value>;
    type Read = Lookup<'static, &'static Value>;
    struct Step(Entity);
    impl SystemFunction<()> for Step {
        type Param = ParamSet<(Write, Read)>;
        fn call<'r>(&mut self, mut set: ParamSetItem<'r, 'r, Write, Read>) {
            set.p0().get_mut(self.0).unwrap().0 += 2;
            assert_eq!(set.p1().get(self.0).unwrap().0, 9);
            set.p0().get_mut(self.0).unwrap().0 *= 2;
            assert_eq!(set.p1().get(self.0).unwrap().0, 18);
        }
    }
    let mut world = World::new().unwrap();
    world
        .register_component::<Value>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let entity = world
        .spawn(AllocationDomain(0), ComponentValue(Value(7)))
        .unwrap()
        .forget();
    run_once(
        &mut world,
        SystemBuilder::new(Step(entity)).build(SystemId(1)).unwrap(),
    );
    // SAFETY: spawn transferred one release responsibility to this test.
    unsafe {
        world.despawn(entity).unwrap();
    }
    world.close().unwrap();
}

#[test]
fn missing_required_resource_is_retryable_and_optional_absence_remains_none() {
    use pi_ecs_schedule::resource_param::{Res, ResMut};
    let mut world = World::new().unwrap();
    world.register_resource::<u32>().unwrap();
    world.register_resource::<u64>().unwrap();
    world.finish_registration().unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    let mut schedule = schedule_for(
        SystemBuilder::new(
            move |required: Res<'_, u32>, optional: Option<ResMut<'_, u64>>| {
                output
                    .borrow_mut()
                    .push((*required, optional.as_deref().copied()));
            },
        )
        .build(SystemId(1))
        .unwrap(),
    );
    let before = world.read_change_tick();
    assert!(schedule.run(&mut world).is_err());
    assert!(!world.is_stopped());
    assert_eq!(world.read_change_tick(), before);
    assert!(seen.borrow().is_empty());
    world.insert_resource(23u32).unwrap();
    schedule.run(&mut world).unwrap();
    schedule.unregister(&mut world).unwrap();
    assert_eq!(*seen.borrow(), [(23, None)]);
    world.close().unwrap();
}

#[test]
fn together_rejects_two_writes_to_the_same_resource_before_running() {
    use pi_ecs_schedule::resource_param::ResMut;
    let mut world = World::new().unwrap();
    world.register_resource::<u32>().unwrap();
    world.finish_registration().unwrap();
    world.insert_resource(7u32).unwrap();
    let mut schedule = schedule_for(
        SystemBuilder::new(|_: ResMut<'_, u32>, _: ResMut<'_, u32>| {
            panic!("overlapping Together writes must not reach the user function");
        })
        .build(SystemId(1))
        .unwrap(),
    );
    assert!(schedule.run(&mut world).is_err());
    assert!(!world.is_stopped());
    schedule.unregister(&mut world).unwrap();
    world.close().unwrap();
}

#[test]
fn scheduled_readers_keep_independent_cursors_for_the_same_stream() {
    use pi_ecs_core::{
        bundle::ComponentValue,
        component::{Component, PlainAccess, StoragePolicy},
        entity::AllocationDomain,
    };
    use pi_ecs_schedule::notification_reader::AddedReader;
    struct Tag;
    impl Component for Tag {
        type Access = PlainAccess;
    }
    let mut world = World::new().unwrap();
    world
        .register_component::<Tag>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let output = seen.clone();
    let mut schedule = schedule_for(
        SystemBuilder::new(
            move |mut first: AddedReader<'_, Tag>, mut second: AddedReader<'_, Tag>| {
                let count = first.read().unwrap().count();
                let other = second.read().unwrap().count();
                output.borrow_mut().push((count, other));
            },
        )
        .build(SystemId(1))
        .unwrap(),
    );
    schedule.run(&mut world).unwrap();
    let entity = world
        .spawn(AllocationDomain(0), ComponentValue(Tag))
        .unwrap()
        .forget();
    schedule.run(&mut world).unwrap();
    schedule.run(&mut world).unwrap();
    schedule.unregister(&mut world).unwrap();
    assert_eq!(*seen.borrow(), [(0, 0), (1, 1), (0, 0)]);
    // SAFETY: spawn transferred one release responsibility to this test.
    unsafe {
        world.despawn(entity).unwrap();
    }
    world.close().unwrap();
}
