//! Spawn 每参数位置的创建配置。
#[path = "spawn_inner.rs"]
mod spawn_inner;

use pi_ecs_core::entity::PartitionId;

pub use pi_ecs_core::spawn::Spawn;

/// # 用途
/// 固定 Spawn 的出生/初始分区；默认 PartitionId(0)，非零显式配置并在参数登记时检查。
pub struct SpawnConfig {
    /// 本位置全部调用使用的 World 分区编号，不得在运行时覆盖。
    pub partition: PartitionId,
}

impl Default for SpawnConfig {
    /// # 用途
    /// 返回配置 PartitionId(0)，不访问 World；有效性由初始化检查。
    fn default() -> Self {
        Self::default_inner()
    }
}
