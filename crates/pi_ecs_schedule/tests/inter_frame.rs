#![cfg(feature = "rc")]

use pi_ecs_core::world::World;
use pi_ecs_schedule::{
    executor::{ExecutionMode, Executor},
    inter_frame::SystemFlow,
    resource_param::{Res, ResMut},
    schedule::Schedule,
    stage::{Stage, StageId},
    system::{PortFunction, PortSignature, SharedInput, SystemId, UnitInput},
    system_builder::SystemBuilder,
};
use std::{cell::RefCell, rc::Rc};

struct Batch(Vec<u32>);
struct Timers {
    now: u32,
    due: Vec<(u32, u32)>,
}

fn poll(_: (), mut timers: ResMut<'_, Timers>) -> Option<Batch> {
    let now = timers.now;
    let mut batch = Vec::new();
    timers.due.retain(|&(when, value)| {
        if when <= now {
            batch.push(value);
            false
        } else {
            true
        }
    });
    (!batch.is_empty()).then_some(Batch(batch))
}

fn deliver(
    batch: &Batch,
    (mut timers, mut seen): (ResMut<'_, Timers>, ResMut<'_, Vec<Vec<u32>>>),
) -> Option<()> {
    seen.push(batch.0.clone());
    let next = timers.now + 1;
    timers.due.push((next, 99));
    None
}

#[test]
fn schedule_runs_whole_due_batch_before_its_stage_and_reuses_entry_on_repeat() {
    let mut world = World::new().unwrap();
    world.register_resource::<Timers>().unwrap();
    world.register_resource::<Vec<Vec<u32>>>().unwrap();
    world.finish_registration().unwrap();
    world
        .insert_resource(Timers {
            now: 5,
            due: vec![(4, 10), (5, 20), (6, 30)],
        })
        .unwrap();
    world.insert_resource(Vec::<Vec<u32>>::new()).unwrap();
    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    let mut stage = Stage::new(StageId(1), executor);
    let mut flow = SystemFlow::new();
    flow.register::<(), _, _>(
        SystemBuilder::<_, PortSignature<ResMut<'static, Timers>>>::new(PortFunction::<
            _,
            UnitInput,
            Batch,
        >::new(poll)),
    )
    .unwrap();
    flow.register_shared(SystemBuilder::<
        _,
        PortSignature<(ResMut<'static, Timers>, ResMut<'static, Vec<Vec<u32>>>)>,
    >::new(PortFunction::<_, SharedInput<Batch>, ()>::new(
        deliver,
    )))
    .unwrap();
    assert!(stage.set_entry_flow(flow).is_ok());
    let output = Rc::new(RefCell::new(Vec::new()));
    let output_at_stage = output.clone();
    stage
        .add_system(
            SystemBuilder::new(move |seen: Res<'_, Vec<Vec<u32>>>| {
                output_at_stage.borrow_mut().push(seen.len());
            })
            .build(SystemId(1))
            .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();
    assert!(schedule
        .run_stages(&mut world, &[StageId(1), StageId(99)])
        .is_err());
    assert!(output.borrow().is_empty());
    schedule
        .run_stages(&mut world, &[StageId(1), StageId(1)])
        .unwrap();
    assert_eq!(&*output.borrow(), &[1, 1]);
    world
        .read_scope(|pin| {
            assert_eq!(&*pin.resource::<Vec<Vec<u32>>>().unwrap(), &[vec![10, 20]]);
            assert_eq!(pin.resource::<Timers>().unwrap().now, 5);
        })
        .unwrap();
    schedule.unregister(&mut world).unwrap();
    world.close().unwrap();
}

#[test]
fn dispatch_continues_independent_shared_consumers_after_prepare_rejection() {
    use pi_ecs_schedule::system::OwnedInput;
    let mut world = World::new().unwrap();
    world.register_resource::<u64>().unwrap();
    world.finish_registration().unwrap();
    let mut flow = SystemFlow::new();
    let source = flow
        .register::<String, _, _>(SystemBuilder::<_, PortSignature<()>>::new(PortFunction::<
            _,
            OwnedInput<String>,
            Batch,
        >::new(
            |_: String, ()| Some(Batch(vec![7, 8])),
        )))
        .unwrap();
    flow.register_shared(SystemBuilder::<_, PortSignature<Res<'static, u64>>>::new(
        PortFunction::<_, SharedInput<Batch>, ()>::new(|_: &Batch, _: Res<'_, u64>| None),
    ))
    .unwrap();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let captured = seen.clone();
    flow.register_shared(SystemBuilder::<_, PortSignature<()>>::new(PortFunction::<
        _,
        SharedInput<Batch>,
        (),
    >::new(
        move |batch: &Batch, ()| {
            captured.borrow_mut().push(batch.0.clone());
            None
        },
    )))
    .unwrap();
    assert!(flow.dispatch(&mut world, &source, String::new()).is_err());
    assert!(!world.is_stopped());
    assert_eq!(&*seen.borrow(), &[vec![7, 8]]);
    world.insert_resource(1u64).unwrap();
    flow.dispatch(&mut world, &source, String::new()).unwrap();
    assert_eq!(&*seen.borrow(), &[vec![7, 8], vec![7, 8]]);
    flow.close(&mut world).unwrap();
    world.close().unwrap();
}

