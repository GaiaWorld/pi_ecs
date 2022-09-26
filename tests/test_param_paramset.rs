/// 测试系统参数Local

use pi_ecs::{prelude::{World, StageBuilder, SingleDispatcher, Dispatcher, ParamSet, ResMut}, sys::system::IntoSystem};
use pi_async::prelude::{multi_thread::MultiTaskRuntime, AsyncRuntimeBuilder};
use std::sync::Arc;

#[derive(Debug)]
pub struct MyRes(usize);

fn param_set(
	mut param_set: ParamSet<(ResMut<MyRes>, ResMut<MyRes>)>,
) {
	let r0 = param_set.p0().0;
	let r1 = param_set.p1_mut().0;
	println!("param_set run, r0: {:?}, r1: {:?}", r0, r1);
}


#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();
	world.insert_resource(MyRes(5));

	let dispatcher = get_dispatcher(&mut world);

	println!("测试ParamSet参数：");
	dispatcher.run();

	std::thread::sleep(std::time::Duration::from_secs(1));
}

fn get_dispatcher(world: &mut World) -> SingleDispatcher<MultiTaskRuntime> {
	let rt = AsyncRuntimeBuilder::default_multi_thread(
		None,
		None,
		None,
		None,
	);
	let system = param_set.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build(world)));
	let mut dispatcher = SingleDispatcher::new(rt);
	dispatcher.init(stages, world);

	dispatcher
}

