/// 测试系统参数Res、ResMut
use pi_ecs::{
    prelude::{
        world::{WorldMut, WorldRead},
        Dispatcher, Res, SingleDispatcher, StageBuilder, World,
    },
    sys::system::IntoSystem,
};
use pi_async::prelude::{
    multi_thread:: MultiTaskRuntime,
    AsyncRuntimeBuilder,
};
use std::sync::Arc;

#[derive(Debug)]
struct Resource1(pub usize);

#[derive(Debug)]
struct Resource2(pub usize);

// res: ResMut<Resource1> 会 崩掉，报和 WorldRead 有冲突
fn sys_read_world(w: WorldRead, res: Res<Resource1>) {
    println!("sys_read_world run, res: {:?}", res.0);

    let id1 = w.get_resource_id::<Resource1>();
    println!("sys_read_world run, id1: {:?}", id1);

    let id2 = w.get_resource_id::<Resource2>();
    println!("sys_read_world run, id2: {:?}", id2);
}

// res: Res<Resource1> 会 崩掉，报和 WorldMut 有冲突
// res: ResMut<Resource1> 会 崩掉，报和 WorldMut 有冲突
fn sys_write_world(w: WorldMut) {
    let id1 = w.get_resource_id::<Resource1>();
    println!("sys_write_world run, id1: {:?}", id1);

    let id2 = w.get_resource_id::<Resource2>();
    println!("sys_write_world run, id2: {:?}", id2);
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

fn get_dispatcher(world: &mut World) -> SingleDispatcher<MultiTaskRuntime> {
    let rt = AsyncRuntimeBuilder::default_multi_thread(
		None,
		None,
		None,
		None,
	);
    let s1 = sys_read_world.system(world);
    let s2 = sys_write_world.system(world);

    let mut stage = StageBuilder::new();
    stage.add_node(s1);
    stage.add_node(s2);

    let mut stages = Vec::new();
    stages.push(Arc::new(stage.build(world)));
    let mut dispatcher = SingleDispatcher::new(rt);
	dispatcher.init(stages, world);

    dispatcher
}
