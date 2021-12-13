use pi_ecs::prelude::*;
// use bevy_5_ecs::prelude::EventReader;
use cgmath::*;

#[derive(Copy, Clone)]
struct Transform(Matrix4<f32>);

#[derive(Copy, Clone)]
struct Position(Vector3<f32>);

#[derive(Copy, Clone)]
struct Rotation(Vector3<f32>);

#[derive(Copy, Clone)]
struct Velocity(Vector3<f32>);
pub struct Node;
pub struct Benchmark(World);

impl Benchmark {
    pub fn new() -> Self {
		let mut world = World::new();
		world.new_archetype::<Node>()
			.add::<Velocity>()
			.add::<Rotation>()
			.add::<Transform>()
			.add::<Position>()
			.create();
        Self(world)
    }

    pub fn run(&mut self) {
        let world = &mut self.0;
		for _i in 0..10_000 {
			world.spawn::<Node>()
			.insert(Velocity(Vector3::unit_x()))
			.insert(Position(Vector3::unit_x()))
			.insert(Transform(Matrix4::from_scale(1.0))) 
			.insert(Rotation(Vector3::unit_x())).id();
		}
    }
}
