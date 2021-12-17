use pi_ecs::prelude::*;
use cgmath::*;

#[derive(Copy, Clone)]
struct Transform(Matrix4<f32>);

#[derive(Copy, Clone)]
struct Position(Vector3<f32>);

#[derive(Copy, Clone)]
struct Rotation(Vector3<f32>);

#[derive(Copy, Clone)]
struct Velocity(Vector3<f32>);

pub struct Benchmark{
	world: World,
	dirtys: Vec<Entity,>
}

pub struct Node;

pub static mut ggg:f32 = 0.0;
impl Benchmark {
    pub fn new() -> Self {
        let mut world = World::new();
		world.new_archetype::<Node>()
			.register::<Velocity>()
			.register::<Rotation>()
			.register::<Transform>()
			.register::<Position>()
			.create();
		let mut dirtys = Vec::new();
		for i in 0..10000 {
			let id = world.spawn::<Node>()
			.insert(Velocity(Vector3::unit_x()))
			.insert(Position(Vector3::unit_x()))
			.insert(Transform(Matrix4::from_scale(1.0))) 
			.insert(Rotation(Vector3::unit_x())).id();
			dirtys.push(id);
				
		}

        Self{world, dirtys}
    }

    pub fn run(&mut self) {
		let mut query = self.world.query::<(&Velocity, &Position)>();
        // let t = std::time::Instant::now();
        for (velocity, position) in query.iter(&mut self.world) {
			unsafe { ggg += velocity.0.x };
            // position.0 += velocity.0;
        }
		// println!("rrr========={:?}, {:?}", std::time::Instant::now() - t, unsafe{ggg});
    }

	pub fn run_query(&mut self) {
		let mut query = self.world.query::<(&Velocity, &Position)>();
		query.validate_world_and_update_archetypes(&self.world);
		let r1 = self.world.last_change_tick();
		let r2 = self.world.read_change_tick();
        // let t = std::time::Instant::now();
		for e in self.dirtys.iter() {
			unsafe { ggg += query.get_unchecked_manual(&mut self.world, e.clone(), r1 , r2).unwrap().unwrap().0.0.x };
		}
    }
}

#[test]
fn tt() {
	let mut bb = Benchmark::new();
	bb.run();
}
