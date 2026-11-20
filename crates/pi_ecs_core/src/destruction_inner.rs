//! 阶段末销毁配对芯。
#[cfg(all(test, feature = "rc"))]
mod destruction_lifecycle_batch {
    use super::*;
    use std::mem::ManuallyDrop;
    struct TestWorld(ManuallyDrop<World>);
    impl TestWorld {
        fn new() -> Self {
            Self(ManuallyDrop::new(World::new().unwrap()))
        }
    }
    impl Drop for TestWorld {
        fn drop(&mut self) {
            let world = &mut *self.0;
            macro_rules! clear { ($($field:ident),*) => { $(unsafe { std::ptr::drop_in_place(&mut world.$field) };)* }; }
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
    fn stopped(world: &World) -> bool {
        let mode = world.access.mode.replace(crate::world::AccessMode::Stopped);
        let result = matches!(mode, crate::world::AccessMode::Stopped);
        world.access.mode.set(mode);
        result
    }
    struct Payload(u32, std::sync::Arc<std::sync::Mutex<Vec<u32>>>);
    impl crate::component::Component for Payload {
        type Access = crate::component::PlainAccess;
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.1.lock().unwrap().push(self.0);
        }
    }
    fn install(world: &mut World, archetype: crate::entity::ArchetypeId) -> Entity {
        let row = world
            .storage
            .as_mut()
            .unwrap()
            .table_mut(archetype.table)
            .unwrap()
            .claim_row()
            .unwrap();
        let entity = world
            .entities
            .borrow_mut()
            .allocate(
                crate::entity::AllocationDomain(0),
                crate::entity::EntityLocation::from_parts(
                    0,
                    0,
                    row.index(),
                    archetype.table,
                    archetype.variant,
                ),
            )
            .unwrap();
        let member = world
            .storage
            .as_mut()
            .unwrap()
            .append_member(archetype, entity, row)
            .unwrap();
        world
            .entities
            .borrow_mut()
            .relocate(
                entity,
                crate::entity::EntityLocation::from_parts(
                    0,
                    member,
                    row.index(),
                    archetype.table,
                    archetype.variant,
                ),
            )
            .unwrap();
        world
            .storage
            .as_mut()
            .unwrap()
            .table_mut(archetype.table)
            .unwrap()
            .publish_row(row, entity)
            .unwrap();
        entity
    }
    #[test]
    fn destruction_lifecycle_batch_removed_retains_then_reclaims_without_tick() {
        let mut w = TestWorld::new();
        let id =
            w.0.registry
                .register_component::<Payload>(crate::component::StoragePolicy::SparseOnly)
                .unwrap();
        let (layout, type_id) = {
            let info = w.0.registry.component(id).unwrap();
            (info.layout, info.type_id)
        };
        w.0.notifications.layouts = vec![Some(layout)];
        w.0.notifications.type_ids = vec![Some(type_id)];
        w.0.finish_registration().unwrap();
        let plan = w.0.registry.plan_archetype(&[id]).unwrap();
        // 单源计划提交，物理准备由已确认的公开 storage 接口完成。
        let archetype = w.0.registry.commit_archetype(plan);
        w.0.storage
            .as_mut()
            .unwrap()
            .prepare_archetype(archetype)
            .unwrap();
        let entity = install(&mut w.0, archetype);
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        w.0.storage
            .as_mut()
            .unwrap()
            .prepare_sparse(id, entity)
            .unwrap();
        let mut payload = ManuallyDrop::new(Payload(7, log.clone()));
        let value = unsafe { OwningPtr::from_raw(std::ptr::NonNull::from(&mut *payload).cast()) };
        w.0.storage
            .as_mut()
            .unwrap()
            .insert_sparse(id, entity, value);
        let mut reader =
            w.0.notifications
                .subscribe(id, crate::notification::NoticeKind::Removed)
                .unwrap();
        assert!(unsafe { w.0.despawn(entity) }.unwrap());
        w.0.flush_destruction().unwrap();
        assert!(w.0.locate(entity).is_none());
        assert!(log.lock().unwrap().is_empty());
        {
            let mut batch =
                w.0.notifications
                    .read_removed::<Payload>(&w.0, &mut reader)
                    .unwrap();
            assert_eq!(batch.next().unwrap().value.0, 7);
            assert!(batch.next().is_none());
        }
        let tick = w.0.change_tick;
        w.0.reclaim_notifications().unwrap();
        assert_eq!(*log.lock().unwrap(), vec![7]);
        assert_eq!(w.0.change_tick, tick);
        w.0.notifications.unsubscribe(reader).unwrap();
    }
    // Seam: World::despawn + flush_destruction；只通过测试夹具注入不可自然到达的序号耗尽。
    #[test]
    fn destruction_lifecycle_batch_removed_exhaustion_releases_plain_once() {
        for policy in [
            crate::component::StoragePolicy::SparseOnly,
            crate::component::StoragePolicy::TableEligible,
        ] {
            let mut w = TestWorld::new();
            let id = w.0.registry.register_component::<Payload>(policy).unwrap();
            let (layout, type_id) = {
                let info = w.0.registry.component(id).unwrap();
                (info.layout, info.type_id)
            };
            w.0.notifications.layouts = vec![Some(layout)];
            w.0.notifications.type_ids = vec![Some(type_id)];
            let table = matches!(policy, crate::component::StoragePolicy::TableEligible);
            let registered = if table {
                Some(w.0.registry.register_table(&[id]).unwrap())
            } else {
                None
            };
            w.0.finish_registration().unwrap();
            let ty = registered.unwrap_or_else(|| archetype(&mut w.0, &[id]));
            let entity = install(&mut w.0, ty);
            let source_row = w.0.locate(entity).unwrap().table_row();
            let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let address = if table {
                let row = w.0.locate(entity).unwrap().table_row();
                let mut value = ManuallyDrop::new(Payload(71, log.clone()));
                let mut storage = w.0.storage.as_mut().unwrap().table_mut(ty.table).unwrap();
                let mut column = storage.column_mut(crate::table::ColumnId(0)).unwrap();
                unsafe {
                    column.insert_raw(
                        row as usize,
                        std::ptr::from_mut(&mut *value).cast(),
                        std::alloc::Layout::new::<Payload>(),
                    );
                    column.get(row as usize).as_non_null().as_ptr()
                }
            } else {
                sparse_value(&mut w.0, id, entity, Payload(71, log.clone()));
                let sparse = w.0.storage.as_ref().unwrap().sparse_address(id).unwrap();
                unsafe { sparse.as_ref().get(entity).unwrap().as_non_null().as_ptr() }
            };
            let reader =
                w.0.notifications
                    .subscribe(id, crate::notification::NoticeKind::Removed)
                    .unwrap();
            {
                let stream = w.0.notifications.components[id.0 as usize]
                    .removed
                    .as_mut()
                    .unwrap()
                    .get_mut();
                stream.notices.range.first = crate::notification::NoticeSequence(u64::MAX);
                stream.notices.range.end = crate::notification::NoticeSequence(u64::MAX);
            }
            assert!(unsafe { w.0.despawn(entity) }.unwrap());
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w
                .0
                .flush_destruction()))
            .is_err());
            assert!(stopped(&w.0));
            assert_eq!(
                *log.lock().unwrap(),
                vec![71],
                "移出后尚未交入 Removed 的值必须清偿"
            );
            // 释放回调未回收来源分配；已析构值不再读取，只核对槽地址与行仍保活。
            if table {
                assert!(w
                    .0
                    .storage
                    .as_ref()
                    .unwrap()
                    .row_entity(ty.table, crate::table::TableRow::new(source_row))
                    .is_none());
            } else {
                let sparse = w.0.storage.as_ref().unwrap().sparse_address(id).unwrap();
                assert!(unsafe { sparse.as_ref().get(entity) }.is_none());
            }
            assert!(!address.is_null());
            assert_eq!(w.0.entities.borrow().counts(entity), Some((0, 0)));
            assert!(w.0.flush_destruction().is_err());
            assert_eq!(*log.lock().unwrap(), vec![71]);
            w.0.notifications.unsubscribe(reader).unwrap();
            drop(w);
            assert_eq!(*log.lock().unwrap(), vec![71]);
            assert_eq!(std::sync::Arc::strong_count(&log), 1);
        }
    }
    #[test]
    fn destruction_lifecycle_batch_removed_exhaustion_releases_list_once() {
        removed_exhaustion_releases_owned(true);
    }
    #[test]
    fn destruction_lifecycle_batch_removed_exhaustion_releases_strong_once() {
        removed_exhaustion_releases_owned(false);
    }
    fn removed_exhaustion_releases_owned(list: bool) {
        use crate::{component::*, entity_container::EntityList, owned_entity::StrongEntities};
        {
            let mut w = TestWorld::new();
            let id = if list {
                w.0.registry
                    .register_component::<EntityList<String>>(StoragePolicy::SparseOnly)
                    .unwrap()
            } else {
                w.0.registry
                    .register_component::<StrongEntities<1>>(StoragePolicy::SparseOnly)
                    .unwrap()
            };
            let (layout, type_id) = {
                let info = w.0.registry.component(id).unwrap();
                (info.layout, info.type_id)
            };
            w.0.notifications.layouts = vec![Some(layout)];
            w.0.notifications.type_ids = vec![Some(type_id)];
            w.0.finish_registration().unwrap();
            let ty = archetype(&mut w.0, &[id]);
            let entity = install(&mut w.0, ty);
            let target = install(
                &mut w.0,
                crate::entity::ArchetypeId {
                    table: 0,
                    variant: 0,
                },
            );
            if list {
                let owner = crate::owned_entity::OwnedEntity {
                    world_identity: w.0.identity,
                    entity: target,
                    entities: &w.0.entities,
                    pending_destruction: &w.0.pending_destruction,
                    access: &w.0.access,
                };
                let mut source = ManuallyDrop::new(EntityList::<String> {
                    head: None,
                    tail: None,
                    len: 0,
                    marker: std::marker::PhantomData,
                });
                let tick =
                    crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(2).unwrap());
                let mut ticks = crate::resource::ResourceTicks {
                    added: tick,
                    changed: tick,
                };
                unsafe {
                    <ListAccess as ComponentPolicy<EntityList<String>>>::write(
                        &mut source,
                        crate::unsafe_world_cell::UnsafeWorldCell::from_ref(&w.0),
                        crate::change_detection::ChangeMarker {
                            tick,
                            target: crate::change_detection::ChangeTarget::Resource(&mut ticks),
                        },
                    )
                }
                .push_front(owner)
                .ok()
                .unwrap();
                // 头不带 Drop；从 ManuallyDrop 读取恰好一次，交给真实 sparse 安装。
                sparse_value(&mut w.0, id, entity, unsafe { std::ptr::read(&*source) });
                assert_eq!(w.0.entity_nodes.links.borrow().slots.len(), 1);
            } else {
                sparse_value(
                    &mut w.0,
                    id,
                    entity,
                    StrongEntities::<1> {
                        entities: [target],
                        marker: std::marker::PhantomData,
                    },
                );
            }
            let reader =
                w.0.notifications
                    .subscribe(id, crate::notification::NoticeKind::Removed)
                    .unwrap();
            {
                let stream = w.0.notifications.components[id.0 as usize]
                    .removed
                    .as_mut()
                    .unwrap()
                    .get_mut();
                stream.notices.range.first = crate::notification::NoticeSequence(u64::MAX);
                stream.notices.range.end = crate::notification::NoticeSequence(u64::MAX);
            }
            assert_eq!(w.0.entities.borrow().counts(target), Some((1, 0)));
            assert!(unsafe { w.0.despawn(entity) }.unwrap());
            assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w
                .0
                .flush_destruction()))
            .is_err());
            assert!(stopped(&w.0));
            assert_eq!(w.0.entities.borrow().counts(target), Some((0, 0)));
            assert_eq!(w.0.entity_nodes.links.borrow().slots.len(), 0);
            assert_eq!(w.0.pending_destruction.pending.len(), 2);
            assert_eq!(w.0.pending_destruction.pending.get(1), Some(&target));
            // 归零仅登记，展开失败不能提前物理回收 target 或源身份。
            assert!(w.0.entities.borrow().locate(target).is_some());
            assert_eq!(w.0.entities.borrow().counts(entity), Some((0, 0)));
            let stream = w.0.notifications.components[id.0 as usize]
                .removed
                .as_ref()
                .unwrap()
                .borrow();
            assert!(stream.values.is_empty());
            assert!(stream.notices.headers.is_empty());
            drop(stream);
            assert!(w.0.flush_destruction().is_err());
            assert_eq!(w.0.entities.borrow().counts(target), Some((0, 0)));
            assert_eq!(w.0.pending_destruction.pending.len(), 2);
            w.0.notifications.unsubscribe(reader).unwrap();
        }
    }
    fn archetype(
        world: &mut World,
        components: &[crate::component::ComponentId],
    ) -> crate::entity::ArchetypeId {
        let plan = world.registry.plan_archetype(components).unwrap();
        let id = world.registry.commit_archetype(plan);
        world
            .storage
            .as_mut()
            .unwrap()
            .prepare_archetype(id)
            .unwrap();
        id
    }
    fn sparse_value<T>(
        world: &mut World,
        id: crate::component::ComponentId,
        entity: Entity,
        value: T,
    ) {
        world
            .storage
            .as_mut()
            .unwrap()
            .prepare_sparse(id, entity)
            .unwrap();
        let mut value = ManuallyDrop::new(value);
        world
            .storage
            .as_mut()
            .unwrap()
            .insert_sparse(id, entity, unsafe {
                OwningPtr::from_raw(std::ptr::NonNull::from(&mut *value).cast())
            });
    }
    #[test]
    fn destruction_lifecycle_batch_cascade_duplicate_targets_and_first_reclaim_order() {
        use crate::owned_entity::{StrongEntities, WeakEntities};
        let mut w = TestWorld::new();
        let strong = w
            .0
            .registry
            .register_component::<StrongEntities<2>>(crate::component::StoragePolicy::SparseOnly)
            .unwrap();
        let weak =
            w.0.registry
                .register_component::<WeakEntities<1>>(crate::component::StoragePolicy::SparseOnly)
                .unwrap();
        let payload =
            w.0.registry
                .register_component::<Payload>(crate::component::StoragePolicy::SparseOnly)
                .unwrap();
        w.0.finish_registration().unwrap();
        let a_type = archetype(&mut w.0, &[strong, weak, payload]);
        let b_type = archetype(&mut w.0, &[payload]);
        let a = install(&mut w.0, a_type);
        let b = install(&mut w.0, b_type);
        // b 的两份 strong 由 a 的重复位置持有；a 的最后 weak 在自身清理中归零，产生重复候选。
        w.0.entities.borrow_mut().acquire_strong(b).unwrap();
        w.0.entities.borrow_mut().acquire_weak(a).unwrap();
        sparse_value(
            &mut w.0,
            strong,
            a,
            StrongEntities::<2> {
                entities: [b, b],
                marker: std::marker::PhantomData,
            },
        );
        sparse_value(
            &mut w.0,
            weak,
            a,
            WeakEntities::<1> {
                entities: [a],
                marker: std::marker::PhantomData,
            },
        );
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        sparse_value(&mut w.0, payload, a, Payload(11, log.clone()));
        sparse_value(&mut w.0, payload, b, Payload(22, log.clone()));
        assert!(unsafe { w.0.despawn(a) }.unwrap());
        w.0.flush_destruction().unwrap();
        assert_eq!(*log.lock().unwrap(), vec![11, 22]);
        assert_eq!(w.0.change_tick, 2);
        assert_eq!(w.0.entities.borrow().counts(a), None);
        assert_eq!(w.0.entities.borrow().counts(b), None);
        let reused = install(&mut w.0, b_type);
        assert_eq!(
            crate::entity::slot_index(&reused),
            crate::entity::slot_index(&b)
        );
        assert_ne!(reused, b);
        // 新实体没有安装 payload，字段清理不执行业务析构；显式撤销身份和物理行。
        assert!(w.0.entities.borrow_mut().release_strong(reused).unwrap());
        let loc = w.0.entities.borrow_mut().begin_destroy(reused).unwrap();
        w.0.entities.borrow_mut().finish_destroy(reused).unwrap();
        assert!(w.0.entities.borrow_mut().remove(reused));
        w.0.storage
            .as_mut()
            .unwrap()
            .remove_member(b_type, loc.archetype_row().index());
        w.0.storage
            .as_mut()
            .unwrap()
            .table_mut(b_type.table)
            .unwrap()
            .retire_row(crate::table::TableRow::new(loc.table_row()));
    }
    #[test]
    fn destruction_lifecycle_batch_tick_exhaustion_keeps_pending_and_reclamation() {
        let mut w = TestWorld::new();
        w.0.finish_registration().unwrap();
        let empty = crate::entity::ArchetypeId {
            table: 0,
            variant: 0,
        };
        let entity = install(&mut w.0, empty);
        assert!(unsafe { w.0.despawn(entity) }.unwrap());
        w.0.change_tick = u64::MAX;
        assert!(w.0.flush_destruction().is_err());
        assert!(stopped(&w.0));
        assert_eq!(w.0.pending_destruction.pending.get(0), Some(&entity));
        assert!(w.0.entities.borrow().locate(entity).is_some());
        assert_eq!(w.0.entities.borrow().counts(entity), Some((0, 0)));
    }
    struct Panics(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl crate::component::Component for Panics {
        type Access = crate::component::PlainAccess;
    }
    impl Drop for Panics {
        fn drop(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            panic!("destruction callback");
        }
    }
    #[test]
    fn destruction_lifecycle_batch_panic_stops_and_never_reclaims_early() {
        let mut w = TestWorld::new();
        let id =
            w.0.registry
                .register_component::<Panics>(crate::component::StoragePolicy::SparseOnly)
                .unwrap();
        w.0.finish_registration().unwrap();
        let ty = archetype(&mut w.0, &[id]);
        let a = install(&mut w.0, ty);
        let b = install(
            &mut w.0,
            crate::entity::ArchetypeId {
                table: 0,
                variant: 0,
            },
        );
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        sparse_value(&mut w.0, id, a, Panics(count.clone()));
        // b 已完成清理但仅登记候选；a 回调失败之前绝不可回收 b。
        assert!(w.0.entities.borrow_mut().release_strong(b).unwrap());
        w.0.entities.borrow_mut().begin_destroy(b).unwrap();
        w.0.entities.borrow_mut().finish_destroy(b).unwrap();
        w.0.pending_destruction.reclamation.push(b);
        assert!(unsafe { w.0.despawn(a) }.unwrap());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.0.flush_destruction()))
                .is_err()
        );
        assert!(stopped(&w.0));
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(w.0.entities.borrow().counts(b), Some((0, 0)));
        assert!(w.0.flush_destruction().is_err());
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
    #[test]
    fn destruction_lifecycle_batch_running_and_conflicts() {
        use crate::world::AccessMode;
        let mut w = TestWorld::new();
        w.0.finish_registration().unwrap();
        for mode in [
            AccessMode::Pinning(1),
            AccessMode::Maintaining,
            AccessMode::Running,
        ] {
            w.0.access.mode.set(mode);
            assert!(w.0.flush_destruction().is_err());
        }
        w.0.access.mode.set(AccessMode::Running);
        unsafe { w.0.flush_destruction_in_run() }.unwrap();
        let mode = w.0.access.mode.replace(AccessMode::Stopped);
        assert!(matches!(mode, AccessMode::Running));
        w.0.access.mode.set(AccessMode::Idle);
        assert!(unsafe { w.0.flush_destruction_in_run() }.is_err());
    }
    #[test]
    fn destruction_lifecycle_batch_removed_panic_cleans_remaining_without_retry() {
        let mut w = TestWorld::new();
        let panic_id =
            w.0.registry
                .register_component::<Panics>(crate::component::StoragePolicy::SparseOnly)
                .unwrap();
        let payload_id =
            w.0.registry
                .register_component::<Payload>(crate::component::StoragePolicy::SparseOnly)
                .unwrap();
        w.0.notifications.layouts =
            w.0.registry
                .components
                .iter()
                .map(|info| Some(info.layout))
                .collect();
        w.0.notifications.type_ids =
            w.0.registry
                .components
                .iter()
                .map(|info| Some(info.type_id))
                .collect();
        w.0.finish_registration().unwrap();
        let r1 =
            w.0.notifications
                .subscribe(panic_id, crate::notification::NoticeKind::Removed)
                .unwrap();
        let r2 =
            w.0.notifications
                .subscribe(payload_id, crate::notification::NoticeKind::Removed)
                .unwrap();
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let tick = crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(2).unwrap());
        let mut first = ManuallyDrop::new(Panics(count.clone()));
        let mut second = ManuallyDrop::new(Payload(93, log.clone()));
        assert!(w
            .0
            .notifications
            .append_removed(panic_id, Entity::default(), tick, unsafe {
                OwningPtr::from_raw(std::ptr::NonNull::from(&mut *first).cast())
            })
            .is_ok());
        assert!(w
            .0
            .notifications
            .append_removed(payload_id, Entity::default(), tick, unsafe {
                OwningPtr::from_raw(std::ptr::NonNull::from(&mut *second).cast())
            })
            .is_ok());
        w.0.notifications.unsubscribe(r1).unwrap();
        w.0.notifications.unsubscribe(r2).unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w
            .0
            .reclaim_notifications()))
        .is_err());
        assert!(stopped(&w.0));
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(*log.lock().unwrap(), vec![93]);
        assert!(w.0.reclaim_notifications().is_err());
        assert_eq!(*log.lock().unwrap(), vec![93]);
    }
    #[test]
    fn destruction_lifecycle_batch_table_plain_release() {
        let mut w = TestWorld::new();
        let id =
            w.0.registry
                .register_component::<Payload>(crate::component::StoragePolicy::TableEligible)
                .unwrap();
        let ty = w.0.registry.register_table(&[id]).unwrap();
        w.0.finish_registration().unwrap();
        let entity = install(&mut w.0, ty);
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut source = ManuallyDrop::new(Payload(48, log.clone()));
        let row = w.0.locate(entity).unwrap().table_row();
        unsafe {
            w.0.storage
                .as_mut()
                .unwrap()
                .table_mut(ty.table)
                .unwrap()
                .column_mut(crate::table::ColumnId(0))
                .unwrap()
                .insert_raw(
                    row as usize,
                    std::ptr::from_mut(&mut *source).cast(),
                    std::alloc::Layout::new::<Payload>(),
                );
        }
        assert!(unsafe { w.0.despawn(entity) }.unwrap());
        w.0.flush_destruction().unwrap();
        assert_eq!(*log.lock().unwrap(), vec![48]);
        assert!(w
            .0
            .storage
            .as_ref()
            .unwrap()
            .row_entity(ty.table, crate::table::TableRow::new(row))
            .is_none());
        assert_eq!(w.0.entities.borrow().counts(entity), None);
    }
    struct RowProbe<const N: u32>(std::sync::Arc<std::sync::Mutex<Vec<u32>>>, bool);
    impl<const N: u32> crate::component::Component for RowProbe<N> {
        type Access = crate::component::PlainAccess;
    }
    impl<const N: u32> Drop for RowProbe<N> {
        fn drop(&mut self) {
            self.0.lock().unwrap().push(N);
            if self.1 {
                panic!("half row callback");
            }
        }
    }
    // Seam: 真实 World::despawn/flush_destruction；首列或中列 panic 后关闭枚举不得再见半行。
    #[test]
    fn destruction_lifecycle_batch_half_row_panic_settles_all_remaining_owners() {
        for (panic_column, retain_prefix) in [(0, false), (1, false), (1, true)] {
            let mut w = TestWorld::new();
            use crate::component::StoragePolicy::*;
            let a =
                w.0.registry
                    .register_component::<RowProbe<0>>(TableEligible)
                    .unwrap();
            let b =
                w.0.registry
                    .register_component::<RowProbe<1>>(TableEligible)
                    .unwrap();
            let c =
                w.0.registry
                    .register_component::<RowProbe<2>>(TableEligible)
                    .unwrap();
            let sparse =
                w.0.registry
                    .register_component::<Payload>(SparseOnly)
                    .unwrap();
            let strong =
                w.0.registry
                    .register_component::<crate::owned_entity::StrongEntities<1>>(SparseOnly)
                    .unwrap();
            w.0.registry.register_table(&[a, b, c]).unwrap();
            w.0.finish_registration().unwrap();
            let ty = archetype(&mut w.0, &[a, b, c, sparse, strong]);
            let reader = if retain_prefix {
                w.0.notifications.layouts =
                    w.0.registry
                        .components
                        .iter()
                        .map(|info| Some(info.layout))
                        .collect();
                w.0.notifications.type_ids =
                    w.0.registry
                        .components
                        .iter()
                        .map(|info| Some(info.type_id))
                        .collect();
                Some(
                    w.0.notifications
                        .subscribe(a, crate::notification::NoticeKind::Removed)
                        .unwrap(),
                )
            } else {
                None
            };
            let entity = install(&mut w.0, ty);
            let target = install(
                &mut w.0,
                crate::entity::ArchetypeId {
                    table: 0,
                    variant: 0,
                },
            );
            let row = w.0.locate(entity).unwrap().table_row();
            let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            macro_rules! put {
                ($n:literal) => {{
                    let mut value =
                        ManuallyDrop::new(RowProbe::<$n>(log.clone(), panic_column == $n));
                    unsafe {
                        w.0.storage
                            .as_mut()
                            .unwrap()
                            .table_mut(ty.table)
                            .unwrap()
                            .column_mut(crate::table::ColumnId($n))
                            .unwrap()
                            .insert_raw(
                                row as usize,
                                std::ptr::from_mut(&mut *value).cast(),
                                std::alloc::Layout::new::<RowProbe<$n>>(),
                            );
                    }
                }};
            }
            put!(0);
            put!(1);
            put!(2);
            sparse_value(&mut w.0, sparse, entity, Payload(3, log.clone()));
            sparse_value(
                &mut w.0,
                strong,
                entity,
                crate::owned_entity::StrongEntities::<1> {
                    entities: [target],
                    marker: std::marker::PhantomData,
                },
            );
            assert!(unsafe { w.0.despawn(entity) }.unwrap());
            let panic =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| w.0.flush_destruction()))
                    .unwrap_err();
            assert_eq!(panic.downcast_ref::<&str>(), Some(&"half row callback"));
            assert!(stopped(&w.0));
            assert!(
                w.0.storage
                    .as_ref()
                    .unwrap()
                    .row_entity(ty.table, crate::table::TableRow::new(row))
                    .is_none(),
                "half-consumed row must be retired before unwind escapes"
            );
            assert_eq!(
                *log.lock().unwrap(),
                if retain_prefix {
                    vec![1, 2, 3]
                } else {
                    vec![0, 1, 2, 3]
                }
            );
            if let Some(reader) = reader {
                w.0.notifications.unsubscribe(reader).unwrap();
                let released = unsafe { w.0.notifications.reclaim_removed_raw() };
                assert_eq!(released.len(), 1);
                let context = ReleaseContext {
                    world: crate::unsafe_world_cell::UnsafeWorldCell::from_ref(&w.0),
                    mode: crate::destruction::ReleaseMode::Normal,
                };
                for (id, pointer) in released {
                    unsafe {
                        release_component_inner(
                            &context,
                            w.0.registry.component(id).unwrap(),
                            OwningPtr::from_raw(std::ptr::NonNull::new(pointer).unwrap()),
                        )
                    };
                }
            }
            assert_eq!(w.0.entities.borrow().counts(target), Some((0, 0)));
            assert!(w
                .0
                .storage
                .as_ref()
                .unwrap()
                .sparse(sparse)
                .unwrap()
                .get(entity)
                .is_none());
            assert!(w
                .0
                .storage
                .as_ref()
                .unwrap()
                .sparse(strong)
                .unwrap()
                .get(entity)
                .is_none());
            assert!(w.0.flush_destruction().is_err());
            // 关闭前置枚举不再取得半行已清偿的稀疏责任。
            assert!(unsafe {
                w.0.storage
                    .as_mut()
                    .unwrap()
                    .take_one_sparse_for_shutdown_raw()
            }
            .is_none());
            drop(w);
            assert_eq!(
                *log.lock().unwrap(),
                if retain_prefix {
                    vec![1, 2, 3, 0]
                } else {
                    vec![0, 1, 2, 3]
                }
            );
            assert_eq!(std::sync::Arc::strong_count(&log), 1);
        }
    }
    // Seam: 宿主 World::flush_destruction；真实构造和冻结，不触发未实现关闭。
    #[test]
    fn destruction_lifecycle_batch_empty_and_stopped_gate() {
        let mut w = TestWorld::new();
        w.0.finish_registration().unwrap();
        let before = w.0.change_tick;
        w.0.flush_destruction().unwrap();
        assert_eq!(w.0.change_tick, before);
        let mode = w.0.access.mode.replace(crate::world::AccessMode::Stopped);
        assert!(matches!(mode, crate::world::AccessMode::Idle));
        w.0.access.mode.set(mode);
        w.0.access.mode.set(crate::world::AccessMode::Stopped);
        assert!(w.0.flush_destruction().is_err());
        assert_eq!(w.0.change_tick, before);
        assert!(stopped(&w.0));
    }
}

