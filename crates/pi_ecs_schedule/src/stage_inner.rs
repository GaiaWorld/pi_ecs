//! 阶段配对芯，经执行器壳协作，不调用另一芯。
use crate::{
    executor::{Executor, ExecutorId},
    stage::{PreparedStage, Stage, StageId},
};
use pi_ecs_core::{error::EcsError, world::World};
pub(crate) fn new_inner(_id: StageId, _executor: ExecutorId) -> Stage {
    todo!("尚未实现")
}
pub(crate) fn prepare_inner<'s>(
    _stage: &'s mut Stage,
    _world: &World,
) -> Result<PreparedStage<'s>, EcsError> {
    todo!("@to_design 系统准备状态与原型版本")
}
pub(crate) fn run_inner(
    _stage: &mut Stage,
    _world: &World,
    _executor: &mut Executor,
) -> Result<(), EcsError> {
    todo!("@to_design 系统执行与阶段收束")
}
