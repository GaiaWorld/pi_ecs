//! 分区系统组装入口，不提供另一套身份实现。
#[path = "plugin_inner.rs"]
mod plugin_inner;
use pi_ecs_app::{app::App, plugin::Plugin};
use pi_ecs_core::error::EcsError;
/// 安装普通判区系统及有序迁移阶段；不持有身份或实体数据。
pub struct PartitionPlugin {
    // @to_design 后续扩展（不属首轮 RC）：判区输入、阶段标签、稳定提交顺序及配置。
    /// # 用途
    /// 判区配置占位，不表示存在默认策略或已登记迁移系统。
    pub(crate) _pending_config: (),
}
impl PartitionPlugin {
    /// 创建分区插件；具体判区策略如何配置尚待设计，不表示默认策略已存在。
    pub fn new() -> Self {
        Self::new_inner()
    }
}
impl Plugin for PartitionPlugin {
    /// 向 app 安装判区和迁移系统；失败遵循 Plugin 的部分登记契约。
    fn build(&self, app: &mut App) -> Result<(), EcsError> {
        Self::build_inner(self, app)
    }
}