use crate::{
    destruction::{PendingDestruction, ReleaseContext},
    entity::Entity,
    error::EcsError,
    ptr::OwningPtr,
    world::{World, WorldRef},
};

impl PendingDestruction {
    pub(crate) fn enqueue_inner(pending: &PendingDestruction, entity: Entity) {
        let _ = pending.pending.push(entity);
    }
}

#[cfg(feature = "rc")]
impl World {
    pub(crate) fn flush_inner(world: WorldRef<'_>) -> Result<(), EcsError> {
        Self::destruction_maintenance(world, false, |cell| unsafe { flush_batches(cell) })
    }

    // 仅本批维护入口使用；守卫只访问独立门禁字段，不恢复整个 World 引用。
    pub(crate) fn destruction_maintenance(
        world: &mut World,
        running: bool,
        work: impl FnOnce(crate::unsafe_world_cell::UnsafeWorldCell<'_>) -> Result<(), EcsError>,
    ) -> Result<(), EcsError> {
        use crate::world::AccessMode;
        let mode = world.access.mode.replace(AccessMode::Stopped);
        let accepted = matches!(
            (&mode, running),
            (AccessMode::Idle, false) | (AccessMode::Running, true)
        );
        world.access.mode.set(mode);
        if !accepted
            || world.shutdown != crate::shutdown::ShutdownState::Open
            || world.storage.is_none()
        {
            return Err(failure("销毁维护需要已冻结的开放 World 及匹配授权"));
        }
        if !running {
            world.access.mode.set(AccessMode::Maintaining);
        }
        let cell = crate::unsafe_world_cell::UnsafeWorldCell::from_mut(world);
        let mut guard = MaintenanceGuard {
            access: unsafe { std::ptr::addr_of!((*cell.pointer.as_ptr()).access) },
            completed: false,
            running,
        };
        work(cell)?;
        guard.completed = true;
        Ok(())
    }
}
#[cfg(feature = "rc")]
impl World {
    pub(crate) fn flush_in_run_inner(world: &mut World) -> Result<(), EcsError> {
        Self::destruction_maintenance(world, true, |cell| unsafe { flush_batches(cell) })
    }
}
#[cfg(not(feature = "rc"))]
impl World {
    pub(crate) fn flush_inner(_world: WorldRef<'_>) -> Result<(), EcsError> {
        todo!("编号：T19，尚未实现， 非 rc 销毁收束尚未实现")
    }
}

#[cfg(feature = "rc")]
fn failure(message: &str) -> EcsError {
    pi_ecs_types::error::classified_error(pi_ecs_types::error::ErrorKind::InvalidState, message)
}
#[cfg(feature = "rc")]
struct MaintenanceGuard {
    access: *const crate::world::AccessState,
    completed: bool,
    running: bool,
}
#[cfg(feature = "rc")]
impl Drop for MaintenanceGuard {
    fn drop(&mut self) {
        // SAFETY: 独占入口保证字段保活，守卫不借业务字段，不分配也不调用用户代码。
        let access = unsafe { &*self.access };
        let old = access.mode.replace(crate::world::AccessMode::Stopped);
        if self.completed && !matches!(old, crate::world::AccessMode::Stopped) {
            access.mode.set(if self.running {
                crate::world::AccessMode::Running
            } else {
                crate::world::AccessMode::Idle
            });
        }
    }
}

#[cfg(feature = "rc")]
unsafe fn flush_batches(
    cell: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
) -> Result<(), EcsError> {
    let root = cell.pointer.as_ptr();
    // 历史值先释放，产生的归零并入首批；本轮不再次扫描通知。
    World::reclaim_notifications_authorized(cell)?;
    if unsafe { &(*root).pending_destruction.pending }.is_empty() {
        return unsafe { reclaim_identities(root) };
    }
    let clock = unsafe { &mut (*root).change_tick };
    *clock = clock
        .checked_add(1)
        .ok_or_else(|| failure("销毁变化时间耗尽"))?;
    let tick = crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(*clock).unwrap());
    let mut consumed = 0;
    loop {
        let queue = unsafe { &(*root).pending_destruction.pending };
        let end = queue.len();
        if consumed == end {
            break;
        }
        // 授权已排除外部追加者；保留原登记直到全部级联成功。
        // 回调只在 end 之后追加，下轮才处理；失败保留尚未处理的身份责任且禁止重试。
        let mut batch: Vec<_> = queue.slice(consumed..end).copied().collect();
        batch.sort_by_key(|entity| (crate::entity::domain_bits(entity), *entity));
        consumed = end;
        for entity in batch {
            let location = unsafe { &(*root).entities }
                .borrow_mut()
                .begin_destroy(entity)?;
            let archetype = unsafe { &(*root).registry }
                .archetype(location.archetype())
                .ok_or_else(|| failure("销毁位置引用了缺失原型"))?;
            // 元数据独立字段只读；物理源分配不 settle/扩容，回调仅可操作身份/节点/队列。
            let mut row_guard = DestructionRow {
                cell,
                entity,
                location,
                entries: &archetype.components,
                next: 0,
                retired: false,
            };
            for entry in &archetype.components {
                let component = entry.component;
                let info = unsafe { &(*root).registry }
                    .component(component)
                    .ok_or_else(|| failure("销毁组件元数据缺失"))?;
                let storage = unsafe { &mut (*root).storage }.as_mut().unwrap();
                let pointer = match entry.source {
                    crate::archetype::ComponentSource::Sparse => {
                        unsafe { storage.remove_sparse_raw(component, entity) }
                            .ok_or_else(|| failure("销毁实体缺少稀疏组件"))?
                    }
                    crate::archetype::ComponentSource::Table(column) => {
                        let mut table = storage
                            .table_mut(location.archetype().table)
                            .ok_or_else(|| failure("销毁 Table 缺失"))?;
                        let mut column = table
                            .column_mut(column)
                            .ok_or_else(|| failure("销毁列缺失"))?;
                        unsafe { column.remove_raw(location.table_row() as usize) }
                    }
                };
                // remove_raw 的成功结果保证非空且源分配保活；先建立责任主体，
                // 不在撤销源 owner 与 guard 之间进行检查、分配或借用通知流。
                let mut transfer = RemovedTransfer {
                    context: ReleaseContext {
                        world: cell,
                        mode: crate::destruction::ReleaseMode::Normal,
                    },
                    info,
                    pointer: Some(unsafe { std::ptr::NonNull::new_unchecked(pointer) }),
                };
                // 当前责任已由 transfer 接管；推进无分配、无回调、不可展开。
                row_guard.next += 1;
                let value =
                    unsafe { OwningPtr::from_raw(std::ptr::NonNull::new_unchecked(pointer)) };
                let notices = unsafe { &(*root).notifications };
                let result = notices.append_removed(component, entity, tick, value);
                // 通知芯保证 push 实际接管后无可展开操作。成功解除来源守卫；
                // 无订阅则直接交给策略，进入回调前解除守卫，回调 panic 不重试。
                transfer.pointer = None;
                if let Err(value) = result {
                    unsafe { release_component_inner(&transfer.context, info, value) };
                }
            }
            row_guard.retire();
            let reclaim = unsafe { &(*root).entities }
                .borrow_mut()
                .finish_destroy(entity)?;
            if reclaim {
                unsafe { &(*root).pending_destruction.reclamation }.push(entity);
            }
        }
    }
    unsafe { &mut (*root).pending_destruction.pending }.clear(0);
    unsafe { reclaim_identities(root) }
}

