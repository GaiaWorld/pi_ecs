//! 通知参数配对芯；rc 读取复用通知管理器，clear 仍待独立游标提交入口。
use crate::{
    param_composition::ParamAccess, param_state::ReaderParamState, system::SystemRunContext,
    system_param::ParamRegistration,
};
use pi_ecs_core::{
    access::ResolvedAccess,
    error::EcsError,
    notification::{NoticeBatch, NoticeKind, RemovedBatch},
    world::WorldRef,
};
impl<T: 'static> crate::notification_reader::AddedReader<'_, T> {
    pub(crate) fn added_empty_inner(
        reader: &crate::notification_reader::AddedReader<'_, T>,
    ) -> Result<bool, EcsError> {
        reader.notifications.is_empty(reader.subscription)
    }
}
impl<T: 'static> crate::notification_reader::AddedReader<'_, T> {
    pub(crate) fn added_clear_inner(
        reader: &mut crate::notification_reader::AddedReader<'_, T>,
    ) -> Result<(), EcsError> {
        let mut batch = reader.notifications.read(reader.subscription)?;
        while batch.next().is_some() {}
        Ok(())
    }
}
impl<T: 'static> crate::notification_reader::ChangedReader<'_, T> {
    pub(crate) fn changed_empty_inner(
        reader: &crate::notification_reader::ChangedReader<'_, T>,
    ) -> Result<bool, EcsError> {
        reader.notifications.is_empty(reader.subscription)
    }
}
impl<T: 'static> crate::notification_reader::ChangedReader<'_, T> {
    pub(crate) fn changed_clear_inner(
        reader: &mut crate::notification_reader::ChangedReader<'_, T>,
    ) -> Result<(), EcsError> {
        let mut batch = reader.notifications.read(reader.subscription)?;
        while batch.next().is_some() {}
        Ok(())
    }
}
impl<T: pi_ecs_core::component::Component> crate::notification_reader::RemovedReader<'_, T> {
    pub(crate) fn removed_empty_inner(
        reader: &crate::notification_reader::RemovedReader<'_, T>,
    ) -> Result<bool, EcsError> {
        reader.notifications.is_empty(reader.subscription)
    }
}
#[cfg(feature = "rc")]
impl<T: pi_ecs_core::component::Component> crate::notification_reader::RemovedReader<'_, T> {
    pub(crate) fn removed_clear_inner(
        reader: &mut crate::notification_reader::RemovedReader<'_, T>,
    ) -> Result<(), EcsError> {
        // SAFETY: 本参数的观察授权排除通知回收和旧值冲突访问。
        let mut batch: RemovedBatch<'_, T> =
            unsafe { reader.world.read_removed(reader.subscription)? };
        while batch.next().is_some() {}
        Ok(())
    }
}
#[cfg(not(feature = "rc"))]
impl<T: pi_ecs_core::component::Component> crate::notification_reader::RemovedReader<'_, T> {
    pub(crate) fn removed_clear_inner(
        _reader: &mut crate::notification_reader::RemovedReader<'_, T>,
    ) -> Result<(), EcsError> {
        todo!("编号：T95，尚未实现， 非 rc RemovedReader clear 尚未实现")
    }
}

