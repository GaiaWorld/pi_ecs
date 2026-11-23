//! 系统参数登记能力；与业务取值和宿主 World 维护分离。
#[path = "parameter_registration_inner.rs"]
mod parameter_registration_inner;
use crate::{component::ComponentId, resource::ResourceId, world::World};
#[cfg(feature = "rc")]
use crate::{
    error::EcsError,
    notification::{NoticeKind, NoticeSubscription},
    query::{QueryData, QueryFilter, QueryState},
};
/// # 用途
/// 栈上不可复制的参数维护能力；构造仅由 World 作用域完成，不暴露整份 World。
/// 不得取业务值、登记新类型、创建实体或启动系统；拥有型登记责任可移入长期 State。
pub struct ParameterRegistration<'a> {
    /// 唯一来源的短独占借用；字段不向 schedule 开放。
    pub(crate) world: &'a mut World,
    /// 阶段初始化或静止点清偿；清偿模式禁止新增任何责任。
    pub(crate) mode: RegistrationMode,
    /// 数据流系统初始化时安装的 Receiver 白名单及本次领取记录；普通 Stage 保持原协议。
    #[cfg(feature = "rc")]
    pub(crate) receiver_policy: Option<ReceiverRegistration>,
    /// 首个致命故障；退出登记作用域后标停，期间保留清偿授权。
    #[cfg(feature = "rc")]
    pub(crate) fatal: &'a std::cell::Cell<Option<RegistrationFault>>,
}

