//! World 命令适配芯；仅调用命令包的服务壳，不引用其他芯。
use crate::{command::*, error::EcsError, world::World};
pub(crate) fn register_inner<C: Send + 'static>(world: &mut World) -> Result<(), EcsError> {
    world.commands.register::<C>()
}
pub(crate) fn sender_inner<C: Send + 'static>(world: &World) -> Result<CommandSender<C>, EcsError> {
    world.commands.sender::<C>(world.epoch)
}
pub(crate) fn receiver_inner<C: Send + 'static>(
    world: &mut World,
) -> Result<CommandReceiver<C>, EcsError> {
    world.commands.receiver::<C>(world.epoch)
}
pub(crate) fn advance_inner(world: &mut World, now: SimulationTick) -> Result<(), EcsError> {
    world.commands.advance(now)
}