// 首次移出前建立栈上责任游标；不改变持久行/列布局。
#[cfg(feature = "rc")]
struct DestructionRow<'a, 'w> {
    cell: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
    entity: Entity,
    location: crate::entity::EntityLocation,
    entries: &'a [crate::archetype::ArchetypeComponent],
    next: usize,
    retired: bool,
}
#[cfg(feature = "rc")]
impl DestructionRow<'_, '_> {
    fn retire(&mut self) {
        let root = self.cell.pointer.as_ptr();
        let storage = unsafe { &mut (*root).storage }
            .as_mut()
            .expect("销毁存储缺失");
        storage.remove_member(
            self.location.archetype(),
            self.location.archetype_row().index(),
        );
        storage
            .table_mut(self.location.archetype().table)
            .expect("销毁 Table 缺失")
            .retire_row(crate::table::TableRow::new(self.location.table_row()));
        self.retired = true;
    }
}
#[cfg(feature = "rc")]
impl Drop for DestructionRow<'_, '_> {
    fn drop(&mut self) {
        if self.retired {
            return;
        }
        let root = self.cell.pointer.as_ptr();
        // 异常收束不补通知；已接管前缀由 Removed/当前 transfer 唯一负责。
        unsafe { &(*root).access }
            .mode
            .set(crate::world::AccessMode::Stopped);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            while self.next < self.entries.len() {
                let entry = &self.entries[self.next];
                let info = unsafe { &(*root).registry }
                    .component(entry.component)
                    .expect("销毁组件元数据缺失");
                let storage = unsafe { &mut (*root).storage }
                    .as_mut()
                    .expect("销毁存储缺失");
                let pointer = match entry.source {
                    crate::archetype::ComponentSource::Sparse => {
                        unsafe { storage.remove_sparse_raw(entry.component, self.entity) }
                            .expect("销毁实体缺少稀疏组件")
                    }
                    crate::archetype::ComponentSource::Table(column) => unsafe {
                        storage
                            .table_mut(self.location.archetype().table)
                            .expect("销毁 Table 缺失")
                            .column_mut(column)
                            .expect("销毁列缺失")
                            .remove_raw(self.location.table_row() as usize)
                    },
                };
                self.next += 1;
                let context = ReleaseContext {
                    world: self.cell,
                    mode: crate::destruction::ReleaseMode::Normal,
                };
                unsafe {
                    release_component_inner(
                        &context,
                        info,
                        OwningPtr::from_raw(std::ptr::NonNull::new(pointer).expect("销毁空值地址")),
                    )
                };
            }
            self.retire();
        }));
        if result.is_err() {
            std::process::abort();
        }
    }
}

