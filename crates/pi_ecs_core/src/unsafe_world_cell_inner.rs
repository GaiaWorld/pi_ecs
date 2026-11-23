//! 统一句柄与运行守卫配对芯。
#[cfg(all(test, feature = "rc"))]
mod rc_system_session_tests {
    use crate::{access::AccessTree, unsafe_world_cell::UnsafeWorldCell, world::World};

    // Seam: 真实系统授权派生通知目录，读取只推进对应订阅。
    #[test]
    fn rc_system_notifications_preserve_source_and_reader_progress() {
        use crate::{
            component::{Component, PlainAccess, StoragePolicy},
            notification::NoticeKind,
        };
        struct Value;
        impl Component for Value {
            type Access = PlainAccess;
        }
        let mut world = World::new().unwrap();
        let id = world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        let mut reader = world
            .notifications
            .subscribe(id, NoticeKind::Changed)
            .unwrap();
        world.finish_registration().unwrap();
        world.notifications.append_change(
            id,
            NoticeKind::Changed,
            Default::default(),
            world.read_change_tick(),
        );
        let declaration = crate::access::AccessTree::Leaf(crate::access::AccessDeclaration {
            resources: Box::new([]),
            multi_resources: Box::new([]),
            components: Box::new([]),
            tick_reads: Box::new([]),
            reads_entities: false,
            scope: crate::access::EntityScope {
                with: Box::new([]),
                without: Box::new([]),
            },
            structure: crate::access::StructuralAccess::None,
            observations: vec![(id, NoticeKind::Changed)].into_boxed_slice(),
        });
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                // SAFETY: 同源订阅且唯一观察叶子；无通知写入或回收，结果留在系统内。
                unsafe {
                    world.system_scope(&declaration, version, |access| {
                        let cell = access.world_cell();
                        let notifications = cell.notifications();
                        let mut batch = notifications.read(&mut reader).unwrap();
                        assert_eq!(batch.next().unwrap().sequence.0, 0);
                        assert!(batch.next().is_none());
                    })?;
                }
                Ok(())
            })
            .unwrap();
        assert!(world.notifications.is_empty(&reader).unwrap());
        world.notifications.unsubscribe(reader).unwrap();
        world.close().unwrap();
    }

    // Seam: 系统 Removed 读取校验类型、保留早停游标且批次保护同流旧值。
    #[test]
    fn rc_system_removed_checks_type_and_retains_unread_values() {
        use crate::{
            component::{Component, PlainAccess, StoragePolicy},
            notification::NoticeKind,
        };
        struct Value(u32);
        impl Component for Value {
            type Access = PlainAccess;
        }
        struct Other;
        impl Component for Other {
            type Access = PlainAccess;
        }
        let mut world = World::new().unwrap();
        let id = world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        let mut reader = world
            .notifications
            .subscribe(id, NoticeKind::Removed)
            .unwrap();
        let mut changed = world
            .notifications
            .subscribe(id, NoticeKind::Changed)
            .unwrap();
        world.finish_registration().unwrap();
        for value in [7, 19] {
            let mut value = std::mem::ManuallyDrop::new(Value(value));
            // SAFETY: 旧值唯一责任移入真实通知流，原局部不再析构。
            unsafe {
                world
                    .notifications
                    .append_removed(
                        id,
                        Default::default(),
                        world.read_change_tick(),
                        crate::ptr::OwningPtr::from_raw(
                            std::ptr::NonNull::from(&mut *value).cast(),
                        ),
                    )
                    .unwrap_or_else(|_| panic!("旧值追加失败"));
            }
        }
        let declaration = crate::access::AccessTree::Leaf(crate::access::AccessDeclaration {
            resources: Box::new([]),
            multi_resources: Box::new([]),
            components: Box::new([]),
            tick_reads: Box::new([]),
            reads_entities: false,
            scope: crate::access::EntityScope {
                with: Box::new([]),
                without: Box::new([]),
            },
            structure: crate::access::StructuralAccess::None,
            observations: vec![(id, NoticeKind::Removed), (id, NoticeKind::Changed)]
                .into_boxed_slice(),
        });
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                // SAFETY: 同源读者声明覆盖 Removed，无活组件或同流修改权限。
                unsafe {
                    world.system_scope(&declaration, version, |access| {
                        let cell = access.world_cell();
                        assert!(cell.read_removed::<Other>(&mut reader).is_err());
                        assert!(cell.read_removed::<Value>(&mut changed).is_err());
                        {
                            let mut batch = cell.read_removed::<Value>(&mut reader).unwrap();
                            let old = batch.next().unwrap();
                            assert_eq!(old.value.0, 7);
                            assert_eq!(old.header.sequence.0, 0);
                            assert!(cell.notifications().components[id.0 as usize]
                                .removed
                                .as_ref()
                                .unwrap()
                                .try_borrow_mut()
                                .is_err());
                        }
                        let mut batch = cell.read_removed::<Value>(&mut reader).unwrap();
                        assert_eq!(batch.next().unwrap().value.0, 19);
                        assert!(batch.next().is_none());
                    })?;
                }
                Ok(())
            })
            .unwrap();
        assert!(world.notifications.is_empty(&reader).unwrap());
        world.notifications.unsubscribe(reader).unwrap();
        world.notifications.unsubscribe(changed).unwrap();
        world.close().unwrap();
    }

    // Seam: 前置失败不进入用户系统；耗尽不回绕且不自行复活停止态。
    #[test]
    fn rc_system_session_rejects_stale_version_and_tick_exhaustion() {
        let mut world = World::new().unwrap();
        world.finish_registration().unwrap();
        let declaration = AccessTree::Together(vec![]);
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                assert!(unsafe {
                    world.system_scope(&declaration, version + 1, |_| panic!("stale"))
                }
                .is_err());
                // 只注入无法通过正常次数到达的时钟上界，授权仍由真实根入口取得。
                world.change_tick = u64::MAX;
                assert!(unsafe { world.next_system_tick() }.is_err());
                assert_eq!(world.read_change_tick().get(), u64::MAX);
                Ok(())
            })
            .unwrap();
        assert!(!world.is_stopped());
        world.close().unwrap();
    }

    // Seam: 真实根调度内分配 tick 并进入系统；正常返回保留 Running，展开后永久标停。
    #[test]
    fn rc_system_session_ticks_authority_and_unwind() {
        let mut world = World::new().unwrap();
        world.finish_registration().unwrap();
        let declaration = AccessTree::Together(vec![]);
        assert!(unsafe { world.next_system_tick() }.is_err());
        assert!(unsafe { world.system_scope(&declaration, 0, |_| ()) }.is_err());
        let before = world.read_change_tick();
        world
            .schedule_scope(|world| {
                let tick = unsafe { world.next_system_tick()? };
                assert_eq!(tick.get(), before.get() + 1);
                let identity = UnsafeWorldCell::from_ref(world).id();
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                unsafe {
                    world.system_scope(&declaration, version, |access| {
                        assert_eq!(access.world_cell().id(), identity);
                    })?;
                }
                assert_eq!(unsafe { world.next_system_tick()? }.get(), tick.get() + 1);
                Ok(())
            })
            .unwrap();
        assert!(!world.is_stopped());
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = world.schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                unsafe { world.system_scope(&declaration, version, |_| panic!("user system")) }
            });
        }))
        .is_err());
        assert!(world.is_stopped());
        world.close().unwrap();
    }
}