impl<T: 'static> crate::notification_reader::AddedReader<'_, T> {
    pub(crate) fn added_read_inner<'a>(
        reader: &'a mut crate::notification_reader::AddedReader<'_, T>,
    ) -> Result<NoticeBatch<'a>, EcsError> {
        reader.notifications.read(reader.subscription)
    }
}
impl<T: 'static> crate::notification_reader::ChangedReader<'_, T> {
    pub(crate) fn changed_read_inner<'a>(
        reader: &'a mut crate::notification_reader::ChangedReader<'_, T>,
    ) -> Result<NoticeBatch<'a>, EcsError> {
        reader.notifications.read(reader.subscription)
    }
}
#[cfg(feature = "rc")]
impl<T: pi_ecs_core::component::Component> crate::notification_reader::RemovedReader<'_, T> {
    pub(crate) fn removed_read_inner<'a>(
        reader: &'a mut crate::notification_reader::RemovedReader<'_, T>,
    ) -> Result<RemovedBatch<'a, T>, EcsError> {
        // SAFETY: 参数观察权限覆盖通知与旧值读取，返回借用绑定 reader。
        unsafe { reader.world.read_removed(reader.subscription) }
    }
}
#[cfg(not(feature = "rc"))]
impl<T: pi_ecs_core::component::Component> crate::notification_reader::RemovedReader<'_, T> {
    pub(crate) fn removed_read_inner<'a>(
        _reader: &'a mut crate::notification_reader::RemovedReader<'_, T>,
    ) -> Result<RemovedBatch<'a, T>, EcsError> {
        todo!("编号：T96，尚未实现， 非 rc RemovedReader 读取尚未实现")
    }
}

fn reader_prepare(state: &ReaderParamState) -> ParamAccess {
    ParamAccess::Leaf(vec![ResolvedAccess {
        resources: Box::new([]),
        multi_resources: Box::new([]),
        observations: Box::new([(state.component, state.kind)]),
        values: Box::new([]),
        // 流观察不读取当前活值 tick，不能以订阅需求伪造 tick 读能力。
        tick_reads: Box::new([]),
        entities: Box::new([]),
        structure: None,
    }])
}

#[cfg(feature = "rc")]
impl ReaderParamState {
    pub(crate) fn added_param_initialize<T: pi_ecs_core::component::Component>(
        registration: &mut ParamRegistration<'_>,
    ) -> Result<ReaderParamState, EcsError> {
        let world = registration.access();
        let component = world.component_id::<T>().ok_or_else(|| {
            pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::NotFound,
                "AddedReader 组件类型未登记",
            )
        })?;
        let subscription = world.subscribe_notice(component, NoticeKind::Added)?;
        Ok(ReaderParamState {
            component,
            kind: NoticeKind::Added,
            subscription,
        })
    }
}

impl ReaderParamState {
    pub(crate) fn added_param_prepare(
        state: &mut ReaderParamState,
        _world: WorldRef<'_>,
    ) -> Result<ParamAccess, EcsError> {
        Ok(reader_prepare(state))
    }
}
impl ReaderParamState {
    pub(crate) fn added_param_begin(_state: &mut ReaderParamState) {}
}

#[cfg(feature = "rc")]
impl ReaderParamState {
    pub(crate) unsafe fn added_param_fetch<'run, T: pi_ecs_core::component::Component>(
        state: &'run mut ReaderParamState,
        _pending: &'run mut (),
        context: SystemRunContext<'run>,
    ) -> crate::notification_reader::AddedReader<'run, T> {
        crate::notification_reader::AddedReader {
            notifications: unsafe { context.access.world_cell().notifications() },
            subscription: &mut state.subscription,
            marker: std::marker::PhantomData,
        }
    }
}

impl ReaderParamState {
    pub(crate) fn added_param_finish(
        _state: &mut ReaderParamState,
        _pending: (),
        _context: &SystemRunContext<'_>,
    ) {
    }
}

#[cfg(feature = "rc")]
impl ReaderParamState {
    pub(crate) fn added_param_unregister<T: pi_ecs_core::component::Component>(
        state: ReaderParamState,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        registration.access().unsubscribe_notice(state.subscription)
    }
}

#[cfg(feature = "rc")]
impl ReaderParamState {
    pub(crate) fn changed_param_initialize<T: pi_ecs_core::component::Component>(
        registration: &mut ParamRegistration<'_>,
    ) -> Result<ReaderParamState, EcsError> {
        let world = registration.access();
        let component = world.component_id::<T>().ok_or_else(|| {
            pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::NotFound,
                "ChangedReader 组件类型未登记",
            )
        })?;
        let subscription = world.subscribe_notice(component, NoticeKind::Changed)?;
        Ok(ReaderParamState {
            component,
            kind: NoticeKind::Changed,
            subscription,
        })
    }
}

