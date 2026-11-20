//! World 命令适配芯；仅调用命令包的服务壳，不引用其他芯。
#[cfg(all(test, feature = "rc"))]
mod rc_host_command_tests {
    use crate::{
        command::{CommandCapacity, SimulationTick},
        world::World,
    };
    // Seam: World 命令登记、阶段前消费资格和定时可见性。
    #[test]
    fn rc_host_command_gates_capacity_and_simulation_time() {
        let mut world = World::new().unwrap();
        world
            .register_command::<u32>(CommandCapacity::Bounded(
                std::num::NonZeroUsize::new(2).unwrap(),
            ))
            .unwrap();
        world
            .register_command::<u32>(CommandCapacity::Bounded(
                std::num::NonZeroUsize::new(2).unwrap(),
            ))
            .unwrap();
        assert!(world
            .register_command::<u32>(CommandCapacity::Unbounded)
            .is_err());
        let sender = world.command_sender::<u32>().unwrap();
        assert!(sender.schedule_at(SimulationTick(3), 7).is_ok());
        world.finish_registration().unwrap();
        let mut receiver = world.command_receiver::<u32>().unwrap();
        assert!(world.command_receiver::<u32>().is_err());
        assert_eq!(receiver.read().unwrap().next(), None);
        world
            .schedule_scope(|world| {
                assert!(world
                    .register_command::<u64>(CommandCapacity::Unbounded)
                    .is_err());
                assert!(world.command_receiver::<u32>().is_err());
                assert!(world.advance_command_time(SimulationTick(3)).is_err());
                Ok(())
            })
            .unwrap();
        world.advance_command_time(SimulationTick(2)).unwrap();
        assert_eq!(receiver.read().unwrap().next(), None);
        assert!(world.advance_command_time(SimulationTick(1)).is_err());
        world.advance_command_time(SimulationTick(3)).unwrap();
        assert_eq!(receiver.read().unwrap().collect::<Vec<_>>(), [7]);
        drop(receiver);
        assert!(world.command_receiver::<u32>().is_ok());
        world.close().unwrap();
        assert!(world
            .register_command::<u32>(CommandCapacity::Bounded(
                std::num::NonZeroUsize::new(2).unwrap()
            ))
            .is_err());
        assert!(world.command_receiver::<u32>().is_err());
        assert!(world.advance_command_time(SimulationTick(4)).is_err());
        assert_eq!(sender.send(11).unwrap_err().command, 11);
    }
}
use crate::{command::*, error::EcsError, world::World};
fn host_idle(world: &World) -> Result<(), EcsError> {
    #[cfg(feature = "rc")]
    let allowed = {
        let mode = world.access.mode.replace(crate::world::AccessMode::Stopped);
        let idle = matches!(mode, crate::world::AccessMode::Idle);
        world.access.mode.set(mode);
        idle && world.shutdown == crate::shutdown::ShutdownState::Open
    };
    #[cfg(not(feature = "rc"))]
    let allowed = world.access.0.load(std::sync::atomic::Ordering::Acquire) == 0;
    if !allowed {
        return Err(pi_ecs_types::error::classified_error(
            pi_ecs_types::error::ErrorKind::InvalidState,
            "命令宿主入口要求未关闭的 Idle World",
        ));
    }
    Ok(())
}
impl World {
    pub(crate) fn command_register_inner<C: Send + 'static>(
        _world: &mut World,
        _capacity: CommandCapacity,
    ) -> Result<(), EcsError> {
        host_idle(_world)?;
        _world.commands.register::<C>(_capacity)
    }
}
impl World {
    pub(crate) fn command_sender_inner<C: Send + 'static>(
        world: &World,
    ) -> Result<CommandSender<C>, EcsError> {
        world.commands.sender::<C>(world.epoch)
    }
}
impl World {
    pub(crate) fn command_receiver_inner<C: Send + 'static>(
        _world: &mut World,
    ) -> Result<CommandReceiver<C>, EcsError> {
        host_idle(_world)?;
        _world.commands.receiver::<C>(_world.epoch)
    }
}
impl World {
    pub(crate) fn command_advance_inner(
        _world: &mut World,
        _now: SimulationTick,
    ) -> Result<(), EcsError> {
        host_idle(_world)?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            _world.commands.advance(_now)
        }));
        match result {
            Ok(result) => {
                result?;
                _world.simulation_tick = _now;
                Ok(())
            }
            Err(payload) => {
                #[cfg(feature = "rc")]
                _world.access.mode.set(crate::world::AccessMode::Stopped);
                #[cfg(not(feature = "rc"))]
                _world
                    .access
                    .0
                    .store(4, std::sync::atomic::Ordering::Release);
                std::panic::resume_unwind(payload)
            }
        }
    }
}
