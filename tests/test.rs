use pi_ecs::prelude::*;

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

}

async fn _async_sys<'a>(
	_query1: Query<'a, Node, &Velocity>,
	_query2: Query<'a, Node, &mut Position, With<Velocity>>, // Query<Node, &mut Position, WithOut<Velocity>>
	_local: Local<'a, DirtyMark>,
	_res: Res<'a, Resource1>,
	_res_mut: ResMut<'a, Resource2>,
) {
	
}
use std::sync::Arc;

async fn _func_sys1(world: Arc<World>) {

}

#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	let f = _func_sys1(Arc::new(world));
	use r#async::rt::multi_thread::MultiTaskRuntimeBuilder;

	let rt = MultiTaskRuntimeBuilder::default().build();
	rt.spawn(rt.alloc(), f);



	// // 创建一个名为Node的原型，为该原型注册组件类型（一旦注册，不可修改）
	// world.new_archetype::<Node>()
	// 	.register::<Velocity>()
	// 	.register::<Position>()
	// 	.create();

	// // 创建原型为Node的实体，并为该实体添加组件（必须是在Node中注册过的组件， 否则无法插入）
	// for _i in 0..10_000 {
	// 	let _id = world.spawn::<Node>()
	// 	.insert(Position(2))
	// 	.insert(Velocity(1))
	// 	.id();
	// }

	// // 创建查询,并迭代查询
	// let mut query = world.query::<Node, (&Velocity, &mut Position)>();
	// for (_velocity, _position) in query.iter_mut(&mut world) {}
}

