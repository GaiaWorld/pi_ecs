//! 组件策略配对芯。
// 本批只验收 RC：非 RC 的 CommandHub::new 尚为延期桩，无法构造合法 World。
#[cfg(all(test, feature = "rc"))]
mod component_release_batch {
    use crate::{unsafe_world_cell::UnsafeWorldCell, world::World};
    use std::mem::ManuallyDrop;

    /// 只用于本批空世界：new 创建全部合法字段，无实体、资源、通知旧值。
    /// 不调用尚未实现的 World::drop；展开时也逐字段释放所有实际分配。
    struct EmptyWorld(ManuallyDrop<World>);
    impl EmptyWorld {
        fn new() -> Self {
            Self(ManuallyDrop::new(World::new().unwrap()))
        }
    }
    impl Drop for EmptyWorld {
        fn drop(&mut self) {
            let world = &mut *self.0;
            macro_rules! clear {
                ($($field:ident),* $(,)?) => { $(
                    // SAFETY: 每个字段恰好清理一次；外层 ManuallyDrop 禁止 World::drop。
                    unsafe { std::ptr::drop_in_place(&mut world.$field) };
                )* };
            }
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
                access
            );
            #[cfg(feature = "rc")]
            clear!(shutdown);
        }
    }

    #[test]
    fn container_release_batch_list_registered_modes_and_views() {
        use crate::{
            component::*, component_policy::ReleaseMode, destruction::*, entity_container::*,
            ptr::OwningPtr,
        };
        use std::{marker::PhantomData, ptr::NonNull};
        for mode in [ReleaseMode::Normal, ReleaseMode::WholeState] {
            let mut w = EmptyWorld::new();
            let id =
                w.0.registry
                    .register_component::<EntityList<()>>(StoragePolicy::SparseOnly)
                    .unwrap();
            let entity =
                w.0.entities
                    .borrow_mut()
                    .allocate(
                        crate::entity::AllocationDomain(0),
                        crate::entity::EntityLocation::from_parts(
                            0,
                            0,
                            0,
                            crate::entity::TableId(0),
                            crate::entity::VariantId(0),
                        ),
                    )
                    .unwrap();
            let owned = crate::owned_entity::OwnedEntity {
                world_identity: w.0.identity,
                entity,
                entities: &w.0.entities,
                pending_destruction: &w.0.pending_destruction,
                access: &w.0.access,
            };
            let duplicate = owned.try_clone().unwrap();
            let mut head = Box::new(ManuallyDrop::new(EntityList::<()> {
                head: None,
                tail: None,
                len: 0,
                marker: PhantomData,
            }));
            let tick =
                crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(2).unwrap());
            let mut ticks = crate::resource::ResourceTicks {
                added: tick,
                changed: tick,
            };
            let cell = UnsafeWorldCell::from_ref(&w.0);
            // SAFETY: 同源唯一头及其变化字段，节点池允许内部可变操作。
            let mut write = unsafe {
                <ListAccess as ComponentPolicy<EntityList<()>>>::write(
                    &mut head,
                    cell,
                    crate::change_detection::ChangeMarker {
                        tick,
                        target: crate::change_detection::ChangeTarget::Resource(&mut ticks),
                    },
                )
            };
            assert!(write.is_empty());
            write.push_front(owned).ok().unwrap();
            write.push_front(duplicate).ok().unwrap();
            drop(write);
            let read =
                unsafe { <ListAccess as ComponentPolicy<EntityList<()>>>::read(&head, cell) };
            assert_eq!(
                read.iter().unwrap().collect::<Vec<_>>(),
                vec![entity, entity]
            );
            drop(read);
            let old = unsafe {
                <ListAccess as ComponentPolicy<EntityList<()>>>::read_removed(&head, cell)
            };
            assert_eq!(old.len(), 2);
            drop(old);
            assert_eq!(w.0.entities.borrow().counts(entity), Some((2, 0)));
            let address = NonNull::from(&mut **head).cast();
            let context = ReleaseContext { world: cell, mode };
            unsafe {
                release_value(
                    &context,
                    w.0.registry.component(id).unwrap(),
                    OwningPtr::from_raw(address),
                );
            }
            assert_eq!(w.0.entities.borrow().counts(entity), Some((0, 0)));
            assert_eq!(
                w.0.pending_destruction.pending.len(),
                usize::from(mode == ReleaseMode::Normal)
            );
            assert!(w.0.pending_destruction.reclamation.is_empty());
            assert_eq!(w.0.entity_nodes.links.borrow().slots.len(), 0);
            assert_eq!(head.len, 0);
            assert!(head.head.is_none() && head.tail.is_none());
            drop(head);
        }
    }

    #[test]
    fn owned_array_batch_registered_weak_modes_and_dead_reclamation() {
        use crate::{
            component::*, component_policy::ReleaseMode, destruction::*, owned_entity::*,
            ptr::OwningPtr,
        };
        use std::{marker::PhantomData, ptr::NonNull};
        for mode in [ReleaseMode::Normal, ReleaseMode::WholeState] {
            let mut world = EmptyWorld::new();
            let id = world
                .0
                .registry
                .register_component::<WeakEntities<3, ContainerPurpose>>(StoragePolicy::SparseOnly)
                .unwrap();
            let entity = world
                .0
                .entities
                .borrow_mut()
                .allocate(
                    crate::entity::AllocationDomain(0),
                    crate::entity::EntityLocation::from_parts(
                        0,
                        0,
                        0,
                        crate::entity::TableId(0),
                        crate::entity::VariantId(0),
                    ),
                )
                .unwrap();
            let owner = OwnedEntity {
                world_identity: world.0.identity,
                entity,
                entities: &world.0.entities,
                pending_destruction: &world.0.pending_destruction,
                access: &world.0.access,
            };
            let mut head = Box::new(ManuallyDrop::new(WeakEntities::<3, ContainerPurpose> {
                entities: [crate::entity::Entity::default(); 3],
                marker: PhantomData,
            }));
            let cell = UnsafeWorldCell::from_ref(&world.0);
            let tick =
                crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(3).unwrap());
            let mut ticks = crate::resource::ResourceTicks {
                added: tick,
                changed: tick,
            };
            let mut write = unsafe {
                <WeakAccess as ComponentPolicy<WeakEntities<3, ContainerPurpose>>>::write(
                    &mut head,
                    cell,
                    crate::change_detection::ChangeMarker {
                        tick,
                        target: crate::change_detection::ChangeTarget::Resource(&mut ticks),
                    },
                )
            };
            assert!(!write.marked);
            write.replace_from(0, &owner).unwrap();
            write.replace_from(1, &owner).unwrap();
            drop(write);
            let read = unsafe {
                <WeakAccess as ComponentPolicy<WeakEntities<3, ContainerPurpose>>>::read(
                    &head, cell,
                )
            };
            drop(read.upgrade(0).unwrap().unwrap());
            assert!(read.upgrade(2).unwrap().is_none());
            drop(read);
            drop(owner);
            world.0.entities.borrow_mut().begin_destroy(entity).unwrap();
            assert!(!world
                .0
                .entities
                .borrow_mut()
                .finish_destroy(entity)
                .unwrap());
            let removed = unsafe {
                <WeakAccess as ComponentPolicy<WeakEntities<3, ContainerPurpose>>>::read_removed(
                    &head, cell,
                )
            };
            assert_eq!(removed.get(1).unwrap(), entity);
            assert!(removed.upgrade(1).unwrap().is_none());
            drop(removed);
            let context = ReleaseContext { world: cell, mode };
            world.0.access.mode.set(crate::world::AccessMode::Stopped);
            unsafe {
                release_value(
                    &context,
                    world.0.registry.component(id).unwrap(),
                    OwningPtr::from_raw(NonNull::from(&mut **head).cast()),
                )
            };
            assert_eq!(world.0.entities.borrow().counts(entity), Some((0, 0)));
            assert_eq!(world.0.pending_destruction.pending.len(), 1);
            assert_eq!(
                world.0.pending_destruction.reclamation.len(),
                usize::from(mode == ReleaseMode::Normal)
            );
            if mode == ReleaseMode::Normal {
                assert_eq!(
                    world.0.pending_destruction.reclamation.get(0),
                    Some(&entity)
                );
            }
            assert!(world.0.entities.borrow_mut().remove(entity));
            drop(head);
        }
    }

    #[test]
    fn owned_array_batch_registered_strong_modes_and_removed() {
        use crate::{
            component::*, component_policy::ReleaseMode, destruction::*, owned_entity::*,
            ptr::OwningPtr,
        };
        use std::{marker::PhantomData, ptr::NonNull};
        for mode in [ReleaseMode::Normal, ReleaseMode::WholeState] {
            let mut world = EmptyWorld::new();
            let id = world
                .0
                .registry
                .register_component::<StrongEntities<2, ContainerPurpose>>(
                    StoragePolicy::SparseOnly,
                )
                .unwrap();
            let entity = world
                .0
                .entities
                .borrow_mut()
                .allocate(
                    crate::entity::AllocationDomain(0),
                    crate::entity::EntityLocation::from_parts(
                        0,
                        0,
                        0,
                        crate::entity::TableId(0),
                        crate::entity::VariantId(0),
                    ),
                )
                .unwrap();
            world
                .0
                .entities
                .borrow_mut()
                .acquire_strong(entity)
                .unwrap();
            let mut head = Box::new(ManuallyDrop::new(StrongEntities::<2, ContainerPurpose> {
                entities: [entity; 2],
                marker: PhantomData,
            }));
            let cell = UnsafeWorldCell::from_ref(&world.0);
            let tick =
                crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(3).unwrap());
            let mut ticks = crate::resource::ResourceTicks {
                added: tick,
                changed: tick,
            };
            let write = unsafe {
                <StrongAccess as ComponentPolicy<StrongEntities<2, ContainerPurpose>>>::write(
                    &mut head,
                    cell,
                    crate::change_detection::ChangeMarker {
                        tick,
                        target: crate::change_detection::ChangeTarget::Resource(&mut ticks),
                    },
                )
            };
            assert!(!write.marked);
            assert_eq!(write.get(1).unwrap(), entity);
            drop(write);
            let read = unsafe {
                <StrongAccess as ComponentPolicy<StrongEntities<2, ContainerPurpose>>>::read(
                    &head, cell,
                )
            };
            assert_eq!(read.get(0).unwrap(), entity);
            drop(read);
            let removed = unsafe {
                <StrongAccess as ComponentPolicy<StrongEntities<2, ContainerPurpose>>>::read_removed(
                    &head, cell,
                )
            };
            let retained = removed.get_owned(1).unwrap();
            drop(removed);
            drop(retained);
            let context = ReleaseContext { world: cell, mode };
            world.0.access.mode.set(crate::world::AccessMode::Stopped);
            unsafe {
                release_value(
                    &context,
                    world.0.registry.component(id).unwrap(),
                    OwningPtr::from_raw(NonNull::from(&mut **head).cast()),
                )
            };
            assert_eq!(world.0.entities.borrow().counts(entity), Some((0, 0)));
            assert_eq!(
                world.0.pending_destruction.pending.len(),
                usize::from(mode == ReleaseMode::Normal)
            );
            drop(head);
            // 第二个整头使最后一份由策略清偿，验证模式分歧。
            let other = world
                .0
                .entities
                .borrow_mut()
                .allocate(
                    crate::entity::AllocationDomain(0),
                    crate::entity::EntityLocation::from_parts(
                        0,
                        0,
                        0,
                        crate::entity::TableId(0),
                        crate::entity::VariantId(0),
                    ),
                )
                .unwrap();
            let mut last = ManuallyDrop::new(StrongEntities::<1, ContainerPurpose> {
                entities: [other],
                marker: PhantomData,
            });
            unsafe {
                <StrongAccess as ComponentPolicy<StrongEntities<1, ContainerPurpose>>>::release(
                    &mut last, cell, mode,
                )
            };
            assert_eq!(world.0.entities.borrow().counts(other), Some((0, 0)));
            assert_eq!(
                world.0.pending_destruction.pending.len(),
                2 * usize::from(mode == ReleaseMode::Normal)
            );
            assert!(world.0.pending_destruction.reclamation.is_empty());
        }
    }

    // Seam: 实际登记生成的 ComponentInfo.release 与具体 ComponentPolicy 视图。
    // T 是非单元且无需 Component/Default/Clone 的用途标签，不能擦成 () 解码。
    struct ContainerPurpose(String);

    macro_rules! registered_container_case {
        ($test:ident, $container:ident, $policy:ident, $push:ident, $empty:expr, $pool:ident, $pool_len:expr) => {
            #[test]
            fn $test() {
                use crate::{
                    component::*, component_policy::ReleaseMode, destruction::*,
                    entity_container::*, ptr::OwningPtr,
                };
                use std::ptr::NonNull;
                for mode in [ReleaseMode::Normal, ReleaseMode::WholeState] {
                    let mut w = EmptyWorld::new();
                    let id =
                        w.0.registry
                            .register_component::<$container<ContainerPurpose>>(
                                StoragePolicy::SparseOnly,
                            )
                            .unwrap();
                    let entity =
                        w.0.entities
                            .borrow_mut()
                            .allocate(
                                crate::entity::AllocationDomain(0),
                                crate::entity::EntityLocation::from_parts(
                                    0,
                                    0,
                                    0,
                                    crate::entity::TableId(0),
                                    crate::entity::VariantId(0),
                                ),
                            )
                            .unwrap();
                    let owned = crate::owned_entity::OwnedEntity {
                        world_identity: w.0.identity,
                        entity,
                        entities: &w.0.entities,
                        pending_destruction: &w.0.pending_destruction,
                        access: &w.0.access,
                    };
                    let second = owned.try_clone().unwrap();
                    let third = owned.try_clone().unwrap();
                    let mut first = Box::new(ManuallyDrop::new($empty));
                    let mut other = Box::new(ManuallyDrop::new($empty));
                    let mut empty = Box::new(ManuallyDrop::new($empty));
                    let original = crate::change_detection::ChangeTick::new(
                        std::num::NonZeroU64::new(2).unwrap(),
                    );
                    let changed = crate::change_detection::ChangeTick::new(
                        std::num::NonZeroU64::new(7).unwrap(),
                    );
                    let mut ticks = crate::resource::ResourceTicks {
                        added: original,
                        changed: original,
                    };
                    let cell = UnsafeWorldCell::from_ref(&w.0);
                    for (head, values) in
                        [(&mut first, vec![owned, second]), (&mut other, vec![third])]
                    {
                        let mut write = unsafe {
                            <$policy as ComponentPolicy<$container<ContainerPurpose>>>::write(
                                head,
                                cell,
                                crate::change_detection::ChangeMarker {
                                    tick: changed,
                                    target: crate::change_detection::ChangeTarget::Resource(
                                        &mut ticks,
                                    ),
                                },
                            )
                        };
                        assert!(!write.marked);
                        for value in values {
                            write.$push(value).ok().unwrap();
                        }
                        assert!(write.marked);
                    }
                    assert_eq!(ticks.added, original);
                    assert_eq!(ticks.changed, changed);
                    // 活值和 Removed 都只读；读视图不能产生 strong 或变化。
                    let view = unsafe {
                        <$policy as ComponentPolicy<$container<ContainerPurpose>>>::read(
                            &first, cell,
                        )
                    };
                    assert_eq!(
                        view.iter().unwrap().collect::<Vec<_>>(),
                        vec![entity, entity]
                    );
                    drop(view);
                    let removed = unsafe {
                        <$policy as ComponentPolicy<$container<ContainerPurpose>>>::read_removed(
                            &first, cell,
                        )
                    };
                    assert_eq!(
                        removed.iter().unwrap().collect::<Vec<_>>(),
                        vec![entity, entity]
                    );
                    drop(removed);
                    assert_eq!(w.0.entities.borrow().counts(entity), Some((3, 0)));
                    let context = ReleaseContext { world: cell, mode };
                    let release = |head: &mut ManuallyDrop<$container<ContainerPurpose>>| {
                        let address = NonNull::from(&mut **head).cast();
                        // SAFETY: 真实登记类型及唯一拥有责任，Box 保留来源分配。
                        unsafe {
                            release_value(
                                &context,
                                w.0.registry.component(id).unwrap(),
                                OwningPtr::from_raw(address),
                            );
                        }
                    };
                    release(&mut empty);
                    assert_eq!(w.0.entities.borrow().counts(entity), Some((3, 0)));
                    assert_eq!(w.0.pending_destruction.pending.len(), 0);
                    release(&mut first);
                    assert_eq!(w.0.entities.borrow().counts(entity), Some((1, 0)));
                    assert_eq!(w.0.pending_destruction.pending.len(), 0);
                    // 共享池中的另一头仍能遍历，当前头节点已回收。
                    let survivor = unsafe {
                        <$policy as ComponentPolicy<$container<ContainerPurpose>>>::read(
                            &other, cell,
                        )
                    };
                    assert_eq!(survivor.iter().unwrap().collect::<Vec<_>>(), vec![entity]);
                    drop(survivor);
                    release(&mut other);
                    assert_eq!(w.0.entities.borrow().counts(entity), Some((0, 0)));
                    assert_eq!(
                        w.0.pending_destruction.pending.len(),
                        usize::from(mode == ReleaseMode::Normal)
                    );
                    assert!(w.0.pending_destruction.reclamation.is_empty());
                    assert_eq!(($pool_len)(&*w.0.entity_nodes.$pool.borrow()), 0);
                    assert_eq!(ticks.added, original);
                    assert_eq!(ticks.changed, changed);
                    // ManuallyDrop 避免二次值析构，Box 自身释放三个来源分配。
                    drop((empty, first, other));
                }
            }
        };
    }
    registered_container_case!(
        container_release_batch_list_typed_shared_heads,
        EntityList,
        ListAccess,
        push_front,
        EntityList::<ContainerPurpose> {
            head: None,
            tail: None,
            len: 0,
            marker: std::marker::PhantomData
        },
        links,
        |pool: &NodePool<EntityLink>| pool.slots.len()
    );
    registered_container_case!(
        container_release_batch_queue_typed_shared_heads,
        EntityQueue,
        QueueAccess,
        push_back,
        EntityQueue::<ContainerPurpose> {
            head: None,
            tail: None,
            len: 0,
            marker: std::marker::PhantomData
        },
        links,
        |pool: &NodePool<EntityLink>| pool.slots.len()
    );

    registered_container_case!(
        container_release_batch_deque_typed_shared_heads,
        EntityDeque,
        DequeAccess,
        push_back,
        EntityDeque::<ContainerPurpose> {
            deque: Default::default(),
            marker: std::marker::PhantomData
        },
        deque,
        |pool: &pi_slot_deque::DequeStorage<DequeKey, EntityNode>| pool.len()
    );

    struct DropCount(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for DropCount {
        fn drop(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    impl crate::component::Component for DropCount {
        type Access = crate::component::PlainAccess;
    }

    #[test]
    fn component_release_batch_plain_release_drops_once_in_each_mode() {
        use crate::{
            component::{ComponentPolicy, PlainAccess},
            component_policy::ReleaseMode,
        };
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let mut world = EmptyWorld::new();
        for mode in [ReleaseMode::Normal, ReleaseMode::WholeState] {
            let count = Arc::new(AtomicUsize::new(0));
            let mut value = ManuallyDrop::new(DropCount(count.clone()));
            // SAFETY: 唯一清偿真实槽，栈分配保活，不再读取或析构已释放值。
            unsafe {
                <PlainAccess as ComponentPolicy<DropCount>>::release(
                    &mut value,
                    UnsafeWorldCell::from_mut(&mut world.0),
                    mode,
                )
            };
            assert_eq!(count.load(Ordering::SeqCst), 1);
            assert_eq!(Arc::strong_count(&count), 1);
        }
    }

    struct Custom {
        drops: DropCount,
        calls: std::sync::Arc<std::sync::Mutex<Vec<(u64, crate::component_policy::ReleaseMode)>>>,
    }
    struct CustomAccess;
    impl crate::component::Component for Custom {
        type Access = CustomAccess;
    }
    // SAFETY: 只读返回同一槽；写不暴露值，release 唯一清偿且不保存槽引用。
    unsafe impl crate::component::ComponentPolicy<Custom> for CustomAccess {
        type Read<'a, 'w> = &'a Custom;
        type Write<'a, 'w> = ();
        unsafe fn read<'a, 'w>(value: &'a Custom, _: UnsafeWorldCell<'w>) -> &'a Custom {
            value
        }
        unsafe fn read_removed<'a>(value: &'a Custom, _: UnsafeWorldCell<'a>) -> &'a Custom {
            value
        }
        unsafe fn write<'a, 'w>(
            _: &'a mut Custom,
            _: UnsafeWorldCell<'w>,
            _: crate::change_detection::ChangeMarker<'a>,
        ) {
        }
        unsafe fn release(
            value: &mut ManuallyDrop<Custom>,
            world: UnsafeWorldCell<'_>,
            mode: crate::component_policy::ReleaseMode,
        ) {
            value.calls.lock().unwrap().push((world.id(), mode));
            unsafe { ManuallyDrop::drop(value) };
        }
    }

    #[test]
    fn component_release_batch_registered_dispatch_preserves_mode_world_and_allocation() {
        use crate::{
            component::StoragePolicy,
            component_policy::ReleaseMode,
            destruction::{release_value, ReleaseContext},
            ptr::OwningPtr,
        };
        use std::{
            ptr::NonNull,
            sync::{
                atomic::{AtomicUsize, Ordering},
                Arc, Mutex,
            },
        };
        let mut world = EmptyWorld::new();
        let identity = world.0.identity;
        let id = world
            .0
            .registry
            .register_component::<Custom>(StoragePolicy::SparseOnly)
            .unwrap();
        // 从实际 World 登记生成元数据，再与真实 World 同时拆借；测试没有伪造 ComponentInfo。
        for mode in [ReleaseMode::Normal, ReleaseMode::WholeState] {
            let count = Arc::new(AtomicUsize::new(0));
            let calls = Arc::new(Mutex::new(Vec::new()));
            let mut allocation = Box::new(ManuallyDrop::new(Custom {
                drops: DropCount(count.clone()),
                calls: calls.clone(),
            }));
            let pointer = NonNull::from(&mut *allocation).cast::<u8>();
            let context = ReleaseContext {
                world: UnsafeWorldCell::from_ref(&world.0),
                mode,
            };
            let info = world.0.registry.component(id).unwrap();
            // SAFETY: 真实 Custom 分配，唯一释放责任，同源登记与 World；Box 保活分配。
            unsafe { release_value(&context, info, OwningPtr::from_raw(pointer)) };
            assert_eq!(*calls.lock().unwrap(), vec![(identity, mode)]);
            assert_eq!(count.load(Ordering::SeqCst), 1);
            assert_eq!(Arc::strong_count(&count), 1);
            assert_eq!(Arc::strong_count(&calls), 1);
            // 适配器只销毁值：来源分配仍由 Box 持有并在这里释放，不重复 Drop Custom。
            drop(allocation);
            assert_eq!(count.load(Ordering::SeqCst), 1);
        }
    }

    struct Plain(u32);
    impl crate::component::Component for Plain {
        type Access = crate::component::PlainAccess;
    }

    #[test]
    fn component_release_batch_write_preserves_marker_without_marking() {
        use crate::{
            change_detection::{ChangeMarker, ChangeTarget},
            component::{ComponentPolicy, PlainAccess},
        };
        use pi_ecs_types::change_detection::{ChangeTick, ComponentTicksMut, TrackingDemand};
        let mut world = EmptyWorld::new();
        let mut value = Plain(73);
        let address = std::ptr::from_mut(&mut value);
        let original = ChangeTick::new(std::num::NonZeroU64::new(3).unwrap());
        let write_tick = ChangeTick::new(std::num::NonZeroU64::new(9).unwrap());
        let mut added = Some(original);
        let mut changed = Some(original);
        let demand = TrackingDemand {
            added_tick: true,
            changed_tick: true,
            ..Default::default()
        };
        let marker = ChangeMarker {
            tick: write_tick,
            target: ChangeTarget::Component {
                ticks: ComponentTicksMut {
                    added: Some(&mut added),
                    changed: Some(&mut changed),
                },
                demand: &demand,
                notice: None,
            },
        };
        // SAFETY: 独占真实单槽，marker 独占该槽变化元数据，World 来源合法。
        let wrapped = unsafe {
            <PlainAccess as ComponentPolicy<Plain>>::write(
                &mut value,
                UnsafeWorldCell::from_mut(&mut world.0),
                marker,
            )
        };
        assert_eq!(std::ptr::from_ref(wrapped.value), address.cast_const());
        assert_eq!(wrapped.value.0, 73);
        assert!(!wrapped.marked);
        assert_eq!(wrapped.marker.tick, write_tick);
        match &wrapped.marker.target {
            ChangeTarget::Component {
                ticks,
                demand: retained,
                notice,
            } => {
                assert!(std::ptr::eq(*retained, &demand));
                assert_eq!(ticks.added.as_deref(), Some(&Some(original)));
                assert_eq!(ticks.changed.as_deref(), Some(&Some(original)));
                assert!(notice.is_none());
            }
            _ => panic!("组件 marker 不得替换目标类别"),
        }
        drop(wrapped);
        assert_eq!(added, Some(original));
        assert_eq!(changed, Some(original));
    }

    #[test]
    fn component_release_batch_plain_read_and_removed_share_the_original_slot() {
        use crate::component::{ComponentPolicy, PlainAccess};
        let world = EmptyWorld::new();
        let value = Plain(41);
        let cell = UnsafeWorldCell::from_ref(&world.0);
        // SAFETY: 单槽只读，合法 World 覆盖共享访问，无冲突修改。
        let read = unsafe { <PlainAccess as ComponentPolicy<Plain>>::read(&value, cell) };
        let removed =
            unsafe { <PlainAccess as ComponentPolicy<Plain>>::read_removed(&value, cell) };
        assert!(std::ptr::eq(read, &value));
        assert!(std::ptr::eq(removed, &value));
        assert_eq!(read.0, 41);
        assert_eq!(removed.0, 41);
    }

    #[test]
    fn component_release_batch_real_world_handles_preserve_source() {
        let mut first = EmptyWorld::new();
        let second = EmptyWorld::new();
        let identity = first.0.identity;
        assert_ne!(identity, 0);
        assert_ne!(identity, second.0.identity);
        first.0.layout_epoch = 7;
        let cell = UnsafeWorldCell::from_mut(&mut first.0);
        assert!(cell.mutable);
        assert_eq!(cell.id(), identity);
        assert_eq!(cell.archetype_revision(), (7, 0));
        let short = cell.reborrow();
        assert_eq!(short.pointer, cell.pointer);
        assert!(short.mutable);
        assert_eq!(short.id(), identity);
        let shared = UnsafeWorldCell::from_ref(&first.0);
        assert!(!shared.mutable);
        assert!(!shared.reborrow().mutable);
        assert_eq!(shared.id(), identity);
        assert_eq!(shared.archetype_revision(), (7, 0));
    }
}

use super::*;

impl PlainAccess {
    pub(super) unsafe fn read_inner<'a, 'w, C: Component<Access = PlainAccess>>(
        _value: &'a C,
        _world: UnsafeWorldCell<'w>,
    ) -> <PlainAccess as ComponentPolicy<C>>::Read<'a, 'w> {
        _value
    }
    pub(super) unsafe fn write_inner<'a, 'w, C: Component<Access = PlainAccess>>(
        _value: &'a mut C,
        _world: UnsafeWorldCell<'w>,
        _marker: crate::change_detection::ChangeMarker<'a>,
    ) -> <PlainAccess as ComponentPolicy<C>>::Write<'a, 'w> {
        crate::change_detection::Mut {
            value: _value,
            marker: _marker,
            marked: false,
        }
    }
    pub(super) unsafe fn read_removed_inner<'a, C: Component<Access = PlainAccess>>(
        _value: &'a C,
        _world: UnsafeWorldCell<'a>,
    ) -> <PlainAccess as ComponentPolicy<C>>::Read<'a, 'a> {
        _value
    }
    pub(super) unsafe fn release_inner<C: Component<Access = PlainAccess>>(
        _value: &mut ManuallyDrop<C>,
        _world: UnsafeWorldCell<'_>,
        _mode: ReleaseMode,
    ) {
        // SAFETY: 调用者移交唯一已初始化 C；普通策略完整析构一次，不释放来源分配。
        unsafe { ManuallyDrop::drop(_value) };
    }
}