#[cfg(feature = "rc")]
impl crate::world::World {
    pub(super) fn schedule_scope_inner<R>(
        &mut self,
        run: impl for<'run> FnOnce(&'run mut crate::world::World) -> Result<R, crate::error::EcsError>,
    ) -> Result<R, crate::error::EcsError> {
        let mode = self.access.mode.replace(crate::world::AccessMode::Stopped);
        let idle = matches!(mode, crate::world::AccessMode::Idle);
        self.access.mode.set(mode);
        if !idle || !self.registry.frozen || self.shutdown != crate::shutdown::ShutdownState::Open {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "调度要求冻结且开放的 Idle World",
            ));
        }
        self.access.mode.set(crate::world::AccessMode::Running);
        let mut session = super::ScheduleSession {
            world: self,
            completed: false,
        };
        let result = run(session.world());
        let completed = session.complete();
        match result {
            Err(error) => Err(error),
            Ok(value) => completed.map(|()| value),
        }
    }
    pub(super) fn is_stopped_inner(&self) -> bool {
        let mode = self.access.mode.replace(crate::world::AccessMode::Stopped);
        let stopped = matches!(mode, crate::world::AccessMode::Stopped);
        self.access.mode.set(mode);
        stopped
    }
    pub(super) fn stop_in_run_inner(&mut self) -> Result<(), crate::error::EcsError> {
        if self.is_stopped() {
            return Ok(());
        }
        let mode = self.access.mode.replace(crate::world::AccessMode::Stopped);
        if matches!(mode, crate::world::AccessMode::Running) {
            return Ok(());
        }
        self.access.mode.set(mode);
        Err(pi_ecs_types::error::classified_error(
            pi_ecs_types::error::ErrorKind::InvalidState,
            "停止执行要求根 Running",
        ))
    }
    pub(super) fn next_system_tick_inner(
        &mut self,
    ) -> Result<crate::change_detection::ChangeTick, crate::error::EcsError> {
        let mode = self.access.mode.replace(crate::world::AccessMode::Stopped);
        let running = matches!(mode, crate::world::AccessMode::Running);
        self.access.mode.set(mode);
        if !running || self.shutdown != crate::shutdown::ShutdownState::Open {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "系统 tick 要求根 Running",
            ));
        }
        let next = self.change_tick.checked_add(1).ok_or_else(|| {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "系统 tick 耗尽",
            )
        })?;
        self.change_tick = next;
        Ok(crate::change_detection::ChangeTick::new(
            std::num::NonZeroU64::new(next).expect("非零系统 tick"),
        ))
    }
    pub(super) fn system_scope_inner<R>(
        &mut self,
        _declaration: &crate::access::AccessTree,
        _version: u64,
        _run: impl for<'run> FnOnce(crate::access::RunAccess<'run>) -> R,
    ) -> Result<R, crate::error::EcsError> {
        let mode = self.access.mode.replace(crate::world::AccessMode::Stopped);
        let running = matches!(mode, crate::world::AccessMode::Running);
        self.access.mode.set(mode);
        if !running
            || self.shutdown != crate::shutdown::ShutdownState::Open
            || !self.registry.frozen
            || _version != self.layout_epoch
        {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "系统会话状态或布局版本无效",
            ));
        }
        let pointer = std::ptr::NonNull::from(&mut *self);
        let mut session = SystemSession {
            access: &self.access,
            completed: false,
        };
        // 来源为根会话的独占借用；守卫仅借独立门禁字段，不还原 World 引用。
        let access = crate::access::RunAccess {
            world: UnsafeWorldCell {
                pointer,
                mutable: true,
                lifetime: std::marker::PhantomData,
            },
            declaration: _declaration,
            archetype_version: _version,
        };
        let result = _run(access);
        session.complete()?;
        Ok(result)
    }
}
use super::{SystemSession, UnsafeWorldCell};
use crate::{error::EcsError, world::World};
impl UnsafeWorldCell<'_> {
    pub(super) fn from_mut_inner(world: &mut World) -> UnsafeWorldCell<'_> {
        UnsafeWorldCell {
            pointer: std::ptr::NonNull::from(world),
            mutable: true,
            lifetime: std::marker::PhantomData,
        }
    }
}
impl UnsafeWorldCell<'_> {
    pub(super) fn from_ref_inner(world: &World) -> UnsafeWorldCell<'_> {
        UnsafeWorldCell {
            pointer: std::ptr::NonNull::from(world),
            mutable: false,
            lifetime: std::marker::PhantomData,
        }
    }
}
impl<'w> UnsafeWorldCell<'w> {
    pub(super) fn archetype_revision_inner(&self) -> (u64, usize) {
        // SAFETY: 来源 World 保活；只读取布局元数据，不还原整份 World 引用。
        unsafe {
            let pointer = self.pointer.as_ptr();
            (
                (*pointer).layout_epoch,
                (&*std::ptr::addr_of!((*pointer).registry.archetypes)).len(),
            )
        }
    }
    pub(super) fn id_inner(&self) -> u64 {
        // SAFETY: 身份在来源借用期间固定且 World 保活。
        unsafe { std::ptr::addr_of!((*self.pointer.as_ptr()).identity).read() }
    }
    pub(super) fn reborrow_inner(&self) -> UnsafeWorldCell<'_> {
        UnsafeWorldCell {
            pointer: self.pointer,
            mutable: self.mutable,
            lifetime: std::marker::PhantomData,
        }
    }
    pub(super) fn notifications_inner(&self) -> &'w crate::notification::Notifications {
        // SAFETY: 来源 World 保活到 'w；只投影冻结通知目录，不形成整份 World 引用。
        // 流的共享守卫由读取批次持有，调用方证明无冲突修改或回收。
        unsafe { &*std::ptr::addr_of!((*self.pointer.as_ptr()).notifications) }
    }
}
impl SystemSession<'_> {
    pub(super) fn complete_inner(&mut self) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        {
            let mode = self.access.mode.replace(crate::world::AccessMode::Stopped);
            if matches!(mode, crate::world::AccessMode::Running) {
                self.access.mode.set(mode);
                self.completed = true;
                Ok(())
            } else {
                Err(pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::InvalidState,
                    "系统会话已停止",
                ))
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T76，尚未实现， 非 RC 系统完成检查延期")
        }
    }
    pub(super) fn drop_inner(&mut self) {
        if !self.completed {
            #[cfg(feature = "rc")]
            self.access.mode.set(crate::world::AccessMode::Stopped);
            #[cfg(not(feature = "rc"))]
            todo!("编号：T77，尚未实现， 非 RC 失败标停守卫延期");
        }
    }
}

