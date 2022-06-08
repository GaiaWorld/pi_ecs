use std::sync::Arc;

use pi_async::rt::{multi_thread::MultiTaskRuntimeBuilder, AsyncRuntime};
use pi_ecs::prelude::{World, StageBuilder, IntoSystem, SingleDispatcher, Dispatcher, Query};

struct Person;
struct Name(String);

fn add_people(world: &mut World) {
	world.spawn::<Person>().insert(Name("Elaina Proctor".to_string()));
	world.spawn::<Person>().insert(Name("Renzo Hume".to_string()));
	world.spawn::<Person>().insert(Name("Zayna Nieves".to_string()));
}

fn greet_people(query: Query<Person, &Name>) {
	for name in query.iter() {
		println!("hello {}!", name.0);
	}
}

fn main() {
	let mut world = World::new();
	// 添加People
	add_people(&mut world);

	// 创建一个运行时
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());

	// 创建阶段
	let mut stage = StageBuilder::new();
	stage.add_node(greet_people.system(&mut world));

	// 创建派发器
	let mut dispatcher = SingleDispatcher::new(rt);
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build(&world)));
	dispatcher.init(stages, &world);

	// 运行派发器，通常每帧推动
	dispatcher.run();
}