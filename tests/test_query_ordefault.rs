/// 测试组件OrDefault查询

use pi_ecs::{prelude::{World, StageBuilder, SingleDispatcher, Dispatcher, Query, OrDefault}, sys::system::IntoSystem, entity::Entity, storage::Offset};
use r#async::rt::{multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool}, AsyncRuntime};
use std::sync::Arc;

#[derive(Debug)]
pub struct Archetype1;

#[derive(Debug)]
pub struct Component1(pub usize);

impl Default for Component1 {
	fn default() -> Self{
		Component1(100)
	}
}


/// 在system中查询OrDefault
/// OrDefault查询组件，如果组件类型未注册，则无法查询，否则如果组件值不存在，返回一个默认值，如果组件存在值，返回该值
/// 要求组件必须实现Default
/// 同时，默认值可以通过Resource来改变，Resource类型为`DefaultComponent`
fn ordefault(
	query: Query<Archetype1, (Entity, OrDefault<Component1>)>,
) {
	for (entity, component) in query.iter() {
		println!("ordefault run, entity: {}, component: {:?}", entity.local().offset(), component);
	}
}

#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 创建一个名为Node1的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Archetype1>()
		.register::<Component1>()
		.create();

	// 创建原型为Archetype1的实体，并为该实体添加组件（必须是在Archetype1中注册过的组件， 否则无法插入）
	for i in 1..7 {
		// 偶数插入Component1，基数不插入
		if (i as f32 % 2.0) == 0.0 {
			world.spawn::<Archetype1>()
				.insert(Component1(i));
		} else {
			world.spawn::<Archetype1>();
		}
	}

	let dispatcher = get_dispatcher(&mut world);

	println!("测试查询OrDefault组件, 其中奇数实体值为100，是默认值；偶数实体为插入的值：");
	dispatcher.run();

	std::thread::sleep(std::time::Duration::from_secs(1));
}

fn get_dispatcher(world: &mut World) -> SingleDispatcher<StealableTaskPool<()>> {
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());
	let system = ordefault.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build()));
	let dispatcher = SingleDispatcher::new(stages, world, rt);

	dispatcher
}

