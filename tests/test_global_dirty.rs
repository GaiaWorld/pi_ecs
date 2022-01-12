use std::sync::Arc;

use pi_ecs::{prelude::{Query, Changed, IntoSystem, StageBuilder, SingleDispatcher, Dispatcher}, entity::Entity, world::World};
use r#async::rt::{AsyncRuntime, multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool}};


pub struct Node;

#[derive(Debug)]
/// 定义一个组件类型
pub struct Position(pub usize);


/// 测试组件脏
///迭代出脏的Position和对应的entity
pub fn iter_dirty(
	q: Query<Node, (Entity, &Position), Changed<Position>>,
) {
	for r in q.iter(){
		println!("modify {:?}, {:?}", r.0, r.1);
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
		.id();
		entitys.push(id);
	}

	println!("change 3: ");
	dispatcher.run();
	std::thread::sleep(std::time::Duration::new(1, 0));
	world.increment_change_tick();
	world.insert_component(entitys[1].clone(), Position(10));
	println!("change 1: ");
	dispatcher.run();

	std::thread::sleep(std::time::Duration::new(2, 0));
}


fn create_dispatcher(world: &mut World) -> SingleDispatcher<StealableTaskPool<()>> {
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());
	let iter_dirty_system = iter_dirty.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(iter_dirty_system);
	
	let mut stages = Vec::new();
	stages.push((Arc::new(stage.build()), false));
	let dispatcher = SingleDispatcher::new(stages , rt);

	return dispatcher;
}