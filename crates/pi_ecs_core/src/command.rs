//! World 命令适配；队列服务由 pi_ecs_command 单源定义。
#[path = "command_inner.rs"]
mod command_inner;
use crate::{error::EcsError, world::World};
pub use pi_ecs_command::command::*;
impl World {
    /// # 用途
    /// 从原 CommandHub 查询 C 的队列状态，供静止点或根运行的无活参数边界使用。
    #[cfg(feature = "rc")]
    pub fn command_queue_status<C: Send + 'static>(&self) -> Result<CommandQueueStatus, EcsError> {
        self.command_queue_status_inner::<C>()
    }
    /// # 用途
    /// Idle 阶段前按 capacity 登记单消费者队列，不要求 C:Clone/Sync。
    /// 同容量重复幂等，不同容量拒绝且保持原登记；关闭或运行中拒绝。
    /// 广播流是独立消费模式，不能把一个拥有型队列注册多个执行者。
    pub fn register_command<C: Send + 'static>(
        &mut self,
        capacity: CommandCapacity,
    ) -> Result<(), EcsError> {
        Self::command_register_inner::<C>(self, capacity)
    }
    /// 返回捕获当前 epoch 的发送端，未登记或关闭返回错误，不隐式登记。
    pub fn command_sender<C: Send + 'static>(&self) -> Result<CommandSender<C>, EcsError> {
        Self::command_sender_inner::<C>(self)
    }
    /// 阶段前领取唯一消费资格，已有有效消费者则返回错误；注销/Drop 交还资格但保留队列。
    /// 宿主仅 Idle 且未关闭时允许；系统初始化必须走 ParameterRegistration::command_receiver。
    pub fn command_receiver<C: Send + 'static>(&mut self) -> Result<CommandReceiver<C>, EcsError> {
        Self::command_receiver_inner::<C>(self)
    }
    /// 静止点将模拟时间推进到 now，禁止倒退；到期项在原队列变为可见，TimerKey 同时失效。
    /// 到期项按定时轮 pop 顺序追加就绪尾，不额外排序；即时项按接受顺序显现。
    /// 不调用业务回调，不再次检查容量；运行中或关闭后拒绝推进。
    pub fn advance_command_time(&mut self, now: SimulationTick) -> Result<(), EcsError> {
        Self::command_advance_inner(self, now)
    }
}