#[test]
fn dispatch_barrier_routes_removed_then_propagates_output_and_added() {
    use pi_ecs_core::{
        bundle::ComponentValue,
        component::{Component, PlainAccess, StoragePolicy},
        notification::NoticeKind,
    };
    use pi_ecs_schedule::{
        inter_frame::NoticeRoute, notification_reader::RemovedReader, spawn::Spawn,
    };
    struct First;
    impl Component for First {
        type Access = PlainAccess;
    }
    struct Second;
    impl Component for Second {
        type Access = PlainAccess;
    }
    let mut world = World::new().unwrap();
    let first_id = world
        .register_component::<First>(StoragePolicy::SparseOnly)
        .unwrap();
    let second_id = world
        .register_component::<Second>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let mut flow = SystemFlow::new();
    let start = flow
        .register(SystemBuilder::new(
            |mut spawn: Spawn<'_, ComponentValue<First>>| {
                drop(spawn.spawn(ComponentValue(First)).unwrap());
            },
        ))
        .unwrap();
    let idle = flow.register(SystemBuilder::new(|| {})).unwrap();
    let removed = flow
        .register::<(), _, _>(SystemBuilder::<
            _,
            PortSignature<RemovedReader<'static, First>>,
        >::new(PortFunction::<_, UnitInput, Batch>::new(
            |(), mut notices: RemovedReader<'_, First>| {
                let mut batch = notices.read().unwrap();
                assert!(batch.next().is_some());
                assert!(batch.next().is_none());
                Some(Batch(vec![42]))
            },
        )))
        .unwrap();
    flow.register_shared(SystemBuilder::<
        _,
        PortSignature<Spawn<'static, ComponentValue<Second>>>,
    >::new(PortFunction::<_, SharedInput<Batch>, ()>::new(
        |batch: &Batch, mut spawn: Spawn<'_, ComponentValue<Second>>| {
            assert_eq!(batch.0, [42]);
            drop(spawn.spawn(ComponentValue(Second)).unwrap());
            None
        },
    )))
    .unwrap();
    let events = Rc::new(RefCell::new(Vec::new()));
    let captured = events.clone();
    let added = flow
        .register(SystemBuilder::new(move || {
            captured.borrow_mut().push("added")
        }))
        .unwrap();
    let captured = events.clone();
    let late = flow
        .register(SystemBuilder::new(move || {
            captured.borrow_mut().push("removed")
        }))
        .unwrap();
    flow.wake_on(
        NoticeRoute {
            component: first_id,
            kind: NoticeKind::Removed,
        },
        &removed,
    )
    .unwrap();
    flow.wake_on(
        NoticeRoute {
            component: second_id,
            kind: NoticeKind::Added,
        },
        &added,
    )
    .unwrap();
    flow.wake_on(
        NoticeRoute {
            component: second_id,
            kind: NoticeKind::Removed,
        },
        &late,
    )
    .unwrap();
    flow.dispatch(&mut world, &start, ()).unwrap();
    assert_eq!(&*events.borrow(), &["added"]);
    flow.dispatch(&mut world, &idle, ()).unwrap();
    assert_eq!(&*events.borrow(), &["added", "removed"]);
    flow.close(&mut world).unwrap();
    world.close().unwrap();
}

#[test]
fn static_dispatch_rejection_leaves_only_cleanup_available() {
    use pi_ecs_schedule::system::OwnedInput;
    let mut world = World::new().unwrap();
    world.finish_registration().unwrap();
    let mut flow = SystemFlow::new();
    let entry = flow
        .register(SystemBuilder::new(|| panic!("rejected entry ran")))
        .unwrap();
    flow.register::<u32, _, _>(SystemBuilder::<_, PortSignature<()>>::new(PortFunction::<
        _,
        OwnedInput<u32>,
        u32,
    >::new(
        |value, ()| Some(value),
    )))
    .unwrap();

    let error = flow.dispatch(&mut world, &entry, ()).unwrap_err();
    assert!(format!("{error:?}").contains("输出触发环"));
    assert!(!world.is_stopped());
    drop(world.pin().unwrap());
    assert!(flow
        .register::<(), _, _>(SystemBuilder::new(|| {}))
        .is_err());
    let error = flow.dispatch(&mut world, &entry, ()).unwrap_err();
    assert!(format!("{error:?}").contains("入口图不可运行"));
    let mut other = World::new().unwrap();
    other.finish_registration().unwrap();
    assert!(flow.close(&mut other).is_err());
    other.close().unwrap();
    flow.close(&mut world).unwrap();
    world.close().unwrap();
}
