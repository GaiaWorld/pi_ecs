//! 调度配对芯；不调用其他芯。
pub(crate) fn add_executor_inner(
    _schedule: &mut Schedule,
    _executor: crate::executor::Executor,
) -> Result<crate::executor::ExecutorId, EcsError> {
    todo!("尚未实现")
}
pub(crate) fn add_stage_inner(
    _schedule: &mut Schedule,
    _stage: crate::stage::Stage,
) -> Result<(), EcsError> {
    todo!("尚未实现")
}
use crate::schedule::Schedule;
use pi_ecs_core::{error::EcsError, world::World};
pub(crate) fn new_inner() -> Schedule {
    todo!("@to_design 系统和执行图表示")
}
pub(crate) fn run_inner(_schedule: &mut Schedule, _world: &World) -> Result<(), EcsError> {
    todo!("@to_design 执行与失败协议")
}
