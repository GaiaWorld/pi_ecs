//! World 全局活值 tick 需求；通知开关单源来自 Notifications 的真实订阅表。
#[path = "tracking_inner.rs"]
mod tracking_inner;
use crate::{component::ComponentId, error::EcsError, world::World};
/// # 用途
/// 一个组件的活值 tick 需求，不含通知需求；同组件按位并集，不能扩散到其他组件。
pub struct ComponentTickDemand {
    /// 本 World 已登记组件。
    pub component: ComponentId,
    /// 需要活值 added tick。
    pub added: bool,
    /// 需要活值 changed tick。
    pub changed: bool,
}
pi_key::new_key_type! {
    /// World 需求表内的完整代际身份，不跨 World 解释。
    pub(crate) struct TickDemandId;
}
/// # 用途
/// World 拥有的全部长期查询需求，覆盖其他 Stage 和宿主，不由某个 Stage 覆盖全局状态。
pub(crate) struct TrackingRegistry {
    /// 单参数规范化需求；无需求不登记，注销只删除该份，槽代际由 SlotMap 管理。
    pub(crate) queries: pi_slotmap::SlotMap<TickDemandId, Box<[ComponentTickDemand]>>,
}
/// # 用途
/// 查询持有的不可复制登记责任，普通 Drop 不操作 World；漏显式解除只保守保留需求至关闭。
pub(crate) struct TickDemandRegistration {
    /// 所属 World 非复用身份。
    pub(crate) world_identity: u64,
    /// 唯一登记键，不能从另一个查询复制。
    pub(crate) id: TickDemandId,
}
impl World {
    /// # 用途
    /// RC 静止点应用全部查询 tick 并集与真实通知订阅并集；Stage 在全部初始化完成后调用。
    /// Idle 或根 Running 的阶段准备点、无参数/批次借用；Stopped/关闭/业务执行中拒绝。
    /// 仅首次启用/重启活值 tick 时分配本轮维护 tick，并初始化对应现存值槽；耗尽在修改前拒绝。
    /// 根 Running 的维护时钟耗尽单向停止并返回原始诊断；独立 Idle 维护拒绝仍保留 Idle。
    /// 已启用项不重置；减少需求只停更新，不影响其他登记，不补通知或清历史通知。
    /// Reader 不强制启用活值 tick；通知流及旧值回收沿 Notifications 原协议，不在此执行析构。
    /// 物理转换统一调用 WorldStorage::apply_tracking，传当前需求差异及同一维护 tick；
    /// 物理提交成功后才更新有效追踪状态，普通 Err 保留旧有效状态，panic 沿根门禁标停。
    #[cfg(feature = "rc")]
    pub fn apply_tracking(&mut self) -> Result<(), EcsError> {
        self.apply_tracking_inner()
    }
}