/// 可在不复制错误报告的情况下还原的登记故障。
#[cfg(feature = "rc")]
#[derive(Clone, Copy)]
pub(crate) enum RegistrationFault {
    /// 新生产者未发布，因为变化时钟耗尽。
    ClockExhausted,
    /// 生产者已撤销，结构版本不能继续增长。
    StructureVersionExhausted,
}
/// 一次数据流系统参数登记的消费约束。
#[cfg(feature = "rc")]
pub enum ReceiverRegistration {
    /// 本系统没有消费队列的资格。
    Denied,
    /// 仅准领取一个绑定类型。
    Bound {
        /// 登记时指定的命令负载类型。
        command: std::any::TypeId,
        /// 成功领取后记录不可复用的资格编号。
        claim: Option<u64>,
    },
}
/// # 用途
/// core 内部的允许操作集合，不改变根 World 运行态。
pub(crate) enum RegistrationMode {
    /// 冻结后根 Running 的阶段首遍；允许登记和失败部分清理。
    Initialize,
    /// Idle/Stopped 的系统注销或关闭收束；只允许清偿已有责任。
    Unregister,
}
/// # 用途
/// 多例生产者唯一清偿责任；不可复制，Drop 不访问 World，显式注销消费。
pub struct ProducerRegistration {
    /// 所属 World，不跨 World 清偿。
    pub(crate) world_identity: u64,
    /// 已冻结多例资源类型。
    pub(crate) resource: ResourceId,
    /// 不随紧凑搬移变化的稳定生产者编号。
    pub(crate) producer: u64,
}
impl ProducerRegistration {
    /// 稳定写槽声明所需的 (资源类型, 生产者编号)；不随物理搬移变化，不授予槽访问。
    #[cfg(feature = "rc")]
    pub fn slot_identity(&self) -> (ResourceId, u64) {
        (self.resource, self.producer)
    }
}
impl ParameterRegistration<'_> {
    /// # 用途
    /// 在专用于单系统的登记作用域中收紧 Receiver 权限；不得撤销、放宽或重设。
    #[cfg(feature = "rc")]
    pub fn restrict_receivers(
        &mut self,
        command: Option<std::any::TypeId>,
    ) -> Result<(), EcsError> {
        self.restrict_receivers_inner(command)
    }

    /// # 用途
    /// 返回本作用域为绑定类型实际领取的资格编号，不转移资格或访问消息。
    #[cfg(feature = "rc")]
    pub fn receiver_claim(&self) -> Option<u64> {
        self.receiver_claim_inner()
    }
    /// # 用途
    /// C 为宿主已登记命令类型；仅初始化模式创建捕获当前 epoch 的发送端。
    /// 未登记、关闭或清偿模式返回错误，不隐式登记队列/容量，不取得消费资格。
    /// 返回拥有型句柄，不借用登记能力，不保活 World；正常 Drop 无队列清偿责任。
    #[cfg(feature = "rc")]
    pub fn command_sender<C: Send + 'static>(
        &mut self,
    ) -> Result<crate::command::CommandSender<C>, EcsError> {
        self.command_sender_inner::<C>()
    }
    /// # 用途
    /// C 为宿主已登记命令类型；仅初始化模式从 Hub 领取唯一 Receiver，不调用 Idle 宿主入口。
    /// 未登记、已有消费者、关闭或清偿模式拒绝且保持原队列/资格；成功将资格移入长期 State。
    /// 失败组合及正常注销只需 Drop 此 Receiver 归还资格，不清空 payload，不另建登记令牌。
    #[cfg(feature = "rc")]
    pub fn command_receiver<C: Send + 'static>(
        &mut self,
    ) -> Result<crate::command::CommandReceiver<C>, EcsError> {
        self.command_receiver_inner::<C>()
    }
    /// # 用途
    /// 返回来源 World 身份，不产生业务引用。
    pub fn world_id(&self) -> u64 {
        self.world_id_inner()
    }
    /// # 用途
    /// T 为待解析组件，只查冻结登记；未登记 None，不隐式添加。
    pub fn component_id<T: crate::component::Component>(&self) -> Option<ComponentId> {
        self.component_id_inner::<T>()
    }
    /// # 用途
    /// T 为唯一资源类型，只查编号，缺类型 None，不检查/创建当前实例。
    pub fn resource_id<T: Send + Sync + 'static>(&self) -> Option<ResourceId> {
        self.resource_id_inner::<T>()
    }
    /// # 用途
    /// T 为多例类型，只查冻结编号；缺类型 None，不创建集合或生产槽。
    pub fn multi_resource_id<T: Send + Sync + 'static>(&self) -> Option<ResourceId> {
        self.multi_resource_id_inner::<T>()
    }
    /// # 用途
    /// Q/F 为参数声明；初始化模式解析并绑定同源 QueryState，登记逐组件 tick 并集。
    /// 失败清理本次部分登记；不应用 tick、不匹配物理地址、不创建原型；清偿模式拒绝。
    #[cfg(feature = "rc")]
    pub fn query_state<Q: QueryData, F: QueryFilter>(
        &mut self,
    ) -> Result<QueryState<Q, F>, EcsError> {
        self.query_state_inner::<Q, F>()
    }
    /// # 用途
    /// state 必须同源、无活结果；复用 QueryState::unregister_tracking 的终态与幂等规则。
    /// 允许初始化失败清理及静止点清偿；异源拒绝并保留原责任，不应用追踪开关。
    #[cfg(feature = "rc")]
    pub fn unregister_query<Q: QueryData, F: QueryFilter>(
        &mut self,
        state: &mut QueryState<Q, F>,
    ) -> Result<(), EcsError> {
        self.unregister_query_inner(state)
    }
    /// # 用途
    /// 仅初始化模式读取 B 的完整静态原型和 partition；分区必须属于冻结 World。
    /// 核对有序组件清单、重复类型及基础 Table；错误不发布状态，成功也不创建原型或登记追踪。
    #[cfg(feature = "rc")]
    pub fn spawn_state<B: crate::bundle::SystemBundle<Shape = B> + 'static>(
        &mut self,
        partition: crate::entity::PartitionId,
    ) -> Result<crate::spawn::SpawnState<B>, EcsError> {
        self.spawn_state_inner::<B>(partition)
    }
    /// # 用途
    /// 仅初始化模式：按 Add/Remove 的静态 Bundle 清单解析冻结编号；() 表示空。
    /// 拒绝未登记、重复或相交的类型；成功才取得 Q/F 追踪责任，不创建目标或读取值。
    /// 返回状态复用编号式 AlterState；Remove 不构造值，错误不遗留本次登记。
    #[cfg(feature = "rc")]
    pub fn typed_query_alter_state<Q, F, Add, Remove>(
        &mut self,
    ) -> Result<crate::alter::QueryAlterState<Q, F>, EcsError>
    where
        Q: QueryData,
        F: QueryFilter,
        Add: crate::bundle::SystemBundle<Shape = Add> + 'static,
        Remove: crate::bundle::Bundle,
    {
        self.typed_query_alter_state_inner::<Q, F, Add, Remove>()
    }
    /// # 用途
    /// 清偿此 Alter 内部查询的登记责任；异源拒绝并保留状态及原责任。
    #[cfg(feature = "rc")]
    pub fn unregister_query_alter<Q: QueryData, F: QueryFilter>(
        &mut self,
        state: &mut crate::alter::QueryAlterState<Q, F>,
    ) -> Result<(), EcsError> {
        self.unregister_query_alter_inner(state)
    }
    /// # 用途
    /// component/kind 为已登记组件和固定通知种类；初始化模式创建独立真实订阅。
    /// 清偿模式/未知组件拒绝；失败无新责任，不初始化活值 tick，不回收旧值。
    #[cfg(feature = "rc")]
    pub fn subscribe_notice(
        &mut self,
        component: ComponentId,
        kind: NoticeKind,
    ) -> Result<NoticeSubscription, EcsError> {
        self.subscribe_notice_inner(component, kind)
    }
    /// # 用途
    /// subscription 为同源唯一责任；消费并清偿，不推进他人游标、不回收旧值。
    /// 允许失败清理及正常注销；正常诊断遵守完整清偿，内部故障标停。
    #[cfg(feature = "rc")]
    pub fn unsubscribe_notice(&mut self, subscription: NoticeSubscription) -> Result<(), EcsError> {
        self.unsubscribe_notice_inner(subscription)
    }
    /// # 用途
    /// T 已冻结登记，initial 为该参数首次初值；初始化模式移入新独立生产槽并返回唯一责任。
    /// 失败不发布半槽并释放输入；不借旧实例，不分配系统执行 tick；实例初始化 tick 沿资源规则。
    /// 槽/结构版本更新完成后才交付责任；不得由类型编号注销其他参数的槽。
    /// 发布同时更新稳定编号到偏移映射；不跨包改写其他 Stage 的 ProducerState 缓存。
    #[cfg(feature = "rc")]
    pub fn register_producer<T: Send + Sync + 'static>(
        &mut self,
        initial: T,
    ) -> Result<ProducerRegistration, EcsError> {
        self.register_producer_inner(initial)
    }
    /// # 用途
    /// registration 为同源 T 的唯一责任；消费并析构槽值，更新紧凑映射及结构版本。
    /// 移动其他槽时同步其 core 偏移映射；其他系统在下次 prepare 自行重绑，不全遍历写回。
    /// 允许失败清理和静止点注销；正常诊断仍清偿，内部故障/析构 panic 标停，不重试已消费责任。
    /// 调用者先按 State 的 World 身份检查，异源不得消费 State 进入本方法。
    #[cfg(feature = "rc")]
    pub fn unregister_producer<T: Send + Sync + 'static>(
        &mut self,
        registration: ProducerRegistration,
    ) -> Result<(), EcsError> {
        self.unregister_producer_inner::<T>(registration)
    }
}
impl ProducerRegistration {
    /// # 用途
    /// 返回所属 World 身份，仅供绑定核对，不复制清偿责任。
    pub fn world_id(&self) -> u64 {
        self.world_identity
    }
    /// # 用途
    /// 返回已登记多例资源类型，仅用于声明与同源绑定。
    pub fn resource_id(&self) -> ResourceId {
        self.resource
    }
    /// # 用途
    /// 返回稳定生产者编号，供 WriteSlot 声明；不是紧凑物理偏移或可用于独立注销的凭据。
    pub fn producer_id(&self) -> u64 {
        self.producer
    }
}
#[cfg(feature = "rc")]
impl World {
    /// # 用途
    /// 唯一根 Running 的阶段首遍入口，须冻结、无活系统参数/通知批次，前置拒绝不调用 initialize。
    /// 保持 Running，不切 Idle/Maintaining；栈上能力覆盖全部活动系统初始化及本次失败清理。
    /// initialize 正常 Err 保留此前成功登记，不应用追踪/不派发；panic 由根守卫标停并传播。
    /// 生产者时钟耗尽或回滚结构版本耗尽，使作用域退出时标停；回滚期间仍保持原授权。
    /// 首个故障为时钟耗尽时优先报告该原因；若只在回滚时耗尽结构版本，保留 initialize 已返回的原错。
    /// initialize 不得吞掉错误后交出新责任；即使误报成功，仍返回记录的致命诊断。
    /// 返回不携带能力借用；全部成功后调用者结束作用域，再 apply_tracking 和 prepare。
    pub fn parameter_registration_scope<R>(
        &mut self,
        initialize: impl for<'a> FnOnce(ParameterRegistration<'a>) -> Result<R, EcsError>,
    ) -> Result<R, EcsError> {
        self.parameter_registration_scope_inner(initialize)
    }
    /// # 用途
    /// 系统注销/关闭入口；Idle/Stopped 且无活参数，关闭中的责任收束允许，Closed 拒绝。
    /// 只交付清偿能力，禁止新增责任；不恢复停止态，不执行 flush/settle 或运行系统。
    /// unregister 普通诊断由上层继续清偿其余系统，panic/内部故障停止并传播。
    /// 结构版本耗尽时继续本作用域的清偿，退出后标停；保留回调已返回的首错。
    pub fn parameter_unregistration_scope<R>(
        &mut self,
        unregister: impl for<'a> FnOnce(ParameterRegistration<'a>) -> Result<R, EcsError>,
    ) -> Result<R, EcsError> {
        self.parameter_unregistration_scope_inner(unregister)
    }
}