impl ListAccess {
    pub(super) unsafe fn read_inner<'a, 'w, T: 'static>(
        _value: &'a crate::entity_container::EntityList<T>,
        _world: UnsafeWorldCell<'w>,
    ) -> <ListAccess as ComponentPolicy<crate::entity_container::EntityList<T>>>::Read<'a, 'w> {
        // SAFETY: 调用方保证同源单槽和授权保活；仅投影所需字段，不恢复整个 World。
        unsafe {
            let world = _world.pointer.as_ptr();
            crate::entity_container::EntityListRef {
                value: _value,
                nodes: &*std::ptr::addr_of!((*world).entity_nodes),
                world_identity: _world.id(),
                entities: &*std::ptr::addr_of!((*world).entities),
                pending_destruction: &*std::ptr::addr_of!((*world).pending_destruction),
                access: &*std::ptr::addr_of!((*world).access),
            }
        }
    }
    pub(super) unsafe fn write_inner<'a, 'w, T: 'static>(
        _value: &'a mut crate::entity_container::EntityList<T>,
        _world: UnsafeWorldCell<'w>,
        _marker: crate::change_detection::ChangeMarker<'a>,
    ) -> <ListAccess as ComponentPolicy<crate::entity_container::EntityList<T>>>::Write<'a, 'w>
    {
        // SAFETY: 调用方保证同源单槽和授权保活；仅投影所需字段，不恢复整个 World。
        unsafe {
            let world = _world.pointer.as_ptr();
            crate::entity_container::EntityListMut {
                value: _value,
                nodes: &*std::ptr::addr_of!((*world).entity_nodes),
                world_identity: _world.id(),
                entities: &*std::ptr::addr_of!((*world).entities),
                pending_destruction: &*std::ptr::addr_of!((*world).pending_destruction),
                access: &*std::ptr::addr_of!((*world).access),
                marker: _marker,
                marked: false,
            }
        }
    }
    pub(super) unsafe fn read_removed_inner<'a, T: 'static>(
        _value: &'a crate::entity_container::EntityList<T>,
        _world: UnsafeWorldCell<'a>,
    ) -> <ListAccess as ComponentPolicy<crate::entity_container::EntityList<T>>>::Read<'a, 'a> {
        // SAFETY: Removed 与活值共用只读能力，不增持、不发布变化。
        unsafe { Self::read_inner::<T>(_value, _world) }
    }
    pub(super) unsafe fn release_inner<T: 'static>(
        _value: &mut ManuallyDrop<crate::entity_container::EntityList<T>>,
        _world: UnsafeWorldCell<'_>,
        _mode: ReleaseMode,
    ) {
        let context = crate::destruction::ReleaseContext {
            world: _world,
            mode: _mode,
        };
        // SAFETY: 单槽唯一释放责任保持真实 T；只交接值，不移交来源分配。
        let pointer = unsafe {
            crate::ptr::OwningPtr::from_raw(std::ptr::NonNull::from(&mut **_value).cast())
        };
        unsafe { crate::entity_container::release_list::<T>(&context, pointer) };
    }
}

