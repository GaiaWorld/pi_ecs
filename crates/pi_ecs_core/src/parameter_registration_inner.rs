//! 参数登记能力配对芯。
#[cfg(all(test, feature = "rc"))]
mod rc_parameter_notice_tests {
    use crate::{
        component::{Component, PlainAccess, StoragePolicy},
        notification::NoticeKind,
        world::World,
    };
    struct Missing;
    impl Component for Missing {
        type Access = PlainAccess;
    }
    struct Value;
    impl Component for Value {
        type Access = PlainAccess;
    }
    // Seam: Running 初始化从已登记 Hub 领取资格，失败和 Drop 不清空 payload。
    #[test]
    fn rc_parameter_command_unique_receiver_and_rollback() {
        use crate::command::CommandCapacity;
        let mut world = World::new().unwrap();
        world
            .register_command::<u32>(CommandCapacity::Unbounded)
            .unwrap();
        world.finish_registration().unwrap();
        let (sender, mut receiver) = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|mut scope| {
                    assert!(scope.command_sender::<u64>().is_err());
                    assert!(scope.command_receiver::<u64>().is_err());
                    let sender = scope.command_sender::<u32>()?;
                    assert!(sender.send(7).is_ok());
                    let rollback = scope.command_receiver::<u32>()?;
                    assert!(scope.command_receiver::<u32>().is_err());
                    drop(rollback);
                    Ok((sender, scope.command_receiver::<u32>()?))
                })
            })
            .unwrap();
        assert_eq!(receiver.read().unwrap().collect::<Vec<_>>(), [7]);
        world
            .parameter_unregistration_scope(|mut scope| {
                assert!(scope.command_sender::<u32>().is_err());
                assert!(scope.command_receiver::<u32>().is_err());
                Ok(())
            })
            .unwrap();
        assert!(sender.send(11).is_ok());
        drop(receiver);
        let mut next = world.command_receiver::<u32>().unwrap();
        assert_eq!(next.read().unwrap().collect::<Vec<_>>(), [11]);
        drop(next);
        world.close().unwrap();
        assert_eq!(sender.send(19).unwrap_err().command, 19);
    }
    // Seam: 冻结后的参数类型解析不创建类型或资源实例。
    #[test]
    fn rc_parameter_type_resolution_preserves_frozen_catalogs() {
        let mut world = World::new().unwrap();
        let component = world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        let resource = world.register_resource::<u32>().unwrap();
        let multi = world.register_multi_resource::<u64>().unwrap();
        world.finish_registration().unwrap();
        world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|scope| {
                    assert_eq!(scope.component_id::<Value>(), Some(component));
                    assert_eq!(scope.component_id::<Missing>(), None);
                    assert_eq!(scope.resource_id::<u32>(), Some(resource));
                    assert_eq!(scope.resource_id::<u64>(), None);
                    assert_eq!(scope.multi_resource_id::<u64>(), Some(multi));
                    Ok(())
                })
            })
            .unwrap();
        world
            .parameter_unregistration_scope(|scope| {
                assert_eq!(scope.component_id::<Value>(), Some(component));
                assert_eq!(scope.resource_id::<u32>(), Some(resource));
                Ok(())
            })
            .unwrap();
        assert!(world.pin().unwrap().resource::<u32>().is_none());
        world.close().unwrap();
    }
    // Seam: 根运行登记与静止点清偿；独立订阅不推进彼此游标。
    #[test]
    fn rc_parameter_notice_registration_rollback_and_stopped_cleanup() {
        let mut world = World::new().unwrap();
        let id = world
            .register_component::<Value>(StoragePolicy::TableEligible)
            .unwrap();
        world.finish_registration().unwrap();
        let before = world.read_change_tick();
        let (first, second) = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|mut scope| {
                    let first = scope.subscribe_notice(id, NoticeKind::Removed)?;
                    let rollback = scope.subscribe_notice(id, NoticeKind::Changed)?;
                    scope.unsubscribe_notice(rollback)?;
                    assert!(scope
                        .subscribe_notice(
                            crate::component::ComponentId(u32::MAX),
                            NoticeKind::Added
                        )
                        .is_err());
                    Ok((first, scope.subscribe_notice(id, NoticeKind::Removed)?))
                })
            })
            .unwrap();
        assert_eq!(world.read_change_tick(), before);
        assert!(world.notifications.is_empty(&first).unwrap());
        world
            .parameter_unregistration_scope(|mut scope| {
                assert!(scope.subscribe_notice(id, NoticeKind::Added).is_err());
                scope.unsubscribe_notice(first)
            })
            .unwrap();
        assert!(world.notifications.is_empty(&second).unwrap());
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = world.schedule_scope::<()>(|_| panic!("停止后仍须清偿"));
        }))
        .is_err());
        assert!(world.is_stopped());
        world
            .parameter_unregistration_scope(|mut scope| scope.unsubscribe_notice(second))
            .unwrap();
        assert!(world.is_stopped());
        world.close().unwrap();
    }
}
#[cfg(feature = "rc")]
fn registration_error(message: &'static str) -> crate::error::EcsError {
    pi_ecs_types::error::classified_error(pi_ecs_types::error::ErrorKind::InvalidState, message)
}
#[cfg(feature = "rc")]
fn check_scope(
    world: &crate::world::World,
    initialize: bool,
) -> Result<(), crate::error::EcsError> {
    let mode = world.access.mode.replace(crate::world::AccessMode::Stopped);
    let allowed = if initialize {
        matches!(mode, crate::world::AccessMode::Running)
    } else {
        matches!(
            mode,
            crate::world::AccessMode::Idle | crate::world::AccessMode::Stopped
        )
    };
    world.access.mode.set(mode);
    let open = if initialize {
        world.shutdown == crate::shutdown::ShutdownState::Open
    } else {
        world.shutdown != crate::shutdown::ShutdownState::Closed
    };
    if !allowed || !open || (initialize && !world.registry.frozen) {
        return Err(registration_error("参数登记/清偿作用域状态不符"));
    }
    // 正常借用排除活参数；遗留通知批次还须经真实 RefCell 门禁检查。
    for streams in &world.notifications.components {
        if let Some(stream) = &streams.added {
            let _guard = stream
                .try_borrow_mut()
                .map_err(|_| registration_error("通知批次仍活跃"))?;
        }
        if let Some(stream) = &streams.changed {
            let _guard = stream
                .try_borrow_mut()
                .map_err(|_| registration_error("通知批次仍活跃"))?;
        }
        if let Some(stream) = &streams.removed {
            let _guard = stream
                .try_borrow_mut()
                .map_err(|_| registration_error("通知批次仍活跃"))?;
        }
    }
    Ok(())
}
use super::*;
impl ParameterRegistration<'_> {
    #[cfg(feature = "rc")]
    pub(super) fn command_sender_inner<C: Send + 'static>(
        &mut self,
    ) -> Result<crate::command::CommandSender<C>, EcsError> {
        check_scope(self.world, true)?;
        if !matches!(self.mode, RegistrationMode::Initialize) {
            return Err(registration_error("清偿能力不能创建命令发送端"));
        }
        self.world.commands.sender::<C>(self.world.epoch)
    }
    #[cfg(feature = "rc")]
    pub(super) fn command_receiver_inner<C: Send + 'static>(
        &mut self,
    ) -> Result<crate::command::CommandReceiver<C>, EcsError> {
        check_scope(self.world, true)?;
        if !matches!(self.mode, RegistrationMode::Initialize) {
            return Err(registration_error("清偿能力不能领取命令消费资格"));
        }
        self.world.commands.receiver::<C>(self.world.epoch)
    }
    pub(super) fn world_id_inner(&self) -> u64 {
        self.world.identity
    }
    pub(super) fn component_id_inner<T: crate::component::Component>(&self) -> Option<ComponentId> {
        self.world
            .registry
            .component_ids
            .get(&std::any::TypeId::of::<T>())
            .copied()
    }
    pub(super) fn resource_id_inner<T: Send + Sync + 'static>(&self) -> Option<ResourceId> {
        #[cfg(feature = "rc")]
        {
            self.world
                .resources
                .ids
                .get(&std::any::TypeId::of::<T>())
                .copied()
        }
        #[cfg(not(feature = "rc"))]
        {
            // 非 RC 尚无资源类型目录，因而不存在可解析的登记。
            None
        }
    }
    pub(super) fn multi_resource_id_inner<T: Send + Sync + 'static>(&self) -> Option<ResourceId> {
        #[cfg(feature = "rc")]
        {
            self.world
                .multi_resources
                .ids
                .get(&std::any::TypeId::of::<T>())
                .copied()
        }
        #[cfg(not(feature = "rc"))]
        todo!("编号：T66，尚未实现， 非 RC 多例类型解析")
    }
    #[cfg(feature = "rc")]
    pub(super) fn query_state_inner<Q: QueryData, F: QueryFilter>(
        &mut self,
    ) -> Result<QueryState<Q, F>, EcsError> {
        check_scope(self.world, true)?;
        if !matches!(self.mode, RegistrationMode::Initialize) {
            return Err(registration_error("清偿能力不能登记查询"));
        }
        QueryState::register_in_scope(self.world)
    }
    #[cfg(feature = "rc")]
    pub(super) fn spawn_state_inner<B: crate::bundle::SystemBundle<Shape = B> + 'static>(
        &mut self,
        partition: crate::entity::PartitionId,
    ) -> Result<crate::spawn::SpawnState<B>, EcsError> {
        use crate::error::{classified_error, ErrorKind};
        check_scope(self.world, true)?;
        if !matches!(self.mode, RegistrationMode::Initialize) {
            return Err(registration_error("清偿能力不能登记 Spawn"));
        }
        if partition.0 as usize >= self.world.partition_count {
            return Err(classified_error(ErrorKind::InvalidInput, "Spawn 分区无效"));
        }
        let ordered_components = B::component_types()
            .into_iter()
            .map(|ty| {
                self.world
                    .registry
                    .component_ids
                    .get(&ty)
                    .copied()
                    .ok_or_else(|| classified_error(ErrorKind::NotFound, "Spawn 组件未登记"))
            })
            .collect::<Result<Box<[_]>, _>>()?;
        // Validate the frozen target without publishing a variant.
        self.world.registry.plan_archetype(&ordered_components)?;
        Ok(crate::spawn::SpawnState {
            world_id: self.world.identity,
            partition,
            ordered_components,
            shape: std::marker::PhantomData,
        })
    }
    #[cfg(feature = "rc")]
    pub(super) fn typed_query_alter_state_inner<Q, F, Add, Remove>(
        &mut self,
    ) -> Result<crate::alter::QueryAlterState<Q, F>, EcsError>
    where
        Q: QueryData,
        F: QueryFilter,
        Add: crate::bundle::SystemBundle<Shape = Add> + 'static,
        Remove: crate::bundle::Bundle,
    {
        check_scope(self.world, true)?;
        if !matches!(self.mode, RegistrationMode::Initialize) {
            return Err(registration_error("清偿能力不能登记 QueryAlter"));
        }
        let resolve = |types: Vec<std::any::TypeId>| -> Result<Vec<ComponentId>, EcsError> {
            types
                .into_iter()
                .map(|ty| {
                    self.world
                        .registry
                        .component_ids
                        .get(&ty)
                        .copied()
                        .ok_or_else(|| {
                            crate::error::classified_error(
                                crate::error::ErrorKind::NotFound,
                                "QueryAlter 增删组件未登记",
                            )
                        })
                })
                .collect()
        };
        let added = resolve(Add::component_types())?;
        let removed = resolve(Remove::component_types())?;
        let alteration = crate::alter::AlterState::new(self.world, &added, &removed)?;
        let query = self.query_state()?;
        Ok(crate::alter::QueryAlterState {
            query,
            alteration,
            added_order: added.into_boxed_slice(),
            iteration_order: Box::new([]),
            table_order: None,
        })
    }
    #[cfg(feature = "rc")]
    pub(super) fn unregister_query_alter_inner<Q: QueryData, F: QueryFilter>(
        &mut self,
        state: &mut crate::alter::QueryAlterState<Q, F>,
    ) -> Result<(), EcsError> {
        self.unregister_query_inner(&mut state.query)
    }
    #[cfg(feature = "rc")]
    pub(super) fn unregister_query_inner<Q: QueryData, F: QueryFilter>(
        &mut self,
        _state: &mut QueryState<Q, F>,
    ) -> Result<(), EcsError> {
        check_scope(
            self.world,
            matches!(self.mode, RegistrationMode::Initialize),
        )?;
        QueryState::unregister_in_scope(_state, self.world)
    }
    #[cfg(feature = "rc")]
    pub(super) fn subscribe_notice_inner(
        &mut self,
        _component: ComponentId,
        _kind: NoticeKind,
    ) -> Result<NoticeSubscription, EcsError> {
        check_scope(self.world, true)?;
        if !matches!(self.mode, RegistrationMode::Initialize) {
            return Err(registration_error("清偿能力不能新增通知订阅"));
        }
        self.world.notifications.subscribe(_component, _kind)
    }
    #[cfg(feature = "rc")]
    pub(super) fn unsubscribe_notice_inner(
        &mut self,
        _subscription: NoticeSubscription,
    ) -> Result<(), EcsError> {
        check_scope(
            self.world,
            matches!(self.mode, RegistrationMode::Initialize),
        )?;
        self.world.notifications.unsubscribe(_subscription)
    }
    #[cfg(feature = "rc")]
    pub(super) fn register_producer_inner<T: Send + Sync + 'static>(
        &mut self,
        initial: T,
    ) -> Result<ProducerRegistration, EcsError> {
        check_scope(self.world, true)?;
        if !matches!(self.mode, RegistrationMode::Initialize) {
            return Err(registration_error("清偿能力不能新增生产者"));
        }
        let resource = self
            .world
            .multi_resources
            .ids
            .get(&std::any::TypeId::of::<T>())
            .copied()
            .ok_or_else(|| registration_error("多例类型未登记"))?;
        let core = self.world.multi_resources.entries[resource.0 as usize]
            .downcast_mut::<crate::resource::MultiResourceCore<T>>()
            .ok_or_else(|| registration_error("多例类型不符"))?;
        let next = core.next_producer.checked_add(1).ok_or_else(|| {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "生产者身份耗尽",
            )
        })?;
        let version = core.structure_version.checked_add(1).ok_or_else(|| {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "生产者结构版本耗尽",
            )
        })?;
        let tick = self.world.change_tick.checked_add(1).ok_or_else(|| {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "变化时钟耗尽",
            )
        })?;
        let tick_value = crate::change_detection::ChangeTick::new(
            std::num::NonZeroU64::new(tick).expect("非零时钟"),
        );
        let producer = crate::resource::ProducerId(core.next_producer);
        // 维护作用域排除所有值/tick 借用；缓存根不是拥有者，重定位后一起发布。
        let values = core.values.get_mut();
        let ticks = core.ticks.get_mut();
        values.reserve(1);
        ticks.reserve(1);
        core.producers.reserve(1);
        core.producer_offsets.reserve(1);
        core.values_root = std::ptr::NonNull::new(values.as_mut_ptr()).expect("Vec 根非空");
        core.ticks_root = std::ptr::NonNull::new(ticks.as_mut_ptr()).expect("Vec 根非空");
        let offset = core.producers.len();
        values.push(initial);
        ticks.push(crate::resource::ResourceTicks {
            added: tick_value,
            changed: tick_value,
        });
        core.producers.push(producer);
        core.producer_offsets.insert(producer, offset);
        core.next_producer = next;
        core.structure_version = version;
        self.world.change_tick = tick;
        Ok(ProducerRegistration {
            world_identity: self.world.identity,
            resource,
            producer: producer.0,
        })
    }
    #[cfg(feature = "rc")]
    pub(super) fn unregister_producer_inner<T: Send + Sync + 'static>(
        &mut self,
        registration: ProducerRegistration,
    ) -> Result<(), EcsError> {
        check_scope(
            self.world,
            matches!(self.mode, RegistrationMode::Initialize),
        )?;
        if registration.world_identity != self.world.identity {
            return Err(registration_error("生产者来自其他 World"));
        }
        let resource = self
            .world
            .multi_resources
            .ids
            .get(&std::any::TypeId::of::<T>())
            .copied()
            .ok_or_else(|| registration_error("多例类型未登记"))?;
        if resource != registration.resource {
            return Err(registration_error("生产者类型不符"));
        }
        let core = self.world.multi_resources.entries[resource.0 as usize]
            .downcast_mut::<crate::resource::MultiResourceCore<T>>()
            .ok_or_else(|| registration_error("多例类型不符"))?;
        let producer = crate::resource::ProducerId(registration.producer);
        let offset = *core
            .producer_offsets
            .get(&producer)
            .ok_or_else(|| registration_error("生产者已注销"))?;
        // 耗尽属于内部不可继续维护状态；仍消费本次值责任，不复用版本。
        let version = core.structure_version.checked_add(1);
        core.producer_offsets.remove(&producer);
        core.producers.swap_remove(offset);
        if let Some(&moved) = core.producers.get(offset) {
            core.producer_offsets.insert(moved, offset);
        }
        let removed = core.values.get_mut().swap_remove(offset);
        core.ticks.get_mut().swap_remove(offset);
        core.values_root =
            std::ptr::NonNull::new(core.values.get_mut().as_mut_ptr()).expect("Vec 根非空");
        core.ticks_root =
            std::ptr::NonNull::new(core.ticks.get_mut().as_mut_ptr()).expect("Vec 根非空");
        if let Some(version) = version {
            core.structure_version = version;
        } else {
            self.world
                .access
                .mode
                .set(crate::world::AccessMode::Stopped);
        }
        // 槽和映射已撤销责任，析构在 core 的独占借用结束后执行；外层作用域负责 panic 标停。
        drop(removed);
        if version.is_none() {
            return Err(registration_error("生产者结构版本耗尽，已停止"));
        }
        Ok(())
    }
}
#[cfg(feature = "rc")]
impl World {
    pub(super) fn parameter_registration_scope_inner<R>(
        &mut self,
        initialize: impl for<'a> FnOnce(ParameterRegistration<'a>) -> Result<R, EcsError>,
    ) -> Result<R, EcsError> {
        check_scope(self, true)?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            initialize(ParameterRegistration {
                world: self,
                mode: RegistrationMode::Initialize,
            })
        }));
        match result {
            Ok(result) => result,
            Err(payload) => {
                self.access.mode.set(crate::world::AccessMode::Stopped);
                std::panic::resume_unwind(payload)
            }
        }
    }
    pub(super) fn parameter_unregistration_scope_inner<R>(
        &mut self,
        unregister: impl for<'a> FnOnce(ParameterRegistration<'a>) -> Result<R, EcsError>,
    ) -> Result<R, EcsError> {
        check_scope(self, false)?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unregister(ParameterRegistration {
                world: self,
                mode: RegistrationMode::Unregister,
            })
        }));
        match result {
            Ok(result) => result,
            Err(payload) => {
                self.access.mode.set(crate::world::AccessMode::Stopped);
                std::panic::resume_unwind(payload)
            }
        }
    }
}
