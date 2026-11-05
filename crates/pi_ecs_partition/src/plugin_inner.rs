//! 自动分区配对芯；纯搬移必须调用 core 的 World 壳。
use crate::plugin::PartitionPlugin;
use pi_ecs_app::app::App;
use pi_ecs_core::error::EcsError;
pub(crate) fn new_inner() -> PartitionPlugin {
    todo!("@to_design 判区配置")
}
pub(crate) fn build_inner(_plugin: &PartitionPlugin, _app: &mut App) -> Result<(), EcsError> {
    todo!("@to_design 普通系统和迁移阶段注册")
}
