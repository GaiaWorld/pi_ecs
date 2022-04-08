// use futures::{future::BoxFuture, FutureExt};
// use pi_async::rt::{single_thread::SingleTaskRunner, AsyncRuntime};
// use pi_ecs::prelude::*;
// use std::{io::Result, sync::Arc};

// fn s2_sys_1() -> BoxFuture<'static, Result<()>> {
//     async move {
//         println!("Begin Running: Stage 2 System 1");
//         println!("End Running: Stage 2 System 1");
//         Ok(())
//     }
//     .boxed()
// }

// fn f() -> BoxFuture<'static, Result<()>> {
//     println!("Begin f()");
//     let r = async move {
//         println!("Begin Await f()");
        
//         println!("End Await f()");
//         Ok(())
//     }
//     .boxed();

//     println!("End f()");
//     r
// }

// fn s2_sys_2() -> BoxFuture<'static, Result<()>> {
//     async move {
//         println!("Begin Running: Stage 2 System 2");

//         let a = f();
//         a.await;
        
//         println!("End Running: Stage 2 System 2");
//         Ok(())
//     }
//     .boxed()
// }

// fn s3_sys_1() -> BoxFuture<'static, Result<()>> {
//     async move {
//         println!("Begin Running: Stage 3 System 1");
//         println!("End Running: Stage 3 System 1");
//         Ok(())
//     }
//     .boxed()
// }

// // 单线程异步运行时 派发
// #[test]
// fn single_runtime() {
//     let mut world = World::new();

//     let mut stages = Vec::new();

//     {
//         let s1 = StageBuilder::new();
//         stages.push(Arc::new(s1.build()));
//     }

//     {
//         let mut s2 = StageBuilder::new();
//         s2.add_node(s2_sys_1.system(&mut world));
//         s2.add_node(s2_sys_2.system(&mut world));
//         stages.push(Arc::new(s2.build()));
//     }

//     {
//         let mut s3 = StageBuilder::new();
//         s3.add_node(s3_sys_1.system(&mut world));
//         stages.push(Arc::new(s3.build()));
//     }

//     // 创建 单线程 异步运行时
//     let runner = SingleTaskRunner::default();
//     let runtime = runner.startup().unwrap();
//     let single = AsyncRuntime::Local(runtime);

//     let dispatcher = SingleDispatcher::new(stages, &world, single);

//     // 单线程 异步运行时，哪个线程推，就由哪个线程执行 future
//     let mut r = 0;
//     loop {
        
//         // 这是一帧：预期是 上面的 3个阶段的 3个system 都要执行一遍。
//         r += 1;
//         println!("==================== frame {}", r);

//         // 执行 一次，就可以 得到 rt的很多的 async 函数
//         dispatcher.run();
//         loop {
//             // 不断的执行async，就好像 执行微任务一样；
//             let count = runner.run().unwrap();
//             if count == 0 {
//                 break;
//             }
//         }

//         std::thread::sleep(std::time::Duration::from_millis(8000));
//     }
// }
