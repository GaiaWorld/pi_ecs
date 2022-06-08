use std::sync::Arc;

use pi_async::rt::{multi_thread::MultiTaskRuntimeBuilder, AsyncRuntime};
use pi_ecs::prelude::{World, StageBuilder, IntoSystem, SingleDispatcher, Dispatcher};


fn hello_world() {
	println!("hello world!");
}

fn main() {
	let mut world = World::new();
	// 创建一个运行时
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());

	// 创建阶段
	let mut stage = StageBuilder::new();
	stage.add_node(hello_world.system(&mut world));

	// 创建派发器
	let mut dispatcher = SingleDispatcher::new(rt);
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build(&world)));
	dispatcher.init(stages, &world);

	// 运行派发器，通常每帧推动
	dispatcher.run();
}