impl QueueAccess {
    pub(super) unsafe fn read_inner<'a, 'w, T: 'static>(
        _value: &'a crate::entity_container::EntityQueue<T>,
        _world: UnsafeWorldCell<'w>,
    ) -> <QueueAccess as ComponentPolicy<crate::entity_container::EntityQueue<T>>>::Read<'a, 'w>
    {
        // SAFETY: 调用方保证同源单槽和授权保活；仅投影所需字段，不恢复整个 World。
        unsafe {
            let world = _world.pointer.as_ptr();
            crate::entity_container::EntityQueueRef {
                value: _value,
                nodes: &*std::ptr::addr_of!((*world).entity_nodes),
                world_identity: _world.id(),
                entities: &*std::ptr::addr_of!((*world).entities),
                pending_destruction: &*std::ptr::addr_of!((*world).pending_destruction),
                access: &*std::ptr::addr_of!((*world).access),
            }
        }
    }
    pub(super) unsafe fn write_inner<'a, 'w, T: 'static>(
        _value: &'a mut crate::entity_container::EntityQueue<T>,
        _world: UnsafeWorldCell<'w>,
        _marker: crate::change_detection::ChangeMarker<'a>,
    ) -> <QueueAccess as ComponentPolicy<crate::entity_container::EntityQueue<T>>>::Write<'a, 'w>
    {
        // SAFETY: 调用方保证同源单槽和授权保活；仅投影所需字段，不恢复整个 World。
        unsafe {
            let world = _world.pointer.as_ptr();
            crate::entity_container::EntityQueueMut {
                value: _value,
                nodes: &*std::ptr::addr_of!((*world).entity_nodes),
                world_identity: _world.id(),
                entities: &*std::ptr::addr_of!((*world).entities),
                pending_destruction: &*std::ptr::addr_of!((*world).pending_destruction),
                access: &*std::ptr::addr_of!((*world).access),
                marker: _marker,
                marked: false,
            }
        }
    }
    pub(super) unsafe fn read_removed_inner<'a, T: 'static>(
        _value: &'a crate::entity_container::EntityQueue<T>,
        _world: UnsafeWorldCell<'a>,
    ) -> <QueueAccess as ComponentPolicy<crate::entity_container::EntityQueue<T>>>::Read<'a, 'a>
    {
        // SAFETY: Removed 与活值共用只读能力，不增持、不发布变化。
        unsafe { Self::read_inner::<T>(_value, _world) }
    }
    pub(super) unsafe fn release_inner<T: 'static>(
        _value: &mut ManuallyDrop<crate::entity_container::EntityQueue<T>>,
        _world: UnsafeWorldCell<'_>,
        _mode: ReleaseMode,
    ) {
        let context = crate::destruction::ReleaseContext {
            world: _world,
            mode: _mode,
        };
        // SAFETY: 单槽唯一释放责任保持真实 T；只交接值，不移交来源分配。
        let pointer = unsafe {
            crate::ptr::OwningPtr::from_raw(std::ptr::NonNull::from(&mut **_value).cast())
        };
        unsafe { crate::entity_container::release_queue::<T>(&context, pointer) };
    }
}

