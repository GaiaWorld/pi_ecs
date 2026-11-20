//! 阶段配对芯，经执行器壳协作，不调用另一芯。
#[cfg(test)]
mod configuration_tests {
    use crate::{
        executor::ExecutorId,
        stage::{Stage, StageId},
        system::SystemId,
        system_builder::SystemBuilder,
    };
    // Seam: 执行器通过 StageSystem::system_mut 借用活动实例，注销空槽不可派发。
    #[test]
    fn configuration_stage_active_slot_borrow_skips_empty_slot() {
        let mut stage = Stage::new(StageId(4), ExecutorId(0));
        stage
            .add_system(SystemBuilder::new(|| {}).build(SystemId(11)).unwrap())
            .unwrap();
        assert_eq!(stage.systems[0].system_mut().unwrap().id(), SystemId(11));
        stage.systems[0].system = None;
        assert!(stage.systems[0].system_mut().is_none());
        assert!(stage
            .add_system(SystemBuilder::new(|| {}).build(SystemId(11)).unwrap())
            .is_err());
    }

    // Seam: 阶段组装验证，不初始化、不消费函数捕获。
    #[test]
    fn configuration_stage_checks_history_and_active_identity() {
        let mut stage = Stage::new(StageId(4), ExecutorId(0));
        stage
            .add_system(SystemBuilder::new(|| {}).build(SystemId(11)).unwrap())
            .unwrap();
        stage
            .add_system(SystemBuilder::new(|| {}).build(SystemId(13)).unwrap())
            .unwrap();
        assert!(stage.validate_configuration().is_ok());
        stage.systems[0].id = SystemId(9);
        assert!(stage.validate_configuration().is_err());
        stage.systems[0].system = None;
        assert!(stage.validate_configuration().is_ok());
        stage.systems[0].id = SystemId(13);
        assert!(stage.validate_configuration().is_err());
    }
}

use crate::{
    executor::{Executor, ExecutorId},
    stage::{PreparedStage, Stage, StageId},
    stage_plan::StagePlan,
    system_param::ParamRegistration,
};
use pi_ecs_core::{error::EcsError, world::WorldRef};
impl Stage {
    pub(crate) fn new_inner(id: StageId, executor: ExecutorId) -> Stage {
        Stage {
            id,
            executor,
            systems: Vec::new(),
            ordering_snapshot: None,
            ordering: None,
        }
    }
}
impl Stage {
    pub(crate) fn add_system_inner(
        stage: &mut Stage,
        system: crate::stage::SystemObject,
    ) -> Result<usize, EcsError> {
        let id = system.id();
        if stage.systems.iter().any(|slot| slot.id == id) {
            return Err(pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::AlreadyExists,
                "系统历史身份重复",
            ));
        }
        let index = stage.systems.len();
        stage.systems.push(crate::stage::StageSystem {
            id,
            system: Some(system),
            resolved: Vec::new(),
        });
        stage.ordering = None;
        stage.ordering_snapshot = None;
        Ok(index)
    }
}

impl crate::stage::StageSystem {
    pub(crate) fn system_mut_inner(
        system: &mut crate::stage::StageSystem,
    ) -> Option<&mut (dyn crate::system::System + '_)> {
        match system.system.as_mut() {
            Some(instance) => Some(instance.as_mut()),
            None => None,
        }
    }
}
impl Stage {
    pub(crate) fn prepare_inner<'s>(
        stage: &'s mut Stage,
        mut world: WorldRef<'_>,
    ) -> Result<PreparedStage<'s>, EcsError> {
        #[cfg(feature = "rc")]
        {
            world.parameter_registration_scope(|access| {
                let mut registration = ParamRegistration::new(access);
                for slot in &mut stage.systems {
                    if let Some(system) = slot.system.as_mut() {
                        system.initialize(&mut registration)?;
                    }
                }
                Ok(())
            })?;
            world.apply_tracking()?;
            for slot in &mut stage.systems {
                slot.resolved.clear();
                if let Some(system) = slot.system.as_mut() {
                    slot.resolved = system.prepare(&mut *world)?;
                }
            }
            let cell = pi_ecs_core::unsafe_world_cell::UnsafeWorldCell::from_ref(&mut *world);
            let (epoch, count) = cell.archetype_revision();
            let snapshot = (cell.id(), epoch, count);
            if stage.ordering_snapshot != Some(snapshot) || stage.ordering.is_none() {
                let active: Vec<_> = stage
                    .systems
                    .iter()
                    .enumerate()
                    .filter_map(|(index, slot)| {
                        slot.system.as_deref().map(|system| (index, system))
                    })
                    .collect();
                let trees = active
                    .iter()
                    .map(|(_, system)| system.access_tree())
                    .collect::<Result<Vec<_>, _>>()?;
                let logical = StagePlan::ordering_archetypes(world, &trees)?;
                let access = active
                    .iter()
                    .map(|(_, system)| system.ordering_access(&logical))
                    .collect::<Result<Vec<_>, _>>()?;
                let slots: Vec<_> = active.iter().map(|(index, _)| *index).collect();
                stage.ordering = Some(StagePlan::build_ordering(&logical, &access, &slots));
                stage.ordering_snapshot = Some(snapshot);
            }
            Ok(PreparedStage { stage })
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (stage, world);
            Err(pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::Unsupported,
                "非 RC 阶段准备延期",
            ))
        }
    }
}

