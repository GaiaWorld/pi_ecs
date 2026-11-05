//! 恢复协调配对芯；不访问 core 私有表示，不调用其他芯。
use crate::{schedule::Schedule, snapshot::GameSnapshot};
use pi_ecs_core::{error::EcsError, world::World};
pub(crate) fn capture_game_inner(
    _world: &World,
    _schedule: &Schedule,
) -> Result<GameSnapshot, EcsError> {
    todo!("@to_design 核心数据捕获接口与运行状态范围")
}
pub(crate) fn restore_game_inner(
    _world: &mut World,
    _schedule: &mut Schedule,
    _snapshot: &GameSnapshot,
) -> Result<(), EcsError> {
    todo!("@to_design 核心准备安装接口、停止态与重建协议")
}
