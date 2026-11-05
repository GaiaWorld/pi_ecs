//! 单 World 组装入口；双 App 协调及主循环归宿主。
use crate::plugin::Plugin;
use pi_ecs_core::{error::EcsError, world::World};
use pi_ecs_schedule::schedule::Schedule;
use std::{any::TypeId, sync::Arc};
/// 类型化调度标签，仅进程内有效，不是序列化身份。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScheduleLabel {
    /// 标记类型身份，同类型得到同标签。
    pub(crate) type_id: TypeId,
}
impl ScheduleLabel {
    /// T 为调度标记类型；不创建或执行调度。
    pub fn of<T: 'static>() -> Self {
        crate::app_inner::label_inner::<T>()
    }
}
/// App 构建与停止状态，不代替 World 的运行门禁。
pub(crate) enum AppState {
    /// 允许安装插件及配置。
    Building,
    /// 正在调用 finish，可补连接，不允许安装插件或重入执行。
    Finishing,
    /// 配置冻结，允许显式运行。
    Ready,
    /// 部分构建或不可恢复执行失败，禁止继续运行。
    Failed,
}
/// 一个 App 管一个 World；游戏/渲染各一 App，服务器无需渲染 App。
pub struct App {
    /// 稳定分配并保活；不公开可替换 Arc 或不受控解包入口。
    pub(crate) world: Arc<World>,
    /// 命名调度；当前仍含阶段/执行器的旧壳，目标改为图引用 App 动作表。
    pub(crate) schedules: Vec<(ScheduleLabel, Schedule)>,
    /// 按成功安装顺序保留插件配置；业务运行数据归资源、组件或 Local。
    pub(crate) plugins: Vec<Box<dyn Plugin>>,
    /// @to_design 类型去重、正在安装集合及回调安全借用的私有表示待细化。
    pub(crate) _pending_plugin_types: (),
    /// 当前生命周期状态。
    pub(crate) state: AppState,
    // @to_design App 统一 Stage 登记表/图引用与多流程快照身份匹配待对接执行图后声明。
    // 单 World 门禁控制 Running/Maintaining 排他；内部状态可变性与具体授权接线待闭合。
}
impl App {
    /// 接管 world 并稳定分配，进入 Building；不安装插件或运行系统。
    pub fn new(world: World) -> Self {
        crate::app_inner::new_inner(world)
    }
    /// plugin 为配置对象，具体类型唯一；仅 Building，立即 build，可安装子插件。
    /// 重复/状态前置拒绝不改登记；build 错误可部分登记，转 Failed，不撤销。
    /// panic 不承诺转 Result，宿主不得捕获后继续使用 App；类型去重也检测安装重入。
    pub fn add_plugin<P: Plugin + 'static>(&mut self, plugin: P) -> Result<(), EcsError> {
        crate::app_inner::add_plugin_inner(self, plugin)
    }
    /// P 须已成功安装，正在 build 不算完成；缺失返回错误，不修改状态或自动安装/排序。
    pub fn require_plugin<P: Plugin + 'static>(&self) -> Result<(), EcsError> {
        crate::app_inner::require_plugin_inner::<P>(self)
    }
    /// 仅构建/finish 登记 label 对应 schedule；不自动接入运行流程。
    /// 标签重复/状态错误返回错误，不替换现行调度。
    pub fn add_schedule(
        &mut self,
        label: ScheduleLabel,
        schedule: Schedule,
    ) -> Result<(), EcsError> {
        crate::app_inner::add_schedule_inner(self, label, schedule)
    }
    /// label 必须存在；仅构建/finish 返回配置借用，缺失或其他状态报错且不改状态。
    pub fn schedule_mut(&mut self, label: ScheduleLabel) -> Result<&mut Schedule, EcsError> {
        crate::app_inner::schedule_mut_inner(self, label)
    }
    /// 返回共享 world，不额外授予写权限，不绕过正常参数及运行门禁。
    pub fn world(&self) -> &World {
        crate::app_inner::world_inner(self)
    }
    /// 仅构建/finish 且具备 Arc 唯一性时返回独占配置访问，否则错误，不强转 Arc。
    /// 仅内部绑定尚未发布时允许；Arc 唯一不等于无内部绑定，禁止移动/替换已发布 World。
    /// 不作为 Ready 的恢复入口，不因独占 App 就假定独占 World。
    pub fn world_mut(&mut self) -> Result<&mut World, EcsError> {
        crate::app_inner::world_mut_inner(self)
    }
    /// 仅 Ready 且 World 静止，内部取得维护排他权，不与运行/维护重叠，不隐式等待。
    /// 覆盖 App 所有 Stage 状态，每 Stage 保存一次，不因图多次引用重复捕获。
    /// 失败不交付半快照、保持当前状态；实际协调委托 GameRecovery，不另写一套复制逻辑。
    pub fn capture_game(&mut self) -> Result<pi_ecs_schedule::snapshot::GameSnapshot, EcsError> {
        crate::app_inner::capture_game_inner(self)
    }
    /// snapshot 为兼容历史，可用于兼容的空目标 App，不消耗源。
    /// 内部取得维护排他权；校验/准备失败保留当前状态，开始清旧后失败 App/World 停止。
    /// 覆盖全部 Stage，委托 GameRecovery；不暴露整个可变 World，不自动重演/渲染。
    pub fn restore_game(
        &mut self,
        snapshot: &pi_ecs_schedule::snapshot::GameSnapshot,
    ) -> Result<(), EcsError> {
        crate::app_inner::restore_game_inner(self, snapshot)
    }
    /// Ready 时幂等；Building 时调用全部 finish 并统一验证、冻结，不执行 Startup。
    /// 重入/Failed 拒绝；钩子或最终验证失败转 Failed，不承诺撤销。
    pub fn finish(&mut self) -> Result<(), EcsError> {
        /* 设计已确认：
        Building 转 Finishing，禁止 add_plugin/run 和重入 finish。
        按成功安装顺序调用插件 finish，允许依赖检查及补全连接。
        统一验证类型/Table/快照能力和系统配置，失败停止使用。
        成功冻结为 Ready；不隐式执行任何调度。具体插件回调借用表示待细化。
        */
        crate::app_inner::finish_inner(self)
    }
    /// 仅 Ready，显式运行 label；缺失/状态或门禁前置拒绝不执行，不隐式 finish。
    /// 不可恢复执行失败转 Failed，收束访问后停止；阶段语义见 Schedule::run。
    /// 不自动运行其他调度、重演、网络或渲染，无内置 update 主循环。
    pub fn run(&mut self, label: ScheduleLabel) -> Result<(), EcsError> {
        crate::app_inner::run_inner(self, label)
    }
}
