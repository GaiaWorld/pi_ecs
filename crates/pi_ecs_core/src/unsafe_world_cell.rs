//! 统一底层 World 访问句柄；参数权限由 RunAccess 和 unsafe 参数协议证明。
#[path = "unsafe_world_cell_inner.rs"]
mod unsafe_world_cell_inner;
use crate::{error::EcsError, world::World};
use std::{cell::UnsafeCell, marker::PhantomData, ptr::NonNull};

/// # 用途
/// 从真实 World 借用派生的底层访问句柄，复制句柄不新增权限，也不证明访问互斥。
/// 普通参数不得通过本句柄还原整份 &World 或 &mut World；不提供这类逃逸接口。
/// RC 不要求 Send/Sync，不手写线程能力；内部仅在访问时定位获准字段/槽，不构造覆盖其他活借用的引用。
/// 经共享引用修改值时必须由对应 UnsafeCell/既有内部可变后端支持，裸指针不替代内部可变性。
#[derive(Clone, Copy)]
pub struct UnsafeWorldCell<'w> {
    /// 来源 World 的原始地址，始终存活；不向安全业务代码公开。
    pub(crate) pointer: NonNull<World>,
    /// true 仅说明来源为独占借用；具体参数权限仍由外部授权证明，不表示任意写权限。
    pub(crate) mutable: bool,
    /// 绑定原借用且保持可变来源的不变性；不是运行时借用计数器。
    pub(crate) lifetime: PhantomData<(&'w World, &'w UnsafeCell<World>)>,
}
impl<'w> UnsafeWorldCell<'w> {
    /// # 用途
    /// 返回 (布局替换代次, 当前已发布原型总量)，仅用于启动前同步，不授予数据权限。
    /// 原型追加只改总量；恢复/替换推进代次，即使数量相同仍重建。系统运行中不自动刷新匹配。
    pub fn archetype_revision(&self) -> (u64, usize) {
        self.archetype_revision_inner()
    }
    /// # 用途
    /// world 为完整独占来源；结果存活期间调用者不能再通过原借用访问 World。
    /// 构造不分配 tick、不进入 Running，也不自动授予参数访问；只能在校验后经 unsafe 入口取值。
    pub fn from_mut(world: &'w mut World) -> Self {
        Self::from_mut_inner(world)
    }
    /// # 用途
    /// world 为共享来源；不得通过结果写业务数据，即使伪造写声明也不能升级来源权限。
    /// 宿主历史值读取可用此来源；不进入运行状态或延长 World 寿命。
    pub fn from_ref(world: &'w World) -> Self {
        Self::from_ref_inner(world)
    }
    /// # 用途
    /// 读取非复用 World 身份，不读取业务组件、不授予权限。
    pub fn id(&self) -> u64 {
        self.id_inner()
    }
    /// # 用途
    /// 按短借用重新绑定同一来源；不升级可变能力或保留额外授权。
    pub fn reborrow(&self) -> UnsafeWorldCell<'_> {
        self.reborrow_inner()
    }
    /// # 用途
    /// 读取 T 的 Removed 旧值，reader 属于本 World 且已声明 T 的通知观察权限。
    /// 返回批次与旧值结果绑定本次短借用；类型/订阅错误不推进游标。
    /// # Safety
    /// 调用方保证通知及内置节点所需读取能力有效，结果期无冲突修改/回收；不得利用本入口访问活组件。
    pub unsafe fn read_removed<'a, T: crate::component::Component>(
        &'a self,
        reader: &'a mut crate::notification::NoticeSubscription,
    ) -> Result<crate::notification::RemovedBatch<'a, T>, EcsError> {
        self.read_removed_inner::<T>(reader)
    }
    /// # 用途
    /// 为已授权 Reader 提供通知只读来源，不访问组件或取得回收能力；不恢复整份 World 引用。
    /// # Safety
    /// 调用者证明整个结果期允许通知共享访问，禁止冲突追加/扩容/回收；订阅目录固定且属于同一 World。
    /// 共享来源只保活冻结目录；流读守卫由批次持有，不排斥其他流追加，禁止恢复整个管理器可变引用。
    pub unsafe fn notifications(&self) -> &'w crate::notification::Notifications {
        self.notifications_inner()
    }
}

/// # 用途
/// 外层串行运行入口的状态守卫，只操作 access；句柄和用户参数不能提前完成本守卫。
/// 由运行入口先建立守卫再建立派生参数，确保展开时参数先清理；不在 Drop 提交或回滚。
pub(crate) struct SystemSession<'a> {
    /// RC Cell 状态，独立于业务存储；运行入口认领 Running，守卫不持整份 World 引用。
    pub(crate) access: &'a crate::world::AccessState,
    /// 仅用户调用、参数释放与 finish 均成功后置位；未完成退出保守标停。
    pub(crate) completed: bool,
}
impl SystemSession<'_> {
    /// # 用途
    /// 全部参数引用及 finish 已结束后完成；成功不分配 tick，不维护实体。
    /// 只完成本系统，不退出根 Schedule 的 Running；已有 Stopped 返回 Err，不能恢复停止态。
    pub(crate) fn complete(&mut self) -> Result<(), EcsError> {
        self.complete_inner()
    }
}
impl Drop for SystemSession<'_> {
    /// # 用途
    /// 未完成退出时标停，已完成无动作；不借身份表、不运行用户析构、不再次 panic。
    fn drop(&mut self) {
        self.drop_inner()
    }
}

