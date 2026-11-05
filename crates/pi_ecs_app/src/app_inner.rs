//! App 配对芯；仅明确未实现桩。
pub(crate) fn capture_game_inner(
    _app: &mut crate::app::App,
) -> Result<pi_ecs_schedule::snapshot::GameSnapshot, pi_ecs_core::error::EcsError> {
    todo!("@to_design 全 Stage 状态协调与维护授权接线")
}
pub(crate) fn restore_game_inner(
    _app: &mut crate::app::App,
    _snapshot: &pi_ecs_schedule::snapshot::GameSnapshot,
) -> Result<(), pi_ecs_core::error::EcsError> {
    todo!("@to_design 全 Stage 恢复与内部存储排他入口")
}

use crate::{
    app::{App, ScheduleLabel},
    plugin::Plugin,
};
use pi_ecs_core::{error::EcsError, world::World};
use pi_ecs_schedule::schedule::Schedule;
pub(crate) fn label_inner<T: 'static>() -> ScheduleLabel {
    todo!("尚未实现类型标签")
}
pub(crate) fn new_inner(_world: World) -> App {
    todo!("尚未实现 App 组装")
}
pub(crate) fn add_plugin_inner<P: Plugin + 'static>(
    _app: &mut App,
    _plugin: P,
) -> Result<(), EcsError> {
    todo!("尚未实现插件安装")
}
pub(crate) fn require_plugin_inner<P: Plugin + 'static>(_app: &App) -> Result<(), EcsError> {
    todo!("尚未实现插件依赖检查")
}
pub(crate) fn add_schedule_inner(
    _app: &mut App,
    _label: ScheduleLabel,
    _schedule: Schedule,
) -> Result<(), EcsError> {
    todo!("尚未实现调度登记")
}
pub(crate) fn schedule_mut_inner(
    _app: &mut App,
    _label: ScheduleLabel,
) -> Result<&mut Schedule, EcsError> {
    todo!("尚未实现调度配置访问")
}
pub(crate) fn world_inner(_app: &App) -> &World {
    todo!("尚未实现共享 World 访问")
}
pub(crate) fn world_mut_inner(_app: &mut App) -> Result<&mut World, EcsError> {
    todo!("@to_design 稳定发布与独占配置访问安全限制")
}
pub(crate) fn finish_inner(_app: &mut App) -> Result<(), EcsError> {
    todo!("尚未实现构建收束")
}
pub(crate) fn run_inner(_app: &mut App, _label: ScheduleLabel) -> Result<(), EcsError> {
    todo!("尚未实现命名调度运行")
}