impl DequeAccess {
    pub(super) unsafe fn read_inner<'a, 'w, T: 'static>(
        _value: &'a crate::entity_container::EntityDeque<T>,
        _world: UnsafeWorldCell<'w>,
    ) -> <DequeAccess as ComponentPolicy<crate::entity_container::EntityDeque<T>>>::Read<'a, 'w>
    {
        // SAFETY: 调用方保证同源单槽和授权保活；仅投影所需字段，不恢复整个 World。
        unsafe {
            let world = _world.pointer.as_ptr();
            crate::entity_container::EntityDequeRef {
                value: _value,
                nodes: &*std::ptr::addr_of!((*world).entity_nodes),
                world_identity: _world.id(),
                entities: &*std::ptr::addr_of!((*world).entities),
                pending_destruction: &*std::ptr::addr_of!((*world).pending_destruction),
                access: &*std::ptr::addr_of!((*world).access),
            }
        }
    }
    pub(super) unsafe fn write_inner<'a, 'w, T: 'static>(
        _value: &'a mut crate::entity_container::EntityDeque<T>,
        _world: UnsafeWorldCell<'w>,
        _marker: crate::change_detection::ChangeMarker<'a>,
    ) -> <DequeAccess as ComponentPolicy<crate::entity_container::EntityDeque<T>>>::Write<'a, 'w>
    {
        // SAFETY: 调用方保证同源单槽和授权保活；仅投影所需字段，不恢复整个 World。
        unsafe {
            let world = _world.pointer.as_ptr();
            crate::entity_container::EntityDequeMut {
                value: _value,
                nodes: &*std::ptr::addr_of!((*world).entity_nodes),
                world_identity: _world.id(),
                entities: &*std::ptr::addr_of!((*world).entities),
                pending_destruction: &*std::ptr::addr_of!((*world).pending_destruction),
                access: &*std::ptr::addr_of!((*world).access),
                marker: _marker,
                marked: false,
            }
        }
    }
    pub(super) unsafe fn read_removed_inner<'a, T: 'static>(
        _value: &'a crate::entity_container::EntityDeque<T>,
        _world: UnsafeWorldCell<'a>,
    ) -> <DequeAccess as ComponentPolicy<crate::entity_container::EntityDeque<T>>>::Read<'a, 'a>
    {
        // SAFETY: Removed 与活值共用只读能力，不增持、不发布变化。
        unsafe { Self::read_inner::<T>(_value, _world) }
    }
    pub(super) unsafe fn release_inner<T: 'static>(
        _value: &mut ManuallyDrop<crate::entity_container::EntityDeque<T>>,
        _world: UnsafeWorldCell<'_>,
        _mode: ReleaseMode,
    ) {
        let context = crate::destruction::ReleaseContext {
            world: _world,
            mode: _mode,
        };
        // SAFETY: 单槽唯一释放责任保持真实 T；只交接值，不移交来源分配。
        let pointer = unsafe {
            crate::ptr::OwningPtr::from_raw(std::ptr::NonNull::from(&mut **_value).cast())
        };
        unsafe { crate::entity_container::release_deque::<T>(&context, pointer) };
    }
}

