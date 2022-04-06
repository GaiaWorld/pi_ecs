/// 测试自定义监听器

use pi_ecs::{
	prelude::{ World, Local, Monitor, Event, ShareSystem, ComponentListen, Modify, EntityListen, Delete, ListenSetup, Listeners}
};

/// 定义一个名为Node原型类型
pub struct Node;

#[derive(Debug)]
/// 定义一个组件类型
pub struct Position(pub usize);

#[derive(Default)]
pub struct Local1(pub u32);

/// 自定义监听器
#[derive(Default)]
pub struct MyListenner;

/// 自定义监听器需要实现Monitor trait
impl Monitor for MyListenner {
	type Listen = (ComponentListen<Node, Position, Modify>, EntityListen<Node, Delete>);
	type Param = Local<Local1>;

	fn monitor(&mut self, e: Event, mut local: Self::Param) {
		local.0 += 1;
		println!("run monitor_component_entity, count: {:?}, entity: {:?}", local.0, e.id);
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
		.id();
		vec.push(id);
	}

	let monitor = ShareSystem::new(MyListenner::default());
	
	monitor.listeners().setup(&mut world);

	// 会触发组件修改事件
	println!("component will modify");
	world.insert_component(vec[4].clone(), Position(4));
	// 会触发实体删除事件
	println!("entity will modify");
	world.despawn(vec[3].clone());


	std::thread::sleep(std::time::Duration::from_secs(5));
}