// 仅拥有移出值的释放责任，不拥有源分配。storage 在整个维护展开期间保活。
#[cfg(feature = "rc")]
struct RemovedTransfer<'a, 'w> {
    context: ReleaseContext<'w>,
    info: &'a crate::component::ComponentInfo,
    pointer: Option<std::ptr::NonNull<u8>>,
}
#[cfg(feature = "rc")]
impl Drop for RemovedTransfer<'_, '_> {
    fn drop(&mut self) {
        if let Some(pointer) = self.pointer.take() {
            // 展开期间当前交接守卫先于行游标执行；二次回调 panic 前必须已标停。
            if std::thread::panicking() {
                unsafe { &(*self.context.world.pointer.as_ptr()).access }
                    .mode
                    .set(crate::world::AccessMode::Stopped);
            }
            // SAFETY: 只有通知尚未接管时保留 Some；先 take 再回调，绝不析构通知中的值。
            unsafe {
                release_component_inner(&self.context, self.info, OwningPtr::from_raw(pointer))
            };
        }
    }
}

#[cfg(feature = "rc")]
unsafe fn reclaim_identities(root: *mut World) -> Result<(), EcsError> {
    let candidates = unsafe { &mut (*root).pending_destruction.reclamation };
    let mut seen = std::collections::HashSet::new();
    // 去重仅判断成员，不遍历 HashSet：首登记顺序直接决定域内 LIFO 回收顺序。
    for entity in candidates.iter().copied() {
        if seen.insert(entity) && !unsafe { &(*root).entities }.borrow_mut().remove(entity) {
            return Err(failure("销毁回收候选不是 Null 零计数身份"));
        }
    }
    candidates.clear(0);
    Ok(())
}

pub(crate) unsafe fn release_component_inner<'a, 'w>(
    context: &ReleaseContext<'w>,
    info: &crate::component::ComponentInfo,
    value: OwningPtr<'a>,
) {
    // SAFETY: 同类型的唯一值责任交给登记时生成的具体策略适配器。
    // 适配器负责 ManuallyDrop 短借；本层不补析构、不释放来源分配、不捕获重试。
    unsafe { (info.release)(context, value) };
}
