//! 调度配对芯；不调用其他芯。
#[cfg(test)]
mod configuration_tests {
    use crate::{
        executor::{ExecutionMode, Executor},
        schedule::Schedule,
        stage::{Stage, StageId},
        system::SystemId,
        system_builder::SystemBuilder,
    };

    // Seam: Schedule 只读组装验证，失败不消费未初始化系统的捕获。
    #[test]
    fn configuration_schedule_rejects_invalid_bindings_without_consuming_systems() {
        let mut schedule = Schedule::new();
        assert!(schedule.validate_configuration().is_ok());
        let executor = schedule
            .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
            .unwrap();
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let capture = calls.clone();
        let mut stage = Stage::new(StageId(19), executor);
        stage
            .add_system(
                SystemBuilder::new(move || {
                    capture.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                })
                .build(SystemId(7))
                .unwrap(),
            )
            .unwrap();
        schedule.add_stage(stage).unwrap();
        assert!(schedule.validate_configuration().is_ok());
        schedule.stages[0].executor = crate::executor::ExecutorId(9);
        assert!(schedule.validate_configuration().is_err());
        schedule.stages[0].executor = executor;
        schedule.stages.push(Stage::new(StageId(19), executor));
        assert!(schedule.validate_configuration().is_err());
        schedule.stages.pop();
        assert!(schedule.validate_configuration().is_ok());
        assert_eq!(std::sync::Arc::strong_count(&calls), 2);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        drop(schedule);
        assert_eq!(std::sync::Arc::strong_count(&calls), 1);
    }

    #[cfg(feature = "rc")]
    #[test]
    fn configuration_schedule_rejects_parallel_before_world_access() {
        let mut schedule = Schedule::new();
        schedule.executors.push(Executor {
            mode: ExecutionMode::Parallel,
        });
        assert!(schedule.validate_configuration().is_err());
    }

    #[cfg(feature = "rc")]
    #[test]
    fn rc_schedule_runs_serial_stages_in_order_on_every_run() {
        use pi_ecs_core::world::World;
        use std::{cell::RefCell, rc::Rc};

        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut schedule = Schedule::new();
        let executor = schedule
            .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
            .unwrap();
        for (stage_id, system_ids) in [(7, [3, 1]), (2, [4, 0])] {
            let mut stage = Stage::new(StageId(stage_id), executor);
            for system_id in system_ids {
                let calls = calls.clone();
                stage
                    .add_system(
                        SystemBuilder::new(move || calls.borrow_mut().push(system_id))
                            .build(SystemId(system_id))
                            .unwrap(),
                    )
                    .unwrap();
            }
            schedule.add_stage(stage).unwrap();
        }
        let mut world = World::new().unwrap();
        world.finish_registration().unwrap();
        let first_tick = world.read_change_tick().get();
        schedule.run(&mut world).unwrap();
        schedule.run(&mut world).unwrap();
        assert_eq!(&*calls.borrow(), &[3, 1, 4, 0, 3, 1, 4, 0]);
        assert_eq!(world.read_change_tick().get(), first_tick + 8);
        schedule.unregister(&mut world).unwrap();
        world.close().unwrap();
    }

    // Seam: 宿主按稳定阶段身份借用配置，缺失不创建阶段。
    #[test]
    fn configuration_stage_lookup_preserves_registration_order() {
        let mut schedule = Schedule::new();
        let executor = schedule
            .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
            .unwrap();
        schedule
            .add_stage(Stage::new(StageId(19), executor))
            .unwrap();
        schedule
            .add_stage(Stage::new(StageId(3), executor))
            .unwrap();
        assert!(schedule.stage_mut(StageId(0)).is_err());
        let mut stage = schedule.stage_mut(StageId(3)).unwrap();
        assert_eq!(
            stage
                .add_system(SystemBuilder::new(|| {}).build(SystemId(7)).unwrap())
                .unwrap(),
            0
        );
        assert!(stage
            .add_system(SystemBuilder::new(|| {}).build(SystemId(7)).unwrap())
            .is_err());
        assert!(schedule
            .stage_mut(StageId(19))
            .unwrap()
            .add_system(SystemBuilder::new(|| {}).build(SystemId(7)).unwrap())
            .is_ok());
    }
}

