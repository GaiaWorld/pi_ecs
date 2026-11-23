#![cfg(feature = "rc")]

use pi_ecs_app::app::App;
use pi_ecs_core::{command::CommandCapacity, world::World};

#[test]
fn app_posts_only_to_an_existing_world_queue() {
    let mut world = World::new().unwrap();
    world
        .register_command::<u32>(CommandCapacity::Unbounded)
        .unwrap();
    let mut app = App::new(world).unwrap();
    app.finish().unwrap();
    assert!(app.post(42u32).is_ok());
    assert!(app.world().command_queue_status::<u32>().unwrap().ready);
    assert!(app.post(17u64).is_err());
    app.close().unwrap();
}

#[test]
fn app_yields_then_runs_one_ready_queue_by_priority_and_binding_order() {
    use pi_ecs_core::command::CommandRead;
    use pi_ecs_schedule::{
        inter_frame::{FrameBudget, QueuePriority, ReadyRun},
        system_builder::SystemBuilder,
    };
    use std::{
        cell::RefCell,
        rc::Rc,
        time::{Duration, Instant},
    };

    let mut world = World::new().unwrap();
    world
        .register_command::<u32>(CommandCapacity::Unbounded)
        .unwrap();
    world
        .register_command::<i32>(CommandCapacity::Unbounded)
        .unwrap();
    let mut app = App::new(world).unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut flow = app.inter_frame_mut().unwrap();
    let captured = calls.clone();
    let first = flow
        .register(SystemBuilder::new(
            move |mut input: CommandRead<'_, u32>| {
                captured
                    .borrow_mut()
                    .push(("first", input.read().unwrap().next().unwrap() as i64));
            },
        ))
        .unwrap();
    let captured = calls.clone();
    let second = flow
        .register(SystemBuilder::new(
            move |mut input: CommandRead<'_, i32>| {
                captured
                    .borrow_mut()
                    .push(("second", i64::from(input.read().unwrap().next().unwrap())));
            },
        ))
        .unwrap();
    flow.bind_queue::<u32>(&first).unwrap();
    flow.bind_queue::<i32>(&second).unwrap();
    app.finish().unwrap();
    assert!(app.post(1u32).is_ok());
    assert!(app.post(2u32).is_ok());
    assert!(app.post(5i32).is_ok());
    let now = Instant::now();
    let budget = || FrameBudget {
        now,
        frame_at: now + Duration::from_secs(1),
        reserve: Duration::ZERO,
    };
    let yielding = FrameBudget {
        now,
        frame_at: now + Duration::from_millis(2),
        reserve: Duration::from_millis(2),
    };
    assert_eq!(
        app.run_next_ready(yielding).unwrap(),
        ReadyRun::YieldToFrame
    );
    assert!(calls.borrow().is_empty());
    assert!(app.world().command_queue_status::<u32>().unwrap().ready);
    assert_eq!(app.run_next_ready(budget()).unwrap(), ReadyRun::Ran);
    app.set_queue_priority::<i32>(QueuePriority::High).unwrap();
    assert_eq!(app.run_next_ready(budget()).unwrap(), ReadyRun::Ran);
    assert_eq!(app.run_next_ready(budget()).unwrap(), ReadyRun::Ran);
    assert_eq!(app.run_next_ready(budget()).unwrap(), ReadyRun::Idle);
    assert_eq!(
        &*calls.borrow(),
        &[("first", 1), ("second", 5), ("first", 2)]
    );
    assert!(app.dispatch(&first.clone(), ()).is_err());
    app.close().unwrap();
}

#[test]
fn app_dispatches_an_owned_value_through_the_original_handle() {
    use pi_ecs_schedule::{
        system::{OwnedInput, PortFunction, PortSignature},
        system_builder::SystemBuilder,
    };
    use std::{cell::RefCell, rc::Rc};
    let mut app = App::new(World::new().unwrap()).unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let seen = log.clone();
    let target = app
        .inter_frame_mut()
        .unwrap()
        .register::<String, _, _>(SystemBuilder::<_, PortSignature<()>>::new(PortFunction::<
            _,
            OwnedInput<String>,
            (),
        >::new(
            move |value: String, ()| {
                seen.borrow_mut().push(value);
                None
            },
        )))
        .unwrap();
    let mut other = App::new(World::new().unwrap()).unwrap();
    app.finish().unwrap();
    other.finish().unwrap();
    assert!(other.dispatch(&target, "foreign".into()).is_err());
    app.dispatch(&target.clone(), "owned".into()).unwrap();
    assert_eq!(&*log.borrow(), &["owned"]);
    app.close().unwrap();
    assert!(app.dispatch(&target, "late".into()).is_err());
    other.close().unwrap();
}

