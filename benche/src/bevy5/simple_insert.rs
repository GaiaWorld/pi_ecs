use bevy_5_ecs::prelude::*;
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

pub struct Benchmark(World);

impl Benchmark {
    pub fn new() -> Self {
		let world = World::new();
        Self(world)
    }

    pub fn run(&mut self) {
        let world = &mut self.0;
		for _i in 0..10_000 {
			world.spawn().insert_bundle((
                Transform(Matrix4::from_scale(1.0)),
                Position(Vector3::unit_x()),
                Rotation(Vector3::unit_x()),
                Velocity(Vector3::unit_x()),
            ));
		}
    }
}


pub struct Benchmark1(World);

impl Benchmark1 {
    pub fn new() -> Self {
		let world = World::new();
        Self(world)
    }

    pub fn run(&mut self) {
        let world = &mut self.0;
		for _i in 0..10_000 {
			world.spawn().insert_bundle((
                Transform(Matrix4::from_scale(1.0)),
                Position(Vector3::unit_x()),
            ));

			world.spawn().insert(Rotation(Vector3::unit_x()));
			world.spawn().insert(Velocity(Vector3::unit_x()));
		}
    }
}