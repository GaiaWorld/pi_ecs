use pi_ecs::{
    entity::AllocationDomain,
    prelude::*,
    schedule::{
        executor::{ExecutionMode, Executor},
        stage::{Stage, StageId},
        system::SystemId,
        system_builder::SystemBuilder,
    },
};

struct Position(i32);
impl Component for Position {
    type Access = PlainAccess;
}

struct Velocity(i32);
impl Component for Velocity {
    type Access = PlainAccess;
}

#[derive(QueryData)]
struct Motion {
    entity: Entity,
    position: &'static Position,
    velocity: Option<&'static Velocity>,
}

fn seed_stationary(mut spawn: Spawn<'_, ComponentValue<Position>>) {
    // Keep the strong responsibility until this one-shot example closes the World.
    spawn.spawn(ComponentValue(Position(4))).unwrap().forget();
}

fn seed_moving(mut spawn: Spawn<'_, (ComponentValue<Position>, ComponentValue<Velocity>)>) {
    spawn
        .spawn((ComponentValue(Position(10)), ComponentValue(Velocity(3))))
        .unwrap()
        .forget();
}

fn read_motion(query: Query<'_, Motion>) {
    for item in query.iter().unwrap() {
        let next = item.position.0 + item.velocity.map_or(0, |velocity| velocity.0);
        println!("position={} next={next}", item.position.0);
        let _entity = item.entity;
    }
}

struct Frame;

fn main() -> Result<(), EcsError> {
    let mut app = App::new(World::new()?).map_err(|rejected| rejected.error)?;
    let mut world = app.world_mut()?;
    let position = world.register_component::<Position>(StoragePolicy::TableEligible)?;
    world.register_component::<Velocity>(StoragePolicy::SparseOnly)?;
    world.register_table(&[position])?;

    let mut schedule = Schedule::new();
    let executor = schedule.add_executor(Executor::new(ExecutionMode::Serial)?)?;
    let mut stage = Stage::new(StageId(0), executor);
    // Register the reader first to demonstrate the inferred Spawn -> reader edge.
    stage
        .add_system(SystemBuilder::new(read_motion).build(SystemId(0))?)
        .map_err(|rejected| rejected.error)?;
    stage
        .add_system(SystemBuilder::new(seed_stationary).build(SystemId(1))?)
        .map_err(|rejected| rejected.error)?;
    stage
        .add_system(SystemBuilder::new(seed_moving).build(SystemId(2))?)
        .map_err(|rejected| rejected.error)?;
    schedule
        .add_stage(stage)
        .map_err(|rejected| rejected.error)?;

    let frame = ScheduleLabel::of::<Frame>();
    app.add_schedule(frame, schedule)
        .map_err(|rejected| rejected.error)?;
    app.finish()?;
    app.run(frame)?;
    app.close()
}
