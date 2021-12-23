use futures::future::{BoxFuture, FutureExt};
use pi_ecs::{prelude::*};
use share::cell::TrustCell;
use r#async::rt::{multi_thread::MultiTaskRuntimeBuilder, AsyncRuntime};

struct Node;
struct Position(pub usize);
struct Velocity(pub usize);

struct Resource1;
struct Resource2;

#[derive(Default)]
struct DirtyMark;

// 系统
fn _sync_sys(
	_query1: Query<Node, &Velocity>,
	_query2: Query<Node, &mut Position, With<Velocity>>, // Query<Node, &mut Position, WithOut<Velocity>>
	_local: Local<DirtyMark>,
	_res: Res<Resource1>,
	_res_mut: ResMut<Resource2>,
) {
	println!("run _sync_sys");
}

// async fn _async_sys(
// 	_query1: Query<Node, &Velocity>,
// 	_query2: Query<Node, &mut Position, With<Velocity>>, // Query<Node, &mut Position, WithOut<Velocity>>
// 	_local: Local<DirtyMark>,
// 	_res: Res<Resource1>,
// 	_res_mut: ResMut<Resource2>,
// ) -> () {
	
// }

fn _async_sys1(
	_query1: Query<Node, &Velocity>,
	_query2: Query<Node, &mut Position, With<Velocity>>, // Query<Node, &mut Position, WithOut<Velocity>>
	_local: Local<DirtyMark>,
	_res: Res<Resource1>,
	_res_mut: ResMut<Resource2>,
) -> BoxFuture<'static, Result<()>> {
	async move {
		println!("run _async_sys1");
		Ok(())
	}.boxed()
}
use std::{sync::Arc, io::Result};

#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 创建一个名为Node的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Node>()
		.register::<Velocity>()
		.register::<Position>()
		.create();
	
	world.insert_resource(Resource1);
	world.insert_resource(Resource2);

	// 创建原型为Node的实体，并为该实体添加组件（必须是在Node中注册过的组件， 否则无法插入）
	for _i in 0..10_000 {
		let _id = world.spawn::<Node>()
		.insert(Position(2))
		.insert(Velocity(1))
		.id();
	}

	// 创建查询,并迭代查询
	let mut query = world.query::<Node, (&Velocity, &mut Position)>();
	for (_velocity, _position) in query.iter_mut(&mut world) {}

	let mut w = Arc::new(TrustCell::new(world));
	test_system(&mut w);
}

fn test_system(world: &mut Arc<TrustCell<World>>) {
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());
	let sync_system = _sync_sys.system(world);
	// let async_system = _async_sys.system(world.clone());
	let async_system = _async_sys1.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(sync_system);
	stage.add_node(async_system);
	
	let mut stages = Vec::new();
	stages.push((Arc::new(stage.build()), false));
	let dispatcher = SingleDispatcher::new(stages , rt);

	dispatcher.run();
}
