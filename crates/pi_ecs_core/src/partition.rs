//! 分区系统产生的纯迁移数据；具体系统与调度声明待设计。
use crate::entity::{Entity, PartitionId};
/// 一个已计算的迁移意图；不隐式修改类别或身份来源。
#[derive(Clone, Copy, Debug)]
pub struct PartitionMove {
    /// 本 World 待迁移实体的完整身份，成功迁移保持身份不变。
    pub entity: Entity,
    /// 当前地图目标；不是重新分配身份的来源域。
    pub target: PartitionId,
}
// @to_design 候选查询、固定提交顺序、重复实体请求、目标不存在和迁移失败协议；显式阶段与普通 System 接口尚待设计。
