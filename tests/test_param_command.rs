/// 测试系统参数Tick

use pi_ecs::prelude::{World, StageBuilder, SingleDispatcher, Dispatcher, Commands, EntityCommands, Local, Query, Id, Offset, IntoSystem};
use pi_async::prelude::{multi_thread::MultiTaskRuntime, AsyncRuntimeBuilder};
use std::sync::Arc;

struct Node;

#[derive(Debug)]
struct Position (pub(crate) usize);


#[derive(Default)]
struct Local1(pub(crate) Vec<Id<Node>>);

/// 测试指令
fn command(
	mut entity_command: EntityCommands<Node>,
	mut command: Commands<Node, Position>,
	mut local: Local<Local1>,
	query: Query<Node, (Id<Node>, Option<&Position>)>
) {

	// 第一次打印，不存在任何实体
	// 第二次打印，有三个实体，并都有Position组件
	// 第三次打印，有两个实体，并且都没有Position组件
	for i in query.iter() {
		println!("command run, entity: {}, position{:?}", i.0.offset(), i.1);
	}

	if local.0.len() == 0 {
		// 测试spawn，insert
		for i in 0..3 {
			let e = entity_command.spawn();
			command.insert(e.clone(), Position(i));
			local.0.push(e);
		}
	} else if local.0.len() == 3 {
		// 测试despawn，delete
		entity_command.despawn(local.0[0].clone()); // 删除第一个实体
		for i in 1..3 {
			// let e = entity_command.spawn();
			command.delete(local.0[i].clone());
		}
	}
	
}

/// 测试系统参数Tick
#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	world.new_archetype::<Node>()
		.register::<Position>()
		.create();

	// 创建派发器
	let dispatcher = get_dispatcher(&mut world);

	// 第一次运行，节拍为1
	println!("第一次运行：");
	dispatcher.run();

	// 第二次运行，节拍为2
	println!("第二次运行：");
	dispatcher.run();

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
	let system = command.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build(world)));
	let mut dispatcher = SingleDispatcher::new(rt);
	dispatcher.init(stages, world);

	dispatcher
}