#[cfg(feature = "rc")]
impl World {
    /// # 用途
    /// 唯一 Schedule 根边界；self 须登记冻结且 Idle，拒绝 Pinning/Running/Maintaining/Stopped/关闭态。
    /// 成功进入 Running 后调用 run，短借 World 仅用于阶段准备、系统会话及阶段末维护。
    /// run 正常返回含 Err，先确认全部借用结束，再退出自有 Running 到 Idle；Stopped 保持并返回错误。
    /// panic 根守卫标停后原样展开，不补 finish/flush，不回滚；返回不能携带本次 World 借用。
    /// 不初始化系统、不分配 tick；前置拒绝不调用 run。与 system_scope 不同，本入口拥有根运行态。
    pub fn schedule_scope<R>(
        &mut self,
        run: impl for<'run> FnOnce(&'run mut World) -> Result<R, EcsError>,
    ) -> Result<R, EcsError> {
        self.schedule_scope_inner(run)
    }
    /// # 用途
    /// 返回是否单向停止；仅读取门禁，不读取业务组件，不恢复 Idle，不授予任何运行权限。
    /// 供 App 在调度返回后判断是否进入 Failed；普通 EcsError 文本不承担严重性分类。
    pub fn is_stopped(&self) -> bool {
        self.is_stopped_inner()
    }
    /// # 用途
    /// RC 运行入口在系统准备/权限校验成功后、构造参数前分配一次非零 tick。
    /// 必须持根 Running 授权；耗尽返回 Err，不回绕、不修改组件，不推进系统 last_run。
    /// # Safety
    /// 调用者是唯一串行运行驱动，无活参数引用，不得由用户参数自行分配。
    pub unsafe fn next_system_tick(
        &mut self,
    ) -> Result<crate::change_detection::ChangeTick, EcsError> {
        self.next_system_tick_inner()
    }
    /// # 用途
    /// RC 串行系统运行边界；declaration/version 已按系统启动前的 World 校验，run 消费本次受限授权。
    /// 调用时根 Schedule 已认领 Running；本入口不初始化参数、不刷新计划，也不退出根运行态。
    /// 栈上先建立 SystemSession，再创建统一句柄与 RunAccess；run 必须涵盖 begin/fetch/用户调用/finish。
    /// 返回值不得携带本次引用；正常完成检查停止态，成功返回 R；前置拒绝不调用 run。
    /// 展开标停不回滚、不补 finish；此前成功操作保持发布。run 正常返回后才允许调度器推进 last_run。
    /// # Safety
    /// 调用者证明 declaration 完整覆盖参数访问、同系统别名已校验且 version 为当前布局版本。
    /// run 不泄漏派生引用，不恢复整份 World 引用，使用 ParamSet 时遵守短借用和缓存重绑规则。
    pub unsafe fn system_scope<R>(
        &mut self,
        declaration: &crate::access::AccessTree,
        version: u64,
        run: impl for<'run> FnOnce(crate::access::RunAccess<'run>) -> R,
    ) -> Result<R, EcsError> {
        self.system_scope_inner(declaration, version, run)
    }
}
/// # 用途
/// RC 根 Schedule 独占会话；持普通可变借用，通过短重借驱动阶段，不保存自引用或共享转可变指针。
#[cfg(feature = "rc")]
pub(crate) struct ScheduleSession<'a> {
    /// 唯一来源，进入时已认领 Running；派生阶段/参数借用结束后方可完整退出。
    pub(crate) world: &'a mut World,
    /// 正常完成含普通 Err 后置位，未完成 Drop 标停，绝不把 Stopped 改回 Idle。
    pub(crate) completed: bool,
}
#[cfg(feature = "rc")]
impl ScheduleSession<'_> {
    /// # 用途
    /// 返回根会话内的短独占来源，期间不得完成/释放会话；不新增运行权或恢复停止态。
    pub(crate) fn world(&mut self) -> &mut World {
        self.world_inner()
    }
    /// # 用途
    /// 无活阶段/任务/参数后显式完成；Running 回 Idle，Stopped 保留并报错，异常状态标停报错。
    /// 不分配 tick/执行业务/维护，不能用正常完成吞掉 panic；完成后不再调用 world。
    pub(crate) fn complete(&mut self) -> Result<(), EcsError> {
        self.complete_inner()
    }
}
#[cfg(feature = "rc")]
impl Drop for ScheduleSession<'_> {
    /// # 用途
    /// 未正常完成时只标停，不调用用户析构/业务操作或再次 panic；已完成无动作。
    fn drop(&mut self) {
        self.drop_inner()
    }
}
