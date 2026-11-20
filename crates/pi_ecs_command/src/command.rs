//! World 拥有的类型化命令服务；后台结果、外部输入及定时命令共用入口。
#[path = "command_inner.rs"]
mod command_inner;
use pi_ecs_types::error::EcsError;
use pi_share::Share;
use std::marker::PhantomData;
/// 模拟时间，宿主推进；不是组件变化 ChangeTick。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SimulationTick(
    /// 非递减模拟步计数；定时轮精度为 1，一个单位映射一个模拟步，不读墙钟。
    /// 库不隐式加一或回绕，调用者计算下一步时处理 u64 溢出。
    pub u64,
);
/// 运行代次，不随游戏快照回退；旧任务结果不得伪装为新代次。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldEpoch(
    /// 单 World 单调代次，恢复时推进。
    pub(crate) u64,
);
impl WorldEpoch {
    /// 新 World 的初始代次；不暴露字段构造。
    pub fn initial() -> Self {
        Self::initial_epoch_inner()
    }
}
/// 非拥有的延迟项取消凭据；到期变可见、取消或关闭后失效。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TimerKey {
    /// 所属服务/队列及 epoch 的校验身份，不允许外部构造。
    pub(crate) owner: u128,
    /// 队列内单调不复用的命令身份，耗尽拒绝接收，不借依赖槽代际回绕延长旧键有效性。
    pub(crate) slot: u64,
}
/// 提交失败交还 payload，不静默丢失调用方所有权。
pub struct CommandSendError<C> {
    /// 未登记/已关闭/旧 epoch/容量或期限校验错误。
    pub error: EcsError,
    /// 未接收的原命令。
    pub command: C,
}
/// Hub 独占的类型目录；句柄只共享控制与队列，不强持目录或 World。
pub(crate) struct CommandEndpoint {
    /// RC 共享控制只含 Cell，句柄保活不延长 World，不依赖全目录 RefCell 进行发送。
    #[cfg(feature = "rc")]
    pub(crate) control: Share<CommandControl>,
    /// RC 目录只在创建句柄/登记时查 TypeId，队列自己的短借独立。
    #[cfg(feature = "rc")]
    pub(crate) queues: std::collections::HashMap<std::any::TypeId, QueueEntry>,
    /// RC 新队列身份，单调不复用，耗尽拒绝登记。
    #[cfg(feature = "rc")]
    pub(crate) next_queue: u64,
    /// 第二轮并发后端占位，不影响本轮 RC 协议。
    #[cfg(not(feature = "rc"))]
    pub(crate) _pending: (),
}
/// World 拥有的命令服务，具体存储可由资源承载，不让插件重复实现生命周期协议。
pub struct CommandHub {
    /// Hub 唯一拥有目录；RC 句柄只保活 control 与类型化队列，不保活此目录。
    pub(crate) endpoint: CommandEndpoint,
}
impl CommandHub {
    /// # 用途
    /// 建立关闭截点并丢弃已接收的就绪/延迟 payload，不调用命令处理器。
    /// 截点后外部 Sender 即使保活队列也拒绝提交并返回原 payload，Receiver 不再消费。
    /// 完整关闭后幂等；清理期间不得持内部借用跨 payload 析构，重入提交必须见已关闭。
    /// 不分配模拟/变化 tick；析构 panic 不支持重试已移出的 payload，外围关闭保持停止。
    pub fn close(&mut self) {
        // 设计已确认：先全局关闭接收与消费，再逐队列移出全部值；
        // 每队列释放状态短借后才析构移出的值，用户 Drop 重入必须见关闭。
        // 析构 panic 不重试已移出值，尚存队列依普通所有权最终清理。
        self.close_inner()
    }
    /// 构造独立命令服务；分配失败返回错误，不发布半入口，不持有 World。
    pub fn new() -> Result<Self, EcsError> {
        Self::new_inner()
    }
    /// # 用途
    /// 阶段前按 C 登记队列与 capacity；同容量重复幂等，不同容量拒绝，不运行中调整。
    /// 有限容量按就绪+延迟 payload 总数计，无限仍受 usize/序号/内存上限约束；失败保持旧登记。
    pub fn register<C: Send + 'static>(
        &mut self,
        capacity: CommandCapacity,
    ) -> Result<(), EcsError> {
        self.register_inner::<C>(capacity)
    }
    /// epoch 为所属 World 当前代次；仅为已登记且未关闭队列创建发送端，失败不改服务。
    pub fn sender<C: Send + 'static>(
        &self,
        epoch: WorldEpoch,
    ) -> Result<CommandSender<C>, EcsError> {
        self.sender_inner::<C>(epoch)
    }
    /// epoch 为当前代次；阶段前领取 C 的唯一消费资格，冲突返回错误且不清队列。
    pub fn receiver<C: Send + 'static>(
        &mut self,
        epoch: WorldEpoch,
    ) -> Result<CommandReceiver<C>, EcsError> {
        self.receiver_inner::<C>(epoch)
    }
    /// now 不得倒退；静止点推进每类型定时轮（精度 1，直接映射模拟步），直到 pop(now) 返回 None。
    /// 不额外排序，不执行处理器；只移动槽键到就绪尾，不复制 payload，不再次检查队列满。
    /// 就绪所需索引空间在接收命令时预留，库滚动可能逐步推进且内部仍可分配，OOM/panic 不承诺恢复。
    /// 前置错误不改状态；开始跨队列推进后内部 panic 不回滚，RC Hub 可用性必须永久撤销。
    pub fn advance(&mut self, now: SimulationTick) -> Result<(), EcsError> {
        self.advance_inner(now)
    }
}
/// 多生产者提交句柄；绑定取得时的 epoch，发送时不自动改为当前代次。
/// rc 使用 Rc 入口，Sender 为 !Send/!Sync，仅限创建它的线程，不直接接收跨线程后台提交。
/// 非 rc 使用 Arc 入口，具体线程安全仍须真实队列同步实现，不以 Arc 单独证明。
/// payload 当前仍保留 Send 约束，本轮不顺带放宽。
pub struct CommandSender<C: Send + 'static> {
    /// RC 直接持类型化队列和共享控制，不逐 send 查 TypeId。
    #[cfg(feature = "rc")]
    pub(crate) queue: Share<CommandQueue<C>>,
    /// 第二轮旧入口表示。
    #[cfg(not(feature = "rc"))]
    pub(crate) endpoint: Share<CommandEndpoint>,
    /// 派发任务时捕获的运行代次。
    pub(crate) epoch: WorldEpoch,
    /// 类型身份，实际队列须校验。
    pub(crate) marker: PhantomData<fn(C)>,
}
impl<C: Send + 'static> CommandSender<C> {
    /// command 被服务接收即入队可见，不代表业务成功；没有活消费者仍可保留。
    /// 失败返回原 payload；跨生产者到达顺序不承诺模拟确定性。
    pub fn send(&self, command: C) -> Result<(), CommandSendError<C>> {
        // 设计已确认：先校验关闭、epoch、容量、命令身份与最坏就绪序号空间；
        // 为全部未消费项预留就绪索引，再发布唯一 payload 与即时索引。
        // 正常拒绝不发布状态，解除短借后交还原输入；分配失败不保证恢复。
        self.send_inner(command)
    }
    /// command 从现在起由同一队列拥有，deadline 前不可消费；返回 TimerKey。
    /// 到期只更新就绪状态/索引，不复制或重新入队，不再产生第二次入队容量失败。
    /// 即时项不被未来定时项阻塞；deadline<=now 立即就绪，返回键不可再取消。
    /// 未来项直接按 pi_cancel_timer::Timer::pop 顺序追加就绪尾，不额外排序，不承诺同截止接收顺序。
    pub fn schedule_at(
        &self,
        deadline: SimulationTick,
        command: C,
    ) -> Result<TimerKey, CommandSendError<C>> {
        self.schedule_inner(deadline, command)
    }
    /// key 必须属本队列及代次；成功取消尚未可见项并普通 Drop payload。
    /// RC 在所属线程的单次短借内移出并释放名额；解除借用后才析构。
    /// 已经可见/无效键返回 false，不撤回就绪命令；关闭/旧 epoch 返回错误。
    pub fn cancel(&self, key: TimerKey) -> Result<bool, EcsError> {
        self.cancel_inner(key)
    }
}
/// 唯一消费资格，不 Clone；注销只交还资格，不自动清空队列。
pub struct CommandReceiver<C: Send + 'static> {
    /// RC 唯一消费资格所在的类型化队列，不保活 Hub 目录或 World。
    #[cfg(feature = "rc")]
    pub(crate) queue: Share<CommandQueue<C>>,
    /// 第二轮旧入口表示。
    #[cfg(not(feature = "rc"))]
    pub(crate) endpoint: Share<CommandEndpoint>,
    /// 领取时固定的代次；第一轮不恢复/重绑，Drop 只交还 occupied，不清队列。
    pub(crate) epoch: WorldEpoch,
    /// 命令类型标记。
    pub(crate) marker: PhantomData<fn() -> C>,
}
impl<C: Send + 'static> CommandReceiver<C> {
    /// 固定当前可见上界；借用 self 排除第二个并发批次，不跨用户代码持队列借用。
    /// next 取得 C 即出队，无业务 ack；早停/drop 保留未返回项，新显现项留给下批。
    pub fn read(&mut self) -> Result<CommandBatch<'_, C>, EcsError> {
        self.read_inner()
    }
}
/// 一次有限的拥有型消费迭代，不读取/修改 World 组件。
pub struct CommandBatch<'a, C: Send + 'static> {
    /// 独占借用唯一消费资格。
    pub(crate) receiver: &'a mut CommandReceiver<C>,
    /// 本批就绪序号上界，不是存储槽下标。
    pub(crate) end: u64,
}
impl<C: Send + 'static> Iterator for CommandBatch<'_, C> {
    type Item = C;
    /// 返回即转移所有权并释放容量；后续处理失败不自动重新入队。
    /// 关闭后返回 None；每次只在短借内移出一个值，新到期/发送项不越过本批上界。
    fn next(&mut self) -> Option<C> {
        self.next_inner()
    }
}
impl<C: Send + 'static> Drop for CommandReceiver<C> {
    /// 交还消费资格，不丢弃尚未消费命令。
    fn drop(&mut self) {
        self.receiver_drop_inner()
    }
}
// 广播订阅：多个独立游标读取 &C，全部消费且无借用后回收，需 C:Send+Sync。
// 快照保存未消费游戏项及模拟定时项：独立 payload、顺序、deadline、就绪/取消状态与键分配状态。
// 恢复不重新入队；保存内容安装为新分支，旧 Sender epoch 不刷新，目标 World 不继承源发送资格。
// 游戏状态内 TimerKey 对应恢复槽；外部旧分支 Key 的取消资格由调用方保证，不自动修复。
// 宿主按历史截点重放外部输入并去重；重演不接现实输入、不启动不可撤销任务；真实时间定时留宿主。
// 快照复制/恢复、消费资格重绑与广播接口延期；本轮不提供对应能力。
// fory 为计划编解码选择，未引入依赖或验证版本；本地类型化命令不强制编解码。
// 任意结构修改由普通命令处理系统在专门阶段执行，队列不授予 World 写权限，不设特殊结构命令队列。

