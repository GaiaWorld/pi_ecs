/// 测试Filter的“or”、“and”

use std::sync::Arc;

use pi_ecs::{prelude::{Query, IntoSystem, StageBuilder, SingleDispatcher, Dispatcher, With, WithOut, Or}, entity::Entity, world::World, storage::Offset};
use r#async::rt::{AsyncRuntime, multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool}};


pub struct Node;

#[derive(Debug)]
/// 定义一个组件类型
pub struct Position(pub usize);

#[derive(Debug)]
/// 定义一个组件类型
pub struct Velocity(pub usize);


/// 测试Or
pub fn or(
	q: Query<Node, Entity, Or<(With<Position>, WithOut<Velocity>)>>,
) {
	for r in q.iter(){
		println!("Or filter run, entity {:?}", r.local().offset());
	}
}

/// 测试and
pub fn and(
	q: Query<Node, Entity, (With<Position>, WithOut<Velocity>)>,
) {
	for r in q.iter(){
		println!("and filter run, entity {:?}", r.local().offset());
	}
}

#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 创建一个名为Node的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Node>()
		.register::<Position>()
		.register::<Velocity>()
		.create();

	let dispatcher = create_dispatcher(&mut world);

	// 创建原型为Node的实体，并为该实体添加组件（必须是在Node中注册过的组件， 否则无法插入）
	for i in 1..7 {
		// 偶数插入Position，基数不插入
		if (i as f32 % 2.0) == 0.0 {
			world.spawn::<Node>()
				.insert(Position(i))
				.insert(Velocity(i));
		} else {
			world.spawn::<Node>()
				.insert(Position(i));
		}
	}

	println!("验证（or将包含所有实体，and仅包含偶数实体）: ");
	dispatcher.run();

	std::thread::sleep(std::time::Duration::new(2, 0));
}


fn create_dispatcher(world: &mut World) -> SingleDispatcher<StealableTaskPool<()>> {
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());
	let system1 = or.system(world);
	let system2 = and.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system1);
	stage.add_node(system2);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build()));
	let dispatcher = SingleDispatcher::new(stages, world, rt);

	return dispatcher;
}