//! 资源参数声明与本次运行的值包装。
#[path = "resource_param_inner.rs"]
mod resource_param_inner;
use pi_ecs_core::{
    change_detection::{ChangeWindow, Mut},
    resource::ResourceTicks,
};
/// 唯一资源共享读取，必须存在；可选参数缺失为 None。
pub struct Res<'a, T: Send + Sync + 'static> {
    /// 授权的当前实例，不缓存跨阶段地址。
    pub(crate) value: &'a T,
    /// 始终维护的实例时间。
    pub(crate) ticks: &'a ResourceTicks,
    /// 本系统观察窗口。
    pub(crate) window: ChangeWindow,
}
/// 唯一资源独占访问，按 Mut 的可写解引用标记规则工作。
pub struct ResMut<'a, T: Send + Sync + 'static> {
    /// 当前值与必定启用的资源变化标记。
    pub(crate) value: Mut<'a, T>,
    /// 本次系统观察窗口，标记后检查应反映当前变化。
    pub(crate) window: ChangeWindow,
}
/// 多例读者遍历紧凑数组，读取不消费/清空值；顺序为当前物理顺序，不承诺登记顺序。
pub struct MultiRes<'a, T: Send + Sync + 'static> {
    /// 本阶段稳定的紧凑实例，执行中禁止结构搬移。
    pub(crate) values: &'a [T],
    /// 与 values 等长的变化时间。
    pub(crate) ticks: &'a [ResourceTicks],
    /// 增删实例的结构版本，不代替实例 changed tick。
    pub(crate) structure_version: u64,
    /// 本读者上次成功执行所观察版本；None 表示首次，首次视为结构变化。
    pub(crate) previous_structure_version: Option<u64>,
    /// 本次逐实例 tick 检查窗口。
    pub(crate) window: ChangeWindow,
}
/// 每个生产者参数实例独占一槽，不同生产者可并行写，集合读者在写者之后。
pub struct MultiResMut<'a, T: Send + Sync + 'static> {
    /// 阶段准备绑定的槽值；取参时 Core 校验当前版本及槽身份。
    pub(crate) value: Mut<'a, T>,
    /// 本次系统观察窗口，标记后检查应反映当前变化。
    pub(crate) window: ChangeWindow,
}
/// 多例写参数长期槽身份，归属由系统和参数位置确定，不以线程编号分配。
pub struct ProducerState {
    /// 稳定系统身份。
    pub(crate) system: crate::system::SystemId,
    /// 系统内从零开始的叶位置，嵌套组合按声明顺序展开。
    pub(crate) parameter: usize,
    /// 首次为 None；prepare 按唯一责任查询 ProducerBinding，版本变化只刷新本参数。
    /// 不保存业务地址；执行前须本次准备有效，不因原型总量未变跳过核对。
    pub(crate) binding: Option<pi_ecs_core::resource::ProducerBinding>,
}

/// 按系统参数位置拥有初值与恢复能力；仅首次 initialize 移入初值，不按次重置。
/// 类型化配置接口已确认，历史能力擦除表示待细化；配置方法不执行复制/恢复或登记槽。
pub struct MultiResConfig<T: Send + Sync + 'static> {
    /// 尚未消费的初值；同类型不同位置独立拥有。
    pub(crate) initial: T,
    /// None 允许普通运行，但游戏快照捕获拒绝；重复选择恢复策略必须报错。
    pub(crate) recovery: Option<pi_ecs_core::resource::StateRecovery>,
    // @to_design 延期的快照恢复：捕获/恢复函数与历史 S 的擦除表示及长期能力保存。
    /// # 用途
    /// 历史能力表示占位；尚未承诺可捕获或恢复，不影响已确认的初值所有权。
    pub(crate) _pending_capabilities: (),
}
impl<T: Send + Sync + 'static> MultiResConfig<T> {
    /// initial 为首次初始化值，只接管所有权，不建立 Local/生产者槽或订阅。
    pub fn new(initial: T) -> Self {
        Self::multiresconfig_new(initial)
    }
    /// 选择 Clone 保存当前值而非初值；调用方保证副本独立，不适用于运行引用计数句柄。
    /// 消费 self；已选择策略则错误，不静默覆盖、不修改 World。错误时配置按普通拥有值释放。
    pub fn save_clone(self) -> Result<Self, pi_ecs_core::error::EcsError>
    where
        T: Clone,
    {
        Self::multiresconfig_clone(self)
    }
    /// capture/restore 为独立 T→S→T 能力；S 析构不得操作运行计数，恢复遵守非活动准备协议。
    /// 消费 self；重复选择错误，不执行回调，失败配置正常释放。
    pub fn save_with<S: 'static>(
        self,
        capture: pi_ecs_core::snapshot_rules::SnapshotCapture<T, S>,
        restore: pi_ecs_core::snapshot_rules::SnapshotRestore<T, S>,
    ) -> Result<Self, pi_ecs_core::error::EcsError> {
        Self::multiresconfig_with(self, capture, restore)
    }
    /// reset 为恢复准备时的合法值工厂，不访问未安装 World；首次仍使用 initial。
    /// 消费 self；重复选择错误，不立即调用 reset，失败配置正常释放。
    pub fn rebuild(
        self,
        reset: pi_ecs_core::snapshot_rules::DerivedInitial<T>,
    ) -> Result<Self, pi_ecs_core::error::EcsError> {
        Self::multiresconfig_rebuild(self, reset)
    }
    /// 选择保留目标当前值，仅用于不影响确定性模拟的状态；空目标需先完成初始化。
    /// 消费 self；重复选择错误，失败配置正常释放。
    pub fn preserve(self) -> Result<Self, pi_ecs_core::error::EcsError> {
        Self::multiresconfig_preserve(self)
    }
}
impl<T: Send + Sync + Default + 'static> Default for MultiResConfig<T> {
    /// 仅用 T::default 提供初值，不隐式选择恢复策略；配置不立即初始化参数。
    fn default() -> Self {
        Self::multiresconfig_default()
    }
}

