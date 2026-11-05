//! 执行配对芯，不引用阶段芯或调度芯。
use crate::{
    executor::{ExecutionMode, Executor},
    stage::PreparedStage,
};
use pi_ecs_core::{error::EcsError, world::World};
pub(crate) fn new_inner(_mode: ExecutionMode) -> Result<Executor, EcsError> {
    todo!("@to_design 线程资源与系统表示")
}
pub(crate) fn execute_inner(
    _executor: &mut Executor,
    _world: &World,
    _prepared: &mut PreparedStage<'_>,
) -> Result<(), EcsError> {
    todo!("@to_design 就绪任务调度与任务收束")
}
