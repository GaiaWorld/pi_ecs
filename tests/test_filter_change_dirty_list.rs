/// 测试Filter: Changed
/// 该过滤器基于监听器，使用组件的修改发出了事件，Changed才能感知到
/// 每Query会创建一个脏列表，组件发生改变后通知监听器，监听器会负责将改变组件对应的实体放入脏列表中

use std::sync::Arc;

use pi_ecs::{prelude::{Query, IntoSystem, StageBuilder, SingleDispatcher, Dispatcher}, entity::Id, world::World, storage::Offset};
use pi_ecs::query::filter::Changed;
use pi_async::rt::{multi_thread::MultiTaskRuntime, AsyncRuntimeBuilder};




pub struct Node;

#[derive(Debug)]
/// 定义一个组件类型
pub struct Position(pub usize);


/// 测试组件脏
///迭代出脏的Position和对应的entity
pub fn iter_dirty(
	q: Query<Node, (Id<Node>, &Position), Changed<Position>>,
) {
	for r in q.iter(){
		println!("modify {:?}, {:?}", r.0.offset(), r.1);
	}
}

#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 创建一个名为Node的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Node>()
		.register::<Position>()
		.create();

	let dispatcher = create_dispatcher(&mut world);

	let mut entitys = Vec::new();
	// 创建原型为Node的实体，并为该实体添加组件（必须是在Node中注册过的组件， 否则无法插入）
	for i in 0..3 {
		let id = world.spawn::<Node>()
		.insert(Position(i))
		.entity();
		entitys.push(id);
	}

	println!("change count 3: ");
	dispatcher.run();
	std::thread::sleep(std::time::Duration::new(1, 0));

	world.insert_component(entitys[1].clone(), Position(10));
	println!("change count 1: ");
	dispatcher.run();

	std::thread::sleep(std::time::Duration::new(2, 0));
}


fn create_dispatcher(world: &mut World) -> SingleDispatcher<MultiTaskRuntime> {
	let rt = AsyncRuntimeBuilder::default_multi_thread(
		None,
		None,
		None,
		None,
	);
	let iter_dirty_system = iter_dirty.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(iter_dirty_system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build(world)));
	let mut dispatcher = SingleDispatcher::new(rt);
	dispatcher.init(stages, world);

	return dispatcher;
}