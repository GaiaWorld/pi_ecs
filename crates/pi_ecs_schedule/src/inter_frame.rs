//! RC 数据流实例：App 帧间入口和 Stage 入口各持独立作用域，共用普通 System 和组图规则。
#[path = "inter_frame_inner.rs"]
mod inter_frame_inner;
use crate::system::{System, SystemId};
use pi_ecs_core::{
    component::ComponentId,
    error::EcsError,
    notification::{NoticeKind, NoticeSubscription},
    world::World,
};
use std::{
    any::{Any, TypeId},
    marker::PhantomData,
};

/// 定位一个登记实例；克隆不复制系统或延长其生命，不要求 I: Clone。
pub struct InterFrameTarget<I> {
    /// 所属图及永不复用的槽号。跨图、关闭、迟到投递都须拒绝。
    key: TargetKey,
    /// 宿主可拥有型派发的输入负载类型。
    input: PhantomData<fn(I) -> I>,
}
impl<I> Clone for InterFrameTarget<I> {
    /// # 用途
    /// 复制投递能力，同一实例的 Local、Reader 和 last_run 不变。
    fn clone(&self) -> Self {
        Self::clone_inner(self)
    }
}

/// 稳定实例键，槽从不指向替代实例。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct TargetKey {
    /// 图的单调身份。
    flow: u64,
    /// 图内固定登记槽号。
    slot: usize,
}

/// 从 World 已发布的实际通知唤醒一个单位输入实例。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoticeRoute {
    /// 同源已登记组件。
    pub component: ComponentId,
    /// 触发的通知种类。
    pub kind: NoticeKind,
}

/// 五级就绪队列优先级；仅参与帧间入口选择。
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum QueuePriority {
    /// 最低优先级。
    Lowest,
    /// 低优先级。
    Low,
    /// 默认中优先级。
    #[default]
    Medium,
    /// 高优先级。
    High,
    /// 最高优先级。
    Highest,
}

/// 启动一轮之前使用的单调时钟时间窗，由宿主提供。
pub struct FrameBudget {
    /// 选择帧间任务之前的当前时间采样。
    pub now: std::time::Instant,
    /// 下一次宿主帧推的期望时间。
    pub frame_at: std::time::Instant,
    /// 距期限不超过该余量就不再开轮；零表示到点才让行。
    pub reserve: std::time::Duration,
}

/// 一次无错误的帧间选择结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadyRun {
    /// 已执行并收束一轮。
    Ran,
    /// 未找到有就绪消息的消费者。
    Idle,
    /// 到达帧推期限或让行时间窗，没有消费消息。
    YieldToFrame,
}

/// 独立数据流作用域；App 的帧间图与每个阶段入口图互不匹配。
pub struct SystemFlow {
    /// 首次登记时分配图身份；空图不占用编号。
    identity: Option<u64>,
    /// 在首笔参数或路由登记前绑定 World，清偿也必须同源。
    world_identity: Option<u64>,
    /// 登记顺序即稳定槽；仅整体关闭移除。
    systems: Vec<RegisteredSystem>,
    /// 只有帧间图允许登记队列候选，队列仍由 World 的 CommandHub 持有。
    queues: Vec<QueueRoute>,
    /// 只观察调度自己的订阅，不借业务 Reader 的游标。
    notices: Vec<RegisteredNotice>,
    /// 预编译计划、活登记与关闭门禁。
    state: FlowState,
}

/// 图生命期和计划发布位置。
enum FlowState {
    /// 尚可登记，不持 World 责任。
    Building,
    /// 参数、通知及计划可用。
    Ready(
        /// 两段顺序及布局快照。
        FlowPlans,
    ),
    /// 绑定后准备不完整，或执行遇协议错误/停机，只允许清偿。
    Failed,
    /// 未登记图的配置清偿已开始；只可在同源 World 上继续关闭，不允许组装或运行。
    ClosingUnregistered,
    /// 已清偿，不接受旧句柄。
    Closed,
}

