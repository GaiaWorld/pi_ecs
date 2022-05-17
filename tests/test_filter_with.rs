/// 测试Filter: With、WidthOut

use std::sync::Arc;

use pi_ecs::{prelude::{Query, IntoSystem, StageBuilder, SingleDispatcher, Dispatcher, With, WithOut}, entity::Id, world::World, storage::Offset};
use pi_async::rt::{AsyncRuntime, multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool}};


pub struct Node;

#[derive(Debug)]
/// 定义一个组件类型
pub struct Position(pub usize);


/// 测试Width
pub fn with(
	q: Query<Node, Id<Node>, With<Position>>,
) {
	for r in q.iter(){
		println!("width<Position> filter run, entity {:?}", r.offset());
	}
}

/// 测试WithOut
pub fn without(
	q: Query<Node, Id<Node>, WithOut<Position>>,
) {
	for r in q.iter(){
		println!("WithOut<Position> filter run, entity {:?}", r.offset());
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

	// 创建原型为Node的实体，并为该实体添加组件（必须是在Node中注册过的组件， 否则无法插入）
	for i in 1..7 {
		// 偶数插入Position，基数不插入
		if (i as f32 % 2.0) == 0.0 {
			world.spawn::<Node>()
				.insert(Position(i));
		} else {
			world.spawn::<Node>();
		}
	}

	println!("验证（With<Position>为偶数，WithOut<Position>为奇数）: ");
	dispatcher.run();

	std::thread::sleep(std::time::Duration::new(2, 0));
}


fn create_dispatcher(world: &mut World) -> SingleDispatcher<StealableTaskPool<()>> {
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());
	let system1 = with.system(world);
	let system2 = without.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system1);
	stage.add_node(system2);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build(world)));
	let mut dispatcher = SingleDispatcher::new(rt);
	dispatcher.init(stages, world);

	return dispatcher;
}