/// 测试Join查询

use std::ops::{Deref, DerefMut};

use pi_ecs::{prelude::{Query, World, Id, StageBuilder, SingleDispatcher, Dispatcher, Join}, sys::system::IntoSystem, storage::Offset};
use pi_async::rt::AsyncRuntimeBuilder;
use std::sync::Arc;

/// 定义一个名为Node原型类型
pub struct Node1;
pub struct Node2;

#[derive(Debug)]
/// 定义一个组件类型,注册在Node1中,用于关联Node2的实体
pub struct Node2Index(Id<Node2>);

/// 
impl Deref for Node2Index {
	type Target = Id<Node2>;
    fn deref(&self) -> &Id<Node2> {
		&self.0
	}
}

impl DerefMut for Node2Index {
    fn deref_mut(&mut self) -> &mut Id<Node2> {
		&mut self.0
	}
}

#[derive(Debug)]
/// 定义一个组件类型
pub struct Position(pub usize);

/// 在system中使用Join
/// 
/// Join：原型A1中的实体E2对原型A2中的实体E1有引用关系，可以通过E1查询到对应E2的所有数据
/// 
/// 应用场景举例： 
/// gui中的节点（原型A1中的实体）需要关联一个其对应渲染对象（原型A1中的实体），
/// Join可以通过节点查询到其对应渲染对象和该渲染对象上的所有组件
/// 
/// 形如： Query<A1, (Q1, Join<C1, A1, Q1>)>
/// * 原型A1中的实体中的某个实体中包含组件C1
/// * 原型A2中的实体中的某个实体中包含组件C2
/// * C1实现了Deref，解引用的值，是原型A2中的某个实体
/// * 该查询可以查询到A1中的数据Q1和A2中的数据Q1
/// * Q1、Q2可以是其原型上的任意查询，如： Entity、&Component、&mut Component、(Entity, &Component...)
fn join(
	query: Query<Node1, (Id<Node1>, Join<Node2Index, Node2, &Position>)>,
) {
	for i in query.iter() {
		println!("join run, entity: {:?}, position: {:?}", i.0.offset(), i.1);
	}
}

/// 测试系统参数Join
#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 创建一个名为Node1的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Node1>()
		.register::<Node2Index>()
		.create();
	
	// 创建一个名为Node2的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Node2>()
	.register::<Position>()
	.create();

	let mut node2_entitys = Vec::new();
	// 创建原型为Node2的实体，并为该实体添加组件（必须是在Node2中注册过的组件， 否则无法插入）
	for i in 0..5 {
		let id = world.spawn::<Node2>()
		.insert(Position(i))
		.id();

		node2_entitys.push(id);
	}

	// 创建原型为Node1的实体，并为该实体添加Node2Index组件, 将Node2Index和Node2关联起来
	for i in 0..3 {
		let _id = world.spawn::<Node1>()
		.insert(Node2Index(node2_entitys[i + 2].clone()))
		.id();
	}

	println!("测试系统参数Join：");
	test_system(&mut world);

	std::thread::sleep(std::time::Duration::from_secs(1));
}

fn test_system(world: &mut World) {
	let rt = AsyncRuntimeBuilder::default_multi_thread(
		None,
		None,
		None,
		None,
	);
	let system = join.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build(world)));
	let mut dispatcher = SingleDispatcher::new(rt);
	dispatcher.init(stages, world);

	dispatcher.run();
}