/// 一个普通 System 实例及登记期冻结的端口。
struct RegisteredSystem {
    /// 供错误诊断的稳定实例身份，由图编配。
    id: SystemId,
    /// 普通单位入口在阶段入口运行；借用型输入只有图内入口。
    system: Box<dyn System>,
}

/// 帧间就绪查表：一个命令类型至多绑定一个单位输入实例。
struct QueueRoute {
    /// CommandHub 的队列负载类型。
    command: TypeId,
    /// 必须由该实例在独立受限参数登记中领取；不可把其他消费者误认成本系统。
    consumer: TargetKey,
    /// 初始化记录的领取编号；首次和每次选择时与队列当前编号核对。
    receiver_claim: Option<u64>,
    /// 默认中，Ready/Idle 时可更新，不改图顺序。
    priority: QueuePriority,
    /// 通过 World 只读查询就绪与唯一资格，不领取或消费 payload。
    status: fn(&World) -> Result<pi_ecs_core::command::CommandQueueStatus, EcsError>,
}

/// 一个实际通知流的独立路由。
struct RegisteredNotice {
    /// 观察的流。
    route: NoticeRoute,
    /// 被激活的单位入口。
    target: TargetKey,
    /// 仅记录已实际登记的唯一清偿责任。
    subscription: Option<NoticeSubscription>,
}

/// 图上前后段共用的调度快照。
struct FlowPlans {
    /// 按每个显式入口分别生成的触发闭包；不合并各外部消息来源。
    by_entry: Vec<Box<[TargetKey]>>,
    /// 阶段入口所有单位根的联合计划，不把具有输入的回调当普通根运行。
    stage_entry: Box<[TargetKey]>,
    /// 帧间屏障所有可能 Removed 根的联合计划。
    after_destruction: Box<[TargetKey]>,
    /// 同图节点的输出消费者和交付方式。
    output_edges: Vec<Option<OutputEdges>>,
    /// 本次分析的 World 布局代次和原型总数。
    layout_revision: (u64, usize),
}

/// 对同一个产出值的一种交付策略。
enum OutputEdges {
    /// 只读扇出；调度工作区保活输出至最后一个借用调用结束。
    Shared(
        /// 在所属执行段中依计划顺序调用的目标。
        Box<[TargetKey]>,
    ),
    /// 唯一拥有型接收者，输出只移动一次。
    Owned(
        /// 唯一消费目标。
        TargetKey,
    ),
}

/// 某一段的拥有型交付区；正常结束或展开时销毁未交付值，不跨轮保存。
struct SegmentInputs {
    /// 按实例槽排列的触发输入，保留实际发布顺序；只读扇出以同一 Rc 保活，不克隆业务值。
    pending: Vec<std::collections::VecDeque<PendingInput>>,
    /// 每目标至多一个未消费的单位通知唤醒；与数据输出分开记账。
    notice_ready: Vec<bool>,
}

/// 一个已激活的调用负载，不持有 World 借用。
enum PendingInput {
    /// 显式单位入口。
    Unit,
    /// 唯一拥有交付，出队即交给本次尝试。
    Owned(
        /// 保存在段工作区的拥有值。
        Box<dyn Any>,
    ),
    /// 共享上游拥有值，只在调用期间兑现只读引用。
    Shared(
        /// 段工作区共享保活；不跨轮、不借 World。
        std::rc::Rc<dyn Any>,
    ),
}

