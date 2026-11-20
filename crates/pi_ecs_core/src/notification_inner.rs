//! 每流内部可变通知芯；RC 旧值责任由调用方在静止点统一释放。
#[cfg(all(test, feature = "rc"))]
mod removed_lifecycle_batch {
    use super::*;
    use std::{alloc::Layout, any::TypeId, mem::ManuallyDrop, ptr::NonNull};

    struct EmptyWorld(ManuallyDrop<crate::world::World>);
    impl EmptyWorld {
        fn new() -> Self {
            Self(ManuallyDrop::new(crate::world::World::new().unwrap()))
        }
    }
    impl Drop for EmptyWorld {
        fn drop(&mut self) {
            let world = &mut *self.0;
            macro_rules! clear { ($($field:ident),*) => { $(unsafe { std::ptr::drop_in_place(&mut world.$field) };)* }; }
            // 空世界的全部合法字段逐一清理，避免尚未实现的 World::drop。
            clear!(
                tracking,
                identity,
                entities,
                commands,
                epoch,
                simulation_tick,
                resources,
                query_defaults,
                multi_resources,
                pending_destruction,
                entity_nodes,
                partition_count,
                change_tick,
                notifications,
                layout_epoch,
                registry,
                storage,
                access,
                shutdown
            );
        }
    }
    struct Counted(u64, std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl crate::component::Component for Counted {
        type Access = crate::component::PlainAccess;
    }
    impl Drop for Counted {
        fn drop(&mut self) {
            self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    // Seam: 真实登记、Notifications::read_removed、release_value。
    #[test]
    fn removed_lifecycle_batch_plain_reads_and_single_release() {
        let mut world = EmptyWorld::new();
        let id = world
            .0
            .registry
            .register_component::<Counted>(crate::component::StoragePolicy::SparseOnly)
            .unwrap();
        let info = world.0.registry.component(id).unwrap();
        let mut notices = Notifications {
            subscriptions: pi_slotmap::SlotMap::with_key(),
            components: Vec::new(),
            layouts: vec![Some(info.layout)],
            type_ids: vec![Some(info.type_id)],
        };
        let mut reader = notices.subscribe(id, NoticeKind::Removed).unwrap();
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut source = ManuallyDrop::new(Counted(83, count.clone()));
        assert!(notices
            .append_removed(id, Default::default(), tick(), unsafe {
                crate::ptr::OwningPtr::from_raw(NonNull::from(&mut *source).cast())
            })
            .is_ok());
        assert!(notices.read(&mut reader).is_err());
        assert!(!notices.is_empty(&reader).unwrap());
        {
            let mut batch = notices
                .read_removed::<Counted>(&world.0, &mut reader)
                .unwrap();
            let old = batch.next().unwrap();
            assert_eq!(old.value.0, 83);
            assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
            assert!(batch.next().is_none());
        }
        assert!(notices.is_empty(&reader).unwrap());
        let released = unsafe { notices.reclaim_removed_raw() };
        let context = crate::destruction::ReleaseContext {
            world: crate::unsafe_world_cell::UnsafeWorldCell::from_ref(&world.0),
            mode: crate::destruction::ReleaseMode::Normal,
        };
        for (component, pointer) in released {
            unsafe {
                crate::destruction::release_value(
                    &context,
                    world.0.registry.component(component).unwrap(),
                    crate::ptr::OwningPtr::from_raw(NonNull::new(pointer).unwrap()),
                )
            };
        }
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
        notices.unsubscribe(reader).unwrap();
        drop(notices);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn removed_lifecycle_batch_changed_early_stop_and_stream_independence() {
        let mut notices = notifications();
        let mut reader = notices
            .subscribe(ComponentId(0), NoticeKind::Changed)
            .unwrap();
        let added = notices
            .subscribe(ComponentId(0), NoticeKind::Added)
            .unwrap();
        for _ in 0..2 {
            notices.append_change(
                ComponentId(0),
                NoticeKind::Changed,
                Default::default(),
                tick(),
            );
        }
        {
            let mut batch = notices.read(&mut reader).unwrap();
            assert_eq!(batch.next().unwrap().sequence.0, 0);
            notices.append_change(
                ComponentId(0),
                NoticeKind::Added,
                Default::default(),
                tick(),
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| notices.append_change(
                    ComponentId(0),
                    NoticeKind::Changed,
                    Default::default(),
                    tick()
                )))
                .is_err()
            );
        }
        notices.reclaim();
        assert_eq!(
            notices
                .read(&mut reader)
                .unwrap()
                .next()
                .unwrap()
                .sequence
                .0,
            1
        );
        notices.reclaim();
        assert!(notices.is_empty(&reader).unwrap());
        notices.unsubscribe(reader).unwrap();
        notices.unsubscribe(added).unwrap();
        notices.reclaim();
    }

    #[test]
    fn removed_lifecycle_batch_container_two_readers_preserve_nodes_and_counts() {
        use crate::{component::*, entity_container::*, unsafe_world_cell::UnsafeWorldCell};
        let mut world = EmptyWorld::new();
        let id = world
            .0
            .registry
            .register_component::<EntityList<String>>(StoragePolicy::SparseOnly)
            .unwrap();
        let info = world.0.registry.component(id).unwrap();
        let mut notices = Notifications {
            subscriptions: pi_slotmap::SlotMap::with_key(),
            components: Vec::new(),
            layouts: vec![Some(info.layout)],
            type_ids: vec![Some(info.type_id)],
        };
        let mut first = notices.subscribe(id, NoticeKind::Removed).unwrap();
        let mut slow = notices.subscribe(id, NoticeKind::Removed).unwrap();
        let entity = world
            .0
            .entities
            .borrow_mut()
            .allocate(
                crate::entity::AllocationDomain(0),
                crate::entity::EntityLocation::from_parts(0, 0, 0, 0, 0),
            )
            .unwrap();
        let owner = crate::owned_entity::OwnedEntity {
            world_identity: world.0.identity,
            entity,
            entities: &world.0.entities,
            pending_destruction: &world.0.pending_destruction,
            access: &world.0.access,
        };
        let mut source = ManuallyDrop::new(EntityList::<String> {
            head: None,
            tail: None,
            len: 0,
            marker: std::marker::PhantomData,
        });
        let mut ticks = crate::resource::ResourceTicks {
            added: tick(),
            changed: tick(),
        };
        let cell = UnsafeWorldCell::from_ref(&world.0);
        unsafe {
            <ListAccess as ComponentPolicy<EntityList<String>>>::write(
                &mut source,
                cell,
                crate::change_detection::ChangeMarker {
                    tick: tick(),
                    target: crate::change_detection::ChangeTarget::Resource(&mut ticks),
                },
            )
        }
        .push_front(owner)
        .ok()
        .unwrap();
        assert!(notices
            .append_removed(id, entity, tick(), unsafe {
                crate::ptr::OwningPtr::from_raw(NonNull::from(&mut *source).cast())
            })
            .is_ok());
        assert!(notices
            .read_removed::<Counted>(&world.0, &mut first)
            .is_err());
        {
            let mut a = notices
                .read_removed::<EntityList<String>>(&world.0, &mut first)
                .unwrap();
            let old = a.next().unwrap();
            assert_eq!(old.value.iter().unwrap().collect::<Vec<_>>(), vec![entity]);
            let mut b = notices
                .read_removed::<EntityList<String>>(&world.0, &mut slow)
                .unwrap();
            let other = b.next().unwrap();
            assert_eq!(other.value.len(), 1);
            assert_eq!(world.0.entities.borrow().counts(entity), Some((1, 0)));
            // 两个批次共享同一旧头；不制造无有效责任的 OwningPtr 来测试冲突。
        }
        let released = unsafe { notices.reclaim_removed_raw() };
        assert_eq!(released.len(), 1);
        for (id, pointer) in released {
            unsafe {
                crate::destruction::release_value(
                    &crate::destruction::ReleaseContext {
                        world: cell,
                        mode: crate::destruction::ReleaseMode::Normal,
                    },
                    world.0.registry.component(id).unwrap(),
                    crate::ptr::OwningPtr::from_raw(NonNull::new(pointer).unwrap()),
                )
            };
        }
        assert_eq!(world.0.entities.borrow().counts(entity), Some((0, 0)));
        assert_eq!(world.0.pending_destruction.pending.len(), 1);
        assert_eq!(world.0.entity_nodes.links.borrow().slots.len(), 0);
        assert!(unsafe { notices.reclaim_removed_raw() }.is_empty());
        let stale = NoticeSubscription { id: first.id };
        notices.unsubscribe(first).unwrap();
        let renewed = notices.subscribe(id, NoticeKind::Removed).unwrap();
        assert!(notices.is_empty(&stale).is_err());
        assert!(notices.unsubscribe(stale).is_err());
        notices.unsubscribe(renewed).unwrap();
        notices.unsubscribe(slow).unwrap();
    }

    struct PanicValue {
        number: u64,
        fail: bool,
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }
    impl crate::component::Component for PanicValue {
        type Access = crate::component::PlainAccess;
    }
    impl Drop for PanicValue {
        fn drop(&mut self) {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.fail {
                panic!("业务析构失败");
            }
        }
    }
    #[test]
    fn removed_lifecycle_batch_callback_panic_does_not_reinstall_or_free_sources() {
        let mut world = EmptyWorld::new();
        let id = world
            .0
            .registry
            .register_component::<PanicValue>(crate::component::StoragePolicy::SparseOnly)
            .unwrap();
        let info = world.0.registry.component(id).unwrap();
        let mut notices = Notifications {
            subscriptions: pi_slotmap::SlotMap::with_key(),
            components: Vec::new(),
            layouts: vec![Some(info.layout)],
            type_ids: vec![Some(info.type_id)],
        };
        let reader = notices.subscribe(id, NoticeKind::Removed).unwrap();
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for (number, fail) in [(47, true), (59, false)] {
            let mut source = ManuallyDrop::new(PanicValue {
                number,
                fail,
                calls: calls.clone(),
            });
            assert!(notices
                .append_removed(id, Default::default(), tick(), unsafe {
                    crate::ptr::OwningPtr::from_raw(NonNull::from(&mut *source).cast())
                })
                .is_ok());
        }
        notices.unsubscribe(reader).unwrap();
        let released = unsafe { notices.reclaim_removed_raw() };
        let context = crate::destruction::ReleaseContext {
            world: crate::unsafe_world_cell::UnsafeWorldCell::from_ref(&world.0),
            mode: crate::destruction::ReleaseMode::Normal,
        };
        let release = |pointer: *mut u8| unsafe {
            crate::destruction::release_value(
                &context,
                info,
                crate::ptr::OwningPtr::from_raw(NonNull::new(pointer).unwrap()),
            )
        };
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_eq!(unsafe { (*released[1].1.cast::<PanicValue>()).number }, 59);
            release(released[0].1);
        }));
        assert!(failed.is_err());
        // 本 seam 不拥有运行授权；停止标记与未交付责任清理由上层收束入口负责。
        // 测试明确清偿尚未交付的第二份，绝不重试已经调用过的第一份。
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(unsafe { (*released[1].1.cast::<PanicValue>()).number }, 59);
        release(released[1].1);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert!(unsafe { notices.reclaim_removed_raw() }.is_empty());
        drop(notices);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(std::sync::Arc::strong_count(&calls), 1);
    }

    fn notifications() -> Notifications {
        Notifications {
            subscriptions: pi_slotmap::SlotMap::with_key(),
            components: Vec::new(),
            layouts: vec![Some(Layout::new::<u64>())],
            type_ids: vec![Some(TypeId::of::<u64>())],
        }
    }
    fn tick() -> crate::change_detection::ChangeTick {
        crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(2).unwrap())
    }
    #[test]
    fn removed_lifecycle_batch_slow_reader_and_unsubscribe_retain_all_sources() {
        let mut notices = notifications();
        let first = notices
            .subscribe(ComponentId(0), NoticeKind::Removed)
            .unwrap();
        let second = notices
            .subscribe(ComponentId(0), NoticeKind::Removed)
            .unwrap();
        for n in [17u64, 29] {
            let mut source = ManuallyDrop::new(n);
            assert!(notices
                .append_removed(ComponentId(0), Default::default(), tick(), unsafe {
                    crate::ptr::OwningPtr::from_raw(NonNull::from(&mut *source).cast())
                })
                .is_ok());
        }
        // 未读订阅各自保留责任，注销其中一个不替另一个消费。
        assert!(unsafe { notices.reclaim_removed_raw() }.is_empty());
        notices.unsubscribe(second).unwrap();
        assert!(unsafe { notices.reclaim_removed_raw() }.is_empty());
        notices.unsubscribe(first).unwrap();
        let released = unsafe { notices.reclaim_removed_raw() };
        assert_eq!(released.len(), 2);
        // 所有回调完成前，不对任何来源队列做 shrink/reserve/reuse。
        for (index, (component, pointer)) in released.iter().enumerate() {
            assert_eq!(*component, ComponentId(0));
            assert_eq!(unsafe { pointer.cast::<u64>().read() }, [17, 29][index]);
            assert_eq!(unsafe { released[1].1.cast::<u64>().read() }, 29);
        }
        assert!(unsafe { notices.reclaim_removed_raw() }.is_empty());
        let renewed = notices
            .subscribe(ComponentId(0), NoticeKind::Removed)
            .unwrap();
        assert_eq!(notices.subscriptions.get(renewed.id).unwrap().next.get(), 2);
        notices.unsubscribe(renewed).unwrap();
    }

    // Seam: Notifications::append_removed：无订阅时不消费也不改变来源地址。
    #[test]
    fn removed_lifecycle_batch_unsubscribed_returns_original_pointer() {
        let notices = notifications();
        let mut value = ManuallyDrop::new(73u64);
        let address = NonNull::from(&mut *value).cast();
        let pointer = unsafe { crate::ptr::OwningPtr::from_raw(address) };
        let returned = notices
            .append_removed(ComponentId(0), Default::default(), tick(), pointer)
            .err()
            .unwrap();
        let raw = unsafe { returned.into_raw() };
        assert_eq!(raw, address.as_ptr());
        assert_eq!(unsafe { raw.cast::<u64>().read() }, 73);
    }
}

