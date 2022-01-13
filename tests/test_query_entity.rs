/// 测试Entity查询

use pi_ecs::{prelude::{World, StageBuilder, SingleDispatcher, Dispatcher, Query}, sys::system::IntoSystem, entity::Entity, storage::Offset};
use r#async::rt::{multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool}, AsyncRuntime};
use std::sync::Arc;

#[derive(Debug)]
pub struct Archetype1;

#[derive(Debug)]
pub struct Component1(pub usize);

#[derive(Debug)]
pub struct Component2(pub usize);


/// 在system中查询实体
fn entity(
	query: Query<Archetype1, Entity>,
) {
	for i in query.iter() {
		println!("entity run, entity: {:?}", i.local().offset());
	}
}

#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 创建一个名为Node1的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Archetype1>()
		.create();

	for _i in 0..2 {
		world.spawn::<Archetype1>();
	}

	let dispatcher = get_dispatcher(&mut world);

	println!("测试查询实体：");
	dispatcher.run();

	std::thread::sleep(std::time::Duration::from_secs(1));
}

fn get_dispatcher(world: &mut World) -> SingleDispatcher<StealableTaskPool<()>> {
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());
	let system = entity.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build()));
	let dispatcher = SingleDispatcher::new(stages, world, rt);

	dispatcher
}

