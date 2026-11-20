#![cfg(feature = "rc")]

use pi_ecs_core::world::World;
use pi_ecs_schedule::{
    executor::{ExecutionMode, Executor},
    local::{Local, LocalConfig},
    param_composition::{NamedParam, ParamSet, ParamSetItem, ParamView},
    resource_param::{MultiRes, MultiResConfig, MultiResMut},
    schedule::Schedule,
    stage::{Stage, StageId},
    system::{SystemFunction, SystemId},
    system_builder::SystemBuilder,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

struct Value {
    tag: u8,
    count: u32,
    drops: Arc<Mutex<Vec<u8>>>,
}

impl Drop for Value {
    fn drop(&mut self) {
        self.drops.lock().unwrap().push(self.tag);
    }
}

type Slot = MultiResMut<'static, Value>;
type Slots = ParamSet<(Slot, Slot)>;
struct View;
impl ParamView<Slots> for View {
    type Item<'r> = ParamSetItem<'r, 'r, Slot, Slot>;
    fn from_param<'r>(item: Self::Item<'r>) -> Self::Item<'r> {
        item
    }
}

struct Update;
impl SystemFunction<()> for Update {
    type Param = (Local<'static, u8>, (Slot, NamedParam<Slots, View>));
    fn call<'r>(
        &mut self,
        (mut runs, (mut direct, mut set)): (
            Local<'r, u8>,
            (MultiResMut<'r, Value>, ParamSetItem<'r, 'r, Slot, Slot>),
        ),
    ) {
        *runs += 1;
        assert_eq!(direct.tag, 2);
        direct.count += 2;
        // Runtime member selection order does not determine registration positions.
        assert_eq!(set.p1().tag, 4);
        set.p1().count += 4;
        assert_eq!(set.p0().tag, 3);
        set.p0().count += 3;
        assert_eq!(set.p1().count, 40 + 4 * u32::from(*runs));
    }
}

#[test]
fn producers_keep_independent_slots_and_rebind_after_removal() {
    let mut world = World::new().unwrap();
    world.register_multi_resource::<Value>().unwrap();
    world.finish_registration().unwrap();
    let drops = Arc::new(Mutex::new(Vec::new()));
    let config = |tag, count| {
        MultiResConfig::new(Value {
            tag,
            count,
            drops: drops.clone(),
        })
    };
    let seen = Rc::new(RefCell::new(Vec::new()));
    let observed = seen.clone();
    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    let stage_id = StageId(1);
    let mut stage = Stage::new(stage_id, executor);
    stage
        .add_system(
            SystemBuilder::new(move |all: MultiRes<'_, Value>| {
                let mut values: Vec<_> = all
                    .iter()
                    .map(|v| (v.tag, v.count, v.is_changed()))
                    .collect();
                values.sort_unstable();
                observed
                    .borrow_mut()
                    .push((all.is_structure_changed(), values));
            })
            .build(SystemId(11))
            .unwrap(),
        )
        .unwrap();
    let removed = SystemId(37);
    stage
        .add_system(
            SystemBuilder::with_config(
                |mut slot: MultiResMut<'_, Value>| slot.count += 1,
                (config(1, 10),),
            )
            .build(removed)
            .unwrap(),
        )
        .unwrap();
    stage
        .add_system(
            SystemBuilder::with_config(
                Update,
                (
                    LocalConfig::new(0),
                    (config(2, 20), (config(3, 30), config(4, 40))),
                ),
            )
            .build(SystemId(62))
            .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();

    schedule.run(&mut world).unwrap();
    schedule.run(&mut world).unwrap();
    assert!(drops.lock().unwrap().is_empty());
    schedule
        .stage_mut(stage_id)
        .unwrap()
        .unregister_system(removed, &mut world)
        .unwrap();
    assert_eq!(*drops.lock().unwrap(), [1]);
    schedule.run(&mut world).unwrap();
    assert_eq!(
        *seen.borrow(),
        [
            (
                true,
                vec![(1, 11, true), (2, 22, true), (3, 33, true), (4, 44, true)]
            ),
            (
                false,
                vec![(1, 12, true), (2, 24, true), (3, 36, true), (4, 48, true)]
            ),
            (true, vec![(2, 26, true), (3, 39, true), (4, 52, true)]),
        ]
    );
    schedule.unregister(&mut world).unwrap();
    world.close().unwrap();
    let mut dropped = drops.lock().unwrap();
    dropped.sort_unstable();
    assert_eq!(*dropped, [1, 2, 3, 4]);
}

#[test]
fn failed_nested_registration_cleans_only_its_own_slots() {
    let mut world = World::new().unwrap();
    world.register_multi_resource::<Value>().unwrap();
    world.finish_registration().unwrap();
    let drops = Arc::new(Mutex::new(Vec::new()));
    let config = |tag| {
        MultiResConfig::new(Value {
            tag,
            count: 0,
            drops: drops.clone(),
        })
    };
    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    let stage_id = StageId(1);
    let mut stage = Stage::new(stage_id, executor);
    let keep = SystemBuilder::with_config(
        |mut slot: MultiResMut<'_, Value>| slot.count += 1,
        (config(1),),
    )
    .build(SystemId(9))
    .unwrap();
    stage.add_system(keep).unwrap();
    let fail_id = SystemId(20);
    let fail = SystemBuilder::with_config(
        |_: MultiResMut<'_, Value>, _: (MultiResMut<'_, Value>, MultiResMut<'_, u64>)| {
            panic!("failed registration must not run")
        },
        (config(2), (config(3), MultiResConfig::new(7_u64))),
    )
    .build(fail_id)
    .unwrap();
    stage.add_system(fail).unwrap();
    schedule.add_stage(stage).unwrap();

    assert!(schedule.run(&mut world).is_err());
    assert!(!world.is_stopped());
    assert_eq!(*drops.lock().unwrap(), [3, 2]);
    let slots = |world: &mut World| {
        world
            .read_scope(|pin| {
                pin.multi_resource::<Value>()
                    .unwrap()
                    .iter()
                    .map(|v| (v.tag, v.count))
                    .collect::<Vec<_>>()
            })
            .unwrap()
    };
    assert_eq!(slots(&mut world), [(1, 0)]);
    schedule
        .stage_mut(stage_id)
        .unwrap()
        .unregister_system(fail_id, &mut world)
        .unwrap();
    schedule.run(&mut world).unwrap();
    assert_eq!(slots(&mut world), [(1, 1)]);
    schedule.unregister(&mut world).unwrap();
    world.close().unwrap();
    assert_eq!(*drops.lock().unwrap(), [3, 2, 1]);
}
