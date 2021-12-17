use pi_ecs::prelude::*;

struct Node;
struct Position(pub usize);
struct Velocity(pub usize);

#[test]
fn test() {
	// 创建world
	let mut world = World::new();
	// 创建一个名为Node的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Node>()
		.register::<Velocity>()
		.register::<Position>()
		.create();

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

}

