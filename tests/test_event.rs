use std::sync::Arc;

use pi_async::rt::{
    multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool},
    AsyncRuntime,
};
use pi_ecs::prelude::{
    event::{EventReader, EventWriter, Events},
    Dispatcher, IntoSystem, SingleDispatcher, StageBuilder, World,
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
        // 这是一帧：预期是 上面的 3个阶段的 3个system 都要执行一遍。
        r += 1;
        println!("==================== frame {}", r);

        // 执行 一次，就可以 得到 rt的很多的 async 函数
        dispatcher.run();

        std::thread::sleep(std::time::Duration::from_millis(8000));
    }
}

fn create_dispatcher(world: &mut World) -> SingleDispatcher<StealableTaskPool<()>> {
    let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());

    let mut stages = Vec::new();

    let mut stage = StageBuilder::new();
    let update_system = Events::<MyEvent>::update_system.system(world);
    stage.add_node(update_system);
    stages.push(Arc::new(stage.build()));

    let mut stage = StageBuilder::new();
    let sending_system = sending_system.system(world);
    stage.add_node(sending_system);
    stages.push(Arc::new(stage.build()));

    let mut stage = StageBuilder::new();
    let receiving_system = receiving_system.system(world);
    stage.add_node(receiving_system);
    stages.push(Arc::new(stage.build()));

    SingleDispatcher::new(stages, world, rt)
}

// This is our event that we will send and receive in systems
struct MyEvent {
    pub message: String,
    pub random_value: f32,
}

// In every frame we will send an event with a 50/50 chance
fn sending_system(mut event_writer: EventWriter<MyEvent>) {
    let random_value: f32 = rand::random();
    if random_value > 0.5 {
        event_writer.send(MyEvent {
            message: "A random event with value > 0.5".to_string(),
            random_value,
        });
    }
}

// This system listens for events of the type MyEvent
// If an event is received it will be printed to the console
fn receiving_system(mut event_reader: EventReader<MyEvent>) {
    for my_event in event_reader.iter() {
        println!(
            "    Received message {:?}, with random value of {}",
            my_event.message, my_event.random_value
        );
    }
}
