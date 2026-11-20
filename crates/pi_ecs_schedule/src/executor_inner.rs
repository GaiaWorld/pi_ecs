//! 执行配对芯，不引用阶段芯或调度芯。
#[cfg(all(test, feature = "rc"))]
mod tests {
    use super::*;
    use crate::{
        executor::ExecutorId,
        stage::{Stage, StageId},
        stage_plan::{StagePlan, StageWave},
        system::SystemId,
        system_builder::SystemBuilder,
        system_param::ParamRegistration,
    };
    use pi_ecs_core::world::World;
    use std::{cell::RefCell, rc::Rc};

    // Seam: Executor 消费固定阶段计划，调用真实 FunctionSystem 的刷新和授权运行入口。
    #[test]
    fn rc_executor_fixed_plan_refreshes_each_system_and_stops_on_error() {
        let mut world = World::new().unwrap();
        world.finish_registration().unwrap();
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut stage = Stage::new(StageId(1), ExecutorId(0));
        for id in 0..3 {
            let calls = calls.clone();
            stage
                .add_system(
                    SystemBuilder::new(move || calls.borrow_mut().push(id))
                        .build(SystemId(id))
                        .unwrap(),
                )
                .unwrap();
        }
        let mut executor = Executor::new(ExecutionMode::Serial).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            world
                .schedule_scope(|world| {
                    world.parameter_registration_scope(|access| {
                        let mut registration = ParamRegistration::new(access);
                        for slot in &mut stage.systems {
                            slot.system_mut().unwrap().initialize(&mut registration)?;
                        }
                        Ok(())
                    })?;
                    stage.ordering = Some(StagePlan {
                        waves: vec![
                            StageWave {
                                systems: vec![2].into_boxed_slice(),
                            },
                            StageWave {
                                systems: vec![0].into_boxed_slice(),
                            },
                            StageWave {
                                systems: vec![1].into_boxed_slice(),
                            },
                        ]
                        .into_boxed_slice(),
                    });
                    let before = world.read_change_tick();
                    executor.execute(world, &mut PreparedStage { stage: &mut stage })?;
                    assert_eq!(&*calls.borrow(), &[2, 0, 1]);
                    assert_eq!(world.read_change_tick().get(), before.get() + 3);
                    assert_eq!(
                        stage.ordering.as_ref().unwrap().waves[0].systems.as_ref(),
                        &[2]
                    );
                    // 未初始化的真实系统在 prepare 返回错误；不得调用它或后续系统，也不分配 tick。
                    let captured = calls.clone();
                    stage.add_system(
                        SystemBuilder::new(move || captured.borrow_mut().push(3))
                            .build(SystemId(3))
                            .unwrap(),
                    )?;
                    stage.ordering = Some(StagePlan {
                        waves: vec![
                            StageWave {
                                systems: vec![0].into_boxed_slice(),
                            },
                            StageWave {
                                systems: vec![3].into_boxed_slice(),
                            },
                            StageWave {
                                systems: vec![1].into_boxed_slice(),
                            },
                        ]
                        .into_boxed_slice(),
                    });
                    let before_failure = world.read_change_tick();
                    assert!(executor
                        .execute(world, &mut PreparedStage { stage: &mut stage })
                        .is_err());
                    assert_eq!(world.read_change_tick().get(), before_failure.get() + 1);
                    assert_eq!(&*calls.borrow(), &[2, 0, 1, 0]);
                    Ok(())
                })
                .unwrap();
        }));
        stage.unregister(&mut world).unwrap();
        world.close().unwrap();
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }
}

use crate::{
    executor::{ExecutionMode, Executor},
    stage::PreparedStage,
};
use pi_ecs_core::{error::EcsError, world::WorldRef};

impl Executor {
    pub(crate) fn new_inner(mode: ExecutionMode) -> Result<Executor, EcsError> {
        #[cfg(feature = "rc")]
        if matches!(mode, ExecutionMode::Parallel) {
            return Err(pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::Unsupported,
                "rc 模式不创建并行执行器",
            ));
        }
        Ok(Executor { mode })
    }
}

impl Executor {
    pub(crate) fn execute_inner(
        _executor: &mut Executor,
        _world: WorldRef<'_>,
        _prepared: &mut PreparedStage<'_>,
    ) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        {
            if matches!(_executor.mode, ExecutionMode::Parallel) {
                return Err(pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::Unsupported,
                    "rc 模式不支持并行执行器",
                ));
            }
            let plan = _prepared.stage.ordering.as_ref().ok_or_else(|| {
                pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::InvalidState,
                    "阶段缺少有效执行计划",
                )
            })?;
            for wave in &plan.waves {
                for &index in &wave.systems {
                    let slot = _prepared.stage.systems.get_mut(index).ok_or_else(|| {
                        pi_ecs_core::error::classified_error(
                            pi_ecs_core::error::ErrorKind::InvalidState,
                            "执行计划包含无效系统槽",
                        )
                    })?;
                    let Some(system) = slot.system.as_mut() else {
                        continue;
                    };
                    slot.resolved.clear();
                    slot.resolved = system.prepare(&mut *_world)?;
                    // SAFETY: RC 独占根运行权；前一系统已完整返回，无活参数或后台任务。
                    // prepare 刷新完整声明，run_prepared 自持声明借用直到会话收束。
                    let tick = unsafe { _world.next_system_tick()? };
                    unsafe { system.run_prepared(&mut *_world, tick)? };
                }
            }
            Ok(())
        }
        #[cfg(not(feature = "rc"))]
        {
            Err(pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::Unsupported,
                "非 RC 执行与并发授权延期至第二轮",
            ))
        }
    }
}
