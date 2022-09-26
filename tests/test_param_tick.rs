/// 测试系统参数Tick

use pi_ecs::{prelude::{World, StageBuilder, SingleDispatcher, Dispatcher, Tick}, sys::system::IntoSystem};
use pi_async::prelude::{multi_thread::MultiTaskRuntime, AsyncRuntimeBuilder};
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

fn get_dispatcher(world: &mut World) -> SingleDispatcher<MultiTaskRuntime> {
	let rt = AsyncRuntimeBuilder::default_multi_thread(
		None,
		None,
		None,
		None,
	);
	let system = tick.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build(world)));
	let mut dispatcher = SingleDispatcher::new(rt);
	dispatcher.init(stages, world);

	dispatcher
}
