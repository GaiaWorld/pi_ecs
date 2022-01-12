use pi_ecs::{
	prelude::{ World, Local, Entity},
	monitor::{Event, ListenSetup, Listeners}
};
use pi_ecs_macros::listen;


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


/// 监听器，监听原型Node中组件Position的修改，和实体Node的删除
#[listen(component = (Node, Position, Modify), entity = (Node, Delete))]
fn listener_component_entity(
	input: Event,
	mut local: Local<Local1>,
) {

	local.0 += 1;
	println!("run listener_component_entity, count: {:?}, entity: {:?}", local.0, input.id);
}

/// 监听资源的修改（资源只会有修改事件）
#[listen(resource = (Resource1, Modify))]
fn listener_resuorce(
	_input: Event,
	mut local: Local<Local1>,
) {

	local.0 += 1;
	println!("run listener_resuorce, count: {:?},", local.0);
}

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
	
	let l1 = listener_component_entity.listeners();
	let l2 = listener_resuorce.listeners();
	l1.setup(&mut world);
	l2.setup(&mut world);

	// 会触发组件修改事件
	println!("component will modify");
	world.insert_component(vec[4].clone(), Position(4));
	// 会触发实体删除事件
	println!("entity will modify");
	world.unspawn(vec[3].clone());

	println!("resoruce will modify");
	world.res::<Resource1>().query_mut(&mut world).modify_event(Entity::default(), "", 0);

	std::thread::sleep(std::time::Duration::from_secs(5));
}