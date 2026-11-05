//! 命令服务配对芯，设计期只含失败桩。
use crate::command::*;
use pi_ecs_types::error::EcsError;
pub(crate) fn new_inner() -> Result<CommandHub, EcsError> {
    todo!("尚未实现独立命令服务构造")
}
pub(crate) fn register_inner<C: Send + 'static>(_hub: &mut CommandHub) -> Result<(), EcsError> {
    todo!("尚未实现命令队列登记")
}
pub(crate) fn sender_inner<C: Send + 'static>(
    _hub: &CommandHub,
    _epoch: WorldEpoch,
) -> Result<CommandSender<C>, EcsError> {
    todo!("尚未实现指定代次发送端绑定")
}
pub(crate) fn receiver_inner<C: Send + 'static>(
    _hub: &mut CommandHub,
    _epoch: WorldEpoch,
) -> Result<CommandReceiver<C>, EcsError> {
    todo!("尚未实现唯一消费资格领取")
}
pub(crate) fn advance_inner(_hub: &mut CommandHub, _now: SimulationTick) -> Result<(), EcsError> {
    todo!("尚未实现定时可见性推进")
}
pub(crate) fn send_inner<C: Send + 'static>(
    _sender: &CommandSender<C>,
    _command: C,
) -> Result<(), CommandSendError<C>> {
    todo!("尚未实现")
}
pub(crate) fn schedule_inner<C: Send + 'static>(
    _sender: &CommandSender<C>,
    _deadline: SimulationTick,
    _command: C,
) -> Result<TimerKey, CommandSendError<C>> {
    todo!("尚未实现")
}
pub(crate) fn cancel_inner<C: Send + 'static>(
    _sender: &CommandSender<C>,
    _key: TimerKey,
) -> Result<bool, EcsError> {
    todo!("尚未实现")
}
pub(crate) fn read_inner<C: Send + 'static>(
    _receiver: &mut CommandReceiver<C>,
) -> Result<CommandBatch<'_, C>, EcsError> {
    todo!("尚未实现")
}
pub(crate) fn next_inner<C: Send + 'static>(_batch: &mut CommandBatch<'_, C>) -> Option<C> {
    todo!("尚未实现")
}
pub(crate) fn receiver_drop_inner<C: Send + 'static>(_receiver: &mut CommandReceiver<C>) {
    todo!("尚未实现")
}