impl<T: Send + Sync + 'static> std::ops::Deref for Res<'_, T> {
    type Target = T;
    /// 只读访问当前值，不推进游标或标记 changed。
    fn deref(&self) -> &T {
        Self::res_deref(self)
    }
}
impl<T: Send + Sync + 'static> Res<'_, T> {
    /// 检查本实例 tick 是否落在系统本次观察窗口，不消费或标记。
    pub fn is_added(&self) -> bool {
        Self::res_is_added(self)
    }
    /// 检查本实例 tick 是否落在系统本次观察窗口，不消费或标记。
    pub fn is_changed(&self) -> bool {
        Self::res_is_changed(self)
    }
}

impl<T: Send + Sync + 'static> std::ops::Deref for ResMut<'_, T> {
    type Target = T;
    /// 只读访问当前值，不推进游标或标记 changed。
    fn deref(&self) -> &T {
        Self::resmut_deref(self)
    }
}
impl<T: Send + Sync + 'static> std::ops::DerefMut for ResMut<'_, T> {
    /// 返回可写引用前标记 changed；同一包装共用一次标记，不比较值相等。
    fn deref_mut(&mut self) -> &mut T {
        Self::resmut_deref_mut(self)
    }
}
impl<T: Send + Sync + 'static> ResMut<'_, T> {
    /// 检查本实例 tick 是否落在系统本次观察窗口，不消费或标记。
    pub fn is_added(&self) -> bool {
        self.value.is_added(self.window)
    }
    /// 检查本实例 tick 是否落在系统本次观察窗口，不消费或标记。
    pub fn is_changed(&self) -> bool {
        self.value.is_changed(self.window)
    }
    /// 显式标记 changed，不额外授予修改权限；与可写解引用共用一次标记，无绕过标记入口。
    pub fn set_changed(&mut self) {
        Self::resmut_set_changed(self)
    }
}

impl<T: Send + Sync + 'static> std::ops::Deref for MultiResMut<'_, T> {
    type Target = T;
    /// 只读访问当前值，不推进游标或标记 changed。
    fn deref(&self) -> &T {
        Self::multiresmut_deref(self)
    }
}
impl<T: Send + Sync + 'static> std::ops::DerefMut for MultiResMut<'_, T> {
    /// 返回可写引用前标记 changed；同一包装共用一次标记，不比较值相等。
    fn deref_mut(&mut self) -> &mut T {
        Self::multiresmut_deref_mut(self)
    }
}
impl<T: Send + Sync + 'static> MultiResMut<'_, T> {
    /// 检查本实例 tick 是否落在系统本次观察窗口，不消费或标记。
    pub fn is_added(&self) -> bool {
        self.value.is_added(self.window)
    }
    /// 检查本实例 tick 是否落在系统本次观察窗口，不消费或标记。
    pub fn is_changed(&self) -> bool {
        self.value.is_changed(self.window)
    }
    /// 显式标记 changed，不额外授予修改权限；与可写解引用共用一次标记，无绕过标记入口。
    pub fn set_changed(&mut self) {
        Self::multiresmut_set_changed(self)
    }
}

/// 多例只读迭代，逐项包装含值和变化窗口，不消费值，不返回稳定生产者下标。
pub struct MultiResIter<'a, T: Send + Sync + 'static> {
    /// 当前参数借用，存活期间禁止实例数组搬移。
    pub(crate) source: &'a MultiRes<'a, T>,
    /// 下一个物理偏移，非稳定身份。
    pub(crate) offset: usize,
}
impl<'a, T: Send + Sync + 'static> Iterator for MultiResIter<'a, T> {
    type Item = Res<'a, T>;
    /// 返回值/tick 属于同一槽的只读包装；到尾返回 None，不消费或清空实例。
    fn next(&mut self) -> Option<Self::Item> {
        Self::multi_next(self)
    }
}
impl<T: Send + Sync + 'static> MultiRes<'_, T> {
    /// 当前槽数量，读取不修改生产者或观察进度。
    pub fn len(&self) -> usize {
        Self::multi_len(self)
    }
    /// 当前是否无槽，读取不修改观察进度。
    pub fn is_empty(&self) -> bool {
        Self::multi_empty(self)
    }
    /// 借用本集合，以当前物理顺序逐项读取值及 tick，不承诺登记顺序。
    pub fn iter(&self) -> MultiResIter<'_, T> {
        Self::multi_iter(self)
    }
    /// 自上次成功执行后有增删即为 true，数量相同也不抹去；首次为 true。
    /// 查询不推进版本，系统成功完成后保存本次观察版本；值修改不算结构变化。
    pub fn is_structure_changed(&self) -> bool {
        Self::multi_structure(self)
    }
}

// RC 登记/清偿使用 ParameterRegistration 责任接口，运行注入使用 RunAccess 的资源单槽/集合读取接口。
// 历史恢复能力的表示延期，不阻塞 RC 初值/槽拥有关系。
// 单例按资源编号判读写冲突；多例不同生产者写写兼容，读全体与全部写者冲突。
// 多例注销在静止点移除值并正常 Drop；跳过运行保留旧值，读者不自动清空。
