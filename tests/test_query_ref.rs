/// 测试组件引用查询（&， &mut）

use pi_ecs::{prelude::{World, StageBuilder, SingleDispatcher, Dispatcher, Query}, sys::system::IntoSystem};
use pi_async::rt::{multi_thread::MultiTaskRuntime, AsyncRuntimeBuilder};
use std::sync::Arc;

#[derive(Debug)]
pub struct Archetype1;

#[derive(Debug)]
pub struct Component1(pub usize);

#[derive(Debug)]
pub struct Component2(pub usize);


/// 在system中查询组件的引用
/// * 可以是只读引用，也可以是可写引用

fn ref_(
	mut query: Query<Archetype1, (&Component1, &mut Component2)>,
) {
	for i in query.iter_mut() {
		println!("local run, {:?} {:?}", i.0, i.1);
	}
	
}

#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 创建一个名为Node1的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Archetype1>()
		.register::<Component1>()
		.register::<Component2>()
		.create();

	// 创建原型为Node2的实体，并为该实体添加组件（必须是在Node2中注册过的组件， 否则无法插入）
	for i in 0..2 {
		world.spawn::<Archetype1>()
		.insert(Component1(i))
		.insert(Component2(i))
		.id();
	}

	let dispatcher = get_dispatcher(&mut world);

	println!("测试查询组件的引用：");
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
	let system = ref_.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build(world)));
	let mut dispatcher = SingleDispatcher::new(rt);
	dispatcher.init(stages, world);

	dispatcher
}

