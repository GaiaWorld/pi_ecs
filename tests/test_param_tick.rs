/// 测试系统参数Tick

use pi_ecs::{prelude::{World, StageBuilder, SingleDispatcher, Dispatcher, Tick}, sys::system::IntoSystem};
use pi_async::rt::{multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool}, AsyncRuntime};
use std::sync::Arc;

/// Tick可以获得World上的节拍
fn tick(
	tick: Tick,
) {
	println!("tick run, change_tick: {:?}, last_change_tick: {:?}", tick.change_tick, tick.last_change_tick);
}

/// 测试系统参数Tick
#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 创建派发器
	let dispatcher = get_dispatcher(&mut world);

	// 第一次运行，节拍为1
	println!("第一次运行：");
	dispatcher.run();

	// 第二次运行，节拍为2
	println!("第二次运行：");
	dispatcher.run();

	std::thread::sleep(std::time::Duration::from_secs(1));
}

fn get_dispatcher(world: &mut World) -> SingleDispatcher<StealableTaskPool<()>> {
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());
	let system = tick.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build()));
	let dispatcher = SingleDispatcher::new(stages, world, rt);

	dispatcher
}
