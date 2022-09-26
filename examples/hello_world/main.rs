use std::sync::Arc;

use pi_async::prelude::multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool};
use pi_ecs::prelude::{World, StageBuilder, IntoSystem, SingleDispatcher, Dispatcher};


fn hello_world() {
	println!("hello world!");
}

fn main() {
	let mut world = World::new();
	let pool = MultiTaskRuntimeBuilder::<(), StealableTaskPool<()>>::default();
	// 创建一个运行时
	let rt = pool.build();

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