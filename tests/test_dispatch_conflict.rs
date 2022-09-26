//! 测试派发器访问冲突
use pi_ecs::prelude::*;
use pi_async::prelude::AsyncRuntimeBuilder;
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
async fn async_stage2_system1() -> Result<()> {
    println!("Running: async system: stage 2, system 1");
	Ok(())
}

// 同步 System
fn sync_stage2_system2() {
    println!("Running: sync system: stage 2, system 2");
}

// 单线程异步运行时 派发
#[test]
fn multi_dispatch() {
    
    let mut world = World::new();

    let mut stages = Vec::new();
    // 创建 单线程 异步运行时
    let single = AsyncRuntimeBuilder::default_worker_thread(
		None,
		None,
		None,
		None,
	);
    {
        let mut s1 = StageBuilder::new();

        s1.add_node(sync_stage1_system1.system(&mut world));
        s1.add_node(sync_stage1_system2.system(&mut world));

        // 第二个参数：是否单线程执行
        stages.push((Arc::new(s1.build(&world)), None));
    }
    {
        let mut s2 = StageBuilder::new();

        s2.add_node(async_stage2_system1.system(&mut world));
        s2.add_node(sync_stage2_system2.system(&mut world));

        // 第二个参数：是否单线程执行
        stages.push((Arc::new(s2.build(&world)), Some(single.clone())));
    }
	let multi = AsyncRuntimeBuilder::default_multi_thread(
		None,
		None,
		None,
		None,
	);

    let dispatcher = MultiDispatcher::new(stages, multi);
    dispatcher.run();

    std::thread::sleep(std::time::Duration::from_secs(1));
}