impl crate::schedule::Schedule {
    pub(super) fn configuration_inner(&mut self) -> crate::schedule::ScheduleConfig<'_> {
        crate::schedule::ScheduleConfig { schedule: self }
    }
    pub(super) fn stage_mut_inner(
        &mut self,
        _id: crate::stage::StageId,
    ) -> Result<crate::schedule::StageConfig<'_>, pi_ecs_core::error::EcsError> {
        self.stages
            .iter_mut()
            .find(|stage| stage.id == _id)
            .map(|stage| crate::schedule::StageConfig { stage })
            .ok_or_else(|| {
                pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::NotFound,
                    "阶段身份未登记",
                )
            })
    }
}
#[cfg(feature = "rc")]
impl crate::schedule::Schedule {
    pub(super) fn unregister_inner(
        &mut self,
        _world: &mut pi_ecs_core::world::World,
    ) -> Result<(), pi_ecs_core::error::EcsError> {
        _world.parameter_unregistration_scope(|_| Ok(()))?;
        let mut first = None;
        for stage in &mut self.stages {
            if let Err(error) = stage.unregister(_world) {
                if first.is_none() {
                    first = Some(error);
                }
            }
        }
        first.map_or(Ok(()), Err)
    }
}
impl Schedule {
    pub(crate) fn add_executor_inner(
        schedule: &mut Schedule,
        executor: crate::executor::Executor,
    ) -> Result<crate::executor::ExecutorId, EcsError> {
        let id = crate::executor::ExecutorId(schedule.executors.len());
        schedule.executors.push(executor);
        Ok(id)
    }
}
impl Schedule {
    pub(super) fn validate_install_inner(&self) -> Result<(), EcsError> {
        self.validate_configuration()?;
        for stage in &self.stages {
            stage.validate_install()?;
        }
        Ok(())
    }
    #[cfg(feature = "rc")]
    pub(super) fn unregister_system_inner(
        &mut self,
        stage: crate::stage::StageId,
        id: crate::system::SystemId,
        world: &mut pi_ecs_core::world::World,
    ) -> Result<(), EcsError> {
        self.stage_mut_inner(stage)?
            .stage
            .unregister_system(id, world)
    }
    pub(crate) fn add_stage_inner(
        schedule: &mut Schedule,
        stage: crate::stage::Stage,
    ) -> Result<(), crate::stage::InstallError<crate::stage::Stage>> {
        use crate::stage::InstallError;
        if let Err(error) = stage.validate_install() {
            return Err(InstallError {
                value: stage,
                error,
            });
        }
        if schedule.stages.iter().any(|value| value.id == stage.id) {
            return Err(InstallError {
                value: stage,
                error: pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::AlreadyExists,
                    "阶段身份重复",
                ),
            });
        }
        if stage.executor.0 >= schedule.executors.len() {
            return Err(InstallError {
                value: stage,
                error: pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::NotFound,
                    "阶段引用了不存在的执行器",
                ),
            });
        }
        schedule.stages.push(stage);
        Ok(())
    }
}
use crate::schedule::Schedule;
use pi_ecs_core::{error::EcsError, world::WorldRef};
impl Schedule {
    pub(crate) fn new_inner() -> Schedule {
        Schedule {
            stages: Vec::new(),
            executors: Vec::new(),
        }
    }
}
#[cfg(feature = "rc")]
impl Schedule {
    pub(crate) fn run_inner(schedule: &mut Schedule, world: WorldRef<'_>) -> Result<(), EcsError> {
        schedule.validate_configuration()?;
        world.schedule_scope(|world| {
            for stage in &mut schedule.stages {
                stage.run(&mut *world, &mut schedule.executors[stage.executor.0])?;
            }
            Ok(())
        })
    }
}

#[cfg(all(test, feature = "rc"))]
#[test]
fn rc_schedule_runs_ordinary_system_with_an_empty_entry_flow() {
    use crate::{
        executor::{ExecutionMode, Executor},
        inter_frame::SystemFlow,
        stage::{Stage, StageId},
        system::SystemId,
        system_builder::SystemBuilder,
    };
    use std::{cell::Cell, rc::Rc};

    let mut world = pi_ecs_core::world::World::new().unwrap();
    world.finish_registration().unwrap();
    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    let mut stage = Stage::new(StageId(0), executor);
    assert!(stage.set_entry_flow(SystemFlow::new()).is_ok());
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    stage
        .add_system(
            SystemBuilder::new(move || observed.set(observed.get() + 1))
                .build(SystemId(0))
                .unwrap(),
        )
        .unwrap();
    schedule.add_stage(stage).unwrap();

    schedule.run(&mut world).unwrap();
    schedule.run(&mut world).unwrap();
    assert_eq!(calls.get(), 2);
    assert!(!world.is_stopped());
    schedule.unregister(&mut world).unwrap();
    let observed = Rc::clone(&calls);
    schedule
        .stage_mut(StageId(0))
        .unwrap()
        .add_system(
            SystemBuilder::new(move || observed.set(observed.get() + 10))
                .build(SystemId(1))
                .unwrap(),
        )
        .unwrap();
    schedule.run(&mut world).unwrap();
    assert_eq!(calls.get(), 12);
    assert!(!world.is_stopped());
    schedule.unregister(&mut world).unwrap();
    world.close().unwrap();
}