use crate::notification::{
    ComponentNoticeStreams, NoticeBatch, NoticeHeader, NoticeKind, NoticeSequence, NoticeStream,
    NoticeStreamRange, NoticeSubscription, RemovedBatch, RemovedNotice, RemovedNoticeStream,
    SubscriptionState,
};
use crate::{component::ComponentId, error::EcsError, notification::Notifications};
#[cfg(feature = "rc")]
use std::collections::VecDeque;

#[cfg(feature = "rc")]
fn error(kind: pi_ecs_types::error::ErrorKind, message: &str) -> EcsError {
    pi_ecs_types::error::classified_error(kind, message)
}

#[cfg(feature = "rc")]
fn empty_stream() -> NoticeStream {
    NoticeStream {
        headers: VecDeque::new(),
        range: NoticeStreamRange {
            first: NoticeSequence(0),
            end: NoticeSequence(0),
        },
    }
}

#[cfg(feature = "rc")]
fn empty_component_streams() -> ComponentNoticeStreams {
    ComponentNoticeStreams {
        added: None,
        changed: None,
        removed: None,
    }
}

#[cfg(feature = "rc")]
fn has_subscribers(
    notifications: &Notifications,
    component: ComponentId,
    kind: NoticeKind,
) -> bool {
    notifications
        .subscriptions
        .values()
        .any(|state| state.stream == (component, kind))
}

