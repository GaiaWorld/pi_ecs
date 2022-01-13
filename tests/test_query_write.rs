/// 测试组件查询Write

use pi_ecs::{prelude::{World, StageBuilder, SingleDispatcher, Dispatcher, Query, Write}, sys::system::IntoSystem, monitor::{Event, Listeners, ListenSetup}};
use pi_ecs_macros::listen;
use r#async::rt::{multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool}, AsyncRuntime};
use std::{sync::Arc};

#[derive(Debug)]
pub struct Archetype1;

#[derive(Debug)]
pub struct Component1(pub usize);


/// 在system中查询组件的引用
/// * 可以是只读引用，也可以是可写引用

fn write(
	mut query: Query<Archetype1, Write<Component1>>,
) {
	for mut i in query.iter_mut() {
		println!("query write run, value {:?}", i.get());
		if let Some(r) = i.get_mut() {
			r.0 = 10;
		}
		println!("query write run, modify value to {:?}", i.get());

		i.write(Component1(20));
		println!("query write run, modify value to {:?}, and notify", i.get());
	}
}

/// 监听器，监听Component1的修改事件
#[listen(component = (Archetype1, Component1, Modify))]
fn listener_component_modify(
	input: Event,
) {
	println!("run listener_component_modify, entity: {:?}", input.id);
}

/// 监听器，监听Component1的修改事件
#[listen(component = (Archetype1, Component1, Create))]
fn listener_component_create(
	input: Event,
) {
	println!("run listener_component_create, entity: {:?}", input.id);
}

/// 测试=系统参数Join
#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 创建一个名为Node1的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Archetype1>()
		.register::<Component1>()
		.create();

	// 创建两个实体，一个不插入组件，一个插入Component1
	// write修改Component1时，第一个entity会产生Component1的创建事件，第二个entity会产生Component1的修改事件
	world.spawn::<Archetype1>();
	world.spawn::<Archetype1>()
		.insert(Component1(2));

	let listener1 = listener_component_create.listeners();
	let listener2 = listener_component_modify.listeners();
	listener1.setup(&mut world);
	listener2.setup(&mut world);

	let dispatcher = get_dispatcher(&mut world);

	println!("测试write查询：");
	dispatcher.run();

	std::thread::sleep(std::time::Duration::from_secs(1));
}

fn get_dispatcher(world: &mut World) -> SingleDispatcher<StealableTaskPool<()>> {
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());
	let system = write.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build()));
	let dispatcher = SingleDispatcher::new(stages , rt);

	dispatcher
}

