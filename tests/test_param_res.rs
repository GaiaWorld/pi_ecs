/// 测试系统参数Res、ResMut

use pi_ecs::{prelude::{World, StageBuilder, SingleDispatcher, Dispatcher, Res, ResMut}, sys::system::IntoSystem};
use pi_async::prelude::{multi_thread::MultiTaskRuntime, AsyncRuntimeBuilder};
use std::sync::Arc;

#[derive(Debug)]
struct Resource1(pub usize);

#[derive(Debug)]
struct Resource2(pub usize);

/// 资源参数，可以从World上取到对应的参数
/// 注意在注册该系统时，应该保证其获取的资源已经注册时World中，否则会崩溃
/// 资源可以有可变和只读两种参数
fn res(
	res1: Res<Resource1>,
	res2: ResMut<Resource2>,
) {
	println!("res run, res1: {:?}, res2: {:?}", *res1, *res2);
}

#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 在创建system之前插入资源
	world.insert_resource(Resource1(1));
	world.insert_resource(Resource2(2));

	// 创建派发器
	let dispatcher = get_dispatcher(&mut world);

	println!("测试资源参数：");
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
	let system = res.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build(world)));
	let mut dispatcher = SingleDispatcher::new(rt);
	dispatcher.init(stages, world);

	dispatcher
}