#[cfg(feature = "rc")]
fn append_header_inner(
    stream: &mut NoticeStream,
    entity: crate::entity::Entity,
    tick: crate::change_detection::ChangeTick,
) -> NoticeSequence {
    let sequence = stream.range.end;
    let next = sequence
        .0
        .checked_add(1)
        .expect("通知序号耗尽，World 必须停止");
    stream.headers.push_back(NoticeHeader {
        entity,
        tick,
        sequence,
    });
    stream.range.end = NoticeSequence(next);
    sequence
}

#[cfg(feature = "rc")]
impl Notifications {
    pub(crate) fn append_change_inner(
        notifications: &Notifications,
        component: ComponentId,
        kind: NoticeKind,
        entity: crate::entity::Entity,
        tick: crate::change_detection::ChangeTick,
    ) {
        if !has_subscribers(notifications, component, kind) {
            return;
        }
        let Some(streams) = notifications.components.get(component.0 as usize) else {
            return;
        };
        let stream = match kind {
            NoticeKind::Added => streams.added.as_ref(),
            NoticeKind::Changed => streams.changed.as_ref(),
            NoticeKind::Removed => panic!("Removed 必须连同旧值发布"),
        };
        let Some(stream) = stream else {
            return;
        };
        let mut stream = stream
            .try_borrow_mut()
            .expect("同流通知借用冲突，运行边界必须停止");
        append_header_inner(&mut stream, entity, tick);
    }
}
#[cfg(not(feature = "rc"))]
impl Notifications {
    pub(crate) fn append_change_inner(
        _notifications: &Notifications,
        _component: ComponentId,
        _kind: NoticeKind,
        _entity: crate::entity::Entity,
        _tick: crate::change_detection::ChangeTick,
    ) {
        todo!("编号：T48，尚未实现， 每流内部可变通知协议：append_change_inner")
    }
}

