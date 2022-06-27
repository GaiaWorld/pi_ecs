use std::sync::Arc;

use pi_async::rt::{AsyncRuntimeBuilder, multi_thread::MultiTaskRuntime};
use pi_ecs::prelude::{
    event::{EventReader, EventWriter, Events}, IntoSystem, SingleDispatcher, StageBuilder, World, Dispatcher,
};

#[test]
fn test_event() {
    let mut world = World::new();
    world.new_archetype::<()>().create();

    let events = Events::<MyEvent>::default();
    world.insert_resource(events);
    let dispatcher = create_dispatcher(&mut world);

    // 单线程 异步运行时，哪个线程推，就由哪个线程执行 future
    let mut r = 0;
    loop {
        // 这是一帧：预期是 上面的 2个阶段的 3个system 都要执行一遍。
        r += 1;
        println!("==================== frame {}", r);

        // 执行 一次，就可以 得到 rt的很多的 async 函数
        dispatcher.run();

        std::thread::sleep(std::time::Duration::from_millis(8000));
    }
}

fn create_dispatcher(world: &mut World) -> SingleDispatcher<MultiTaskRuntime> {
	let rt = AsyncRuntimeBuilder::default_multi_thread(
		None,
		None,
		None,
		None,
	);

    let mut stages = Vec::new();

    let mut stage1 = StageBuilder::new();
    // 更新 Event：交换缓冲区，必须在 所有事件系统 运行 之前 执行
    // 所以：单独设置一个阶段
    stage1.add_node(Events::<MyEvent>::update_system.system(world));
    stages.push(Arc::new(stage1.build(&world)));

    // 所以：Event的实现，EventWritter 先于 EvenReader 执行
    let mut stage2 = StageBuilder::new();
    stage2.add_node(sending_system.system(world));
    stage2.add_node(receiving_system.system(world));
    stages.push(Arc::new(stage2.build(&world)));

    let mut dispatcher = SingleDispatcher::new(rt);
	dispatcher.init(stages, world);
	dispatcher
}

// This is our event that we will send and receive in systems
struct MyEvent {
    pub message: String,
    pub random_value: f32,
}

// In every frame we will send an event with a 50/50 chance
fn sending_system(mut event_writer: EventWriter<MyEvent>) {
    println!("2 execute sending_system");

    let random_value: f32 = rand::random();
    if random_value > 0.5 {
        println!("============ send MyEvent");
        event_writer.send(MyEvent {
            message: "A random event with value > 0.5".to_string(),
            random_value,
        });
    }
}

fn receiving_system(mut event_reader: EventReader<MyEvent>) {
    println!("3 execute receiving_system");
    for my_event in event_reader.iter() {
        println!(
            "    Received message {:?}, with random value of {}",
            my_event.message, my_event.random_value
        );
    }
}
