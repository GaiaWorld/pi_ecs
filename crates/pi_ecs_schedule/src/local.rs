//! 系统参数实例私有值，不登记 World 访问、tick 或通知。
#[path = "local_inner.rs"]
mod local_inner;
use pi_ecs_core::resource::StateRecovery;
/// 长期值由 System 持有；同类型在不同系统或参数位置均不共享。
pub struct Local<'a, T: Send + 'static> {
    /// 本次独占借用，正常返回后原值继续留在参数长期状态。
    pub(crate) value: &'a mut T,
}
/// Local 长期状态，不要求 T:Sync；初始化用显式初值或 Default。
pub struct LocalState<T: Send + 'static> {
    /// 跨执行保留的私有值，移除系统时普通 Drop。
    pub(crate) value: T,
    /// 游戏系统必须配置；None 时拒绝捕获，不能把隐含模拟状态漏出快照。
    pub(crate) recovery: Option<StateRecovery>,
    // RC 初值由 LocalConfig.initial 移入，运行只借 value；Save/Rebuild 历史能力表示延期。
}

/// 按系统参数位置拥有初值与恢复能力；仅首次 initialize 移入初值，不按次重置。
/// 类型化配置接口已确认，历史能力擦除表示待细化；配置方法不执行复制/恢复或登记槽。
pub struct LocalConfig<T: Send + 'static> {
    /// 尚未消费的初值；同类型不同位置独立拥有。
    pub(crate) initial: T,
    /// None 允许普通运行，但游戏快照捕获拒绝；重复选择恢复策略必须报错。
    pub(crate) recovery: Option<pi_ecs_core::resource::StateRecovery>,
    // @to_design 延期的快照恢复：捕获/恢复函数与历史 S 的擦除表示及长期能力保存。
    /// # 用途
    /// 历史能力表示占位；尚未承诺可捕获或恢复，不影响已确认的初值所有权。
    pub(crate) _pending_capabilities: (),
}
impl<T: Send + 'static> LocalConfig<T> {
    /// initial 为首次初始化值，只接管所有权，不建立 Local/生产者槽或订阅。
    pub fn new(initial: T) -> Self {
        Self::localconfig_new(initial)
    }
    /// 选择 Clone 保存当前值而非初值；调用方保证副本独立，不适用于运行引用计数句柄。
    /// 消费 self；已选择策略则错误，不静默覆盖、不修改 World。错误时配置按普通拥有值释放。
    pub fn save_clone(self) -> Result<Self, pi_ecs_core::error::EcsError>
    where
        T: Clone,
    {
        Self::localconfig_clone(self)
    }
    /// capture/restore 为独立 T→S→T 能力；S 析构不得操作运行计数，恢复遵守非活动准备协议。
    /// 消费 self；重复选择错误，不执行回调，失败配置正常释放。
    pub fn save_with<S: 'static>(
        self,
        capture: pi_ecs_core::snapshot_rules::SnapshotCapture<T, S>,
        restore: pi_ecs_core::snapshot_rules::SnapshotRestore<T, S>,
    ) -> Result<Self, pi_ecs_core::error::EcsError> {
        Self::localconfig_with(self, capture, restore)
    }
    /// reset 为恢复准备时的合法值工厂，不访问未安装 World；首次仍使用 initial。
    /// 消费 self；重复选择错误，不立即调用 reset，失败配置正常释放。
    pub fn rebuild(
        self,
        reset: pi_ecs_core::snapshot_rules::DerivedInitial<T>,
    ) -> Result<Self, pi_ecs_core::error::EcsError> {
        Self::localconfig_rebuild(self, reset)
    }
    /// 选择保留目标当前值，仅用于不影响确定性模拟的状态；空目标需先完成初始化。
    /// 消费 self；重复选择错误，失败配置正常释放。
    pub fn preserve(self) -> Result<Self, pi_ecs_core::error::EcsError> {
        Self::localconfig_preserve(self)
    }
}
impl<T: Send + Default + 'static> Default for LocalConfig<T> {
    /// 仅用 T::default 提供初值，不隐式选择恢复策略；配置不立即初始化参数。
    fn default() -> Self {
        Self::localconfig_default()
    }
}
// Local 跳过执行保留，替换系统不自动继承；首版非 Send Local 后置。

impl<T: Send + 'static> std::ops::Deref for Local<'_, T> {
    type Target = T;
    /// 只读访问当前值，不推进游标或标记 changed。
    fn deref(&self) -> &T {
        Self::local_deref(self)
    }
}
impl<T: Send + 'static> std::ops::DerefMut for Local<'_, T> {
    /// 只修改私有值，不记 tick 或通知。
    fn deref_mut(&mut self) -> &mut T {
        Self::local_deref_mut(self)
    }
}
// 不提供 take；替换使用标准 mem::replace 并交回旧值。注入仅借 LocalState.value，无 core 绑定层；芯待实现。