#[cfg(feature = "rc")]
impl RemovedNoticeStream {
    pub(crate) fn append_removed_rc_inner<'a>(
        stream: &mut RemovedNoticeStream,
        entity: crate::entity::Entity,
        tick: crate::change_detection::ChangeTick,
        value: crate::ptr::OwningPtr<'a>,
    ) -> NoticeSequence {
        let sequence = stream.notices.range.end;
        let next = sequence
            .0
            .checked_add(1)
            .expect("通知序号耗尽，World 必须停止");
        // 头分配必须在值接管前完成；失败时来源责任仍由调用者持有。
        stream.notices.headers.reserve(1);
        // SAFETY: 订阅创建时已用组件 Layout 初始化值队列，value 类型由组件登记保证。
        // RawVecDeque::push_back 的 reserve/容量检查均在复制前；提交 len 后无可展开操作。
        // 成功接管后头已有容量，以下仅无分配发布头和 end，不能再 panic。
        unsafe { stream.values.push_back(value) };
        stream.notices.headers.push_back(NoticeHeader {
            entity,
            tick,
            sequence,
        });
        stream.notices.range.end = NoticeSequence(next);
        sequence
    }
}
#[cfg(not(feature = "rc"))]
impl RemovedNoticeStream {
    pub(crate) fn append_removed_rc_inner<'a>(
        _stream: &mut crate::notification::RemovedNoticeStream,
        _entity: crate::entity::Entity,
        _tick: crate::change_detection::ChangeTick,
        _value: crate::ptr::OwningPtr<'a>,
    ) -> NoticeSequence {
        todo!("编号：T49，尚未实现， 非 rc Removed 通知追加尚未实现")
    }
}

