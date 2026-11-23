//! 单 World 组装入口；双 App 协调及主循环归宿主。
#[path = "app_inner.rs"]
mod app_inner;
use crate::plugin::Plugin;
use pi_ecs_core::{error::EcsError, world::World};
#[cfg(feature = "rc")]
use pi_ecs_schedule::inter_frame::{
    FrameBudget, InterFrameTarget, QueuePriority, ReadyRun, SystemFlow,
};
use pi_ecs_schedule::schedule::Schedule;
#[cfg(not(feature = "rc"))]
use pi_share::Share;
use std::any::TypeId;
/// 类型化调度标签，仅进程内有效，不是序列化身份。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScheduleLabel {
    /// 标记类型身份，同类型得到同标签。
    pub(crate) type_id: TypeId,
}
impl ScheduleLabel {
    /// T 为调度标记类型；不创建或执行调度。
    pub fn of<T: 'static>() -> Self {
        Self::label_inner::<T>()
    }
}
/// App 构建与停止状态，不代替 World 的运行门禁。
pub(crate) enum AppState {
    /// 允许安装插件及配置。
    Building,
    /// 正在调用 finish，可补连接，不允许安装插件或重入执行。
    Finishing,
    /// 组装结构合法且 World 已冻结，允许运行；不保证参数初始化/准备一定成功。
    Ready,
    /// 正在注销及关闭；禁止运行、配置、重入关闭。
    Closing,
    /// 已完成关闭；close 幂等，其他操作拒绝。
    Closed,
    /// 部分构建或不可恢复执行失败，禁止继续运行。
    Failed,
}
/// # 用途
/// RC App 运行展开守卫，只借生命周期字段；先于 World 根会话建立，最后释放。
#[cfg(feature = "rc")]
pub(crate) struct AppRunGuard<'a> {
    /// 来源 App 的 Ready 状态，独占字段借用，不借 World/调度或整个 App。
    pub(crate) state: &'a mut AppState,
    /// 正常返回含 Err 后显式置位；未完成退出一律 Failed，不吞 panic。
    pub(crate) completed: bool,
}
#[cfg(feature = "rc")]
impl AppRunGuard<'_> {
    /// # 用途
    /// stopped 为根会话退出后的 World::is_stopped；健康普通 Err 保持 Ready，停止则 Failed。
    /// 根运行借用已结束；不把原 Failed 改回 Ready，不执行业务或维护。
    pub(crate) fn complete(&mut self, stopped: bool) {
        self.complete_inner(stopped)
    }
}
#[cfg(feature = "rc")]
impl Drop for AppRunGuard<'_> {
    /// # 用途
    /// 未完成展开记 Failed，已完成无动作；不调用用户代码或再次 panic。
    fn drop(&mut self) {
        self.drop_inner()
    }
}
/// # 用途
/// 插件类型安装进度；与对象临时移出无关，依赖检查只认可 Installed。
pub(crate) enum PluginInstallState {
    /// build 尚未成功，阻止该类型递归安装；不能作为依赖满足条件。
    Installing,
    /// build 成功且对象已按完成顺序登记；finish 暂时移出对象仍保持本状态。
    Installed,
}
/// # 用途
/// 已成功安装插件的拥有槽，按完成顺序排列；嵌套安装子插件先于父插件。
pub(crate) struct InstalledPlugin {
    /// 类型身份，与类型状态表对应；不依赖 trait 对象地址或 downcast。
    pub(crate) type_id: TypeId,
    /// 通常 Some；finish 回调期间仅当前槽为 None，对象由调用栈拥有且回调后放回。
    /// 空槽不是卸载状态；任何异常使 App Failed，不能继续 finish 或运行。
    pub(crate) plugin: Option<Box<dyn Plugin>>,
}
/// 一个 App 管一个 World；游戏/渲染各一 App，服务器无需渲染 App。
pub struct App {
    /// RC 直接拥有唯一 World；运行时与 schedules 拆分独占借用，不复制共享所有权。
    /// 长期参数只保留逻辑身份/计划，不保留 World 自引用地址；运行句柄限于本次借用。
    #[cfg(feature = "rc")]
    pub(crate) world: World,
    /// 非 RC 稳定分配并保活；不公开可替换 Share 或不受控解包入口。
    #[cfg(not(feature = "rc"))]
    pub(crate) world: Share<World>,
    /// 命名调度；Schedule 当前直接拥有 Stage/Executor，App 不另存重复阶段表。
    pub(crate) schedules: Vec<(ScheduleLabel, Schedule)>,
    /// App 所有的帧间图；复用唯一 World，关闭时先清偿图再关闭 World。
    #[cfg(feature = "rc")]
    pub(crate) inter_frame: SystemFlow,
    /// 按成功安装顺序保留插件配置；业务运行数据归资源、组件或 Local。
    pub(crate) plugins: Vec<InstalledPlugin>,
    /// 类型安装状态；Installing 阻止自递归/循环安装，Installed 在对象移出期间仍可供依赖检查。
    /// 失败转 App Failed，不用删除此表伪装可重试，不自动排序依赖。
    pub(crate) plugin_types: std::collections::HashMap<TypeId, PluginInstallState>,
    /// 当前生命周期状态。
    pub(crate) state: AppState,
    // 后备 App 快照恢复与比较细节暂缓，不据此迁移当前 Schedule/Stage 所有权。
    // 单 World 门禁控制 Running/Maintaining 排他；内部状态可变性与具体授权接线待闭合。
}
/// 仅登记类型、表、队列和初始资源；不允许冻结、运行、注销或替换 World。
pub struct WorldConfig<'a> {
    /// 原 World 仍由 App 持有。
    pub(crate) world: &'a mut World,
}
impl WorldConfig<'_> {
    /// # 用途
    /// 按 storage 策略登记组件 T。
    pub fn register_component<T: pi_ecs_core::component::Component>(
        &mut self,
        storage: pi_ecs_core::component::StoragePolicy,
    ) -> Result<pi_ecs_core::component::ComponentId, EcsError> {
        self.world.register_component::<T>(storage)
    }
    /// # 用途
    /// 为指定组件登记表，不冻结布局。
    pub fn register_table(
        &mut self,
        components: &[pi_ecs_core::component::ComponentId],
    ) -> Result<pi_ecs_core::entity::ArchetypeId, EcsError> {
        self.world.register_table(components)
    }
    /// # 用途
    /// 登记唯一资源类型。
    pub fn register_resource<T: Send + Sync + 'static>(
        &mut self,
    ) -> Result<pi_ecs_core::resource::ResourceId, EcsError> {
        self.world.register_resource::<T>()
    }
    /// # 用途
    /// 登记空多例资源集合，不领取生产者资格。
    #[cfg(feature = "rc")]
    pub fn register_multi_resource<T: Send + Sync + 'static>(
        &mut self,
    ) -> Result<pi_ecs_core::resource::ResourceId, EcsError> {
        self.world.register_multi_resource::<T>()
    }
    /// # 用途
    /// T 须已登记；插入或替换 value，成功返回旧值（首次返回 None）。
    /// 普通拒绝保留原资源和时钟；所传 value 依原资源接口消费，不返回给调用者。
    pub fn insert_resource<T: Send + Sync + 'static>(
        &mut self,
        value: T,
    ) -> Result<Option<T>, EcsError> {
        self.world.insert_resource(value)
    }
    /// # 用途
    /// 登记命令队列与容量，不领取消费资格。
    pub fn register_command<C: Send + 'static>(
        &mut self,
        capacity: pi_ecs_core::command::CommandCapacity,
    ) -> Result<(), EcsError> {
        self.world.register_command::<C>(capacity)
    }
}
impl App {
    /// # 用途
    /// 仅 Building/Finishing 配置帧间入口、消费者及通知；返回借用不开放 World。
    #[cfg(feature = "rc")]
    pub fn inter_frame_mut(
        &mut self,
    ) -> Result<pi_ecs_schedule::inter_frame::FlowConfig<'_>, EcsError> {
        self.inter_frame_mut_inner()
    }

    /// # 用途
    /// Ready/Idle 时立即从 target 启动一轮，App 内部借用所属 World；异源目标拒绝。
    /// 本次 input 可是一整批拥有值；同线程不允许重入，忙碌不转为隐式入队。
    #[cfg(feature = "rc")]
    pub fn dispatch<I: 'static>(
        &mut self,
        target: &InterFrameTarget<I>,
        input: I,
    ) -> Result<(), EcsError> {
        self.dispatch_inner(target, input)
    }

    /// # 用途
    /// 将 command 投入 World 已有类型化队列，不执行系统。拒绝时交还原值。
    /// 帧推期间产生的拥有型 RPC 完成消息经宿主交付；不要求在帧推调用栈里重借 App。
    #[cfg(feature = "rc")]
    pub fn post<C: Send + 'static>(
        &mut self,
        command: C,
    ) -> Result<(), pi_ecs_core::command::CommandSendError<C>> {
        self.post_inner(command)
    }

    /// # 用途
    /// 组装期或 Ready/Idle 为已绑定的 C 修改五级优先级，未设置默认中。
    #[cfg(feature = "rc")]
    pub fn set_queue_priority<C: Send + 'static>(
        &mut self,
        priority: QueuePriority,
    ) -> Result<(), EcsError> {
        self.set_queue_priority_inner::<C>(priority)
    }

    /// # 用途
    /// Ready/Idle 的帧间入口；budget 已到或进入让行窗则不消费消息，否则最多选一个入口跑完。
    /// 宿主负责采样时钟、驱动帧推及再次调用；本方法不抢占已启动轮次。
    #[cfg(feature = "rc")]
    pub fn run_next_ready(&mut self, budget: FrameBudget) -> Result<ReadyRun, EcsError> {
        self.run_next_ready_inner(budget)
    }

    /// # 用途
    /// 在 App 自有 World 上显式运行 label 的阶段序列；允许重复同一阶段，按 run 的状态协议收束。
    #[cfg(feature = "rc")]
    pub fn run_stages(
        &mut self,
        label: ScheduleLabel,
        stages: &[pi_ecs_schedule::stage::StageId],
    ) -> Result<(), EcsError> {
        self.run_stages_inner(label, stages)
    }
    /// # 用途
    /// RC 宿主仅 Ready/World Idle 下补齐或替换已登记 T，返回旧值所有权；不登记新类型。
    /// label/调度无关；错误不自动运行系统，状态/类型前置拒绝不改资源，value 按普通拥有值释放。
    /// tick/恢复能力及替换语义沿 World::insert_resource，不改变稳定编号，不使排序失效。
    /// 用于修正缺必需资源等准备错误；不开放整份可变 World，停止态拒绝。
    #[cfg(feature = "rc")]
    pub fn insert_resource<T: Send + Sync + 'static>(
        &mut self,
        value: T,
    ) -> Result<Option<T>, EcsError> {
        self.insert_resource_inner(value)
    }
    /// # 用途
    /// RC 宿主在 Ready 且 World Idle 时向 label/stage 登记 system；不重新开放类型/Table/插件配置。
    /// 标签/阶段/状态前置拒绝不改登记，身份冲突由 Stage::add_system 检查；消费 system，不初始化。
    /// 只接收未初始化实例；拒绝时返回原 system 及错误。
    /// 成功返回稳定槽位并使阶段排序失效，下一次准备初始化；新参数只能使用已有冻结登记。
    /// 初始化失败遵守 System::initialize 失败协议，不撤销已成功登记，不自动重建实例。
    #[cfg(feature = "rc")]
    pub fn add_system<S: pi_ecs_schedule::stage::StageSystemBound + 'static>(
        &mut self,
        label: ScheduleLabel,
        stage: pi_ecs_schedule::stage::StageId,
        system: S,
    ) -> Result<usize, pi_ecs_schedule::stage::InstallError<S>> {
        self.add_system_inner(label, stage, system)
    }
    /// # 用途
    /// RC 宿主在 Ready 且 World Idle 时注销 label/stage/system，内部拆分借用 World 与 Schedule。
    /// 标签/阶段/未知系统/状态前置拒绝不改排序；已注销幂等，主体遵守 Stage::unregister_system。
    /// 正常清偿诊断返回 Err 但系统已注销，App 可继续；内部故障/panic 停止，不以 Err 隐藏未清责任。
    /// 不返回 &mut World，不运行维护；Failed 下清理使用 close，不从本入口恢复运行。
    #[cfg(feature = "rc")]
    pub fn unregister_system(
        &mut self,
        label: ScheduleLabel,
        stage: pi_ecs_schedule::stage::StageId,
        system: pi_ecs_schedule::system::SystemId,
    ) -> Result<(), EcsError> {
        self.unregister_system_inner(label, stage, system)
    }
    /// # 用途
    /// RC 在 World Idle/Stopped 且无活参数时关闭；Closed 幂等，Running/维护/Closing 前置拒绝。
    /// 先进入 Closing，按调度/阶段/系统登记顺序注销所有参数，释放 Local/生产者/订阅。
    /// 同源帧间图与阶段入口图也须在 World 仍存活时清偿；功能接通后沿用首错继续协议。
    /// 正常注销诊断继续收齐其他系统，清空调度及插件配置后调用 World::close；保留首个诊断返回。
    /// 注销过程中 World 元数据和存储必须存活；不运行系统、命令、flush 或 settle。
    /// 完成后进入 Closed；内部故障或析构 panic 标记 Failed/Stopped，不重试已消费状态。
    #[cfg(feature = "rc")]
    pub fn close(&mut self) -> Result<(), EcsError> {
        self.close_inner()
    }
    /// # 用途
    /// 接管 world，进入 Building；RC 直接保存其所有权，非 RC 沿用 Share 稳定分配。
    /// 不安装插件或运行系统；构建后不提供提取/替换 World 的入口。
    /// 仅接管开放、未冻结且无外部订阅、查询登记或活命令接收者的 World；拒绝归还原 World 及诊断。
    /// 已取得的命令发送端不阻止接管。
    pub fn new(world: World) -> Result<Self, pi_ecs_schedule::stage::InstallError<World>> {
        Self::new_inner(world)
    }
    /// plugin 为配置对象，具体类型唯一；仅 Building，立即 build，可安装子插件。
    /// 重复/状态前置拒绝不改登记；build 错误可部分登记，转 Failed，不撤销。
    /// panic 不承诺转 Result，宿主不得捕获后继续使用 App；类型去重也检测安装重入。
    /// 当前 plugin 由局部调用栈拥有，设置 Installing 后调用 build；不借 App 插件表跨回调。
    /// build 成功且 App 仍为 Building 才追加 Installed 槽并标记成功；子插件先成功先登记。
    /// 回调捕获嵌套安装错误也不能解除 Failed；前置重复/状态拒绝不改变既有登记。
    pub fn add_plugin<P: Plugin + 'static>(&mut self, plugin: P) -> Result<(), EcsError> {
        Self::add_plugin_inner(self, plugin)
    }
    /// P 须已成功安装，正在 build 不算完成；缺失返回错误，不修改状态或自动安装/排序。
    /// 查询类型状态表，不依赖对象是否暂时在槽中；finish 中可确认自己及其他已安装插件。
    pub fn require_plugin<P: Plugin + 'static>(&self) -> Result<(), EcsError> {
        Self::require_plugin_inner::<P>(self)
    }
    /// 仅构建/finish 登记 label 对应 schedule；不自动接入运行流程。
    /// 只接管无活登记责任的调度；拒绝时返回原 schedule 供调用者在原 World 上清偿。
    /// 标签重复/状态错误返回错误，不替换现行调度。
    pub fn add_schedule(
        &mut self,
        label: ScheduleLabel,
        schedule: Schedule,
    ) -> Result<(), pi_ecs_schedule::stage::InstallError<Schedule>> {
        Self::add_schedule_inner(self, label, schedule)
    }
    /// label 必须存在；仅构建/finish 返回配置借用，缺失或其他状态报错且不改状态。
    pub fn schedule_mut(
        &mut self,
        label: ScheduleLabel,
    ) -> Result<pi_ecs_schedule::schedule::ScheduleConfig<'_>, EcsError> {
        Self::schedule_mut_inner(self, label)
    }
    /// 返回共享 world，不额外授予写权限，不绕过正常参数及运行门禁。
    pub fn world(&self) -> &World {
        Self::world_inner(self)
    }
    /// # 用途
    /// 仅构建/finish 提供受限配置操作；World 只由 App::finish 冻结。
    pub fn world_mut(&mut self) -> Result<WorldConfig<'_>, EcsError> {
        Self::world_mut_inner(self)
    }
    /// 仅 Ready 且 World 静止，内部取得维护排他权，不与运行/维护重叠，不隐式等待。
    /// 覆盖 App 所有 Stage 状态，每 Stage 保存一次，不因图多次引用重复捕获。
    /// 失败不交付半快照、保持当前状态；实际协调委托 GameRecovery，不另写一套复制逻辑。
    pub fn capture_game(&mut self) -> Result<pi_ecs_schedule::snapshot::GameSnapshot, EcsError> {
        Self::capture_game_inner(self)
    }
    /// snapshot 为兼容历史，可用于兼容的空目标 App，不消耗源。
    /// 内部取得维护排他权；校验/准备失败保留当前状态，开始清旧后失败 App/World 停止。
    /// 覆盖全部 Stage，委托 GameRecovery；不暴露整个可变 World，不自动重演/渲染。
    pub fn restore_game(
        &mut self,
        snapshot: &pi_ecs_schedule::snapshot::GameSnapshot,
    ) -> Result<(), EcsError> {
        Self::restore_game_inner(self, snapshot)
    }
    /// # 用途
    /// Ready 时幂等；Building 时收束插件并验证组装、冻结 World，不执行 Startup。
    /// 不调用系统 initialize/prepare 或消费初值；参数有效性留阶段准备，快照能力留 capture_game。
    /// 重入/Failed 拒绝；钩子或最终验证失败转 Failed，不承诺撤销。
    pub fn finish(&mut self) -> Result<(), EcsError> {
        /* 设计已确认：
        Building 转 Finishing，禁止 add_plugin/run 和重入 finish。
        按成功完成 build 顺序调用插件 finish（嵌套子插件先于父插件）。
        每次从当前槽移出 Box 至局部，结束插件表借用后调用 finish(&mut App)，返回后放回原槽。
        类型表保留 Installed，依赖检查不因对象暂时移出而失败；不安装新插件或重入 finish/close/run。
        回调 Err 先放回对象再记 Failed，保留此前成功连接，不补剩余回调；不把 Failed 改回 Ready。
        调用 Schedule::validate_configuration 检查组装引用/身份/执行模式，再 World::finish_registration。
        任何失败转 Failed，不撤销插件已做修改；不初始化/准备系统、不消费初值或登记订阅/生产者。
        全部成功 Ready，不分配系统 tick；参数有效性留阶段准备，恢复策略能力留 capture_game。
        */
        Self::finish_inner(self)
    }
    /// 仅 Ready，显式运行 label；缺失/状态或门禁前置拒绝不执行，不隐式 finish。
    /// 不可恢复执行失败转 Failed，收束访问后停止；阶段语义见 Schedule::run。
    /// 正常 Err 且 World 未停止时保持 Ready；此前系统写入/last_run/游标保留，不自动补 flush 或重试。
    /// 再次显式调用从该 Schedule 开头作为新一轮执行，不是失败位置续跑；可能再次运行先前成功系统。
    /// World::is_stopped 为真则 Failed；不按 EcsError 字符串判断。panic 由展开守卫记 Failed 并原样传播。
    /// 不自动运行其他调度、重演、网络或渲染，无内置 update 主循环。
    /// RC 拆分借用自身 world 与选中的 schedule，以 &mut World 调用 Schedule::run，借用限本次运行。
    /// 不克隆 World 所有权、不取出替换占位 World，不缓存跨运行 World 地址或业务缓冲地址。
    /// 查询状态的同源冻结容器根缓存沿 core 的独立契约，不由 App 缓存运行授权。
    pub fn run(&mut self, label: ScheduleLabel) -> Result<(), EcsError> {
        /* 设计已确认：
        前置检查标签/Ready/World 状态；拒绝不建立守卫，已停止则 App Failed。
        拆分 state、world、schedules 字段，先建立 AppRunGuard，再调用 Schedule::run 进入 World 根会话。
        正常返回含 Err 后读取 World 停止态，完成 AppRunGuard；健康保持 Ready，停止记 Failed。
        panic 时先清参数/World 根会话，再释放 AppRunGuard 记 Failed，原样传播，不补维护。
        */
        Self::run_inner(self, label)
    }
}
#[cfg(feature = "rc")]
impl Drop for App {
    /// # 用途
    /// 自动复用 close 的注销再关闭顺序；关闭失败无法完整清偿、残留 Closing 或析构 panic 时 abort。
    /// 不让字段默认析构先关闭 World 再释放系统；不展开重试，不补业务 finish。
    fn drop(&mut self) {
        self.drop_inner()
    }
}