impl ReaderParamState {
    pub(crate) fn changed_param_prepare(
        state: &mut ReaderParamState,
        _world: WorldRef<'_>,
    ) -> Result<ParamAccess, EcsError> {
        Ok(reader_prepare(state))
    }
}
impl ReaderParamState {
    pub(crate) fn changed_param_begin(_state: &mut ReaderParamState) {}
}

#[cfg(feature = "rc")]
impl ReaderParamState {
    pub(crate) unsafe fn changed_param_fetch<'run, T: pi_ecs_core::component::Component>(
        state: &'run mut ReaderParamState,
        _pending: &'run mut (),
        context: SystemRunContext<'run>,
    ) -> crate::notification_reader::ChangedReader<'run, T> {
        crate::notification_reader::ChangedReader {
            notifications: unsafe { context.access.world_cell().notifications() },
            subscription: &mut state.subscription,
            marker: std::marker::PhantomData,
        }
    }
}

impl ReaderParamState {
    pub(crate) fn changed_param_finish(
        _state: &mut ReaderParamState,
        _pending: (),
        _context: &SystemRunContext<'_>,
    ) {
    }
}

#[cfg(feature = "rc")]
impl ReaderParamState {
    pub(crate) fn changed_param_unregister<T: pi_ecs_core::component::Component>(
        state: ReaderParamState,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        registration.access().unsubscribe_notice(state.subscription)
    }
}

#[cfg(feature = "rc")]
impl ReaderParamState {
    pub(crate) fn removed_param_initialize<T: pi_ecs_core::component::Component>(
        registration: &mut ParamRegistration<'_>,
    ) -> Result<ReaderParamState, EcsError> {
        let world = registration.access();
        let component = world.component_id::<T>().ok_or_else(|| {
            pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::NotFound,
                "RemovedReader 组件类型未登记",
            )
        })?;
        let subscription = world.subscribe_notice(component, NoticeKind::Removed)?;
        Ok(ReaderParamState {
            component,
            kind: NoticeKind::Removed,
            subscription,
        })
    }
}

impl ReaderParamState {
    pub(crate) fn removed_param_prepare<T: pi_ecs_core::component::Component>(
        state: &mut ReaderParamState,
        _world: WorldRef<'_>,
    ) -> Result<ParamAccess, EcsError> {
        Ok(ParamAccess::Leaf(vec![ResolvedAccess {
            resources: Box::new([]),
            multi_resources: Box::new([]),
            observations: Box::new([(state.component, NoticeKind::Removed)]),
            values: Box::new([]),
            // 流观察不读取当前活值 tick，不能以订阅需求伪造 tick 读能力。
            tick_reads: Box::new([]),
            entities: Box::new([]),
            structure: None,
        }]))
    }
}

impl ReaderParamState {
    pub(crate) fn removed_param_begin<T: pi_ecs_core::component::Component>(
        _state: &mut ReaderParamState,
    ) {
    }
}

#[cfg(feature = "rc")]
impl ReaderParamState {
    pub(crate) unsafe fn removed_param_fetch<'run, T: pi_ecs_core::component::Component>(
        state: &'run mut ReaderParamState,
        _pending: &'run mut (),
        context: SystemRunContext<'run>,
    ) -> crate::notification_reader::RemovedReader<'run, T> {
        let world = context.access.world_cell();
        crate::notification_reader::RemovedReader {
            notifications: unsafe { world.notifications() },
            subscription: &mut state.subscription,
            world,
            marker: std::marker::PhantomData,
        }
    }
}

impl ReaderParamState {
    pub(crate) fn removed_param_finish<T: pi_ecs_core::component::Component>(
        _state: &mut ReaderParamState,
        _pending: (),
        _context: &SystemRunContext<'_>,
    ) {
    }
}

#[cfg(feature = "rc")]
impl ReaderParamState {
    pub(crate) fn removed_param_unregister<T: pi_ecs_core::component::Component>(
        state: ReaderParamState,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        registration.access().unsubscribe_notice(state.subscription)
    }
}
