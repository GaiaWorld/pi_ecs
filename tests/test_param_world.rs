/// 测试系统参数Res、ResMut
use pi_ecs::{
    prelude::{Dispatcher, SingleDispatcher, StageBuilder, World},
    sys::system::IntoSystem,
};
use r#async::rt::{
    multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool},
    AsyncRuntime,
};
use std::sync::Arc;

#[derive(Debug)]
struct Resource1(pub usize);

#[derive(Debug)]
struct Resource2(pub usize);

fn res(w: World) {
    let id1 = w.get_resource_id::<Resource1>();
    println!("res run, id1: {:?}", id1);

    let id2 = w.get_resource_id::<Resource2>();
    println!("res run, id2: {:?}", id2);
}

#[test]
fn test() {
    // 创建world
    let mut world = World::new();

    // 在创建system之前插入资源
    world.insert_resource(Resource1(1));

    // 创建派发器
    let dispatcher = get_dispatcher(&mut world);

    dispatcher.run();

    std::thread::sleep(std::time::Duration::from_secs(1));
}

fn get_dispatcher(world: &mut World) -> SingleDispatcher<StealableTaskPool<()>> {
    let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());
    let system = res.system(world);

    let mut stage = StageBuilder::new();
    stage.add_node(system);

    let mut stages = Vec::new();
    stages.push(Arc::new(stage.build()));
    let dispatcher = SingleDispatcher::new(stages, world, rt);

    dispatcher
}