#[cfg(feature = "rc")]
impl Notifications {
    pub(crate) fn has_removed_inner(notifications: &Notifications, component: ComponentId) -> bool {
        has_subscribers(notifications, component, NoticeKind::Removed)
    }
}
#[cfg(not(feature = "rc"))]
impl Notifications {
    pub(crate) fn has_removed_inner(
        _notifications: &Notifications,
        _component: ComponentId,
    ) -> bool {
        todo!("编号：T50，尚未实现， 非 rc Removed 订阅判断尚未实现")
    }
}

#[cfg(feature = "rc")]
impl Notifications {
    pub(crate) fn append_removed_component_inner<'a>(
        notifications: &Notifications,
        component: ComponentId,
        entity: crate::entity::Entity,
        tick: crate::change_detection::ChangeTick,
        value: crate::ptr::OwningPtr<'a>,
    ) -> Result<NoticeSequence, crate::ptr::OwningPtr<'a>> {
        if !has_subscribers(notifications, component, NoticeKind::Removed) {
            return Err(value);
        }
        let stream = notifications.components[component.0 as usize]
            .removed
            .as_ref()
            .expect("Removed 订阅必须有流");
        let mut stream = stream
            .try_borrow_mut()
            .expect("同流通知借用冲突，运行边界必须停止");
        Ok(stream.append(entity, tick, value))
    }
}
#[cfg(not(feature = "rc"))]
impl Notifications {
    pub(crate) fn append_removed_component_inner<'a>(
        _notifications: &Notifications,
        _component: ComponentId,
        _entity: crate::entity::Entity,
        _tick: crate::change_detection::ChangeTick,
        _value: crate::ptr::OwningPtr<'a>,
    ) -> Result<NoticeSequence, crate::ptr::OwningPtr<'a>> {
        todo!("编号：T51，尚未实现， 每流内部可变通知协议：append_removed_component_inner")
    }
}