#[cfg(all(test, feature = "rc"))]
#[test]
fn rc_schedule_stops_on_invalid_published_slot_and_keeps_original_diagnostic() {
    use crate::{
        executor::{ExecutionMode, Executor},
        stage::{Stage, StageId},
        system::SystemId,
        system_builder::SystemBuilder,
    };
    use std::{cell::Cell, rc::Rc};

    let mut world = pi_ecs_core::world::World::new().unwrap();
    world.finish_registration().unwrap();
    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    let calls = Rc::new(Cell::new(0));
    for index in 0..2 {
        let mut stage = Stage::new(StageId(index), executor);
        let calls = calls.clone();
        stage
            .add_system(
                SystemBuilder::new(move || calls.set(calls.get() + 1))
                    .build(SystemId(0))
                    .unwrap(),
            )
            .unwrap();
        schedule.add_stage(stage).unwrap();
    }
    schedule.run(&mut world).unwrap();
    assert_eq!(calls.get(), 2);
    // 注入已发布而非候选计划故障；布局不变，阶段复用此计划。
    schedule.stages[0].ordering.as_mut().unwrap().waves[0].systems = Box::new([0, usize::MAX]);
    let error = schedule.run(&mut world).unwrap_err();
    assert!(format!("{error:?}").contains("执行计划包含无效系统槽"));
    assert!(world.is_stopped());
    assert_eq!(calls.get(), 2);
    schedule.unregister(&mut world).unwrap();
    world.close().unwrap();
}
#[cfg(not(feature = "rc"))]
impl Schedule {
    pub(crate) fn run_inner(
        schedule: &mut Schedule,
        mut world: WorldRef<'_>,
    ) -> Result<(), EcsError> {
        for index in 0..schedule.stages.len() {
            let executor_id = schedule.stages[index].executor.0;
            let executor = schedule.executors.get_mut(executor_id).ok_or_else(|| {
                pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::Internal,
                    "阶段执行器缺失",
                )
            })?;
            #[cfg(feature = "rc")]
            schedule.stages[index].run(&mut *world, executor)?;
            #[cfg(not(feature = "rc"))]
            schedule.stages[index].run(world, executor)?;
        }
        Ok(())
    }
}

impl crate::schedule::Schedule {
    pub(super) fn validate_configuration_inner(&self) -> Result<(), pi_ecs_core::error::EcsError> {
        #[cfg(feature = "rc")]
        if self
            .executors
            .iter()
            .any(|executor| matches!(executor.mode, crate::executor::ExecutionMode::Parallel))
        {
            return Err(pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::Unsupported,
                "rc 模式不支持并行执行器",
            ));
        }
        let mut identities = std::collections::HashSet::new();
        for stage in &self.stages {
            if !identities.insert(stage.id) {
                return Err(pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::AlreadyExists,
                    "阶段身份重复",
                ));
            }
            if stage.executor.0 >= self.executors.len() {
                return Err(pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::NotFound,
                    "阶段执行器未登记",
                ));
            }
            stage.validate_configuration()?;
        }
        Ok(())
    }
}
#[cfg(feature = "rc")]
impl Schedule {
    pub(super) fn run_stages_inner(
        &mut self,
        _world: &mut pi_ecs_core::world::World,
        _stages: &[crate::stage::StageId],
    ) -> Result<(), EcsError> {
        self.validate_configuration()?;
        let indices = _stages.iter().map(|id| {
            self.stages.iter().position(|stage| stage.id == *id).ok_or_else(|| {
                pi_ecs_core::error::classified_error(pi_ecs_core::error::ErrorKind::NotFound, "阶段序列包含未登记身份")
            })
        }).collect::<Result<Vec<_>, _>>()?;
        _world.schedule_scope(|world| {
            for index in indices {
                let stage = &mut self.stages[index];
                stage.run(&mut *world, &mut self.executors[stage.executor.0])?;
            }
            Ok(())
        })
    }
}
