/// 测试系统参数Local

use pi_ecs::{prelude::{World, StageBuilder, SingleDispatcher, Dispatcher, Local}, sys::system::IntoSystem, world::FromWorld};
use r#async::rt::{multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool}, AsyncRuntime};
use std::sync::Arc;

#[derive(Debug)]
pub struct Local1;

impl FromWorld for Local1 {
	fn from_world(_world: &mut World) -> Self {
		Local1
	}
}

#[derive(Debug, Default)]

pub struct Local2;


/// 在system中使用Local
/// * Local通常时system自身的数据，system2无法看到system1的Local
/// * 所有的Local中的泛型都必须实现FromWorld
/// * pi_ecs框架已经为所有实现的Default的类型自动实现的FromWorld
/// 	+ 如Local1显示的实现了FromWorld，可以作为Local的泛型
/// 	+ 如Local2实现了Default，也可以作为Local的泛型

fn local(
	local1: Local<Local1>,
	local2: Local<Local2>,
) {
	println!("local run, local1: {:?}, local2: {:?}", *local1, *local2);
}

/// 测试=系统参数Join
#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	let dispatcher = get_dispatcher(&mut world);

	println!("测试Local参数：");
	dispatcher.run();

	std::thread::sleep(std::time::Duration::from_secs(1));
}

fn get_dispatcher(world: &mut World) -> SingleDispatcher<StealableTaskPool<()>> {
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());
	let system = local.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build()));
	let dispatcher = SingleDispatcher::new(stages , rt);

	dispatcher
}

