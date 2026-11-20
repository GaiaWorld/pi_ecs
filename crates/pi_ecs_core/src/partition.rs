//! 纯迁区请求；首轮由宿主 World::move_partition 在冻结后 Idle 静止点提交。
use crate::entity::{Entity, PartitionId};
/// # 用途
/// 一个已计算的迁区请求；不隐式修改原型、身份来源、组件值或变化 tick。
/// 目标范围和实体活性由 World::move_partition 校验；首轮无系统内提交入口。
#[derive(Clone, Copy, Debug)]
pub struct PartitionMove {
    /// 本 World 待迁移实体的完整身份，成功迁移保持身份不变。
    pub entity: Entity,
    /// 当前地图目标；不是重新分配身份的来源域。
    pub target: PartitionId,
}
// @to_design 后续自动判区扩展（不属首轮 RC）：候选查询、固定提交顺序、重复请求、目标不存在与迁移失败协议。
