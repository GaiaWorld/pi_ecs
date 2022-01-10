use futures::{future::BoxFuture, FutureExt};
use pi_ecs::prelude::*;
use r#async::rt::{
    multi_thread::MultiTaskRuntimeBuilder, single_thread::SingleTaskRunner, AsyncRuntime,
};
use share::cell::TrustCell;
use std::{io::Result, sync::Arc};

// 同步 System
fn sync_stage1_system1() {
    println!("Running: sync system: stage 1, system 1");
}

// 同步 System
fn sync_stage1_system2() {
    println!("Running: sync system: stage 1, system 2");
}

// 异步 System
fn async_stage2_system1() -> BoxFuture<'static, Result<()>> {
    async move {
        println!("Running: async system: stage 2, system 1");

        Ok(())
    }
    .boxed()
}

// 同步 System
fn sync_stage2_system2() {
    println!("Running: sync system: stage 2, system 2");
}

// 单线程异步运行时 派发
#[test]
fn single_runtime() {
    
    let world = World::new();
    let mut w = Arc::new(TrustCell::new(world));

    let mut stages = Vec::new();

    {
        let mut s1 = StageBuilder::new();
        s1.add_node(sync_stage1_system1.system(&mut w));
        s1.add_node(sync_stage1_system2.system(&mut w));

        // 第二个参数：是否单线程执行
        stages.push(Arc::new(s1.build()));
    }

    {
        let mut s2 = StageBuilder::new();

        s2.add_node(async_stage2_system1.system(&mut w));
        s2.add_node(sync_stage2_system2.system(&mut w));

        // 第二个参数：是否单线程执行
        stages.push(Arc::new(s2.build()));
    }

    // 创建 单线程 异步运行时
    let runner = SingleTaskRunner::default();
    let runtime = runner.startup().unwrap();
    let single = AsyncRuntime::Local(runtime);

    let dispatcher = SingleDispatcher::new(stages, single);
    dispatcher.run();
    
    // 单线程 异步运行时，哪个线程推，就由哪个线程执行 future
    for _ in 0..10 {
        let _ = runner.run();
        // 推一次 休眠一次
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    std::thread::sleep(std::time::Duration::from_secs(1));
}
