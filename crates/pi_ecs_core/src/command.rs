//! World 命令适配；队列服务由 pi_ecs_command 单源定义。
use crate::{error::EcsError, world::World};
pub use pi_ecs_command::command::*;
impl World {
    /// 阶段前登记单消费者队列，不要求 C:Clone/Sync；已有相同登记复用。
    /// 广播流是独立消费模式，不能把一个拥有型队列注册多个执行者。
    pub fn register_command<C: Send + 'static>(&mut self) -> Result<(), EcsError> {
        crate::command_inner::register_inner::<C>(self)
    }
    /// 返回捕获当前 epoch 的发送端，未登记或关闭返回错误，不隐式登记。
    pub fn command_sender<C: Send + 'static>(&self) -> Result<CommandSender<C>, EcsError> {
        crate::command_inner::sender_inner::<C>(self)
    }
    /// 阶段前领取唯一消费资格，已有有效消费者则返回错误；注销/Drop 交还资格但保留队列。
    pub fn command_receiver<C: Send + 'static>(&mut self) -> Result<CommandReceiver<C>, EcsError> {
        crate::command_inner::receiver_inner::<C>(self)
    }
    /// 静止点将模拟时间推进到 now，禁止倒退；到期项在原队列变为可见，TimerKey 同时失效。
    /// 同 deadline 用登记序号排序，即时项按接受顺序显现；不在内部调用业务回调。
    pub fn advance_command_time(&mut self, now: SimulationTick) -> Result<(), EcsError> {
        crate::command_inner::advance_inner(self, now)
    }
}