impl UnsafeWorldCell<'_> {
    pub(super) fn read_removed_inner<'a, T: crate::component::Component>(
        &'a self,
        _reader: &'a mut crate::notification::NoticeSubscription,
    ) -> Result<crate::notification::RemovedBatch<'a, T>, EcsError> {
        #[cfg(feature = "rc")]
        {
            let invalid = |message| {
                pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::InvalidState,
                    message,
                )
            };
            let notifications = self.notifications_inner();
            let state = notifications
                .subscriptions
                .get(_reader.id)
                .ok_or_else(|| invalid("通知订阅已失效"))?;
            let index = state.stream.0 .0 as usize;
            if state.stream.1 != crate::notification::NoticeKind::Removed
                || notifications.type_ids.get(index).copied().flatten()
                    != Some(std::any::TypeId::of::<T>())
            {
                return Err(pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::InvalidInput,
                    "Removed 订阅类型不匹配",
                ));
            }
            let stream = notifications
                .components
                .get(index)
                .and_then(|streams| streams.removed.as_ref())
                .ok_or_else(|| invalid("Removed 订阅流缺失"))?
                .try_borrow()
                .map_err(|_| invalid("通知流借用冲突"))?;
            Ok(crate::notification::RemovedBatch {
                notifications,
                reader: _reader,
                state,
                end: stream.notices.range.end,
                stream,
                world: self.reborrow(),
                marker: std::marker::PhantomData,
            })
        }
        #[cfg(not(feature = "rc"))]
        {
            // 非 RC 后端延期；该入口不伪造流守卫或还原 World。
            Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "非 RC Removed 后端尚未实现",
            ))
        }
    }
}

#[cfg(feature = "rc")]
impl crate::unsafe_world_cell::ScheduleSession<'_> {
    pub(super) fn world_inner(&mut self) -> &mut crate::world::World {
        &mut *self.world
    }
    pub(super) fn complete_inner(&mut self) -> Result<(), crate::error::EcsError> {
        let mode = self
            .world
            .access
            .mode
            .replace(crate::world::AccessMode::Stopped);
        self.completed = true;
        if matches!(mode, crate::world::AccessMode::Running) {
            self.world.access.mode.set(crate::world::AccessMode::Idle);
            Ok(())
        } else {
            Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "根调度已停止或运行状态异常",
            ))
        }
    }
    pub(super) fn drop_inner(&mut self) {
        if !self.completed {
            self.world
                .access
                .mode
                .set(crate::world::AccessMode::Stopped);
        }
    }
}
