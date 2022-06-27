/// 测试System

use pi_ecs::{prelude::{Query, With,Res, World, Local, ResMut, StageBuilder, SingleDispatcher, Dispatcher, Id}, sys::system::IntoSystem};
use pi_async::rt::AsyncRuntimeBuilder;
use std::{sync::Arc, io::Result};

/// 定义一个名为Node原型类型
pub struct Node;

#[derive(Debug)]
/// 定义一个组件类型
pub struct Position(pub usize);

#[derive(Debug)]
/// 定义一个组件类型
pub struct Velocity(pub usize);

#[derive(Debug)]
/// 定义一个资源类型
pub struct Resource1(pub usize);

#[derive(Debug)]
/// 定义一个资源类型
pub struct Resource2(pub usize);


/// 定义一个系统的本地数据类型
#[derive(Default, Debug)]
pub struct DirtyMark(pub usize);

#[derive(Default)]
pub struct Local1(pub u32);

// 同步系统
fn _sync_sys(
	_query1: Query<Node, &Velocity>,
	_query2: Query<Node, &mut Position, With<Velocity>>, // Query<Node, &mut Position, WithOut<Velocity>>
	_local: Local<DirtyMark>,
	_res: Res<Resource1>,
	_res_mut: ResMut<Resource2>,
) {
	println!("run _sync_sys");
}

// 异步系统()
async fn _async_sys1<'a>(
	_query1: Query<Node, &'a Velocity>,
	mut query2: Query<Node, (Id<Node>, &'a mut Position), With<Velocity>>, // Query<Node, &mut Position, WithOut<Velocity>>
	local: Local<'a, DirtyMark>,
	res: Res<'a, Resource1>,
	res_mut: ResMut<'a, Resource2>,
) -> Result<()> {
	// async move {
	// 	// let r1 = &*res;
	// 	// let r2 = &*res_mut;
	// 	// let r2 = &*local;
	// 	// println!("run _async_sys1, res1: {:?}, {:?}, {:?}", &*res, &*res_mut, &*local);
	// 	println!("run _async_sys1, res1: {:?}, {:?}, {:?}", 1, 2,3);
	// 	for (entity, position) in query2.iter_mut() {
	// 		println!("run _async_sys1, entity: {:?}, position:{:?}",entity, &*position);
	// 	}
	// 	Ok(())
	// }.boxed()
	println!("run _async_sys1, res1: {:?}, {:?}, {:?}", &*res, &*res_mut, &*local);
	println!("run _async_sys1, res1: {:?}, {:?}, {:?}", 1, 2,3);
	for (entity, position) in query2.iter_mut() {
		println!("run _async_sys1, entity: {:?}, position:{:?}",entity, &*position);
	}
	Ok(())
}

// async fn aa(x: &usize) {

// }

// async fn bb() {
// 	let x = 0;
// 	aa(&x).await;
// }

#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 创建一个名为Node的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Node>()
		.register::<Velocity>()
		.register::<Position>()
		.create();
	
	world.insert_resource(Resource1(1));
	world.insert_resource(Resource2(2));

	// 创建原型为Node的实体，并为该实体添加组件（必须是在Node中注册过的组件， 否则无法插入）
	for i in 0..5 {
		let _id = world.spawn::<Node>()
		.insert(Position(i))
		.id();
	}

	let mut vec = Vec::new();
	for i in 0..5 {
		let id = world.spawn::<Node>()
		.insert(Position(i + 5))
		.insert(Velocity(i + 5))
		.id();

		vec.push(id);
	}

	// 创建查询,并迭代查询
	let mut query = world.query::<Node, (Id<Node>, &Velocity, &mut Position)>();
	for (entity, velocity, position) in query.iter_mut(&mut world) {
		println!("iter_mut:{:?}, {:?}, {:?}", entity, velocity, position);
	}

	let mut query = world.query::<Node, (Id<Node>, &Position)>();
	for (entity, position) in query.iter(&mut world) {
		println!("iter_mut1:{:?}, {:?}", entity, position);
	}
	

	test_system(&mut world);

	std::thread::sleep(std::time::Duration::from_secs(5));
}

fn test_system(world: &mut World) {
	let rt = AsyncRuntimeBuilder::default_multi_thread(
		None,
		None,
		None,
		None,
	);
	let sync_system = _sync_sys.system(world);
	// let async_system = _async_sys.system(world.clone());
	let async_system = _async_sys1.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(sync_system);
	stage.add_node(async_system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build(world)));
	let mut dispatcher = SingleDispatcher::new(rt);
	dispatcher.init(stages, world);

	dispatcher.run();
}