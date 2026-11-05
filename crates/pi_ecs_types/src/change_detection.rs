//! 不依赖 World 的变化时间、观察窗口与需求定义。
/// 单调变化时间，执行上下文分配，普通搬移和整理保持原时间。
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ChangeTick(
    /// 非零 u64 时钟，合法范围 1..=u64::MAX；耗尽停止模拟并报错，不静默回绕；恢复按历史分配进度继续。
    pub(crate) std::num::NonZeroU64,
);
/// 普通窗口 last_run < value_tick <= this_run，首次无下界，仅判断 value_tick <= this_run。
/// 成功提交后推进 last_run，跳过不推进；当前系统标记 this_run 后立即可观察。
/// Added/Changed 过滤及资源变化检查共用规则，恢复沿用历史窗口，不制造业务变化。
pub struct ChangeWindow {
    /// 上次成功执行的观察点；None 表示首次无下界，不用零值伪造。
    pub last_run: Option<ChangeTick>,
    /// 本次执行取得的观察点，并发 tick 顺序不替代依赖关系。
    pub this_run: ChangeTick,
}
/// 每组件汇总的订阅需求，无需求的状态不分配/更新对应 tick 或通知。
#[derive(Clone, Copy, Debug, Default)]
pub struct TrackingDemand {
    /// 活组件 Added 查询需要的状态。
    pub added_tick: bool,
    /// 活组件 Changed 查询需要的状态。
    pub changed_tick: bool,
    /// 需要 Added 通知。
    pub added_notice: bool,
    /// 需要 Changed 通知，不强制开启活值 changed_tick。
    pub changed_notice: bool,
    /// 需要 Removed 旧值通知。
    pub removed_notice: bool,
}
/// 按需求可省略各项；值与其变化元数据一起迁移，不制造变化。
pub struct ComponentTicks {
    /// 未启用 Added 状态时为 None。
    pub added: Option<ChangeTick>,
    /// 未启用 Changed 状态时为 None。
    pub changed: Option<ChangeTick>,
}