#[test]
fn app_rejects_misbound_receiver_and_releases_prior_claim_on_close() {
    use pi_ecs_core::command::CommandRead;
    use pi_ecs_schedule::{inter_frame::FrameBudget, system_builder::SystemBuilder};
    use std::time::{Duration, Instant};
    let mut world = World::new().unwrap();
    world
        .register_command::<u32>(CommandCapacity::Unbounded)
        .unwrap();
    world
        .register_command::<i32>(CommandCapacity::Unbounded)
        .unwrap();
    let mut app = App::new(world).unwrap();
    let mut flow = app.inter_frame_mut().unwrap();
    let first = flow
        .register(SystemBuilder::new(|_: CommandRead<'_, u32>| {
            panic!("queue must not run")
        }))
        .unwrap();
    flow.bind_queue::<u32>(&first).unwrap();
    let invalid = flow
        .register(SystemBuilder::new(|| panic!("queue must not run")))
        .unwrap();
    flow.bind_queue::<i32>(&invalid).unwrap();
    app.finish().unwrap();
    assert!(app.post(11u32).is_ok());
    let now = Instant::now();
    let error = app
        .run_next_ready(FrameBudget {
            now,
            frame_at: now + Duration::from_secs(1),
            reserve: Duration::ZERO,
        })
        .unwrap_err();
    assert!(format!("{error:?}").contains("实际接收资格"));
    assert!(!app.world().is_stopped());
    let status = app.world().command_queue_status::<u32>().unwrap();
    assert!(status.ready && status.receiver_claim.is_some());
    app.close().unwrap();
}

#[test]
fn completion_sent_during_frame_waits_for_a_non_yielding_inter_frame_round() {
    use pi_ecs_app::app::ScheduleLabel;
    use pi_ecs_core::command::CommandRead;
    use pi_ecs_schedule::{
        executor::{ExecutionMode, Executor},
        inter_frame::{FrameBudget, ReadyRun},
        schedule::Schedule,
        stage::{Stage, StageId},
        system::SystemId,
        system_builder::SystemBuilder,
    };
    use std::{
        cell::Cell,
        rc::Rc,
        time::{Duration, Instant},
    };

    struct Frame;
    let mut world = World::new().unwrap();
    world
        .register_command::<String>(CommandCapacity::Unbounded)
        .unwrap();
    let sender = world.command_sender::<String>().unwrap();
    let mut app = App::new(world).unwrap();
    let consumed = Rc::new(Cell::new(false));
    let seen = consumed.clone();
    let mut flow = app.inter_frame_mut().unwrap();
    let target = flow
        .register(SystemBuilder::new(
            move |mut input: CommandRead<'_, String>| {
                assert_eq!(input.read().unwrap().next().as_deref(), Some("complete"));
                assert!(!seen.replace(true));
            },
        ))
        .unwrap();
    flow.bind_queue::<String>(&target).unwrap();

    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    let mut stage = Stage::new(StageId(1), executor);
    let seen = consumed.clone();
    stage
        .add_system(
            SystemBuilder::new(move || {
                assert!(sender.send(String::from("complete")).is_ok());
                assert!(!seen.get());
            })
            .build(SystemId(1))
            .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();
    let label = ScheduleLabel::of::<Frame>();
    app.add_schedule(label, schedule).unwrap();
    app.finish().unwrap();
    let now = Instant::now();
    let available = || FrameBudget {
        now,
        frame_at: now + Duration::from_secs(1),
        reserve: Duration::ZERO,
    };
    assert_eq!(app.run_next_ready(available()).unwrap(), ReadyRun::Idle);
    app.run_stages(label, &[StageId(1)]).unwrap();
    assert!(!consumed.get());
    assert!(app.world().command_queue_status::<String>().unwrap().ready);
    let yielding = FrameBudget {
        now,
        frame_at: now,
        reserve: Duration::ZERO,
    };
    assert_eq!(
        app.run_next_ready(yielding).unwrap(),
        ReadyRun::YieldToFrame
    );
    assert!(!consumed.get());
    assert!(app.world().command_queue_status::<String>().unwrap().ready);
    assert_eq!(app.run_next_ready(available()).unwrap(), ReadyRun::Ran);
    assert!(consumed.get());
    assert_eq!(app.run_next_ready(available()).unwrap(), ReadyRun::Idle);
    app.close().unwrap();
}
