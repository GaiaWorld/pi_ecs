//! 系统参数长期状态；与本次 Item 和运行授权分离；资源状态不缓存实例地址，查询状态按自身协议保存稳定容器地址。
use pi_ecs_core::{
    component::ComponentId,
    notification::{NoticeKind, NoticeSubscription},
    resource::ResourceId,
};
/// # 用途
/// 每个命令发送参数位置独立的同源句柄；不保存运行授权或业务引用。
#[cfg(feature = "rc")]
pub struct CommandSenderParamState<C: Send + 'static> {
    /// 初始化时所属 World，prepare/fetch/unregister 核对，不跨 World 注入。
    pub(crate) world_identity: u64,
    /// 已登记队列的拥有型发送句柄；fetch 只借用，显式 Clone 可外逃但不保活 World。
    pub(crate) sender: pi_ecs_core::command::CommandSender<C>,
}
/// # 用途
/// 每个命令接收参数位置唯一拥有消费资格；不按每次执行重新领取。
/// 命令参数结果是借用，不把 Receiver 所有权交给系统函数。
/// ```no_run
/// use pi_ecs_core::command::{CommandRead, CommandSender};
/// use pi_ecs_schedule::system_param::SystemParam;
/// fn receiver_item<'r>(item: <CommandRead<'static, u32> as SystemParam>::Item<'r>) -> CommandRead<'r, u32> { item }
/// fn sender_item<'r>(item: <&'static CommandSender<u32> as SystemParam>::Item<'r>) -> &'r CommandSender<u32> { item }
/// ```
/// 批次存活时不能再次读取同一个 Receiver，即使队列内部采用短借。
/// ```compile_fail,E0499
/// use pi_ecs_core::command::CommandRead;
/// fn overlapping(mut receiver: CommandRead<'_, u32>) {
///     let mut first = receiver.read().unwrap();
///     let second = receiver.read().unwrap();
///     let _ = first.next();
/// }
/// ```
/// 系统不能把借用的消费资格移出长期 State。
/// ```compile_fail,E0616
/// use pi_ecs_core::command::CommandReceiver;
/// fn take(view: pi_ecs_core::command::CommandRead<'_, u32>, foreign: CommandReceiver<u32>) {
///     let _ = std::mem::replace(view.receiver, foreign);
/// }
/// ```
#[cfg(feature = "rc")]
pub struct CommandReceiverParamState<C: Send + 'static> {
    /// 取得资格时所属 World，准备与注销前核对；同 C 第二次初始化必须拒绝。
    pub(crate) world_identity: u64,
    /// 唯一 Receiver；fetch 独占短借，初始化回滚/注销/State Drop 归还资格，不清队列。
    pub(crate) receiver: pi_ecs_core::command::CommandReceiver<C>,
}
/// 单资源参数已解析身份；可选参数复用此状态，缺实例时返回 None，不吞其他错误。
pub struct ResourceParamState {
    /// 本 World 登记的资源类型，不缓存当前实例地址。
    pub(crate) resource: ResourceId,
}
/// 多例集合读参数；每参数位置独立记录结构观察点。
pub struct MultiResourceParamState {
    /// 本 World 的多例资源类型。
    pub(crate) resource: ResourceId,
    /// 上次成功 finish 所见结构版本，首次 None；跳过或执行失败不推进。
    pub(crate) previous_version: Option<u64>,
}
/// 多例写参数长期槽；初值只在初始化移入 World 槽，State 不重复拥有 T。
pub struct MultiResourceProducerState {
    /// core 交付的唯一生产槽清偿责任；显式注销消费，不以紧凑偏移或参数编号代替。
    pub(crate) registration: pi_ecs_core::parameter_registration::ProducerRegistration,
    /// 系统/参数位置及阶段重绑的紧凑槽位置。
    pub(crate) producer: crate::resource_param::ProducerState,
    /// 已选恢复策略；具体历史能力接线随恢复功能暂缓。
    pub(crate) recovery: Option<pi_ecs_core::resource::StateRecovery>,
}
/// 多例读参数本次观察结果；不含业务引用，成功 finish 才写回长期版本。
pub struct MultiResourcePending {
    /// fetch 记录的本次版本；尚未 fetch 为 None。
    pub(crate) observed_version: Option<u64>,
}
/// 每个 Reader 参数位置拥有一个独立订阅，不按相同 T 合并。
pub struct ReaderParamState {
    /// 已解析组件，须与 T 一致。
    pub(crate) component: ComponentId,
    /// Added/Changed/Removed，对应实现固定，不能运行时切换。
    pub(crate) kind: NoticeKind,
    /// 唯一订阅句柄；fetch 借用，unregister 消费并注销。
    pub(crate) subscription: NoticeSubscription,
}
