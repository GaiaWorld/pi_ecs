//! 自动分区配对芯；纯搬移必须调用 core 的 World 壳。
use crate::plugin::PartitionPlugin;
use pi_ecs_app::app::App;
use pi_ecs_core::error::EcsError;
impl PartitionPlugin {
    pub(super) fn new_inner() -> PartitionPlugin {
        todo!("尚未实现判区配置")
    }
    pub(super) fn build_inner(_plugin: &PartitionPlugin, _app: &mut App) -> Result<(), EcsError> {
        todo!("尚未实现普通系统和迁移阶段注册")
    }
}