#[cfg(feature = "rc")]
impl Notifications {
    pub(crate) fn subscribe_inner(
        notifications: &mut Notifications,
        component: ComponentId,
        kind: NoticeKind,
    ) -> Result<NoticeSubscription, EcsError> {
        let index = component.0 as usize;
        let layout = notifications
            .layouts
            .get(index)
            .copied()
            .flatten()
            .ok_or_else(|| error(pi_ecs_types::error::ErrorKind::InvalidInput, "组件未登记"))?;
        if notifications
            .type_ids
            .get(index)
            .copied()
            .flatten()
            .is_none()
        {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "组件类型未登记",
            ));
        }
        if notifications.components.len() <= index {
            notifications
                .components
                .resize_with(index + 1, empty_component_streams);
        }
        let streams = &mut notifications.components[index];
        let end = match kind {
            NoticeKind::Added => {
                streams
                    .added
                    .get_or_insert_with(|| std::cell::RefCell::new(empty_stream()))
                    .get_mut()
                    .range
                    .end
                    .0
            }
            NoticeKind::Changed => {
                streams
                    .changed
                    .get_or_insert_with(|| std::cell::RefCell::new(empty_stream()))
                    .get_mut()
                    .range
                    .end
                    .0
            }
            NoticeKind::Removed => {
                streams
                    .removed
                    .get_or_insert_with(|| {
                        std::cell::RefCell::new(RemovedNoticeStream {
                            notices: empty_stream(),
                            // SAFETY: layout 来自真实组件登记，唯一值责任由 World 清偿。
                            values: unsafe {
                                pi_ecs_storage::raw_vec_deque::RawVecDeque::new(layout)
                            },
                        })
                    })
                    .get_mut()
                    .notices
                    .range
                    .end
                    .0
            }
        };
        let id = notifications.subscriptions.insert(SubscriptionState {
            stream: (component, kind),
            next: std::cell::Cell::new(end),
        });
        Ok(NoticeSubscription { id })
    }
}
#[cfg(not(feature = "rc"))]
impl Notifications {
    pub(crate) fn subscribe_inner(
        _notifications: &mut Notifications,
        _component: ComponentId,
        _kind: NoticeKind,
    ) -> Result<NoticeSubscription, EcsError> {
        todo!("编号：T52，尚未实现， 每流内部可变通知协议：subscribe_inner")
    }
}

#[cfg(feature = "rc")]
impl Notifications {
    pub(crate) fn read_inner<'a>(
        notifications: &'a Notifications,
        reader: &'a mut NoticeSubscription,
    ) -> Result<NoticeBatch<'a>, EcsError> {
        let state = notifications.subscriptions.get(reader.id).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "通知订阅已失效",
            )
        })?;
        let streams = &notifications.components[state.stream.0 .0 as usize];
        let stream = match state.stream.1 {
            NoticeKind::Added => streams.added.as_ref(),
            NoticeKind::Changed => streams.changed.as_ref(),
            NoticeKind::Removed => {
                return Err(error(
                    pi_ecs_types::error::ErrorKind::InvalidInput,
                    "Removed 必须类型化读取",
                ))
            }
        }
        .expect("订阅必须有流")
        .try_borrow()
        .map_err(|_| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "通知流借用冲突",
            )
        })?;
        let end = stream.range.end;
        Ok(NoticeBatch {
            notifications,
            reader,
            state,
            stream,
            end,
        })
    }
}
#[cfg(not(feature = "rc"))]
impl Notifications {
    pub(crate) fn read_inner<'a>(
        _notifications: &'a Notifications,
        _reader: &'a mut NoticeSubscription,
    ) -> Result<NoticeBatch<'a>, EcsError> {
        todo!("编号：T53，尚未实现， 每流内部可变通知协议：read_inner")
    }
}

#[cfg(feature = "rc")]
impl Notifications {
    pub(crate) fn unsubscribe_inner(
        notifications: &mut Notifications,
        reader: NoticeSubscription,
    ) -> Result<(), EcsError> {
        notifications
            .subscriptions
            .remove(reader.id)
            .map(|_| ())
            .ok_or_else(|| {
                error(
                    pi_ecs_types::error::ErrorKind::InvalidState,
                    "通知订阅已失效",
                )
            })
    }
}
#[cfg(not(feature = "rc"))]
impl Notifications {
    pub(crate) fn unsubscribe_inner(
        _notifications: &mut Notifications,
        _reader: NoticeSubscription,
    ) -> Result<(), EcsError> {
        todo!("编号：T54，尚未实现， 非 rc 通知注销尚未实现")
    }
}

#[cfg(feature = "rc")]
impl NoticeBatch<'_> {
    pub(crate) fn next_inner(batch: &mut NoticeBatch<'_>) -> Option<NoticeHeader> {
        let next = batch.state.next.get();
        if next >= batch.end.0 {
            return None;
        }
        let index = usize::try_from(
            next.checked_sub(batch.stream.range.first.0)
                .expect("读者落后于保留范围"),
        )
        .expect("通知下标溢出");
        let header = *batch.stream.headers.get(index).expect("通知头缺失");
        batch
            .state
            .next
            .set(next.checked_add(1).expect("通知游标溢出"));
        Some(header)
    }
}
#[cfg(not(feature = "rc"))]
impl NoticeBatch<'_> {
    pub(crate) fn next_inner(_batch: &mut NoticeBatch<'_>) -> Option<NoticeHeader> {
        todo!("编号：T55，尚未实现， 每流内部可变通知协议：next_inner")
    }
}