impl Stage {
    #[cfg(feature = "rc")]
    pub(crate) fn run_inner(
        stage: &mut Stage,
        world: WorldRef<'_>,
        executor: &mut Executor,
    ) -> Result<(), EcsError> {
        {
            let mut prepared = stage.prepare(&mut *world)?;
            executor.execute(&mut *world, &mut prepared)?;
        }
        // SAFETY: the root schedule still owns Running; no stage task or parameter remains live.
        unsafe { world.flush_destruction_in_run() }
    }
    #[cfg(not(feature = "rc"))]
    pub(crate) fn run_inner(
        stage: &mut Stage,
        world: WorldRef<'_>,
        executor: &mut Executor,
    ) -> Result<(), EcsError> {
        let mut prepared = stage.prepare(world)?;
        executor.execute(world, &mut prepared)
    }
}

#[cfg(feature = "rc")]
impl crate::stage::Stage {
    pub(super) fn unregister_system_inner(
        &mut self,
        id: crate::system::SystemId,
        world: &mut pi_ecs_core::world::World,
    ) -> Result<(), EcsError> {
        let slot = self
            .systems
            .iter_mut()
            .find(|slot| slot.id == id)
            .ok_or_else(|| {
                pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::NotFound,
                    "系统身份未登记",
                )
            })?;
        world.parameter_unregistration_scope(|access| {
            let Some(system) = slot.system.as_mut() else { return Ok(()); };
            let ordering = self.ordering.take();
            let snapshot = self.ordering_snapshot.take();
            let result = system.unregister(&mut ParamRegistration::new(access));
            if matches!(&result, Err(error) if crate::system::SystemRunContext::is_foreign_unregistration(error)) {
                self.ordering = ordering;
                self.ordering_snapshot = snapshot;
                return result;
            }
            slot.resolved.clear();
            drop(slot.system.take());
            result
        })
    }
    pub(super) fn unregister_inner(
        &mut self,
        _world: &mut pi_ecs_core::world::World,
    ) -> Result<(), pi_ecs_core::error::EcsError> {
        _world.parameter_unregistration_scope(|access| {
            let mut registration = ParamRegistration::new(access);
            self.ordering = None;
            self.ordering_snapshot = None;
            let mut first = None;
            for slot in &mut self.systems {
                if let Some(system) = slot.system.as_mut() {
                    let result = system.unregister(&mut registration);
                    if !matches!(&result, Err(error) if crate::system::SystemRunContext::is_foreign_unregistration(error)) {
                        slot.resolved.clear();
                        drop(slot.system.take());
                    }
                    if let Err(error) = result {
                        if first.is_none() {
                            first = Some(error);
                        }
                    }
                }
            }
            first.map_or(Ok(()), Err)
        })
    }
}

impl crate::stage::Stage {
    pub(super) fn validate_configuration_inner(&self) -> Result<(), pi_ecs_core::error::EcsError> {
        let mut identities = std::collections::HashSet::new();
        for slot in &self.systems {
            if !identities.insert(slot.id) {
                return Err(pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::AlreadyExists,
                    "阶段系统历史身份重复",
                ));
            }
            if slot
                .system
                .as_ref()
                .is_some_and(|system| system.id() != slot.id)
            {
                return Err(pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::InvalidState,
                    "活动系统身份与槽身份不一致",
                ));
            }
        }
        Ok(())
    }
}