#[cfg(feature = "rc")]
fn array_release_failure(access: &crate::world::AccessState) -> ! {
    access.mode.set(crate::world::AccessMode::Stopped);
    std::process::abort()
}

impl StrongAccess {
    pub(super) unsafe fn read_inner<'a, 'w, const N: usize, T: 'static>(
        _value: &'a crate::owned_entity::StrongEntities<N, T>,
        _world: UnsafeWorldCell<'w>,
    ) -> <StrongAccess as ComponentPolicy<crate::owned_entity::StrongEntities<N, T>>>::Read<'a, 'w>
    {
        // SAFETY: 原授权覆盖字段寿命，仅投影身份/登记/停止状态。
        unsafe {
            let world = _world.pointer.as_ptr();
            crate::owned_entity::StrongEntitiesRef {
                value: _value,
                world_identity: _world.id(),
                entities: &*std::ptr::addr_of!((*world).entities),
                pending_destruction: &*std::ptr::addr_of!((*world).pending_destruction),
                access: &*std::ptr::addr_of!((*world).access),
            }
        }
    }
    pub(super) unsafe fn write_inner<'a, 'w, const N: usize, T: 'static>(
        _value: &'a mut crate::owned_entity::StrongEntities<N, T>,
        _world: UnsafeWorldCell<'w>,
        _marker: crate::change_detection::ChangeMarker<'a>,
    ) -> <StrongAccess as ComponentPolicy<crate::owned_entity::StrongEntities<N, T>>>::Write<'a, 'w>
    {
        // SAFETY: 原授权覆盖字段寿命，仅投影身份/登记/停止状态。
        unsafe {
            let world = _world.pointer.as_ptr();
            crate::owned_entity::StrongEntitiesMut {
                value: _value,
                world_identity: _world.id(),
                entities: &*std::ptr::addr_of!((*world).entities),
                pending_destruction: &*std::ptr::addr_of!((*world).pending_destruction),
                access: &*std::ptr::addr_of!((*world).access),
                marker: _marker,
                marked: false,
            }
        }
    }
    pub(super) unsafe fn read_removed_inner<'a, const N: usize, T: 'static>(
        _value: &'a crate::owned_entity::StrongEntities<N, T>,
        _world: UnsafeWorldCell<'a>,
    ) -> <StrongAccess as ComponentPolicy<crate::owned_entity::StrongEntities<N, T>>>::Read<'a, 'a>
    {
        unsafe { Self::read_inner::<N, T>(_value, _world) }
    }
    pub(super) unsafe fn release_inner<const N: usize, T: 'static>(
        _value: &mut ManuallyDrop<crate::owned_entity::StrongEntities<N, T>>,
        _world: UnsafeWorldCell<'_>,
        _mode: ReleaseMode,
    ) {
        #[cfg(feature = "rc")]
        unsafe {
            let context = crate::destruction::ReleaseContext {
                world: _world,
                mode: _mode,
            };
            // The policy received exclusive detached ownership of this exact array type.
            let pointer = std::ptr::NonNull::from(_value).cast();
            crate::builtin_init::release_strong::<N, T>(
                &context,
                crate::ptr::OwningPtr::from_raw(pointer),
            );
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (_value, _world, _mode);
            todo!("编号：T8，尚未实现， 非 RC 强数组策略释放延期")
        }
    }
}