#[cfg(feature = "rc")]
impl Notifications {
    pub(crate) fn read_removed_inner<'a, T: crate::component::Component>(
        notifications: &'a Notifications,
        world: &'a crate::world::World,
        reader: &'a mut NoticeSubscription,
    ) -> Result<RemovedBatch<'a, T>, EcsError> {
        let state = notifications.subscriptions.get(reader.id).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "通知订阅已失效",
            )
        })?;
        let index = state.stream.0 .0 as usize;
        if state.stream.1 != NoticeKind::Removed
            || notifications.type_ids.get(index).copied().flatten()
                != Some(std::any::TypeId::of::<T>())
        {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "Removed 订阅类型不匹配",
            ));
        }
        let stream = notifications.components[index]
            .removed
            .as_ref()
            .expect("订阅必须有流")
            .try_borrow()
            .map_err(|_| {
                error(
                    pi_ecs_types::error::ErrorKind::InvalidState,
                    "通知流借用冲突",
                )
            })?;
        let end = stream.notices.range.end;
        Ok(RemovedBatch {
            notifications,
            reader,
            state,
            end,
            stream,
            world: crate::unsafe_world_cell::UnsafeWorldCell::from_ref(world),
            marker: std::marker::PhantomData,
        })
    }
}
#[cfg(not(feature = "rc"))]
impl Notifications {
    pub(crate) fn read_removed_inner<'a, T: crate::component::Component>(
        _notifications: &'a Notifications,
        _world: &'a crate::world::World,
        _reader: &'a mut NoticeSubscription,
    ) -> Result<RemovedBatch<'a, T>, EcsError> {
        todo!("编号：T56，尚未实现， 每流内部可变通知协议：read_removed_inner")
    }
}

#[cfg(feature = "rc")]
impl<T: crate::component::Component> RemovedBatch<'_, T> {
    pub(crate) fn removed_next_inner<'a>(
        batch: &'a mut RemovedBatch<'_, T>,
    ) -> Option<RemovedNotice<'a, T>> {
        let next = batch.state.next.get();
        if next >= batch.end.0 {
            return None;
        }
        let index = usize::try_from(
            next.checked_sub(batch.stream.notices.range.first.0)
                .expect("读者落后于保留范围"),
        )
        .expect("通知下标溢出");
        let header = *batch.stream.notices.headers.get(index).expect("通知头缺失");
        let pointer = batch
            .stream
            .values
            .get(index)
            .expect("Removed 旧值缺失")
            .as_non_null();
        // SAFETY: 登记 TypeId 已验证，守卫保活不可变旧值；world 短借同一批次。
        let value = unsafe {
            <T::Access as crate::component::ComponentPolicy<T>>::read_removed(
                pointer.cast::<T>().as_ref(),
                batch.world.reborrow(),
            )
        };
        batch
            .state
            .next
            .set(next.checked_add(1).expect("通知游标溢出"));
        Some(RemovedNotice { header, value })
    }
}
#[cfg(not(feature = "rc"))]
impl<T: crate::component::Component> RemovedBatch<'_, T> {
    pub(crate) fn removed_next_inner<'a>(
        _batch: &'a mut RemovedBatch<'_, T>,
    ) -> Option<RemovedNotice<'a, T>> {
        todo!("编号：T57，尚未实现， 每流内部可变通知协议：removed_next_inner")
    }
}

#[cfg(feature = "rc")]
impl Notifications {
    pub(crate) fn reclaim_inner(notifications: &mut Notifications) {
        for index in 0..notifications.components.len() {
            let component = ComponentId(u32::try_from(index).expect("组件编号溢出"));
            let added = minimum_reader(notifications, component, NoticeKind::Added);
            let changed = minimum_reader(notifications, component, NoticeKind::Changed);
            let streams = &mut notifications.components[index];
            if let Some(stream) = streams.added.as_mut() {
                reclaim_headers(stream.get_mut(), added);
            }
            if let Some(stream) = streams.changed.as_mut() {
                reclaim_headers(stream.get_mut(), changed);
            }
        }
    }
}
#[cfg(not(feature = "rc"))]
impl Notifications {
    pub(crate) fn reclaim_inner(_notifications: &mut Notifications) {
        todo!("编号：T58，尚未实现， 每流内部可变通知协议：reclaim_inner")
    }
}

