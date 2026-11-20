use bevy_5_ecs::prelude::*;
use cgmath::*;

#[derive(Copy, Clone)]
struct Transform(Matrix4<f32>);

#[derive(Copy, Clone)]
struct Position(Vector3<f32>);

#[derive(Copy, Clone)]
struct Rotation(Vector3<f32>);

#[derive(Copy, Clone)]
struct Velocity(Vector3<f32>);

pub static mut ggg:f32 = 0.0;
pub struct Benchmark{
	world: World,
	dirtys: Vec<Entity,>
}

impl Benchmark {
    pub fn new() -> Self {
        let mut world = World::new();
		let mut dirtys = Vec::new();
		for i in 0..10_000 {
			let e = world.spawn().insert_bundle((
				Transform(Matrix4::from_scale(1.0)),
                Position(Vector3::unit_x()),
                Rotation(Vector3::unit_x()),
                Velocity(Vector3::unit_x()),
			)).id();
			dirtys.push(e);
		}
        Self{world, dirtys}
    }

    pub fn run(&mut self) {
		let mut query = self.world.query::<(&Velocity, &Position)>();
        for (velocity, position) in query.iter(&mut self.world) {
			unsafe { ggg += velocity.0.x };
        }
    }

	pub fn run_query(&mut self) {
		let mut query = self.world.query::<(&Velocity, &Position)>();
		query.validate_world_and_update_archetypes(&self.world);
		let r1 = self.world.last_change_tick();
		let r2 = self.world.read_change_tick();
        // let t = std::time::Instant::now();
		for e in self.dirtys.iter() {
			unsafe { ggg += query.get_unchecked_manual(&mut self.world, e.clone(), r1 , r2).unwrap().0.0.x };
		}
    }
}

#[test]
fn tt() {
	let mut bb = Benchmark::new();
	bb.run_query();
}


// #[test]
// fn tt() {
// 	let mut world = World::new();
// 	let i = world.spawn().insert_bundle((
// 		Transform(Matrix4::from_scale(1.0)),
// 		Position(Vector3::unit_x()),
// 		Rotation(Vector3::unit_x()),
// 		Velocity(Vector3::unit_x()),
// 	)).id();
// 	world.spawn_batch((0..10_000).map(|_| {
// 		(
// 			Transform(Matrix4::from_scale(1.0)),
// 			Position(Vector3::unit_x()),
// 			Rotation(Vector3::unit_x()),
// 			Velocity(Vector3::unit_x()),
// 		)
// 	}));
	
// 	// let mut query = world.query::<(&mut Velocity, &mut Position)>();
// 	let mut query = world.query::<(&Velocity, &Position)>();
// 	let r = query.get(&world, i);
// 	let r = query.get(&world, i);
// 	// for (velocity, mut position) in query.iter_mut(&mut world) {
// 	// 	position.0 += velocity.0;
// 	// }
// }