/// # 用途
/// 每类型未消费 payload 的总数上限；不是字节预算，不包括 next 已转给用户的值。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandCapacity {
    /// 限制就绪+延迟命令总条数；满时立即交还输入，不阻塞或覆盖。
    Bounded(
        /// 非零最大条数。
        std::num::NonZeroUsize,
    ),
    /// 不设业务条数上限，仍校验索引/序号溢出；不承诺不会耗尽内存。
    Unbounded,
}
/// # 用途
/// RC 关闭与时钟单源；所有句柄共享只读控制，Cell 仅在所属线程操作。
#[cfg(feature = "rc")]
pub(crate) struct CommandControl {
    /// 服务非复用身份，与队列身份组合进 TimerKey.owner；不是内存地址。
    pub(crate) identity: u64,
    /// 所有队列共享代次，第一轮初始化后不变，句柄校验来源。
    pub(crate) epoch: WorldEpoch,
    /// Hub 关闭或推进内部失败时先置 true；用户析构重入发送必须拒绝。
    pub(crate) closed: std::cell::Cell<bool>,
    /// 最近成功接受的模拟时间；advance 前拒绝倒退，schedule 以本值判断立即可见。
    pub(crate) now: std::cell::Cell<SimulationTick>,
}
/// # 用途
/// RC 目录中的一个类型化队列及只处理索引/清理的擦除操作；不逐命令 Box<dyn Any>。
#[cfg(feature = "rc")]
pub(crate) struct QueueEntry {
    /// 唯一保存 Share<CommandQueue<C>>，创建句柄时校验真实 C 后克隆该 Share。
    pub(crate) typed: Box<dyn std::any::Any>,
    /// 对 typed 执行已登记类型的到期索引迁移，不调用用户代码，不复制 payload。
    pub(crate) advance: fn(&dyn std::any::Any, SimulationTick),
    /// 从 typed 移出并清空全部 payload，解除短借后才析构；禁止在借用中调用 C::drop。
    pub(crate) close: fn(&dyn std::any::Any),
}
/// # 用途
/// RC 单类型队列句柄；固定控制与身份在短借外，payload 状态按操作短借。
#[cfg(feature = "rc")]
pub(crate) struct CommandQueue<C> {
    /// 关闭/时钟与服务身份，晚存活句柄只保活它，不保活 World。
    pub(crate) control: Share<CommandControl>,
    /// 服务内单调队列编号，组合形成 owner，拒绝跨类型/跨 Hub 取消键。
    pub(crate) identity: u64,
    /// 固定容量策略，不因克隆 Sender 或更换 Receiver 改变。
    pub(crate) capacity: CommandCapacity,
    /// 每次操作短借；转交或析构 payload 前必须释放 guard，不持借用跨批次用户代码。
    pub(crate) state: std::cell::RefCell<CommandQueueState<C>>,
}
/// # 用途
/// RC payload 与两个索引；一份 payload 仅归 pending，ready/timer 只保存命令编号。
#[cfg(feature = "rc")]
pub(crate) struct CommandQueueState<C> {
    /// 命令编号不复用，取消/next 从此移出后释放容量；绝不留一份定时 payload 副本。
    pub(crate) pending: std::collections::HashMap<u64, CommandEntry<C>>,
    /// 按成为可见的顺序保存 (就绪序号,命令编号)，不被未来定时项阻塞。
    pub(crate) ready: std::collections::VecDeque<(u64, u64)>,
    /// 只保存命令编号，精度固定 1；256 首层槽、64 高层槽、3 高层，配置为内部索引选择。
    /// 到期使用 pop 原顺序，无排序工作区；库内远期键迁移沿用依赖实现。
    pub(crate) timer: pi_cancel_timer::Timer<u64, 256, 64, 3, 1>,
    /// 下一命令编号，checked 分配，耗尽拒绝接收并交还输入，不回绕复用旧取消键。
    pub(crate) next_command: u64,
    /// 下一就绪序号；接收时检查未来全部 pending 显现仍可表示，advance 不二次耗尽。
    pub(crate) next_ready: u64,
    /// 一个 Receiver 独占资格，Drop 归还，不清空 pending；forget 只泄漏资格。
    pub(crate) occupied: bool,
}
/// # 用途
/// 唯一拥有 C 的未消费命令；到期只清 timer_key 并追加 ready，不再次接收或申请名额。
#[cfg(feature = "rc")]
pub(crate) struct CommandEntry<C> {
    /// 接收后唯一 payload，移出前不能在索引回调复制或析构。
    pub(crate) value: C,
    /// 延迟且未可见时的依赖取消键；即时/已到期为 None，外部取消返回 false。
    pub(crate) timer_key: Option<pi_slot_wheel::TimerKey>,
}
impl<C: Send + 'static> Clone for CommandSender<C> {
    /// # 用途
    /// 仅复制原队列句柄及原 epoch，不复制 C、不取得消费资格，不刷新来源代次。
    fn clone(&self) -> Self {
        self.clone_inner()
    }
}
impl Drop for CommandHub {
    /// # 用途
    /// RC 未显式关闭时复用 close，先关闭控制再逐队列移出清理，不留存活 Sender 可继续接收。
    /// payload 析构期间不得持内部借用；析构 panic 不支持半关闭重试。
    fn drop(&mut self) {
        self.drop_inner()
    }
}