#[cfg(feature = "rc")]
fn minimum_reader(
    notifications: &Notifications,
    component: ComponentId,
    kind: NoticeKind,
) -> Option<u64> {
    notifications
        .subscriptions
        .values()
        .filter(|state| state.stream == (component, kind))
        .map(|state| state.next.get())
        .min()
}

#[cfg(feature = "rc")]
fn reclaim_headers(stream: &mut NoticeStream, minimum: Option<u64>) {
    let target = minimum
        .unwrap_or(stream.range.end.0)
        .min(stream.range.end.0);
    let mut first = stream.range.first.0;
    while first < target {
        if stream.headers.pop_front().is_none() {
            break;
        }
        first += 1;
    }
    stream.range.first = NoticeSequence(first);
}

#[cfg(feature = "rc")]
impl Notifications {
    pub(crate) fn is_empty_inner(
        notifications: &Notifications,
        reader: &NoticeSubscription,
    ) -> Result<bool, EcsError> {
        let state = notifications.subscriptions.get(reader.id).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "通知订阅已失效",
            )
        })?;
        let streams = &notifications.components[state.stream.0 .0 as usize];
        let end = match state.stream.1 {
            NoticeKind::Added => streams
                .added
                .as_ref()
                .expect("订阅必须有流")
                .try_borrow()
                .map(|s| s.range.end.0),
            NoticeKind::Changed => streams
                .changed
                .as_ref()
                .expect("订阅必须有流")
                .try_borrow()
                .map(|s| s.range.end.0),
            NoticeKind::Removed => streams
                .removed
                .as_ref()
                .expect("订阅必须有流")
                .try_borrow()
                .map(|s| s.notices.range.end.0),
        }
        .map_err(|_| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "通知流借用冲突",
            )
        })?;
        Ok(state.next.get() >= end)
    }
}
#[cfg(not(feature = "rc"))]
impl Notifications {
    pub(crate) fn is_empty_inner(
        _notifications: &Notifications,
        _reader: &NoticeSubscription,
    ) -> Result<bool, EcsError> {
        todo!("编号：T59，尚未实现， 每流内部可变通知协议：is_empty_inner")
    }
}
#[cfg(not(feature = "rc"))]
impl Notifications {
    pub(crate) fn reclaim_removed_raw_inner(
        _notifications: &mut Notifications,
    ) -> Vec<(ComponentId, *mut u8)> {
        todo!("编号：T60，尚未实现， 每流内部可变通知协议：reclaim_removed_raw_inner")
    }
}
#[cfg(feature = "rc")]
impl Notifications {
    pub(crate) fn reclaim_removed_raw_inner(
        notifications: &mut Notifications,
    ) -> Vec<(ComponentId, *mut u8)> {
        // 分配在撤销首份责任前完成；pop 不搬移、不收缩任何来源分配。
        let count = notifications
            .components
            .iter()
            .filter_map(|streams| streams.removed.as_ref())
            .map(|stream| stream.borrow().values.len())
            .try_fold(0usize, |total, len| total.checked_add(len))
            .expect("通知回收数量溢出");
        let mut released = Vec::with_capacity(count);
        for index in 0..notifications.components.len() {
            let component = ComponentId(u32::try_from(index).expect("组件编号溢出"));
            let minimum = minimum_reader(notifications, component, NoticeKind::Removed);
            let Some(stream) = notifications.components[index].removed.as_mut() else {
                continue;
            };
            let stream = stream.get_mut();
            let target = minimum
                .unwrap_or(stream.notices.range.end.0)
                .min(stream.notices.range.end.0);
            assert_eq!(stream.notices.headers.len(), stream.values.len());
            while stream.notices.range.first.0 < target {
                let value = stream.values.pop_front().expect("Removed 旧值与头不一致");
                // SAFETY: unsafe 调用者保活全部来源分配，接收全部唯一责任。
                released.push((component, unsafe { value.into_raw() }));
                stream.notices.headers.pop_front().expect("Removed 头缺失");
                stream.notices.range.first.0 += 1;
            }
        }
        released
    }
}