impl WeakAccess {
    pub(super) unsafe fn read_inner<'a, 'w, const N: usize, T: 'static>(
        _value: &'a crate::owned_entity::WeakEntities<N, T>,
        _world: UnsafeWorldCell<'w>,
    ) -> <WeakAccess as ComponentPolicy<crate::owned_entity::WeakEntities<N, T>>>::Read<'a, 'w>
    {
        // SAFETY: 单槽原授权覆盖字段寿命，不还原整 World。
        unsafe {
            let world = _world.pointer.as_ptr();
            crate::owned_entity::WeakEntitiesRef {
                value: _value,
                world_identity: _world.id(),
                entities: &*std::ptr::addr_of!((*world).entities),
                pending_destruction: &*std::ptr::addr_of!((*world).pending_destruction),
                access: &*std::ptr::addr_of!((*world).access),
            }
        }
    }
    pub(super) unsafe fn write_inner<'a, 'w, const N: usize, T: 'static>(
        _value: &'a mut crate::owned_entity::WeakEntities<N, T>,
        _world: UnsafeWorldCell<'w>,
        _marker: crate::change_detection::ChangeMarker<'a>,
    ) -> <WeakAccess as ComponentPolicy<crate::owned_entity::WeakEntities<N, T>>>::Write<'a, 'w>
    {
        // SAFETY: 单槽原授权覆盖字段寿命，不还原整 World。
        unsafe {
            let world = _world.pointer.as_ptr();
            crate::owned_entity::WeakEntitiesMut {
                value: _value,
                world_identity: _world.id(),
                entities: &*std::ptr::addr_of!((*world).entities),
                pending_destruction: &*std::ptr::addr_of!((*world).pending_destruction),
                access: &*std::ptr::addr_of!((*world).access),
                marker: _marker,
                marked: false,
            }
        }
    }
    pub(super) unsafe fn read_removed_inner<'a, const N: usize, T: 'static>(
        _value: &'a crate::owned_entity::WeakEntities<N, T>,
        _world: UnsafeWorldCell<'a>,
    ) -> <WeakAccess as ComponentPolicy<crate::owned_entity::WeakEntities<N, T>>>::Read<'a, 'a>
    {
        unsafe { Self::read_inner::<N, T>(_value, _world) }
    }
    pub(super) unsafe fn release_inner<const N: usize, T: 'static>(
        _value: &mut ManuallyDrop<crate::owned_entity::WeakEntities<N, T>>,
        _world: UnsafeWorldCell<'_>,
        _mode: ReleaseMode,
    ) {
        #[cfg(feature = "rc")]
        unsafe {
            let context = crate::destruction::ReleaseContext {
                world: _world,
                mode: _mode,
            };
            // The policy received exclusive detached ownership of this exact array type.
            let pointer = std::ptr::NonNull::from(_value).cast();
            crate::builtin_init::release_weak::<N, T>(
                &context,
                crate::ptr::OwningPtr::from_raw(pointer),
            );
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (_value, _world, _mode);
            todo!("编号：T9，尚未实现， 非 RC 弱数组策略释放延期")
        }
    }
}
