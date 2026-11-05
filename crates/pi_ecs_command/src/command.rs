//! World 拥有的类型化命令服务；后台结果、外部输入及定时命令共用入口。
use pi_ecs_types::error::EcsError;
use std::{marker::PhantomData, sync::Arc};
/// 模拟时间，宿主推进；不是组件变化 ChangeTick。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SimulationTick(
    /// 模拟步数，推进及溢出规则待定。
    pub u64,
);
/// 运行代次，不随游戏快照回退；旧任务结果不得伪装为新代次。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldEpoch(
    /// 单 World 单调代次，恢复时推进。
    pub(crate) u64,
);
/// 非拥有的延迟项取消凭据；到期变可见、取消或关闭后失效。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TimerKey {
    /// 所属服务/队列及 epoch 的校验身份，不允许外部构造。
    pub(crate) owner: u128,
    /// 槽及代际，普通复用必须拒绝旧键。
    pub(crate) slot: u64,
}
/// 提交失败交还 payload，不静默丢失调用方所有权。
pub struct CommandSendError<C> {
    /// 未登记/已关闭/旧 epoch/容量或期限校验错误。
    pub error: EcsError,
    /// 未接收的原命令。
    pub command: C,
}
/// 共享入口控制块，只拥有命令服务，不强持整个 World，关闭后发送返回错误。
pub(crate) struct CommandEndpoint {
    // @to_design 并发 payload 槽、就绪索引、定时索引和关闭保活表示待闭合。
    pub(crate) _pending: (),
}
/// World 拥有的命令服务，具体存储可由资源承载，不让插件重复实现生命周期协议。
pub struct CommandHub {
    /// 外部发送端可独立保活入口，不能借此访问组件。
    pub(crate) endpoint: Arc<CommandEndpoint>,
}
impl CommandHub {
    /// 构造独立命令服务；分配失败返回错误，不发布半入口，不持有 World。
    pub fn new() -> Result<Self, EcsError> {
        crate::command_inner::new_inner()
    }
    /// 阶段前登记 C 的唯一消费者队列；重复相同登记幂等，失败保持已有登记。
    pub fn register<C: Send + 'static>(&mut self) -> Result<(), EcsError> {
        crate::command_inner::register_inner::<C>(self)
    }
    /// epoch 为所属 World 当前代次；仅为已登记且未关闭队列创建发送端，失败不改服务。
    pub fn sender<C: Send + 'static>(
        &self,
        epoch: WorldEpoch,
    ) -> Result<CommandSender<C>, EcsError> {
        crate::command_inner::sender_inner::<C>(self, epoch)
    }
    /// epoch 为当前代次；阶段前领取 C 的唯一消费资格，冲突返回错误且不清队列。
    pub fn receiver<C: Send + 'static>(
        &mut self,
        epoch: WorldEpoch,
    ) -> Result<CommandReceiver<C>, EcsError> {
        crate::command_inner::receiver_inner::<C>(self, epoch)
    }
    /// now 不得倒退；静止点只更新到期项可见性，不重入队或执行回调，前置失败不改状态。
    pub fn advance(&mut self, now: SimulationTick) -> Result<(), EcsError> {
        crate::command_inner::advance_inner(self, now)
    }
}
/// 多生产者提交句柄；绑定取得时的 epoch，发送时不自动改为当前代次。
pub struct CommandSender<C: Send + 'static> {
    /// 保活独立入口而非整个 World。
    pub(crate) endpoint: Arc<CommandEndpoint>,
    /// 派发任务时捕获的运行代次。
    pub(crate) epoch: WorldEpoch,
    /// 类型身份，实际队列须校验。
    pub(crate) marker: PhantomData<fn(C)>,
}
impl<C: Send + 'static> CommandSender<C> {
    /// command 被服务接收即入队可见，不代表业务成功；没有活消费者仍可保留。
    /// 失败返回原 payload；跨生产者到达顺序不承诺模拟确定性。
    pub fn send(&self, command: C) -> Result<(), CommandSendError<C>> {
        crate::command_inner::send_inner(self, command)
    }
    /// command 从现在起由同一队列拥有，deadline 前不可消费；返回 TimerKey。
    /// 到期只更新就绪状态/索引，不复制或重新入队，不再产生第二次入队容量失败。
    /// 即时项不能被未来定时项阻塞；同到期时间按登记序号稳定显现。
    pub fn schedule_at(
        &self,
        deadline: SimulationTick,
        command: C,
    ) -> Result<TimerKey, CommandSendError<C>> {
        crate::command_inner::schedule_inner(self, deadline, command)
    }
    /// key 必须属本队列及代次；成功取消尚未可见项并普通 Drop payload。
    /// 到期与取消并发必须线性化；已经可见/无效键返回 false，不撤回就绪命令。
    pub fn cancel(&self, key: TimerKey) -> Result<bool, EcsError> {
        crate::command_inner::cancel_inner(self, key)
    }
}
/// 唯一消费资格，不 Clone；注销只交还资格，不自动清空队列。
pub struct CommandReceiver<C: Send + 'static> {
    /// 队列入口保活，不授予组件修改权限。
    pub(crate) endpoint: Arc<CommandEndpoint>,
    /// 本次资格与运行代次，复用/注销规则待定。
    pub(crate) epoch: WorldEpoch,
    /// 命令类型标记。
    pub(crate) marker: PhantomData<fn() -> C>,
}
impl<C: Send + 'static> CommandReceiver<C> {
    /// 固定当前可见上界；借用 self 排除第二个并发批次。
    /// next 取得 C 即出队，无业务 ack；早停/drop 保留未返回项，新显现项留给下批。
    pub fn read(&mut self) -> Result<CommandBatch<'_, C>, EcsError> {
        crate::command_inner::read_inner(self)
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
    /// 返回即转移所有权；后续处理失败不自动重新入队。
    fn next(&mut self) -> Option<C> {
        crate::command_inner::next_inner(self)
    }
}
impl<C: Send + 'static> Drop for CommandReceiver<C> {
    /// 交还消费资格，不丢弃尚未消费命令。
    fn drop(&mut self) {
        crate::command_inner::receiver_drop_inner(self)
    }
}
// 广播订阅：多个独立游标读取 &C，全部消费且无借用后回收，需 C:Send+Sync。
// 快照保存未消费游戏项及模拟定时项：独立 payload、顺序、deadline、就绪/取消状态与键分配状态。
// 恢复不重新入队；保存内容安装为新分支，旧 Sender epoch 不刷新，目标 World 不继承源发送资格。
// 游戏状态内 TimerKey 对应恢复槽；外部旧分支 Key 的取消资格由调用方保证，不自动修复。
// 宿主按历史截点重放外部输入并去重；重演不接现实输入、不启动不可撤销任务；真实时间定时留宿主。
// @to_design payload 复制能力登记/存储接口、消费资格重绑、广播接口、发送端 Clone、背压及关闭实现待闭合。
// fory 为计划编解码选择，未引入依赖或验证版本；本地类型化命令不强制编解码。
// 任意结构修改由普通命令处理系统在专门阶段执行，队列不授予 World 写权限，不设特殊结构命令队列。