/// 只开放图登记操作，不公开底层图的替换、派发或关闭权。
pub struct FlowConfig<'a> {
    /// 由宿主保留所有权的原图。
    flow: &'a mut SystemFlow,
}
impl FlowConfig<'_> {
    /// # 用途
    /// 登记拥有输入或单位入口，返回原图的类型化句柄。
    pub fn register<I: 'static, F, M>(
        &mut self,
        builder: crate::system_builder::SystemBuilder<F, M>,
    ) -> Result<InterFrameTarget<I>, EcsError>
    where
        F: crate::system::SystemFunction<M> + 'static,
        F::Param: 'static,
        <F::Param as crate::system_param::SystemParam>::Config: 'static,
        M: 'static,
    {
        self.flow.register(builder)
    }
    /// # 用途
    /// 登记只能由图内触发的只读输入系统。
    pub fn register_shared<F, M>(
        &mut self,
        builder: crate::system_builder::SystemBuilder<F, M>,
    ) -> Result<(), EcsError>
    where
        F: crate::system::SystemFunction<M> + 'static,
        F::Param: 'static,
        <F::Param as crate::system_param::SystemParam>::Config: 'static,
        M: 'static,
    {
        self.flow.register_shared(builder)
    }
    /// # 用途
    /// 将 C 绑定到实际 Unit 入口。
    pub fn bind_queue<C: Send + 'static>(
        &mut self,
        target: &InterFrameTarget<()>,
    ) -> Result<(), EcsError> {
        self.flow.bind_queue::<C>(target)
    }
    /// # 用途
    /// 登记实际通知到 Unit 入口的路由。
    pub fn wake_on(
        &mut self,
        route: NoticeRoute,
        target: &InterFrameTarget<()>,
    ) -> Result<(), EcsError> {
        self.flow.wake_on(route, target)
    }
}
impl SystemFlow {
    /// # 用途
    /// 借出只可组装的视图；各操作仍须检查 Building。
    pub fn configuration(&mut self) -> FlowConfig<'_> {
        self.configuration_inner()
    }
    /// # 用途
    /// 创建空作用域，不申请 World 或实例编号。
    pub fn new() -> Self {
        Self::new_inner()
    }

    /// # 用途
    /// 为具有 Unit（I 为 ()）或 Owned<I> 端口的普通系统分配实例身份并消费 builder。
    /// Building 才允许；只接收尚未构建的配置，不能传入已领取登记责任的 System。
    pub fn register<I: 'static, F, M>(
        &mut self,
        builder: crate::system_builder::SystemBuilder<F, M>,
    ) -> Result<InterFrameTarget<I>, EcsError>
    where
        F: crate::system::SystemFunction<M> + 'static,
        F::Param: 'static,
        <F::Param as crate::system_param::SystemParam>::Config: 'static,
        M: 'static,
    {
        Self::register_inner(self, builder)
    }

    /// # 用途
    /// 登记未初始化的只读消费者；实际端口须为 Shared，不能绕过图从宿主派发。
    pub fn register_shared<F, M>(
        &mut self,
        builder: crate::system_builder::SystemBuilder<F, M>,
    ) -> Result<(), EcsError>
    where
        F: crate::system::SystemFunction<M> + 'static,
        F::Param: 'static,
        <F::Param as crate::system_param::SystemParam>::Config: 'static,
        M: 'static,
    {
        Self::register_shared_inner(self, builder)
    }

    /// # 用途
    /// 在帧间 Building 阶段声明 C 的唯一就绪消费者；参数使用 CommandRead<C>。
    /// 校验冻结端口确为 Unit；即使句柄类型一致，Owned<()> 也不是 Unit。
    /// 同一目标只绑定一个帧间队列，避免一个优先级入口顺带消费另一绑定队列。
    pub fn bind_queue<C: Send + 'static>(
        &mut self,
        target: &InterFrameTarget<()>,
    ) -> Result<(), EcsError> {
        Self::bind_queue_inner::<C>(self, target)
    }

    /// # 用途
    /// 更新已绑定 C 的优先级；Building 或两轮之间的 Ready，未绑定不隐式登记。
    pub fn set_queue_priority<C: Send + 'static>(
        &mut self,
        priority: QueuePriority,
    ) -> Result<(), EcsError> {
        Self::set_queue_priority_inner::<C>(self, priority)
    }

    /// # 用途
    /// 注册通知路由须核对冻结的 Unit 端口，Owned<()> 不适用；重复同一对幂等。
    pub fn wake_on(
        &mut self,
        route: NoticeRoute,
        target: &InterFrameTarget<()>,
    ) -> Result<(), EcsError> {
        Self::wake_on_inner(self, route, target)
    }

    /// # 用途
    /// App 完成所有组装后检查静态组图；stage_entry 为真时拒绝帧间队列及通知路由。
    /// 由已冻结端口检查类型歧义、拥有/借用冲突和纯输出触发环，不读取 World 或初始化参数。
    /// 空图合法；运行期还需在同源登记后检查实际通知边及访问顺序。
    pub fn validate_configuration(&self, stage_entry: bool) -> Result<(), EcsError> {
        self.validate_configuration_inner(stage_entry)
    }

    /// # 用途
    /// 安装前要求 Building、未绑定 World、没有队列及通知路由；只读检查，不初始化或发布计划。
    pub(crate) fn validate_stage_install(&self) -> Result<(), EcsError> {
        self.validate_stage_install_inner()
    }

    /// # 用途
    /// 根 Running 中从冻结元数据登记参数与路由，应用跟踪，再生成计划；不准备运行值。
    /// 在首笔登记前确立同源与 Failed；记录已完成责任，候选验证通过才进入 Ready。
    /// 初始化每个系统时使用受限 Receiver 登记；队列入口只准领取绑定的 C 并须留下可验证资格，其他帧间实例不允许领取。
    /// 候选边由非单位端口按负载类型推导。多生产者争同一输入、多个拥有型消费者、借用与拥有混用均报错。
    /// 通知候选边由声明叶子潜在 Write 匹配路由，运行只看真实发布。全图触发自环/环报错。
    /// 全图触发边先判环，再按各显式/队列入口、全部屏障 Removed 根的联合闭包、或阶段单位根的联合闭包叠加严格访问边。
    /// 各闭包用全部 Spawn/Alter 的保守原型集合，不采用普通 Stage 回退；计划冲突只使图 Failed，不停止 World。
    /// 首次登记资格不匹配也是候选拒绝：图 Failed、World 不标停，不消费消息，保留成功登记供关闭清偿。
    /// 首次必须拿到 Some(claim) 且探针返回同一 Some(claim)；None 与 None 永远不算有效。
    /// 参数登记（含回滚）或追踪维护返回 Err 时，先检查 World::is_stopped。
    /// 若已停，图保持 Failed，保留已登记的清偿责任，原样返回致命诊断；不得进入后续准备或派发。
    /// dispatch、run_next_ready 和阶段入口均沿用此出口，不用通用停机错误替换原诊断。
    fn prepare_in_run(&mut self, world: &mut World) -> Result<(), EcsError> {
        Self::prepare_in_run_inner(self, world)
    }

    /// # 用途
    /// 同源且 Idle 时同步启动单入口；成功输出在参数会话结束后交付；None 不唤醒，批次整体交付一次。
    /// 普通 prepare 拒绝只影响当前调用，独立支链继续，收束后返回首错。tick/系统会话/路由协议失败立即终止。
    /// tick 耗尽或运行协议故障拒绝剩余调用、标停 World 并使图 Failed；App 观察 World 停止后转 Failed。
    /// 已验证的资格丢失、已发布计划的无效槽及 run_with_input 的协议拒绝均属于运行协议故障。
    /// 宿主在进入运行前传入无效目标或输入模式，直接拒绝，不属于运行故障。
    /// 根作用域保留本轮原始诊断；停机诊断优先于先前普通失败。
    /// 帧间仅一次销毁屏障；以屏障新 Removed 为第二段根，第二段内 Added/Changed/Removed 与输出仍沿闭包传播。
    /// 第二段新增归零留到下次收束。已绑定队列的目标不可立即派发或由通知启动。
    pub fn dispatch<I: 'static>(
        &mut self,
        world: &mut World,
        target: &InterFrameTarget<I>,
        input: I,
    ) -> Result<(), EcsError> {
        /* 设计已确认：
        flowchart TD
          A[校验 Idle、World 与目标] --> B[根 Running 内确保计划；确认轮首路由尾界并跳过历史]
          B --> C[沿当前段计划执行实际激活输入]
          C --> D{本次结果}
          D -->|普通参数准备拒绝| E[留首错；独立后继继续]
          D -->|完成参数及系统会话| F[交付可选输出；观测实际通知]
          D -->|停机或调度协议错误| G[保存原诊断；Failed；释放段输入]
          E --> H{段结束}
          F --> H
          H -->|未结束| C
          H -->|一段结束| I[单次销毁屏障；实际新 Removed 激活第二段]
          I --> C
          H -->|二段结束| J[根会话退出；报告首个普通错误或成功]
          B -->|配置或计划拒绝| P[图 Failed；根退出到 Idle 并返回诊断]
          B -->|登记或追踪维护返回 Err 且 World 已停止| S[图 Failed；保留清偿责任；原样返回致命诊断]
          B -->|Ready 后资格缺失、变更或探针错误；或运行期路由确认失败| G
          I -->|收束或确认失败| G
          G --> K[标停 World；通过根作用域返回原始致命诊断]
        */
        Self::dispatch_inner(self, world, target, input)
    }

    /// # 用途
    /// 先按 budget 检查让行；随后在同一个根会话中完成必要登记、资格核验和计划重验，再探测就绪。
    /// 核验失败不消费消息；成功后按优先级降序、同级按绑定顺序选一个。不嵌套调用公开 dispatch。
    /// 首次资格拒绝与 Ready 后资格失效使用 dispatch 的同一失败分流；后者必须标停并保留原诊断。
    /// 不预读 payload；被选系统以 Unit 启动一整轮并自己通过 Receiver 消费。错误按 dispatch 语义返回。
    pub fn run_next_ready(
        &mut self,
        world: &mut World,
        budget: FrameBudget,
    ) -> Result<ReadyRun, EcsError> {
        Self::run_next_ready_inner(self, world, budget)
    }

    /// # 用途
    /// 已在父 Schedule 根 Running 中；每次阶段入口选全部单位根并沿同阶段隐式输出连线派发。
    /// 未绑定的合法空入口图不登记也不执行，直接继续普通阶段。
    /// 首次在根内登记，后续布局变更时重编译；候选验证不绑定运行值。
    /// 只传播输出边；不嵌套根授权或执行帧间销毁屏障。普通错误保留图 Ready，但本次主体和后续阶段不启动。
    /// 同样区分候选拒绝和运行协议故障；后者在父根内标停并将原始诊断交给 Schedule 根出口。
    /// 定时轮询器可返回 None 或一整批拥有输出；框架不读取应用时钟/定时器，不重放已取走输入。
    pub(crate) fn run_stage_entry(&mut self, world: &mut World) -> Result<(), EcsError> {
        Self::run_stage_entry_inner(self, world)
    }

    /// # 用途
    /// 校验同源 Idle/Stopped 后，先撤销组装和运行资格再移除任何槽；未登记图首次关闭时绑定清偿所用 World。
    /// Building 未绑定时仅释放配置；正常注销诊断记录首错并继续全部清偿。
    /// 释放配置及注销均在 World::parameter_unregistration_scope 内；析构或注销 panic 由该作用域标停并原样传播。
    /// 先消费单项凭据再注销，展开不重试已消费项；未消费的项仍可清偿，已消费项遗留责任交 World 整体关闭。
    /// 不执行系统或维护；完整清偿后置 Closed，幂等。异源/忙碌前置拒绝不动凭据。
    pub fn close(&mut self, world: &mut World) -> Result<(), EcsError> {
        Self::close_inner(self, world)
    }

    /// # 用途
    /// 仅在全部清偿后为真；正常清偿诊断不妨碍移除已关闭的图。
    pub(crate) fn is_closed(&self) -> bool {
        self.is_closed_inner()
    }
}
