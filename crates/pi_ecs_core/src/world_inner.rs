#[cfg(all(test, feature = "rc"))]
mod caught_notification_panics {
    use crate::{
        bundle::ComponentValue,
        component::{Component, PlainAccess, StoragePolicy},
        entity::AllocationDomain,
        error::{prelude::ErrorInspectExt, ErrorKind},
        notification::NoticeKind,
        world::World,
    };
    use std::{
        panic::{catch_unwind, AssertUnwindSafe},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };

    struct Value(Arc<AtomicUsize>);
    impl Component for Value {
        type Access = PlainAccess;
    }
    impl Drop for Value {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn assert_notice_conflict(result: std::thread::Result<()>) {
        let payload = result.expect_err("the real Changed append must panic");
        assert!(payload
            .downcast_ref::<String>()
            .is_some_and(|s| s.ends_with("BorrowMutError")));
    }

    #[test]
    fn scope_stays_stopped_after_catching_spawn_notification_panic() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new().unwrap();
        let id = world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let mut added = world.subscribe_notice(id, NoticeKind::Added).unwrap();
        let changed = world.subscribe_notice(id, NoticeKind::Changed).unwrap();
        world
            .entity_scope(|scope| {
                // SAFETY: the cell is Scope's exclusive root. Only the disjoint notification
                // directory is projected; the temporary guard ends before returning to World.
                let notices = unsafe { &(*scope.world.pointer.as_ptr()).notifications };
                let conflict = notices.components[id.0 as usize]
                    .changed
                    .as_ref()
                    .unwrap()
                    .borrow();
                let panic = catch_unwind(AssertUnwindSafe(|| {
                    let _ = scope.spawn(AllocationDomain(0), ComponentValue(Value(drops.clone())));
                }));
                drop(conflict);
                assert_notice_conflict(panic);
                let entity = notices.read(&mut added).unwrap().next().unwrap().entity;
                assert_eq!(scope.entities.borrow().counts(entity), Some((0, 0)));
                assert_eq!(
                    drops.load(Ordering::Relaxed),
                    0,
                    "the installed row still owns the value"
                );
                let read = scope.get::<Value>(entity);
                assert!(
                    matches!(read, Err(error) if error.current_kind() == &ErrorKind::InvalidState)
                );
                let next = scope.spawn(AllocationDomain(0), ());
                assert!(
                    matches!(next, Err(error) if error.current_kind() == &ErrorKind::InvalidState)
                );
                Ok(())
            })
            .unwrap();
        assert!(world.is_stopped());
        assert!(world
            .entity_scope::<()>(|_| panic!("stopped scope reopened"))
            .is_err());
        world.unsubscribe_notice(added).unwrap();
        world.unsubscribe_notice(changed).unwrap();
        world.close().unwrap();
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    fn tracked_value() -> (
        World,
        crate::component::ComponentId,
        crate::entity::Entity,
        Arc<AtomicUsize>,
    ) {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new().unwrap();
        let id = world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        world.register_resource::<u32>().unwrap();
        world.insert_resource(42_u32).unwrap();
        world.finish_registration().unwrap();
        world.subscribe_notice(id, NoticeKind::Changed).unwrap();
        world.apply_tracking().unwrap();
        let entity = world
            .spawn(AllocationDomain(0), ComponentValue(Value(drops.clone())))
            .unwrap()
            .forget();
        (world, id, entity, drops)
    }

    #[test]
    fn scope_stays_stopped_after_catching_mut_notification_panic() {
        let (mut world, id, entity, drops) = tracked_value();
        world.entity_scope(|scope| {
            let held = scope.try_acquire(entity)?;
            // SAFETY: only the notification field is projected from the Scope root;
            // neither the point write nor any other access takes this field exclusively.
            let notices = unsafe { &(*scope.world.pointer.as_ptr()).notifications };
            let conflict = notices.components[id.0 as usize].changed.as_ref().unwrap().borrow();
            let panic = catch_unwind(AssertUnwindSafe(|| {
                let mut value = scope.get_mut::<Value>(entity).unwrap().unwrap();
                let _ = &mut *value;
            }));
            drop(conflict);
            assert_notice_conflict(panic);
            assert!(matches!(scope.get::<Value>(entity), Err(error) if error.current_kind() == &ErrorKind::InvalidState));
            assert!(matches!(scope.get_mut::<Value>(entity), Err(error) if error.current_kind() == &ErrorKind::InvalidState));
            assert!(scope.try_acquire(entity).is_err());
            assert!(held.try_clone().is_err());
            drop(held);
            Ok(())
        }).unwrap();
        assert!(world.is_stopped());
        assert!(world
            .entity_scope::<()>(|_| panic!("stopped scope reopened"))
            .is_err());
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        world.close().unwrap();
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn world_write_stays_stopped_after_catching_mut_notification_panic() {
        for via_query in [true, false] {
            let (mut world, id, entity, drops) = tracked_value();
            world.write_scope(|write| {
                assert_eq!(write.resource::<u32>(), Some(&42));
                // Point queries reborrow the entire World. Leave only the RefCell
                // borrow flag, then reconstruct the cell from the same stream before
                // asserting or returning; no reference overlaps the mutable reborrow.
                let stream = &mut write.world.notifications.components[id.0 as usize].changed;
                std::mem::forget(stream.as_ref().unwrap().borrow());
                let panic = catch_unwind(AssertUnwindSafe(|| {
                    let mut value = if via_query {
                        write.query_one_mut::<&mut Value>(entity).unwrap()
                    } else {
                        write.get_mut::<Value>(entity).unwrap()
                    };
                    value.set_changed();
                }));
                let stream = &mut write.world.notifications.components[id.0 as usize].changed;
                *stream = Some(std::cell::RefCell::new(stream.take().unwrap().into_inner()));
                assert_notice_conflict(panic);
                let refused = write.query_one_mut::<&Value>(entity);
                assert!(matches!(refused, Err(error) if error.current_kind() == &ErrorKind::InvalidState), "via_query={via_query}");
                assert!(catch_unwind(AssertUnwindSafe(|| write.get::<Value>(entity))).is_err());
                assert!(catch_unwind(AssertUnwindSafe(|| write.resource::<u32>())).is_err());
            }).unwrap();
            assert!(world.is_stopped());
            assert!(world.write().is_err());
            assert_eq!(drops.load(Ordering::Relaxed), 0);
            world.close().unwrap();
            assert_eq!(drops.load(Ordering::Relaxed), 1);
        }
    }
}

#[cfg(all(test, feature = "rc"))]
mod host_registry_resource_batch {
    use crate::{
        component::{Component, PlainAccess, StoragePolicy},
        world::World,
    };
    struct Value;
    impl Component for Value {
        type Access = PlainAccess;
    }
    struct NewType;
    impl Component for NewType {
        type Access = PlainAccess;
    }
    // Seam: 宿主通知订阅只能 Idle 新增，停止态仍可清偿已有独立订阅。
    #[test]
    fn rc_host_notice_idle_registration_and_stopped_cleanup() {
        use crate::notification::NoticeKind;
        let mut world = World::new().unwrap();
        let id = world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let before = world.read_change_tick();
        let first = world.subscribe_notice(id, NoticeKind::Removed).unwrap();
        let second = world.subscribe_notice(id, NoticeKind::Removed).unwrap();
        assert!(world
            .subscribe_notice(crate::component::ComponentId(u32::MAX), NoticeKind::Removed)
            .is_err());
        world.unsubscribe_notice(first).unwrap();
        assert!(world.notifications().is_empty(&second).unwrap());
        world
            .schedule_scope(|world| {
                assert!(world.subscribe_notice(id, NoticeKind::Removed).is_err());
                Ok(())
            })
            .unwrap();
        assert_eq!(world.read_change_tick(), before);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = world.schedule_scope::<()>(|_| panic!("stop"));
        }))
        .is_err());
        assert!(world.subscribe_notice(id, NoticeKind::Removed).is_err());
        world.unsubscribe_notice(second).unwrap();
        assert!(world.is_stopped());
        world.close().unwrap();
        assert!(world.subscribe_notice(id, NoticeKind::Removed).is_err());
    }
    // Seam: 重复低编号登记不能撤销后登记类型的通知布局或订阅。
    #[test]
    fn host_registry_resource_batch_repeat_preserves_other_types() {
        let mut world = World::new().unwrap();
        let first = world
            .register_component::<Value>(StoragePolicy::TableEligible)
            .unwrap();
        let second = world
            .register_component::<NewType>(StoragePolicy::SparseOnly)
            .unwrap();
        let reader = world
            .notifications
            .subscribe(second, crate::notification::NoticeKind::Removed)
            .unwrap();
        assert_eq!(
            world
                .register_component::<Value>(StoragePolicy::TableEligible)
                .unwrap(),
            first
        );
        world.notifications.unsubscribe(reader).unwrap();
        let reader = world
            .notifications
            .subscribe(second, crate::notification::NoticeKind::Removed)
            .unwrap();
        world.notifications.unsubscribe(reader).unwrap();
        world.finish_registration().unwrap();
        assert_eq!(
            world.component_info(second).unwrap().type_id,
            std::any::TypeId::of::<NewType>()
        );
    }
    // Seam: World 公开登记、冻结及声明目录；直接通知接口校验布局目录。
    #[test]
    fn host_registry_resource_batch_component_registration_and_freeze() {
        let mut world = World::new().unwrap();
        let id = world
            .register_component::<Value>(StoragePolicy::TableEligible)
            .unwrap();
        assert!(world.table_layouts().is_err());
        assert!(world.archetypes().is_err());
        assert_eq!(
            world.component_info(id).unwrap().type_id,
            std::any::TypeId::of::<Value>()
        );
        assert_eq!(
            world
                .register_component::<Value>(StoragePolicy::TableEligible)
                .unwrap(),
            id
        );
        assert!(world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .is_err());
        let table = world.register_table(&[id]).unwrap();
        let reader = world
            .notifications
            .subscribe(id, crate::notification::NoticeKind::Removed)
            .unwrap();
        world.notifications.unsubscribe(reader).unwrap();
        world.finish_registration().unwrap();
        world.finish_registration().unwrap();
        assert_eq!(world.table_layouts().unwrap().len(), 2);
        assert_eq!(world.archetypes().unwrap().len(), 2);
        assert!(world.table_layout(table).is_some());
        assert_eq!(
            world
                .register_component::<Value>(StoragePolicy::TableEligible)
                .unwrap(),
            id
        );
        assert!(world
            .register_component::<Value>(StoragePolicy::SparseOnly)
            .is_err());
        assert!(world
            .register_component::<NewType>(StoragePolicy::TableEligible)
            .is_err());
        world.close().unwrap();
        assert!(world
            .register_component::<Value>(StoragePolicy::TableEligible)
            .is_err());
        assert!(world.finish_registration().is_err());
    }
}
#[cfg(feature = "rc")]
impl World {
    pub(super) fn subscribe_notice_inner(
        &mut self,
        _component: crate::component::ComponentId,
        _kind: crate::notification::NoticeKind,
    ) -> Result<crate::notification::NoticeSubscription, EcsError> {
        let mode = self.access.mode.replace(crate::world::AccessMode::Stopped);
        let idle = matches!(mode, crate::world::AccessMode::Idle);
        self.access.mode.set(mode);
        if !idle || self.shutdown != crate::shutdown::ShutdownState::Open {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "宿主通知登记要求开放的 Idle World",
            ));
        }
        self.notifications.subscribe(_component, _kind)
    }
    pub(super) fn unsubscribe_notice_inner(
        &mut self,
        _subscription: crate::notification::NoticeSubscription,
    ) -> Result<(), EcsError> {
        let mode = self.access.mode.replace(crate::world::AccessMode::Stopped);
        let quiescent = matches!(
            mode,
            crate::world::AccessMode::Idle | crate::world::AccessMode::Stopped
        );
        self.access.mode.set(mode);
        if !quiescent || self.shutdown != crate::shutdown::ShutdownState::Open {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "宿主通知注销要求开放的 Idle/Stopped World",
            ));
        }
        self.notifications.unsubscribe(_subscription)
    }
}
impl World {
    pub(super) fn allocate_identity_inner() -> Result<u64, EcsError> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        NEXT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .map_err(|_| {
                pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::ResourceExhausted,
                    "World 身份已永久耗尽",
                )
            })
    }
    pub(crate) fn entity_scope_inner<R>(
        world: &mut World,
        run: impl for<'s> FnOnce(&mut crate::entity_scope::EntityScope<'s>) -> Result<R, EcsError>,
    ) -> Result<R, EcsError> {
        use crate::error::{classified_error, ErrorKind};
        #[cfg(feature = "rc")]
        {
            use crate::{
                entity_scope::{EntityScope, EntityScopeSession},
                world::AccessMode,
            };
            host_idle(world)?;
            if !world.registry.frozen || world.storage.is_none() {
                return Err(classified_error(
                    ErrorKind::InvalidState,
                    "实体作用域要求冻结存储",
                ));
            }
            let world = crate::unsafe_world_cell::UnsafeWorldCell::from_mut(world);
            let root = world.pointer.as_ptr();
            // SAFETY: 唯一宿主根借用保活 World；只共享投影门禁与身份责任字段。
            // 业务字段由 Scope 在每次操作时投影，外层守卫不借用这些字段。
            let (access, entities, pending_destruction) = unsafe {
                (
                    &(*root).access,
                    &(*root).entities,
                    &(*root).pending_destruction,
                )
            };
            access.mode.set(AccessMode::Maintaining);
            let mut session = EntityScopeSession {
                access,
                completed: false,
            };
            let result = run(&mut EntityScope {
                world_identity: world.id(),
                entities,
                pending_destruction,
                world,
                access,
            });
            session.complete();
            result
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (world, run);
            Err(classified_error(
                ErrorKind::Unsupported,
                "非 RC 实体作用域延期",
            ))
        }
    }
}
impl World {
    #[cfg(feature = "rc")]
    pub(super) fn spawn_from_system_inner<'w, B: crate::bundle::Bundle>(
        access: &crate::access::RunAccess<'w>,
        partition: crate::entity::PartitionId,
        ordered_components: &[ComponentId],
        tick: crate::change_detection::ChangeTick,
        values: B,
    ) -> Result<crate::owned_entity::OwnedEntity<'w>, EcsError> {
        let cell = access.world_cell();
        // SAFETY: bind_spawn checked this mutable leaf against the frozen state. The
        // running system excludes other structural and entity views for this call.
        unsafe {
            install_spawn(
                cell,
                crate::entity::AllocationDomain(partition.0),
                Some(ordered_components),
                Some(tick),
                values,
            )
        }
    }

    pub(crate) fn spawn_inner<'w, B: crate::bundle::Bundle>(
        _world: WorldRef<'w>,
        _domain: crate::entity::AllocationDomain,
        _values: B,
    ) -> Result<crate::owned_entity::OwnedEntity<'w>, EcsError> {
        #[cfg(feature = "rc")]
        {
            use crate::{
                entity_scope::EntityScopeSession, unsafe_world_cell::UnsafeWorldCell,
                world::AccessMode,
            };
            host_idle(_world)?;
            let world = UnsafeWorldCell::from_mut(_world);
            // SAFETY: the host supplied exclusive World access and passed the Idle gate.
            unsafe {
                let access = &*std::ptr::addr_of!((*world.pointer.as_ptr()).access);
                access.mode.set(AccessMode::Maintaining);
                let mut session = EntityScopeSession {
                    access,
                    completed: false,
                };
                let result = install_spawn(world, _domain, None, None, _values);
                session.complete();
                result
            }
        }
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC 创建延期",
        ))
    }
}

#[cfg(feature = "rc")]
impl World {
    pub(super) unsafe fn install_spawn_inner<'w, B: crate::bundle::Bundle>(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
        domain: crate::entity::AllocationDomain,
        authorized: Option<&[ComponentId]>,
        system_tick: Option<crate::change_detection::ChangeTick>,
        values: B,
    ) -> Result<crate::owned_entity::OwnedEntity<'w>, EcsError> {
        unsafe { install_spawn(cell, domain, authorized, system_tick, values) }
    }
}

#[cfg(feature = "rc")]
unsafe fn install_spawn<'w, B: crate::bundle::Bundle>(
    world: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
    domain: crate::entity::AllocationDomain,
    authorized: Option<&[ComponentId]>,
    system_tick: Option<crate::change_detection::ChangeTick>,
    values: B,
) -> Result<crate::owned_entity::OwnedEntity<'w>, EcsError> {
    use crate::{
        archetype::ComponentSource,
        bundle::{BundleContext, BundleWriter},
        change_detection::ChangeTick,
        error::{classified_error, ErrorKind},
        owned_entity::OwnedEntity,
        world::AccessMode,
    };
    let raw = world.pointer.as_ptr();
    let host = system_tick.is_none();
    let expected_mode = if host {
        AccessMode::Maintaining
    } else {
        AccessMode::Running
    };
    // SAFETY: the caller excludes entity/storage views. Disjoint field borrows
    // leave resource/Local siblings alone; no storage borrow spans the Bundle callback.
    unsafe {
        let access = &*std::ptr::addr_of!((*raw).access);
        if access.mode.get() != expected_mode {
            return Err(classified_error(
                ErrorKind::InvalidState,
                "创建实体门禁状态无效",
            ));
        }
        if domain.0 as usize >= (*raw).partition_count {
            return Err(classified_error(ErrorKind::InvalidInput, "出生分区无效"));
        }
        let registry = &*std::ptr::addr_of!((*raw).registry);
        let input_types = B::component_types();
        let resolved;
        let components = if let Some(ordered) = authorized {
            if input_types.len() != ordered.len()
                || input_types.iter().zip(ordered).any(|(ty, &id)| {
                    registry
                        .component(id)
                        .is_none_or(|info| info.type_id != *ty)
                })
            {
                return Err(classified_error(
                    ErrorKind::InvalidInput,
                    "Bundle 与 Spawn 授权有序清单不一致",
                ));
            }
            ordered
        } else {
            resolved =
                input_types
                    .iter()
                    .map(|ty| {
                        registry.component_ids.get(ty).copied().ok_or_else(|| {
                            classified_error(ErrorKind::NotFound, "Bundle 组件未登记")
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            resolved.as_slice()
        };
        let plan = registry.plan_archetype(components)?;
        let archetype = plan.id;
        let entities = &*std::ptr::addr_of!((*raw).entities);
        let pending = &*std::ptr::addr_of!((*raw).pending_destruction);
        values.validate(&BundleContext {
            world_identity: world.id(),
            entities,
            pending,
            access,
        })?;
        let storage = (&mut *std::ptr::addr_of_mut!((*raw).storage))
            .as_mut()
            .ok_or_else(|| classified_error(ErrorKind::InvalidState, "创建实体要求冻结存储"))?;
        let member = if plan.is_new {
            0
        } else {
            storage.member_len(archetype)?
        };
        if member >= 1 << 20 || storage.published_rows(archetype.table)? == u32::MAX {
            return Err(classified_error(
                ErrorKind::ResourceExhausted,
                "实体行容量耗尽",
            ));
        }
        let tick = match system_tick {
            Some(tick) => tick,
            None => (*raw)
                .change_tick
                .checked_add(1)
                .and_then(std::num::NonZeroU64::new)
                .map(ChangeTick::new)
                .ok_or_else(|| classified_error(ErrorKind::ResourceExhausted, "变化时钟耗尽"))?,
        };
        let target = plan
            .archetype
            .as_ref()
            .or_else(|| registry.archetype(archetype))
            .expect("目标原型缺失");
        let mut slots = Vec::with_capacity(components.len());
        let mut layouts = Vec::with_capacity(components.len());
        let mut types = Vec::with_capacity(components.len());
        for &id in components {
            let info = registry.component(id).expect("组件登记缺失");
            let entries = target.components();
            let index = entries
                .binary_search_by_key(&id, |entry| entry.component())
                .expect("原型缺失 Bundle 组件");
            slots.push((id, entries[index].source(), info.tracking));
            layouts.push(info.layout);
            types.push(info.type_id);
        }
        let entity = entities
            .borrow_mut()
            .allocate(domain, EntityLocation::NULL)?;
        let mut install = UnpublishedBundle {
            world,
            provisional_entity: Some(entity),
            slots: Vec::with_capacity(slots.len()),
            written: 0,
        };
        // All ordinary reservation errors occur before a row is claimed. Sparse
        // preparation may reserve backing memory but does not publish a value.
        let mut reserved_sparse = Vec::new();
        let row = (|| {
            for &(id, source, _) in &slots {
                if matches!(source, ComponentSource::Sparse) {
                    storage.prepare_sparse_tracked(id, entity)?;
                    reserved_sparse.push(id);
                }
            }
            if plan.is_new {
                storage.prepare_archetype(archetype)?;
            }
            storage
                .table_mut(archetype.table)
                .expect("冻结 Table 缺失")
                .claim_row()
        })();
        let row = match row {
            Ok(row) => row,
            Err(error) => {
                for id in reserved_sparse {
                    storage.discard_sparse_reservation(id, entity);
                }
                entities.borrow_mut().discard_unpublished_spawn(entity);
                install.provisional_entity = None;
                return Err(error);
            }
        };
        let location = EntityLocation::from_parts(
            domain.0,
            member,
            row.index(),
            archetype.table,
            archetype.variant,
        );
        let mut targets = Vec::with_capacity(slots.len());
        for &(id, source, _) in &slots {
            let target = match source {
                ComponentSource::Table(column) => storage
                    .table_mut(archetype.table)
                    .unwrap()
                    .claimed_column_slot(row, column),
                ComponentSource::Sparse => storage.prepared_sparse_slot(id, entity),
            }
            .expect("已准备的安装槽缺失");
            install.slots.push((id, target));
            targets.push(target);
        }
        values.write(&mut BundleWriter {
            world,
            context: BundleContext {
                world_identity: world.id(),
                entities,
                pending,
                access,
            },
            components,
            types: &types,
            layouts: &layouts,
            targets: &targets,
            written: &mut install.written,
        });
        // A safe Bundle can catch the writer's rejection, but cannot clear it.
        assert!(
            access.mode.get() == expected_mode && install.written == components.len(),
            "Bundle 未按声明完整写入"
        );
        let storage = (&mut *std::ptr::addr_of_mut!((*raw).storage))
            .as_mut()
            .unwrap();
        // After the first publication, a failure is an internal invariant
        // violation. Neither the guard nor shutdown may see a half commit.
        let commit = IncompleteRemoval { access };
        (&mut *std::ptr::addr_of_mut!((*raw).registry)).commit_archetype(plan);
        for &(id, source, demand) in &slots {
            let ticks = pi_ecs_types::change_detection::ComponentTicks {
                added: demand.added_tick.then_some(tick),
                changed: demand.changed_tick.then_some(tick),
            };
            match source {
                ComponentSource::Table(column) => storage
                    .table_mut(archetype.table)
                    .unwrap()
                    .column_mut(column)
                    .unwrap()
                    .set_ticks(row.index() as usize, ticks.added, ticks.changed),
                ComponentSource::Sparse => storage.publish_prepared_sparse(id, entity, ticks),
            }
        }
        assert_eq!(
            storage
                .append_member(archetype, entity, row)
                .expect("目标原型成员已预检"),
            member
        );
        storage
            .table_mut(archetype.table)
            .unwrap()
            .publish_row(row, entity)
            .expect("目标表行发布失败");
        entities.borrow_mut().publish_spawn(entity, location);
        install.provisional_entity = None;
        install.written = 0;
        std::mem::forget(commit);
        if host {
            std::ptr::addr_of_mut!((*raw).change_tick).write(tick.get());
        }
        let owned = OwnedEntity {
            world_identity: (*raw).identity,
            entity,
            entities,
            pending_destruction: pending,
            access,
        };
        // Storage owns the row now. Unwind stops this world before the owned
        // handle releases its strong; normal shutdown can still clean the row.
        let mut publish = RemovalGuard {
            access,
            armed: true,
        };
        let notifications = &*std::ptr::addr_of!((*raw).notifications);
        for &(id, _, _) in &slots {
            use crate::notification::NoticeKind;
            notifications.append_change(id, NoticeKind::Added, entity, tick);
            notifications.append_change(id, NoticeKind::Changed, entity, tick);
        }
        publish.armed = false;
        Ok(owned)
    }
}

impl World {
    pub(crate) fn change_archetype_inner<B: crate::bundle::Bundle>(
        _world: WorldRef<'_>,
        _entity: Entity,
        _added: B,
        _removed: &[ComponentId],
    ) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        {
            host_idle(_world)?;
            let cell = crate::unsafe_world_cell::UnsafeWorldCell::from_mut(_world);
            // SAFETY: 宿主独占来源已通过 Idle 门禁，无活业务视图。
            unsafe { Self::change_archetype_in_cell(cell, _entity, _added, _removed) }
        }
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC 结构修改延期",
        ))
    }
}
#[cfg(feature = "rc")]
impl World {
    pub(super) unsafe fn change_archetype_in_cell_inner<B: crate::bundle::Bundle>(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
        entity: Entity,
        values: B,
        removed: &[ComponentId],
    ) -> Result<(), EcsError> {
        use crate::{
            bundle::BundleContext,
            error::{classified_error, ErrorKind},
        };
        let root = cell.pointer.as_ptr();
        // SAFETY: 调用方持结构权限；只投影同源字段，身份表 Ref 不跨 Bundle 回调。
        let (entities, registry, pending, access) = unsafe {
            (
                &(*root).entities,
                &(*root).registry,
                &(*root).pending_destruction,
                &(*root).access,
            )
        };
        let location = {
            let identities = entities.borrow();
            let location = identities
                .locate(entity)
                .ok_or_else(|| classified_error(ErrorKind::NotFound, "实体位置不存在"))?;
            if identities
                .counts(entity)
                .is_none_or(|(strong, _)| strong == 0)
            {
                return Err(classified_error(
                    ErrorKind::InvalidState,
                    "实体不可结构修改",
                ));
            }
            location
        };
        let source = registry
            .archetype(location.archetype())
            .expect("活原型缺失");
        let contains = |id: ComponentId| {
            source
                .components()
                .binary_search_by_key(&id, |entry| entry.component())
                .is_ok()
        };
        let added = B::component_types()
            .into_iter()
            .map(|ty| {
                registry
                    .component_ids
                    .get(&ty)
                    .copied()
                    .ok_or_else(|| classified_error(ErrorKind::NotFound, "新增组件未登记"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut sorted_added = added.clone();
        sorted_added.sort_unstable();
        let mut removed = removed.to_vec();
        removed.sort_unstable();
        if sorted_added.windows(2).any(|pair| pair[0] == pair[1])
            || removed.windows(2).any(|pair| pair[0] == pair[1])
            || added.iter().copied().any(contains)
            || removed.iter().copied().any(|id| !contains(id))
        {
            return Err(classified_error(
                ErrorKind::InvalidInput,
                "严格增删与当前原型不符",
            ));
        }
        values.validate(&BundleContext {
            world_identity: cell.id(),
            entities,
            pending,
            access,
        })?;
        if added.is_empty() && removed.is_empty() {
            return Ok(());
        }
        let next = unsafe { (*root).change_tick }
            .checked_add(1)
            .and_then(std::num::NonZeroU64::new)
            .ok_or_else(|| classified_error(ErrorKind::ResourceExhausted, "变化时钟耗尽"))?;
        let tick = crate::change_detection::ChangeTick::new(next);
        // SAFETY: 同源值、有序清单和严格增删已验证；直到返回均独占业务视图。
        unsafe { Self::change_components_rc(cell, entity, &added, values, &removed, tick, true) }
    }
}

#[cfg(feature = "rc")]
impl World {
    pub(crate) unsafe fn remove_components_rc(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
        entity: Entity,
        removed: &[ComponentId],
        tick: crate::change_detection::ChangeTick,
        host: bool,
    ) -> Result<(), EcsError> {
        unsafe { Self::change_components_rc(cell, entity, &[], (), removed, tick, host) }
    }

    // The caller must have checked the same-source Bundle and its ordered component IDs.
    pub(crate) unsafe fn change_components_rc<B: crate::bundle::Bundle>(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
        entity: Entity,
        added: &[ComponentId],
        values: B,
        removed: &[ComponentId],
        tick: crate::change_detection::ChangeTick,
        host: bool,
    ) -> Result<(), EcsError> {
        use crate::{
            archetype::ComponentSource,
            bundle::{BundleContext, BundleWriter},
            error::{classified_error, ErrorKind},
            world::AccessMode,
        };
        let root = cell.pointer.as_ptr();
        // SAFETY: the caller excludes conflicting views, including views of sibling
        // columns that could be relocated by claiming a row. No whole World is borrowed.
        unsafe {
            let access = &*std::ptr::addr_of!((*root).access);
            let mode = access.mode.get();
            let accepted = if host {
                matches!(mode, AccessMode::Idle | AccessMode::Maintaining)
            } else {
                matches!(mode, AccessMode::Maintaining | AccessMode::Running)
            };
            if !accepted {
                return Err(classified_error(
                    ErrorKind::InvalidState,
                    "结构移除缺少授权",
                ));
            }
            let entities = &*std::ptr::addr_of!((*root).entities);
            let location = {
                let identities = entities.borrow();
                identities.locate(entity).filter(|_| {
                    identities
                        .counts(entity)
                        .is_some_and(|(strong, _)| strong > 0)
                })
            }
            .ok_or_else(|| classified_error(ErrorKind::NotFound, "结构移除要求存活实体"))?;
            let source = location.archetype();
            let registry = &mut *std::ptr::addr_of_mut!((*root).registry);
            let entries = registry
                .archetype(source)
                .expect("活原型缺失")
                .components
                .iter()
                .map(|entry| (entry.component, entry.source))
                .collect::<Vec<_>>();
            if (added.is_empty() && removed.is_empty())
                || added
                    .iter()
                    .any(|id| entries.iter().any(|entry| entry.0 == *id))
                || removed.windows(2).any(|pair| pair[0] >= pair[1])
                || removed
                    .iter()
                    .any(|id| !entries.iter().any(|(component, _)| *component == *id))
            {
                return Err(classified_error(
                    ErrorKind::InvalidInput,
                    "严格移除与当前原型不符",
                ));
            }
            let retained: Vec<_> = entries
                .iter()
                .map(|&(component, _)| component)
                .filter(|id| !removed.contains(id))
                .chain(added.iter().copied())
                .collect();
            let plan = registry.plan_archetype(&retained)?;
            let target = plan.id;
            let storage = (&mut *std::ptr::addr_of_mut!((*root).storage))
                .as_mut()
                .expect("活实体缺少存储");
            let member = if plan.is_new {
                0
            } else {
                storage.member_len(target)?
            };
            let cross_table = target.table != source.table;
            if member >= 1 << 20 || cross_table && storage.published_rows(target.table)? == u32::MAX
            {
                return Err(classified_error(
                    ErrorKind::ResourceExhausted,
                    "目标实体行容量耗尽",
                ));
            }
            let target_entries = plan
                .archetype
                .as_ref()
                .or_else(|| registry.archetype(target))
                .expect("目标原型缺失")
                .components
                .iter()
                .map(|entry| (entry.component, entry.source))
                .collect::<Vec<_>>();
            let mut guard = RemovalGuard {
                access,
                armed: false,
            };
            let mut removed_values = DetachedComponents {
                cell,
                values: Vec::with_capacity(removed.len()),
            };
            guard.armed = true;
            let restore_idle = mode == AccessMode::Idle;
            if restore_idle {
                access.mode.set(AccessMode::Maintaining);
            }
            if plan.is_new {
                storage.prepare_archetype(target).expect("目标成员准备失败");
            }
            registry.commit_archetype(plan);
            let old_row = TableRow::new(location.table_row());
            let row = if cross_table {
                storage
                    .table_mut(target.table)
                    .unwrap()
                    .claim_row()
                    .expect("目标 Table 行准备失败")
            } else {
                old_row
            };
            for &(component, target_source) in &target_entries {
                if matches!(target_source, ComponentSource::Sparse)
                    && entries.iter().any(|&(old, source)| {
                        old == component && matches!(source, ComponentSource::Table(_))
                    })
                {
                    storage
                        .prepare_sparse_tracked(component, entity)
                        .expect("目标稀疏槽准备失败");
                }
            }
            let mut install = UnpublishedBundle {
                world: cell,
                provisional_entity: None,
                slots: Vec::with_capacity(added.len()),
                written: 0,
            };
            let mut types = Vec::with_capacity(added.len());
            let mut layouts = Vec::with_capacity(added.len());
            let mut targets = Vec::with_capacity(added.len());
            for &id in added {
                let info = registry.component(id).expect("新增组件登记缺失");
                let source = target_entries
                    .iter()
                    .find(|entry| entry.0 == id)
                    .expect("目标原型必须包含新增组件")
                    .1;
                let pointer = match source {
                    ComponentSource::Table(column) => storage
                        .table_mut(target.table)
                        .unwrap()
                        .claimed_column_slot(row, column),
                    ComponentSource::Sparse => {
                        storage
                            .prepare_sparse_tracked(id, entity)
                            .expect("新增稀疏槽准备失败");
                        storage.prepared_sparse_slot(id, entity)
                    }
                }
                .expect("新增组件目标空槽缺失");
                types.push(info.type_id);
                layouts.push(info.layout);
                targets.push(pointer);
                install.slots.push((id, pointer));
            }
            values.write(&mut BundleWriter {
                world: cell,
                context: BundleContext {
                    world_identity: cell.id(),
                    entities,
                    pending: &*std::ptr::addr_of!((*root).pending_destruction),
                    access,
                },
                components: added,
                types: &types,
                layouts: &layouts,
                targets: &targets,
                written: &mut install.written,
            });
            let mode = access.mode.replace(AccessMode::Stopped);
            assert!(
                matches!(mode, AccessMode::Maintaining | AccessMode::Running)
                    && install.written == added.len(),
                "新增 Bundle 未按声明完整写入"
            );
            access.mode.set(mode);
            // A safe Bundle may drop its own strong handle to this entity
            // while writing. Check before detaching any source responsibility.
            assert!(
                entities
                    .borrow()
                    .counts(entity)
                    .is_some_and(|(strong, _)| strong > 0),
                "新增 Bundle 写入期间源实体已归零",
            );
            let registry = &*std::ptr::addr_of!((*root).registry);
            let storage = (&mut *std::ptr::addr_of_mut!((*root).storage))
                .as_mut()
                .unwrap();
            // If a storage invariant fails during ownership transfer, neither row can
            // be interpreted by shutdown. Stop and abort rather than scan a partial row.
            let commit = IncompleteRemoval { access };
            // No callbacks run during the transfer; no source buffer can grow here.
            for (component, source_kind) in entries {
                let target_source = target_entries
                    .iter()
                    .find(|entry| entry.0 == component)
                    .map(|entry| entry.1);
                if matches!(
                    (source_kind, target_source),
                    (ComponentSource::Sparse, Some(ComponentSource::Sparse))
                ) || !cross_table && matches!(source_kind, ComponentSource::Table(_))
                {
                    continue;
                }
                let (pointer, ticks) = match source_kind {
                    ComponentSource::Table(column) => {
                        let (ptr, added, changed) = storage
                            .table_mut(source.table)
                            .unwrap()
                            .column_mut(column)
                            .unwrap()
                            .remove_raw_with_ticks(old_row.index() as usize);
                        (
                            ptr,
                            pi_ecs_types::change_detection::ComponentTicks { added, changed },
                        )
                    }
                    ComponentSource::Sparse => {
                        let ticks = storage
                            .sparse_ticks(component, entity)
                            .expect("活稀疏 tick 缺失");
                        (
                            storage
                                .remove_sparse_raw(component, entity)
                                .expect("活稀疏值缺失"),
                            ticks,
                        )
                    }
                };
                let pointer = std::ptr::NonNull::new(pointer).expect("活值地址为空");
                let info = registry.component(component).expect("组件登记缺失");
                match target_source {
                    Some(ComponentSource::Sparse) => storage.insert_sparse_tracked(
                        component,
                        entity,
                        crate::ptr::OwningPtr::from_raw(pointer),
                        ticks,
                    ),
                    Some(ComponentSource::Table(column)) => {
                        let mut table = storage.table_mut(target.table).unwrap();
                        let mut column = table.column_mut(column).unwrap();
                        column.insert_raw(row.index() as usize, pointer.as_ptr(), info.layout);
                        column.set_ticks(row.index() as usize, ticks.added, ticks.changed);
                    }
                    None => removed_values.values.push((info, Some(pointer))),
                }
            }
            for &id in added {
                let source = target_entries.iter().find(|entry| entry.0 == id).unwrap().1;
                let demand = registry.component(id).unwrap().tracking;
                let ticks = pi_ecs_types::change_detection::ComponentTicks {
                    added: demand.added_tick.then_some(tick),
                    changed: demand.changed_tick.then_some(tick),
                };
                match source {
                    ComponentSource::Table(column) => storage
                        .table_mut(target.table)
                        .unwrap()
                        .column_mut(column)
                        .unwrap()
                        .set_ticks(row.index() as usize, ticks.added, ticks.changed),
                    ComponentSource::Sparse => storage.publish_prepared_sparse(id, entity, ticks),
                }
            }
            assert_eq!(
                storage
                    .append_member(target, entity, row)
                    .expect("目标成员发布失败"),
                member
            );
            let target_location = EntityLocation::from_parts(
                location.partition().0,
                member,
                row.index(),
                target.table,
                target.variant,
            );
            entities
                .borrow_mut()
                .relocate(entity, target_location)
                .expect("结构移除丢失原身份");
            storage.remove_member(source, location.archetype_row().index());
            if cross_table {
                storage.table_mut(source.table).unwrap().retire_row(old_row);
                storage
                    .table_mut(target.table)
                    .unwrap()
                    .publish_row(row, entity)
                    .expect("目标行发布失败");
            }
            if host {
                std::ptr::addr_of_mut!((*root).change_tick).write(tick.get());
            }
            install.written = 0;
            std::mem::forget(commit);
            let notifications = &*std::ptr::addr_of!((*root).notifications);
            for &id in added {
                use crate::notification::NoticeKind;
                notifications.append_change(id, NoticeKind::Added, entity, tick);
                notifications.append_change(id, NoticeKind::Changed, entity, tick);
            }
            let context = removed_values.context();
            for (info, pointer) in &mut removed_values.values {
                let value = crate::ptr::OwningPtr::from_raw(pointer.expect("移除责任缺失"));
                let result = notifications.append_removed(info.id, entity, tick, value);
                *pointer = None;
                if let Err(value) = result {
                    crate::destruction::release_value(&context, info, value);
                }
            }
            if restore_idle {
                access.mode.set(AccessMode::Idle);
            }
            guard.armed = false;
            Ok(())
        }
    }
}

#[cfg(feature = "rc")]
struct UnpublishedBundle<'w> {
    world: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
    provisional_entity: Option<Entity>,
    slots: Vec<(ComponentId, std::ptr::NonNull<u8>)>,
    written: usize,
}

#[cfg(feature = "rc")]
impl Drop for UnpublishedBundle<'_> {
    fn drop(&mut self) {
        if self.provisional_entity.is_none() && self.written == 0 {
            return;
        }
        let root = self.world.pointer.as_ptr();
        // SAFETY: these slots were never published; the original allocation
        // and registry survive this guard and no dynamic borrow spans release.
        unsafe {
            (&*std::ptr::addr_of!((*root).access))
                .mode
                .set(crate::world::AccessMode::Stopped);
            if let Some(entity) = self.provisional_entity.take() {
                (&*std::ptr::addr_of!((*root).entities))
                    .borrow_mut()
                    .discard_unpublished_spawn(entity);
            }
            let registry = &*std::ptr::addr_of!((*root).registry);
            let context = crate::destruction::ReleaseContext {
                world: self.world,
                mode: crate::destruction::ReleaseMode::Normal,
            };
            for &(id, pointer) in &self.slots[..self.written] {
                let info = registry.component(id).expect("未发布组件无登记");
                crate::destruction::release_value(
                    &context,
                    info,
                    crate::ptr::OwningPtr::from_raw(pointer),
                );
            }
        }
    }
}

#[cfg(feature = "rc")]
struct IncompleteRemoval<'a> {
    access: &'a crate::world::AccessState,
}

#[cfg(feature = "rc")]
impl Drop for IncompleteRemoval<'_> {
    fn drop(&mut self) {
        self.access.mode.set(crate::world::AccessMode::Stopped);
        std::process::abort();
    }
}

#[cfg(feature = "rc")]
struct RemovalGuard<'a> {
    access: &'a crate::world::AccessState,
    armed: bool,
}

#[cfg(feature = "rc")]
impl Drop for RemovalGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.access.mode.set(crate::world::AccessMode::Stopped);
        }
    }
}

#[cfg(feature = "rc")]
struct DetachedComponents<'a, 'w> {
    cell: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
    values: Vec<(&'a ComponentInfo, Option<std::ptr::NonNull<u8>>)>,
}

#[cfg(feature = "rc")]
impl<'w> DetachedComponents<'_, 'w> {
    fn context(&self) -> crate::destruction::ReleaseContext<'w> {
        crate::destruction::ReleaseContext {
            world: self.cell,
            mode: crate::destruction::ReleaseMode::Normal,
        }
    }
}

#[cfg(feature = "rc")]
impl Drop for DetachedComponents<'_, '_> {
    fn drop(&mut self) {
        let context = self.context();
        for (info, pointer) in &mut self.values {
            if let Some(pointer) = pointer.take() {
                // SAFETY: these slots were detached once and never handed to Removed.
                unsafe {
                    let root = self.cell.pointer.as_ptr();
                    (&*std::ptr::addr_of!((*root).access))
                        .mode
                        .set(crate::world::AccessMode::Stopped);
                    crate::destruction::release_value(
                        &context,
                        info,
                        crate::ptr::OwningPtr::from_raw(pointer),
                    );
                }
            }
        }
    }
}
impl World {
    pub(crate) fn read_change_tick_inner(_world: &World) -> crate::change_detection::ChangeTick {
        #[cfg(feature = "rc")]
        let tick = _world.change_tick;
        #[cfg(not(feature = "rc"))]
        let tick = _world
            .change_tick
            .load(std::sync::atomic::Ordering::Relaxed);
        crate::change_detection::ChangeTick::new(
            std::num::NonZeroU64::new(tick).expect("World 非零变化时钟"),
        )
    }
}
impl World {
    pub(crate) fn try_acquire_inner(
        world: WorldRef<'_>,
        entity: Entity,
    ) -> Result<crate::owned_entity::OwnedEntity<'_>, EcsError> {
        #[cfg(feature = "rc")]
        {
            host_idle(world)?;
            world.entities.get_mut().acquire_strong(entity)?;
            Ok(crate::owned_entity::OwnedEntity {
                world_identity: world.identity,
                entity,
                entities: &world.entities,
                pending_destruction: &world.pending_destruction,
                access: &world.access,
            })
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (world, entity);
            Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::Unsupported,
                "非 RC 拥有实体后端延期",
            ))
        }
    }
}
impl World {
    pub(crate) fn pin_inner(_world: WorldRef<'_>) -> Result<crate::world::WorldPin<'_>, EcsError> {
        #[cfg(feature = "rc")]
        {
            host_idle(_world)?;
            _world.access.mode.set(crate::world::AccessMode::Pinning(1));
            Ok(crate::world::WorldPin { world: _world })
        }
        #[cfg(not(feature = "rc"))]
        todo!("编号：T86，尚未实现， 非 RC 多读授权申请")
    }
}
impl<'w> crate::world::WorldPin<'w> {
    pub(crate) fn pin_get_inner<'a, C: crate::component::Component>(
        _pin: &'a crate::world::WorldPin<'w>,
        _entity: Entity,
    ) -> Option<crate::component::ComponentRead<'a, 'w, C>> {
        #[cfg(feature = "rc")]
        {
            let world = crate::unsafe_world_cell::UnsafeWorldCell::from_ref(_pin.world);
            let location = _pin.world.locate(_entity)?;
            // SAFETY: the original pin covers shared access for the returned short borrow.
            unsafe { World::read_component_at::<C>(world, _entity, location) }
        }
        #[cfg(not(feature = "rc"))]
        panic!("非 RC pin 读取延期")
    }
}
impl<'w> crate::world::WorldPin<'w> {
    pub(crate) fn drop_pin_inner(_pin: &mut crate::world::WorldPin<'w>) {
        #[cfg(feature = "rc")]
        {
            let mode = _pin
                .world
                .access
                .mode
                .replace(crate::world::AccessMode::Stopped);
            match mode {
                crate::world::AccessMode::Pinning(1) => {
                    _pin.world.access.mode.set(crate::world::AccessMode::Idle)
                }
                crate::world::AccessMode::Pinning(n) if n > 1 => _pin
                    .world
                    .access
                    .mode
                    .set(crate::world::AccessMode::Pinning(n - 1)),
                crate::world::AccessMode::Stopped => {}
                _ => std::process::abort(),
            }
        }
        #[cfg(not(feature = "rc"))]
        todo!("编号：T87，尚未实现， 非 RC 读授权释放")
    }
}
impl World {
    pub(crate) fn read_scope_inner<'w, R>(
        _world: WorldRef<'w>,
        _run: impl for<'a> FnOnce(&'a crate::world::WorldPin<'w>) -> R,
    ) -> Result<R, EcsError> {
        let pin = Self::pin_inner(_world)?;
        Ok(_run(&pin))
    }
}
impl World {
    pub(crate) fn write_inner(
        _world: WorldRef<'_>,
    ) -> Result<crate::world::WorldWrite<'_>, EcsError> {
        #[cfg(feature = "rc")]
        {
            host_idle(_world)?;
            let next = _world.change_tick.checked_add(1).ok_or_else(|| {
                pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::ResourceExhausted,
                    "变化时钟耗尽",
                )
            })?;
            _world.change_tick = next;
            _world
                .access
                .mode
                .set(crate::world::AccessMode::Maintaining);
            let tick = crate::change_detection::ChangeTick::new(
                std::num::NonZeroU64::new(next).expect("非零时钟"),
            );
            Ok(crate::world::WorldWrite {
                world: _world,
                tick,
                point_declaration: None,
            })
        }
        #[cfg(not(feature = "rc"))]
        todo!("编号：T88，尚未实现， 非 RC 宿主独占组件授权")
    }
}
impl<'w> crate::world::WorldWrite<'w> {
    pub(crate) fn drop_write_inner(_access: &mut crate::world::WorldWrite<'w>) {
        #[cfg(feature = "rc")]
        {
            let mode = _access
                .world
                .access
                .mode
                .replace(crate::world::AccessMode::Stopped);
            match mode {
                crate::world::AccessMode::Maintaining if !std::thread::panicking() => _access
                    .world
                    .access
                    .mode
                    .set(crate::world::AccessMode::Idle),
                crate::world::AccessMode::Maintaining | crate::world::AccessMode::Stopped => {}
                _ => std::process::abort(),
            }
        }
        #[cfg(not(feature = "rc"))]
        todo!("编号：T89，尚未实现， 非 RC 写授权释放")
    }
}
impl World {
    pub(crate) fn write_scope_inner<'w, R>(
        _world: WorldRef<'w>,
        _run: impl for<'a> FnOnce(&'a mut crate::world::WorldWrite<'w>) -> R,
    ) -> Result<R, EcsError> {
        let mut write = Self::write_inner(_world)?;
        Ok(_run(&mut write))
    }
}
#[cfg(feature = "rc")]
impl<'w> crate::world::WorldWrite<'w> {
    pub(crate) fn write_get_inner<'a, C: crate::component::Component>(
        _access: &'a crate::world::WorldWrite<'w>,
        _entity: Entity,
    ) -> Option<crate::component::ComponentRead<'a, 'a, C>> {
        _access.require_active();
        let location = _access.world.locate(_entity)?;
        let world = crate::unsafe_world_cell::UnsafeWorldCell::from_ref(_access.world);
        // SAFETY: shared reborrow of the exclusive token excludes mutable results.
        unsafe { World::read_component_at::<C>(world, _entity, location) }
    }
}
#[cfg(not(feature = "rc"))]
impl<'w> crate::world::WorldWrite<'w> {
    pub(crate) fn write_get_inner<'a, C: crate::component::Component>(
        _access: &'a crate::world::WorldWrite<'w>,
        _entity: Entity,
    ) -> Option<crate::component::ComponentRead<'a, 'w, C>> {
        todo!("编号：T90，尚未实现， 授权共享读取")
    }
}
#[cfg(feature = "rc")]
impl<'w> crate::world::WorldWrite<'w> {
    pub(crate) fn write_get_mut_inner<'a, C: crate::component::Component>(
        _access: &'a mut crate::world::WorldWrite<'w>,
        _entity: Entity,
    ) -> Option<crate::component::ComponentWrite<'a, 'a, C>> {
        _access.require_active();
        let location = _access.world.locate(_entity)?;
        let tick = _access.tick;
        let world = crate::unsafe_world_cell::UnsafeWorldCell::from_mut(_access.world);
        // SAFETY: the mutable token reborrow excludes conflicting storage access.
        unsafe { World::write_component_at::<C>(world, _entity, location, tick) }
    }
}
#[cfg(not(feature = "rc"))]
impl<'w> crate::world::WorldWrite<'w> {
    pub(crate) fn write_get_mut_inner<'a, C: crate::component::Component>(
        _access: &'a mut crate::world::WorldWrite<'w>,
        _entity: Entity,
    ) -> Option<crate::component::ComponentWrite<'a, 'w, C>> {
        todo!("编号：T91，尚未实现， 授权写读取")
    }
}
#[cfg(feature = "rc")]
impl<'w> crate::world::WorldWrite<'w> {
    pub(crate) fn write_query_one_inner<'a, Q: crate::query::QueryData>(
        _access: &'a mut crate::world::WorldWrite<'w>,
        _entity: Entity,
    ) -> Result<Q::Item<'a, 'a>, EcsError> {
        use crate::{
            error::{classified_error, ErrorKind},
            query::{
                missing_entity, unmatched_entity, FetchStateResult, NoFilter,
                QueryAccessDeclaration, QueryFilter,
            },
            query_fetch::{FetchAccess, FetchBinding, FetchEntity, FetchScope, QueryFetch},
            unsafe_world_cell::UnsafeWorldCell,
        };
        let world = &*_access.world;
        if world.access.mode.get() != crate::world::AccessMode::Maintaining
            || !world.registry.frozen
        {
            return Err(classified_error(
                ErrorKind::InvalidState,
                "点查要求冻结的写授权",
            ));
        }
        let state = Q::init_state(world)?;
        let data = Q::declare_access(&state)?;
        let declaration = QueryAccessDeclaration {
            data,
            filter: NoFilter::declare_access(&())?,
        }
        .to_access()?;
        if !declaration.tick_reads.is_empty() {
            return Err(classified_error(
                ErrorKind::InvalidState,
                "点查需要显式观察窗口",
            ));
        }
        let location = world
            .locate(_entity)
            .ok_or_else(|| missing_entity(_entity))?;
        let target = FetchEntity {
            entity: _entity,
            location,
        };
        let scope = FetchScope::Archetype {
            archetype: location.archetype(),
        };
        let binding = FetchBinding {
            storage: world
                .storage
                .as_ref()
                .ok_or_else(|| classified_error(ErrorKind::InvalidState, "冻结点查存储缺失"))?,
            world_identity: world.identity,
            archetypes: &world.registry.archetypes,
        };
        let plan = match Q::init_fetch_state(&state, scope, &binding)? {
            FetchStateResult::Ready(plan) => plan,
            FetchStateResult::NotMatched => {
                return Err(unmatched_entity(_entity));
            }
            FetchStateResult::NeedsVariants => {
                return Err(classified_error(
                    ErrorKind::InvalidState,
                    "具体原型不能请求变体",
                ));
            }
        };
        // 声明与 World 是令牌的独立字段；结果可保留 FetchAccess 至独占短借用结束。
        let declaration = _access.point_declaration.insert(declaration);
        let access = FetchAccess {
            world: UnsafeWorldCell::from_mut(&mut *_access.world),
            declaration,
            window: None,
            write_tick: Some(_access.tick),
        };
        // SAFETY: 计划只绑定同源目标原型，Q 的声明已排除别名，且直到结果结束令牌保持独占。
        unsafe {
            let mut fetch = Q::Fetch::prepare(&access, scope, &state, &plan)?;
            Ok(fetch.fetch_at(&target))
        }
    }
}
#[cfg(not(feature = "rc"))]
impl<'w> crate::world::WorldWrite<'w> {
    pub(crate) fn write_query_one_inner<'a, Q: crate::query::QueryData>(
        _access: &'a mut crate::world::WorldWrite<'w>,
        _entity: Entity,
    ) -> Result<Q::Item<'a, 'w>, EcsError> {
        todo!("编号：T92，尚未实现， 目标原型组合及别名验证")
    }
}
impl World {
    pub(crate) fn query_inner<Q: crate::query::QueryData>(
        _world: &mut World,
    ) -> Result<crate::query::QueryState<Q>, EcsError> {
        Self::query_filtered_inner::<Q, crate::query::NoFilter>(_world)
    }
}
impl World {
    pub(crate) fn query_filtered_inner<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        _world: &mut World,
    ) -> Result<crate::query::QueryState<Q, F>, EcsError> {
        #[cfg(feature = "rc")]
        return crate::query::QueryState::new(_world);
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC 查询状态延期",
        ))
    }
}

#[cfg(feature = "rc")]
impl World {
    pub(super) unsafe fn scope_write_component_at_inner<
        'a,
        'w: 'a,
        C: crate::component::Component,
    >(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
        entity: Entity,
        location: EntityLocation,
    ) -> Option<crate::component::ComponentWrite<'a, 'w, C>> {
        // SAFETY: Scope holds the unique operation borrow; the clock is disjoint from all value views.
        unsafe {
            Self::write_component_with_tick::<C>(cell, entity, location, || {
                let root = cell.pointer.as_ptr();
                let clock = &mut (*root).change_tick;
                let Some(next) = clock.checked_add(1).and_then(std::num::NonZeroU64::new) else {
                    (*root).access.mode.set(crate::world::AccessMode::Stopped);
                    panic!("变化时钟耗尽");
                };
                *clock = next.get();
                crate::change_detection::ChangeTick::new(next)
            })
        }
    }

    pub(super) unsafe fn read_component_at_inner<'a, 'w: 'a, C: crate::component::Component>(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
        entity: Entity,
        location: EntityLocation,
    ) -> Option<crate::component::ComponentRead<'a, 'w, C>> {
        unsafe {
            let (storage, info, source) = Self::component_source_at::<C>(cell, location)?;
            let pointer = match source {
                crate::archetype::ComponentSource::Table(column) => storage
                    .table(location.archetype().table)
                    .expect("活行 Table 缺失")
                    .column_address(column)
                    .expect("活行列缺失")
                    .as_ref()
                    .bind_view()
                    .read(location.table_row() as usize),
                crate::archetype::ComponentSource::Sparse => storage
                    .sparse(info.id)
                    .expect("稀疏存储缺失")
                    .bind_view()
                    .read(entity)
                    .expect("活稀疏值缺失"),
            };
            Some(crate::component::read_component::<C>(
                &*pointer.as_non_null().cast::<C>().as_ptr(),
                cell,
            ))
        }
    }

    pub(crate) unsafe fn write_component_at<'a, 'w: 'a, C: crate::component::Component>(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
        entity: Entity,
        location: EntityLocation,
        tick: crate::change_detection::ChangeTick,
    ) -> Option<crate::component::ComponentWrite<'a, 'w, C>> {
        unsafe { Self::write_component_with_tick::<C>(cell, entity, location, || tick) }
    }

    unsafe fn write_component_with_tick<'a, 'w: 'a, C: crate::component::Component>(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
        entity: Entity,
        location: EntityLocation,
        tick: impl FnOnce() -> crate::change_detection::ChangeTick,
    ) -> Option<crate::component::ComponentWrite<'a, 'w, C>> {
        use crate::change_detection::{ChangeMarker, ChangeTarget, NoticeWrite};
        unsafe {
            let (storage, info, source) = Self::component_source_at::<C>(cell, location)?;
            let tick = tick();
            // Read obtains only the raw address, without creating a shared value reference.
            // The original exclusive authorization independently permits the write.
            let (address, ticks) = match source {
                crate::archetype::ComponentSource::Table(column) => {
                    let view = storage
                        .table(location.archetype().table)
                        .expect("活行 Table 缺失")
                        .column_address(column)
                        .expect("活行列缺失")
                        .as_ref()
                        .bind_view();
                    let row = location.table_row() as usize;
                    let address = view.read(row).as_non_null();
                    let (_, ticks) = view.write(row);
                    (address, ticks)
                }
                crate::archetype::ComponentSource::Sparse => {
                    let view = storage.sparse(info.id).expect("稀疏存储缺失").bind_view();
                    let address = view.read(entity).expect("活稀疏值缺失").as_non_null();
                    let (_, ticks) = view.write(entity).expect("活稀疏值缺失");
                    (address, ticks)
                }
            };
            let marker = ChangeMarker {
                tick,
                target: ChangeTarget::Component {
                    ticks,
                    demand: &info.tracking,
                    notice: info.tracking.changed_notice.then(|| NoticeWrite {
                        notifications: &*std::ptr::addr_of!((*cell.pointer.as_ptr()).notifications),
                        access: &*std::ptr::addr_of!((*cell.pointer.as_ptr()).access),
                        component: info.id,
                        entity,
                        borrow: std::marker::PhantomData,
                    }),
                },
            };
            Some(crate::component::write_component::<C>(
                &mut *address.cast::<C>().as_ptr(),
                cell,
                marker,
            ))
        }
    }

    unsafe fn component_source_at<'w, C: crate::component::Component>(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
        location: EntityLocation,
    ) -> Option<(
        &'w crate::storage::WorldStorage,
        &'w ComponentInfo,
        crate::archetype::ComponentSource,
    )> {
        unsafe {
            let world = cell.pointer.as_ptr();
            let registry = &*std::ptr::addr_of!((*world).registry);
            let id = registry.component_ids.get(&std::any::TypeId::of::<C>())?;
            let archetype = registry
                .archetype(location.archetype())
                .expect("活实体原型缺失");
            let index = archetype
                .components
                .binary_search_by_key(id, |entry| entry.component)
                .ok()?;
            let storage = (&*std::ptr::addr_of!((*world).storage))
                .as_ref()
                .expect("活实体存储缺失");
            Some((
                storage,
                registry.component(*id).expect("组件登记缺失"),
                archetype.components[index].source,
            ))
        }
    }
}

#[cfg(feature = "rc")]
impl crate::world::WorldWrite<'_> {
    pub(crate) fn require_active(&self) {
        // Option-returning APIs reserve None for a missing value.
        assert!(
            self.world.access.mode.get() == crate::world::AccessMode::Maintaining,
            "组件或资源访问要求有效的宿主写授权"
        );
    }
}

#[cfg(feature = "rc")]
fn host_idle(world: &World) -> Result<(), EcsError> {
    // Cell 不实现 Copy 的模式以 replace/read/restore 短读，绝不解除授权。
    let mode = world.access.mode.replace(crate::world::AccessMode::Stopped);
    let idle = matches!(mode, crate::world::AccessMode::Idle);
    world.access.mode.set(mode);
    if !idle || !matches!(world.shutdown, crate::shutdown::ShutdownState::Open) {
        return Err(pi_ecs_types::error::classified_error(
            pi_ecs_types::error::ErrorKind::InvalidState,
            "宿主入口要求未关闭且 Idle",
        ));
    }
    Ok(())
}
use crate::{
    archetype::Archetype,
    component::{ComponentId, ComponentInfo, StoragePolicy},
    entity::ArchetypeId,
    entity::{Entity, EntityLocation},
    error::EcsError,
    partition::PartitionMove,
    table::{TableLayout, TableRow},
    world::{World, WorldRef},
};
impl World {
    pub(crate) fn register_component_inner<T: crate::component::Component>(
        world: &mut World,
        storage: StoragePolicy,
    ) -> Result<ComponentId, EcsError> {
        #[cfg(feature = "rc")]
        host_idle(world)?;
        let id = world.registry.register_component::<T>(storage)?;
        let info = world.registry.component(id).expect("已登记组件");
        let index = id.0 as usize;
        if world.notifications.components.len() <= index {
            world.notifications.components.resize_with(index + 1, || {
                crate::notification::ComponentNoticeStreams {
                    added: None,
                    changed: None,
                    removed: None,
                }
            });
        }
        if world.notifications.layouts.len() <= index {
            world.notifications.layouts.resize(index + 1, None);
        }
        if world.notifications.type_ids.len() <= index {
            world.notifications.type_ids.resize(index + 1, None);
        }
        world.notifications.layouts[index] = Some(info.layout);
        world.notifications.type_ids[index] = Some(info.type_id);
        Ok(id)
    }
}
impl World {
    pub(crate) fn register_table_inner(
        world: &mut World,
        components: &[ComponentId],
    ) -> Result<ArchetypeId, EcsError> {
        #[cfg(feature = "rc")]
        host_idle(world)?;
        world.registry.register_table(components)
    }
}
impl World {
    pub(crate) fn finish_registration_inner(world: &mut World) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        host_idle(world)?;
        if world.storage.is_some() {
            return Ok(());
        }
        let next_epoch = world.layout_epoch.checked_add(1).ok_or_else(|| {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "布局代次耗尽",
            )
        })?;
        // 先构建全部物理存储；失败时 registry 未冻结，局部存储直接丢弃。
        let storage = build_world_storage_inner(&world.registry)?;
        // 再提交元数据冻结；其失败保持 registry 未冻结，不安装 storage。
        world.registry.freeze(world.partition_count)?;
        world.storage = Some(storage);
        world.layout_epoch = next_epoch;
        Ok(())
    }
}
/// 按当前 registry 构建全局物理存储；不提交冻结、不安装到 World。
/// 顺序固定：Table 升序 → 组件升序，构建 Table/列/全局稀疏/基础原型成员；不按分区复制。
pub(crate) fn build_world_storage_inner(
    registry: &crate::registry::TypeRegistry,
) -> Result<crate::storage::WorldStorage, EcsError> {
    let component_count = registry.components.len();
    let mut sparse: Vec<Option<pi_ecs_storage::sparse_set::SparseSetStorage>> =
        (0..component_count).map(|_| None).collect();
    for info in &registry.components {
        let slot = info.id.0 as usize;
        if slot >= sparse.len() {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::Internal,
                "组件编号与注册表长度不一致",
            ));
        }
        sparse[slot] = Some(pi_ecs_storage::sparse_set::SparseSetStorage::sparse_array(
            info.id,
            info.layout,
        )?);
    }

    let mut tables = Vec::with_capacity(registry.tables.len());
    for layout in &registry.tables {
        let mut columns = Vec::with_capacity(layout.columns().len());
        for &component in layout.columns() {
            let info = registry
                .components
                .get(component.0 as usize)
                .ok_or_else(|| {
                    pi_ecs_types::error::classified_error(
                        pi_ecs_types::error::ErrorKind::Internal,
                        "Table 列引用了未登记组件",
                    )
                })?;
            columns.push(pi_ecs_storage::column::Column::new(component, info.layout)?);
        }
        tables.push(crate::table::Table::new(
            layout,
            columns.into_boxed_slice(),
        )?);
    }

    crate::storage::WorldStorage::new(tables.into_boxed_slice(), sparse.into_boxed_slice())
}
impl World {
    pub(crate) fn register_archetype_inner(
        world: &mut World,
        components: &[ComponentId],
    ) -> Result<ArchetypeId, EcsError> {
        #[cfg(feature = "rc")]
        host_idle(world)?;
        let plan = world.registry.plan_archetype(components)?;
        install_archetype(world, plan)
    }
}
fn install_archetype(
    world: &mut World,
    plan: crate::registry::ArchetypePlan,
) -> Result<ArchetypeId, EcsError> {
    if plan.is_new {
        let storage = world.storage.as_mut().ok_or_else(|| {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::Internal,
                "冻结 World 缺少物理存储",
            )
        })?;
        storage.prepare_archetype(plan.id)?;
    }
    Ok(world.registry.commit_archetype(plan))
}

impl World {
    pub(crate) fn component_info_inner(world: &World, id: ComponentId) -> Option<&ComponentInfo> {
        world.registry.component(id)
    }
}
pub(crate) fn component_id_inner<C: crate::component::Component>(
    world: &World,
) -> Option<ComponentId> {
    world
        .registry
        .component_ids
        .get(&std::any::TypeId::of::<C>())
        .copied()
}
impl World {
    pub(crate) fn table_layout_inner(world: &World, id: ArchetypeId) -> Option<&TableLayout> {
        world.registry.table(id)
    }
}
impl World {
    pub(crate) fn archetype_inner(world: &World, id: ArchetypeId) -> Option<&Archetype> {
        world.registry.archetype(id)
    }
    pub(super) fn table_layouts_inner(&self) -> Result<&[TableLayout], EcsError> {
        if !self.registry.frozen {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "登记尚未冻结",
            ));
        }
        Ok(&self.registry.tables)
    }
    pub(super) fn archetypes_inner(&self) -> Result<&[Archetype], EcsError> {
        if !self.registry.frozen {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "登记尚未冻结",
            ));
        }
        Ok(&self.registry.archetypes)
    }
}
// 旧 fetch_add 分配会在溢出后重新分配历史身份，已停用；新分配协议见 World::allocate_identity。

impl World {
    pub(crate) fn with_partitions_inner<const PARTITION_COUNT: usize>() -> Result<World, EcsError> {
        if !(1..=crate::entity::MAX_PARTITIONS).contains(&PARTITION_COUNT) {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "分区数量超出允许范围",
            ));
        }
        let identity = Self::allocate_identity();
        let identity = identity?;
        let entities = crate::entity_store::EntityStore::new(PARTITION_COUNT)?;
        let commands = crate::command::CommandHub::new()?;

        #[cfg(feature = "rc")]
        let resources = crate::resource::ResourceStore {
            entries: Vec::new(),
            ids: Default::default(),
        };
        #[cfg(not(feature = "rc"))]
        let resources = crate::resource::ResourceStore { _pending: () };

        #[cfg(feature = "rc")]
        let multi_resources = crate::resource::MultiResourceStore {
            entries: Default::default(),
            ids: Default::default(),
        };
        #[cfg(not(feature = "rc"))]
        let multi_resources = crate::resource::MultiResourceStore { _pending: () };

        #[cfg(feature = "rc")]
        let entity_nodes = crate::entity_container::EntityNodePools {
            links: std::cell::RefCell::new(crate::entity_container::NodePool {
                slots: Default::default(),
            }),
            deque: std::cell::RefCell::new(pi_slot_deque::DequeStorage::new()),
        };
        #[cfg(not(feature = "rc"))]
        let entity_nodes = crate::entity_container::EntityNodePools { _pending: () };

        #[cfg(feature = "rc")]
        let access = crate::world::AccessState {
            mode: std::cell::Cell::new(crate::world::AccessMode::Idle),
        };
        #[cfg(not(feature = "rc"))]
        let access = crate::world::AccessState(std::sync::atomic::AtomicU32::new(0));

        #[cfg(feature = "rc")]
        let change_tick = 1;
        #[cfg(not(feature = "rc"))]
        let change_tick = std::sync::atomic::AtomicU64::new(1);

        Ok(World {
            tracking: crate::tracking::TrackingRegistry {
                queries: pi_slotmap::SlotMap::with_key(),
            },
            #[cfg(feature = "rc")]
            shutdown: crate::shutdown::ShutdownState::Open,
            identity,
            entities: entities.into(),
            commands,
            epoch: crate::command::WorldEpoch::initial(),
            simulation_tick: crate::command::SimulationTick(0),
            resources,
            query_defaults: crate::query_data::QueryDefaults {
                values: Default::default(),
            },
            multi_resources,
            pending_destruction: crate::destruction::PendingDestruction {
                pending: Default::default(),
                reclamation: Default::default(),
            },
            entity_nodes,
            partition_count: PARTITION_COUNT,
            change_tick,
            notifications: crate::notification::Notifications {
                subscriptions: Default::default(),
                components: Vec::new(),
                layouts: Vec::new(),
                type_ids: Vec::new(),
            },
            layout_epoch: 0,
            registry: crate::registry::TypeRegistry::new(),
            storage: None,
            access,
        })
    }
}
impl World {
    pub(crate) fn despawn_inner(world: &mut World, entity: Entity) -> Result<bool, EcsError> {
        #[cfg(feature = "rc")]
        let zero = world.entities.get_mut().release_strong(entity)?;
        #[cfg(not(feature = "rc"))]
        let zero = world.entities.release_strong(entity)?;
        if zero {
            world.pending_destruction.enqueue(entity);
        }
        Ok(zero)
    }
}
impl World {
    pub(crate) fn register_archetype_on_inner(
        world: &mut World,
        base: ArchetypeId,
        components: &[ComponentId],
    ) -> Result<ArchetypeId, EcsError> {
        #[cfg(feature = "rc")]
        host_idle(world)?;
        let plan = world.registry.plan_archetype_on(base, components)?;
        install_archetype(world, plan)
    }
}
impl World {
    pub(crate) fn locate_inner(world: &World, entity: Entity) -> Option<EntityLocation> {
        #[cfg(feature = "rc")]
        {
            world.entities.borrow().locate(entity)
        }
        #[cfg(not(feature = "rc"))]
        {
            world.entities.locate(entity)
        }
    }
}

#[cfg(feature = "rc")]
impl World {
    pub(crate) fn reclaim_notifications_inner(world: &mut World) -> Result<(), EcsError> {
        Self::destruction_maintenance(world, false, Self::reclaim_notifications_authorized)
    }
    pub(crate) fn reclaim_notifications_authorized(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        let root = cell.pointer.as_ptr();
        // SAFETY: 仅独占维护主体调用，所有通知批次已退出；移出后始终保活原队列分配。
        let pointers = unsafe {
            let notices = &mut (*root).notifications;
            notices.reclaim();
            notices.reclaim_removed_raw()
        };
        let context = crate::destruction::ReleaseContext {
            world: cell,
            mode: crate::destruction::ReleaseMode::Normal,
        };
        struct Remaining<'w> {
            context: crate::destruction::ReleaseContext<'w>,
            values: std::vec::IntoIter<(ComponentId, *mut u8)>,
        }
        impl Remaining<'_> {
            unsafe fn release(&self, component: ComponentId, pointer: *mut u8) {
                let root = self.context.world.pointer.as_ptr();
                unsafe {
                    crate::destruction::release_value(
                        &self.context,
                        (&(*root).registry)
                            .component(component)
                            .expect("Removed 组件元数据缺失"),
                        crate::ptr::OwningPtr::from_raw(
                            std::ptr::NonNull::new(pointer).expect("Removed 值地址为空"),
                        ),
                    );
                }
            }
        }
        impl Drop for Remaining<'_> {
            fn drop(&mut self) {
                // 已移出的整批责任必须清偿；首个回调 panic 后其余值不重投来源队列。
                // 展开中第二个业务析构失败不能再次展开，沿不可恢复清理故障终止。
                if std::thread::panicking() {
                    unsafe { &(*self.context.world.pointer.as_ptr()).access }
                        .mode
                        .set(crate::world::AccessMode::Stopped);
                }
                while let Some((component, pointer)) = self.values.next() {
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                        self.release(component, pointer)
                    }))
                    .is_err()
                    {
                        std::process::abort();
                    }
                }
            }
        }
        let mut remaining = Remaining {
            context,
            values: pointers.into_iter(),
        };
        while let Some((component, pointer)) = remaining.values.next() {
            // 从责任迭代器取走后才调用，panic 不会重复释放当前值；源队列一直保活。
            unsafe { remaining.release(component, pointer) };
        }
        Ok(())
    }
}
#[cfg(not(feature = "rc"))]
impl World {
    pub(crate) fn reclaim_notifications_inner(_world: &mut World) -> Result<(), EcsError> {
        todo!("编号：T93，尚未实现， 非 rc 通知旧值回收尚未实现")
    }
}
impl World {
    pub(crate) fn settle_inner(world: &mut World) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        {
            let (mut guard, storage, entities) = begin_host_maintenance(world)?;
            let result = storage.settle().map(|positions| {
                for repair in positions {
                    let domain = crate::entity::domain_bits(&repair.entity) as usize;
                    let local = crate::entity::with_domain_bits(repair.entity, 0);
                    let record = entities
                        .slots
                        .get_mut(domain)
                        .and_then(|slots| slots.get_mut(local))
                        .filter(|record| {
                            !record.location.is_null()
                                && record.location.archetype() == repair.archetype
                        })
                        .expect("整理返回失效实体或错误原型");
                    record.location = EntityLocation::from_parts(
                        record.location.partition().0,
                        repair.archetype_row,
                        repair.table_row.index(),
                        repair.archetype.table,
                        repair.archetype.variant,
                    );
                }
            });
            guard.complete();
            result
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = world;
            Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::Unsupported,
                "非 RC 整理延期",
            ))
        }
    }
}
impl World {
    pub(crate) fn move_partition_inner(
        world: &mut World,
        request: PartitionMove,
    ) -> Result<(), EcsError> {
        use pi_ecs_types::error::{classified_error, ErrorKind};
        #[cfg(feature = "rc")]
        {
            host_idle(world)?;
            if request.target.0 as usize >= world.partition_count {
                return Err(classified_error(ErrorKind::InvalidInput, "目标分区未配置"));
            }
            let (mut guard, _, entities) = begin_host_maintenance(world)?;
            let result = (|| {
                let old = entities.locate(request.entity).ok_or_else(|| {
                    classified_error(ErrorKind::NotFound, "迁区实体没有已发布位置")
                })?;
                let archetype = old.archetype();
                let new = EntityLocation::from_parts(
                    request.target.0,
                    old.archetype_row().index(),
                    old.table_row(),
                    archetype.table,
                    archetype.variant,
                );
                // Even same-partition requests must validate the live (strong > 0) record.
                entities.relocate(request.entity, new)
            })();
            guard.complete();
            result
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (world, request);
            Err(classified_error(
                ErrorKind::Unsupported,
                "非 RC 宿主迁区延期",
            ))
        }
    }
}

#[cfg(feature = "rc")]
fn begin_host_maintenance(
    world: &mut World,
) -> Result<
    (
        crate::entity_scope::EntityScopeSession<'_>,
        &mut crate::storage::WorldStorage,
        &mut crate::entity_store::EntityStore,
    ),
    EcsError,
> {
    use crate::world::AccessMode;
    host_idle(world)?;
    let storage = world
        .storage
        .as_mut()
        .filter(|_| world.registry.frozen)
        .ok_or_else(|| {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "维护要求冻结存储",
            )
        })?;
    world.access.mode.set(AccessMode::Maintaining);
    let guard = crate::entity_scope::EntityScopeSession {
        access: &world.access,
        completed: false,
    };
    Ok((guard, storage, world.entities.get_mut()))
}

#[cfg(feature = "rc")]
pub fn reborrow_world<'a, 'b>(world: &'a mut WorldRef<'b>) -> WorldRef<'a> {
    &mut **world
}
#[cfg(not(feature = "rc"))]
pub fn reborrow_world<'a, 'b>(world: &'a mut WorldRef<'b>) -> WorldRef<'a> {
    &**world
}

#[cfg(test)]
mod build_tests {
    use super::*;
    use crate::component::{Component, PlainAccess, StoragePolicy};

    struct Position(u32);

    impl Component for Position {
        type Access = PlainAccess;
    }

    struct Velocity(u32);

    impl Component for Velocity {
        type Access = PlainAccess;
    }

    struct Other(u32);

    impl Component for Other {
        type Access = PlainAccess;
    }

    struct SparseTag(u32);

    impl Component for SparseTag {
        type Access = PlainAccess;
    }

    struct Dropped(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl Component for Dropped {
        type Access = PlainAccess;
    }

    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    #[test]
    fn world_constructs_with_fixed_partition_count() {
        let world = World::new().unwrap();
        assert_eq!(world.partition_count, 1);
        assert!(world.storage.is_none());

        let world = World::with_partitions::<2>().unwrap();
        assert_eq!(world.partition_count, 2);

        assert!(World::with_partitions::<0>().is_err());
        assert!(World::with_partitions::<4097>().is_err());
    }

    #[test]
    fn empty_world_freezes_and_installs_storage() {
        let mut world = World::new().unwrap();
        world.finish_registration().unwrap();
        assert!(world.storage.is_some());
        assert!(world.registry.frozen);
        assert_eq!(world.layout_epoch, 1);
    }

    #[cfg(feature = "rc")]
    #[test]
    fn rc_host_component_read_and_write_use_the_published_slot() {
        let mut world = World::new().unwrap();
        let position = world
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let archetype = world.register_table(&[position]).unwrap();
        world.finish_registration().unwrap();
        let storage = world.storage.as_mut().unwrap();
        let mut table = storage.table_mut(archetype.table).unwrap();
        let row = table.claim_row().unwrap();
        let mut value = std::mem::ManuallyDrop::new(Position(17));
        // SAFETY: this physical seam consumes the single prepared value exactly once.
        unsafe {
            table
                .column_mut(crate::table::ColumnId(0))
                .unwrap()
                .insert_raw(
                    row.index() as usize,
                    std::ptr::addr_of_mut!(*value).cast(),
                    std::alloc::Layout::new::<Position>(),
                );
        }
        let entity = world
            .entities
            .get_mut()
            .allocate(
                crate::entity::AllocationDomain(0),
                EntityLocation::from_parts(0, 0, row.index(), archetype.table, 0),
            )
            .unwrap();
        table.publish_row(row, entity).unwrap();
        storage.append_member(archetype, entity, row).unwrap();

        assert_eq!(world.pin().unwrap().get::<Position>(entity).unwrap().0, 17);
        {
            let mut write = world.write().unwrap();
            assert!(write.get::<Velocity>(entity).is_none());
            assert_eq!(write.get::<Position>(entity).unwrap().0, 17);
            write.get_mut::<Position>(entity).unwrap().0 = 63;
        }
        assert_eq!(world.pin().unwrap().get::<Position>(entity).unwrap().0, 63);
        {
            let mut write = world.write().unwrap();
            // Model a fatal operation caught while the host still owns its write token.
            write
                .world
                .access
                .mode
                .set(crate::world::AccessMode::Stopped);
            let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = write.get::<Position>(entity);
            }));
            assert!(read.is_err(), "stopped token must reject component reads");
            let write_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = write.get_mut::<Position>(entity);
            }));
            assert!(
                write_result.is_err(),
                "stopped token must reject component writes"
            );
        }
        assert!(world.is_stopped());
        assert!(world.pin().is_err());
        assert!(world.write().is_err());
        world.close().unwrap();
    }

    #[cfg(feature = "rc")]
    #[test]
    fn rc_host_scopes_release_authority_and_stop_on_unwind() {
        let mut world = World::new().unwrap();
        assert!(world.entity_scope(|_| Ok(())).is_err());
        world.finish_registration().unwrap();
        let missing = Entity::default();
        assert!(world.try_acquire(missing).is_err());
        let before = world.read_change_tick();
        world
            .entity_scope(|scope| {
                assert!(scope.try_acquire(missing).is_err());
                Ok(())
            })
            .unwrap();
        world.write_scope(|_| ()).unwrap();
        assert_eq!(world.read_change_tick().get(), before.get() + 1);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            world.entity_scope::<()>(|_| panic!("entity scope panic"))
        }));
        assert!(result.is_err());
        assert!(world.is_stopped());
        assert!(world.entity_scope(|_| Ok(())).is_err());
        assert!(world.try_acquire(missing).is_err());
        assert!(world.write_scope(|_| ()).is_err());
        world.close().unwrap();
    }

    #[cfg(feature = "rc")]
    #[test]
    fn rc_host_move_and_settle_preserve_identity_without_changing_tick() {
        let mut world = World::with_partitions::<2>().unwrap();
        world.finish_registration().unwrap();
        let archetype = ArchetypeId {
            table: 0,
            variant: 0,
        };
        // Populate the empty Table using its existing physical publication seam.
        let mut publish = || {
            let storage = world.storage.as_mut().unwrap();
            let row = storage.table_mut(0).unwrap().claim_row().unwrap();
            let member = storage.member_len(archetype).unwrap();
            let location = EntityLocation::from_parts(0, member, row.index(), 0, 0);
            let entity = world
                .entities
                .get_mut()
                .allocate(crate::entity::AllocationDomain(0), location)
                .unwrap();
            assert_eq!(
                storage.append_member(archetype, entity, row).unwrap(),
                member
            );
            storage
                .table_mut(0)
                .unwrap()
                .publish_row(row, entity)
                .unwrap();
            entity
        };
        let first = publish();
        let last = publish();
        let mv = |entity, target| PartitionMove {
            entity,
            target: crate::entity::PartitionId(target),
        };
        let before = world.locate(last).unwrap();
        assert!(world.move_partition(mv(last, 2)).is_err());
        assert_eq!(world.locate(last), Some(before));
        world.move_partition(mv(last, 1)).unwrap();
        assert_eq!(world.locate(last).unwrap().table_row(), before.table_row());
        assert!(unsafe { world.despawn(first) }.unwrap());
        assert!(world.move_partition(mv(first, 0)).is_err());
        world.flush_destruction().unwrap();
        let tick = world.read_change_tick();
        // A published row with strong=0 is still present until the next destruction round.
        assert!(unsafe { world.despawn(last) }.unwrap());
        world.settle().unwrap();
        let after = world.locate(last).unwrap();
        assert_eq!(after.partition(), crate::entity::PartitionId(1));
        assert_eq!((after.table_row(), after.archetype_row().index()), (0, 0));
        assert_eq!(world.read_change_tick(), tick);
        world.flush_destruction().unwrap();
        assert!(world.locate(last).is_none());
    }

    #[cfg(feature = "rc")]
    #[test]
    fn rc_host_archetype_registration_is_atomic_and_idempotent() {
        let mut world = World::new().unwrap();
        let position = world
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let tag = world
            .register_component::<SparseTag>(StoragePolicy::SparseOnly)
            .unwrap();
        let base = world.register_table(&[position]).unwrap();
        assert!(world.register_archetype(&[position, tag]).is_err());
        world.finish_registration().unwrap();
        let count = world.archetypes().unwrap().len();
        assert!(world.register_archetype_on(base, &[tag]).is_err());
        assert!(world.register_archetype(&[tag, tag]).is_err());
        world
            .schedule_scope(|world| {
                assert!(world.register_archetype(&[position, tag]).is_err());
                Ok(())
            })
            .unwrap();
        assert_eq!(world.archetypes().unwrap().len(), count);
        let id = world.register_archetype_on(base, &[tag, position]).unwrap();
        assert_eq!(id.variant, 1);
        assert_eq!(world.archetype(id).unwrap().table(), base);
        assert_eq!(world.register_archetype(&[position, tag]).unwrap(), id);
        assert_eq!(world.archetypes().unwrap().len(), count + 1);
    }

    #[cfg(feature = "rc")]
    #[test]
    fn rc_empty_spawn_rejects_invalid_domain_and_publishes_owned_entity() {
        use crate::entity::AllocationDomain;
        let mut world = World::with_partitions::<2>().unwrap();
        world.finish_registration().unwrap();
        let before = world.read_change_tick();
        assert!(world.spawn(AllocationDomain(2), ()).is_err());
        assert_eq!(world.read_change_tick(), before);

        let entity = world.spawn(AllocationDomain(1), ()).unwrap().forget();
        assert_eq!(world.locate(entity).unwrap().partition().0, 1);
        let after_spawn = world.read_change_tick();
        assert_eq!(after_spawn.get(), before.get() + 1);
        let location = world.locate(entity);
        assert!(world
            .change_archetype(entity, (), &[ComponentId(0)])
            .is_err());
        world.change_archetype(entity, (), &[]).unwrap();
        assert_eq!(world.read_change_tick(), after_spawn);
        assert_eq!(world.locate(entity), location);
        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();
        assert!(world.locate(entity).is_none());
        world.close().unwrap();
    }

    #[cfg(feature = "rc")]
    #[test]
    fn rc_spawn_rejects_writer_violation_even_if_bundle_catches_panic() {
        use crate::{
            bundle::{Bundle, BundleContext, BundleWriter},
            entity::AllocationDomain,
        };
        use std::{
            any::TypeId,
            panic::{catch_unwind, AssertUnwindSafe},
            sync::{
                atomic::{AtomicU32, Ordering},
                Arc,
            },
        };

        struct Undeclared(Arc<AtomicU32>);
        impl Component for Undeclared {
            type Access = PlainAccess;
        }
        impl Drop for Undeclared {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
        struct CatchesRejection(Undeclared);
        impl Bundle for CatchesRejection {
            fn component_types() -> Vec<TypeId> {
                Vec::new()
            }
            fn validate(&self, _: &BundleContext<'_>) -> Result<(), EcsError> {
                Ok(())
            }
            fn write(self, writer: &mut BundleWriter<'_>) {
                let _ = catch_unwind(AssertUnwindSafe(|| writer.write(self.0)));
            }
        }

        let mut world = World::new().unwrap();
        world
            .register_component::<Undeclared>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let before = world.read_change_tick();
        let drops = Arc::new(AtomicU32::new(0));
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = world.spawn(
                AllocationDomain(0),
                CatchesRejection(Undeclared(Arc::clone(&drops))),
            );
        }));
        assert!(result.is_err());
        assert!(world.is_stopped());
        assert_eq!(world.read_change_tick(), before);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert!(world.spawn(AllocationDomain(0), ()).is_err());
        world.close().unwrap();
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    #[cfg(feature = "rc")]
    #[test]
    fn spawn_locate_despawn_and_flush_table_entity() {
        let mut world = World::new().unwrap();
        let position = world
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let archetype = world.register_table(&[position]).unwrap();
        world.finish_registration().unwrap();

        let values = (crate::bundle::ComponentValue(Position(7)),);
        let owned = world
            .spawn(crate::entity::AllocationDomain(0), values)
            .unwrap();
        let entity = owned.entity();
        let _ = owned.forget();
        assert!(world.locate(entity).is_some());

        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();
        assert!(world.locate(entity).is_none());
    }

    #[cfg(feature = "rc")]
    #[test]
    fn alter_cross_table_publishes_and_moves_retained() {
        let mut world = World::new().unwrap();
        let position = world
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let velocity = world
            .register_component::<Velocity>(StoragePolicy::TableEligible)
            .unwrap();
        let source_table = world.register_table(&[position]).unwrap();
        let target_table = world.register_table(&[position, velocity]).unwrap();
        world.finish_registration().unwrap();

        let values = (crate::bundle::ComponentValue(Position(9)),);
        let owned = world
            .spawn(crate::entity::AllocationDomain(0), values)
            .unwrap();
        let entity = owned.entity();
        let _ = owned.forget();

        let mut alteration = crate::alter::AlterState::new(&world, &[velocity], &[]).unwrap();
        let added = crate::bundle::ComponentValue(Velocity(4));
        alteration
            .scope(&mut world, |alter| alter.alter(entity, added))
            .unwrap()
            .unwrap();

        let location = world.locate(entity).unwrap();
        assert_eq!(location.archetype(), target_table);
        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();
        assert!(world.locate(entity).is_none());
    }

    #[cfg(feature = "rc")]
    #[test]
    fn alter_same_table_adds_sparse_variant_member() {
        let mut world = World::new().unwrap();
        let position = world
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let tag = world
            .register_component::<SparseTag>(StoragePolicy::SparseOnly)
            .unwrap();
        let source_table = world.register_table(&[position]).unwrap();
        world.finish_registration().unwrap();
        let target_variant = world.register_archetype(&[position, tag]).unwrap();
        assert_eq!(target_variant.table, source_table.table);

        let values = (crate::bundle::ComponentValue(Position(8)),);
        let owned = world
            .spawn(crate::entity::AllocationDomain(0), values)
            .unwrap();
        let entity = owned.entity();
        let _ = owned.forget();

        let mut alteration = crate::alter::AlterState::new(&world, &[tag], &[]).unwrap();
        let added = crate::bundle::ComponentValue(SparseTag(12));
        alteration
            .scope(&mut world, |alter| alter.alter(entity, added))
            .unwrap()
            .unwrap();

        assert_eq!(world.locate(entity).unwrap().archetype(), target_variant);
        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();
        assert!(world.locate(entity).is_none());
    }

    #[cfg(feature = "rc")]
    #[test]
    fn alter_cross_table_drops_removed_component() {
        let mut world = World::new().unwrap();
        let position = world
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let dropped = world
            .register_component::<Dropped>(StoragePolicy::TableEligible)
            .unwrap();
        let source_table = world.register_table(&[position, dropped]).unwrap();
        let target_table = world.register_table(&[position]).unwrap();
        world.finish_registration().unwrap();

        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let values = (
            crate::bundle::ComponentValue(Position(2)),
            crate::bundle::ComponentValue(Dropped(counter.clone())),
        );
        let owned = world
            .spawn(crate::entity::AllocationDomain(0), values)
            .unwrap();
        let entity = owned.entity();
        let _ = owned.forget();

        let mut alteration = crate::alter::AlterState::new(&world, &[], &[dropped]).unwrap();
        let empty = ();
        alteration
            .scope(&mut world, |alter| alter.alter(entity, ()))
            .unwrap()
            .unwrap();

        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert_eq!(world.locate(entity).unwrap().archetype(), target_table);
        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();
    }

    #[cfg(feature = "rc")]
    #[test]
    fn alter_appends_added_changed_and_removed_notices() {
        let mut world = World::new().unwrap();
        let position = world
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let velocity = world
            .register_component::<Velocity>(StoragePolicy::TableEligible)
            .unwrap();
        let dropped = world
            .register_component::<Dropped>(StoragePolicy::TableEligible)
            .unwrap();
        let source_table = world.register_table(&[position, dropped]).unwrap();
        let target_table = world.register_table(&[position, velocity]).unwrap();
        world.finish_registration().unwrap();

        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let values = (
            crate::bundle::ComponentValue(Position(1)),
            crate::bundle::ComponentValue(Dropped(counter.clone())),
        );
        let owned = world
            .spawn(crate::entity::AllocationDomain(0), values)
            .unwrap();
        let entity = owned.entity();
        let _ = owned.forget();

        let mut added_reader = world
            .notifications
            .subscribe(velocity, crate::notification::NoticeKind::Added)
            .unwrap();
        let mut changed_reader = world
            .notifications
            .subscribe(velocity, crate::notification::NoticeKind::Changed)
            .unwrap();
        let mut removed_reader = world
            .notifications
            .subscribe(dropped, crate::notification::NoticeKind::Removed)
            .unwrap();

        let mut alteration =
            crate::alter::AlterState::new(&world, &[velocity], &[dropped]).unwrap();
        let added = crate::bundle::ComponentValue(Velocity(6));
        alteration
            .scope(&mut world, |alter| alter.alter(entity, added))
            .unwrap()
            .unwrap();

        let added_batch = world.notifications.read(&mut added_reader).unwrap();
        let added_header = added_batch.into_iter().next().unwrap();
        assert_eq!(added_header.entity, entity);
        let changed_batch = world.notifications.read(&mut changed_reader).unwrap();
        let changed_header = changed_batch.into_iter().next().unwrap();
        assert_eq!(changed_header.entity, entity);
        assert!(world.notifications.has_removed(dropped));
        let streams = &world.notifications.components[dropped.0 as usize];
        let removed = streams.removed.as_ref().unwrap().borrow();
        assert_eq!(removed.notices.range.end.0, 1);
        assert_eq!(removed.values.len(), 1);
        drop(removed);
        let mut removed_batch = world
            .notifications
            .read_removed::<Dropped>(&world, &mut removed_reader)
            .unwrap();
        let removed_notice = removed_batch.next().unwrap();
        assert_eq!(removed_notice.header.entity, entity);
        assert!(std::sync::Arc::ptr_eq(&removed_notice.value.0, &counter));
        drop(removed_batch);
        world.notifications.unsubscribe(added_reader).unwrap();
        world.notifications.unsubscribe(changed_reader).unwrap();
        world.notifications.unsubscribe(removed_reader).unwrap();
        world.reclaim_notifications().unwrap();
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 1);
        let streams = &world.notifications.components[dropped.0 as usize];
        let removed = streams.removed.as_ref().unwrap().borrow();
        assert_eq!(removed.notices.range.first.0, 1);
        assert_eq!(removed.values.len(), 0);
        drop(removed);
        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();
    }

    #[cfg(feature = "rc")]
    #[test]
    fn change_archetype_cross_table_moves_and_notifies() {
        let mut world = World::new().unwrap();
        let position = world
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let velocity = world
            .register_component::<Velocity>(StoragePolicy::TableEligible)
            .unwrap();
        let source_table = world.register_table(&[position]).unwrap();
        let target_table = world.register_table(&[position, velocity]).unwrap();
        world.finish_registration().unwrap();
        let mut reader = world
            .notifications
            .subscribe(velocity, crate::notification::NoticeKind::Added)
            .unwrap();

        let values = (crate::bundle::ComponentValue(Position(5)),);
        let owned = world
            .spawn(crate::entity::AllocationDomain(0), values)
            .unwrap();
        let entity = owned.entity();
        let _ = owned.forget();

        let added = (crate::bundle::ComponentValue(Velocity(10)),);
        world.change_archetype(entity, added, &[]).unwrap();
        assert_eq!(world.locate(entity).unwrap().archetype(), target_table);
        let batch = world.notifications.read(&mut reader).unwrap();
        assert_eq!(batch.into_iter().next().unwrap().entity, entity);
        world.notifications.unsubscribe(reader).unwrap();
        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();
    }

    #[cfg(feature = "rc")]
    #[test]
    fn rc_structural_removal_preserves_table_value_and_identity() {
        for route in ["world", "alter", "query_alter"] {
            for subscribe in [false, true] {
                check_rc_structural_removal(route, subscribe);
            }
        }
    }

    #[cfg(feature = "rc")]
    fn check_rc_structural_removal(route: &str, subscribe: bool) {
        let mut world = World::new().unwrap();
        let keep = world
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let remove = world
            .register_component::<Dropped>(StoragePolicy::TableEligible)
            .unwrap();
        let source = world.register_table(&[keep, remove]).unwrap();
        let target = world.register_table(&[keep]).unwrap();
        world.finish_registration().unwrap();
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut position = std::mem::ManuallyDrop::new(Position(71));
        let mut dropped = std::mem::ManuallyDrop::new(Dropped(counter.clone()));
        let storage = world.storage.as_mut().unwrap();
        let mut table = storage.table_mut(source.table).unwrap();
        let row = table.claim_row().unwrap();
        for (column, address, layout) in [
            (
                0,
                std::ptr::from_mut(&mut *position).cast(),
                std::alloc::Layout::new::<Position>(),
            ),
            (
                1,
                std::ptr::from_mut(&mut *dropped).cast(),
                std::alloc::Layout::new::<Dropped>(),
            ),
        ] {
            // SAFETY: the fixture owns one initialized value for each prepared column.
            unsafe {
                table
                    .column_mut(crate::table::ColumnId(column))
                    .unwrap()
                    .insert_raw(row.index() as usize, address, layout)
            };
        }
        let location = EntityLocation::from_parts(0, 0, row.index(), source.table, source.variant);
        let entity = world
            .entities
            .get_mut()
            .allocate(crate::entity::AllocationDomain(0), location)
            .unwrap();
        table.publish_row(row, entity).unwrap();
        storage.append_member(source, entity, row).unwrap();
        let mut reader = subscribe.then(|| {
            world
                .notifications
                .subscribe(remove, crate::notification::NoticeKind::Removed)
                .unwrap()
        });
        match route {
            "world" => world.change_archetype(entity, (), &[remove]).unwrap(),
            "alter" => {
                let mut alter = crate::alter::AlterState::new(&world, &[], &[remove]).unwrap();
                alter
                    .scope(&mut world, |access| access.alter(entity, ()))
                    .unwrap()
                    .unwrap();
            }
            "query_alter" => {
                use crate::{
                    access::{AccessTree, StructuralAccess},
                    alter::{AlterState, QueryAlterState},
                    change_detection::ChangeWindow,
                    query::QueryState,
                    query_data::Has,
                };
                struct MatchAll;
                // SAFETY: this fixture has no dynamic predicate or value access.
                unsafe impl crate::query::QueryFilter for MatchAll {
                    const IS_ARCHETYPAL: bool = true;
                    type State = ();
                    type FetchState = ();
                    type Fetch<'w> = crate::query::NoFilterFetch;
                    fn init_state(_: &World) -> Result<(), EcsError> {
                        Ok(())
                    }
                    fn declare_access(
                        _: &(),
                    ) -> Result<crate::query::QueryFilterDeclaration, EcsError>
                    {
                        <crate::query::NoFilter as crate::query::QueryFilter>::declare_access(&())
                    }
                    fn init_fetch_state(
                        _: &(),
                        _: crate::query_fetch::FetchScope,
                        _: &crate::query_fetch::FetchBinding<'_>,
                    ) -> Result<crate::query::FetchStateResult<()>, EcsError> {
                        Ok(crate::query::FetchStateResult::Ready(()))
                    }
                    unsafe fn prepare<'w>(
                        access: &crate::query_fetch::FetchAccess<'w>,
                        scope: crate::query_fetch::FetchScope,
                        _: &(),
                        _: &(),
                    ) -> Result<Self::Fetch<'w>, EcsError> {
                        unsafe {
                            <crate::query::NoFilter as crate::query::QueryFilter>::prepare(
                                access,
                                scope,
                                &(),
                                &(),
                            )
                        }
                    }
                }
                let query = QueryState::<Has<Dropped>, MatchAll>::new(&mut world).unwrap();
                let mut declaration = query.declaration().unwrap().to_access().unwrap();
                declaration.structure = StructuralAccess::Alter {
                    added: Box::new([]),
                    removed: Box::new([remove]),
                };
                let tree = AccessTree::Leaf(declaration);
                let mut state = QueryAlterState {
                    query,
                    alteration: AlterState::new(&world, &[], &[remove]).unwrap(),
                    added_order: Box::new([]),
                    iteration_order: Box::new([]),
                    table_order: None,
                };
                world
                    .schedule_scope(|world| {
                        let epoch = crate::unsafe_world_cell::UnsafeWorldCell::from_ref(world)
                            .archetype_revision()
                            .0;
                        // SAFETY: this is the sole leaf. Cursor results are consumed before the next operation.
                        unsafe {
                            let this_run = world.next_system_tick()?;
                            world.system_scope(&tree, epoch, |access| {
                                let access = access.refresh_alter(&mut state)?;
                                let mut alter = access
                                    .bind_alter::<_, _, (), crate::bundle::ComponentValue<Dropped>>(
                                        &mut state,
                                        ChangeWindow {
                                            last_run: None,
                                            this_run,
                                        },
                                    )?;
                                let mut cursor = alter.iter_mut();
                                assert_eq!(cursor.fetch_next(), Some(true));
                                cursor.alter(())?;
                                assert!(cursor.alter(()).is_err());
                                assert!(cursor.fetch_next().is_none());
                                Ok(())
                            })?
                        }
                    })
                    .unwrap();
                state.query.unregister_tracking(&mut world).unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(world.locate(entity).unwrap().archetype(), target);
        assert_eq!(world.pin().unwrap().get::<Position>(entity).unwrap().0, 71);
        if let Some(mut reader) = reader.take() {
            assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 0);
            {
                let mut notices = world
                    .notifications
                    .read_removed::<Dropped>(&world, &mut reader)
                    .unwrap();
                let notice = notices.next().unwrap();
                assert_eq!(notice.header.entity, entity);
                assert_eq!(notice.header.tick, world.read_change_tick());
                assert!(std::sync::Arc::ptr_eq(&notice.value.0, &counter));
                assert!(notices.next().is_none());
            }
            world.notifications.unsubscribe(reader).unwrap();
            world.reclaim_notifications().unwrap();
        }
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(world
            .storage
            .as_ref()
            .unwrap()
            .row_entity(source.table, row)
            .is_none());
        world.close().unwrap();
    }

    #[cfg(feature = "rc")]
    #[test]
    fn change_archetype_removes_and_drops_component() {
        let mut world = World::new().unwrap();
        let position = world
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let dropped = world
            .register_component::<Dropped>(StoragePolicy::TableEligible)
            .unwrap();
        let source_table = world.register_table(&[position, dropped]).unwrap();
        let target_table = world.register_table(&[position]).unwrap();
        world.finish_registration().unwrap();
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let values = (
            crate::bundle::ComponentValue(Position(5)),
            crate::bundle::ComponentValue(Dropped(counter.clone())),
        );
        let owned = world
            .spawn(crate::entity::AllocationDomain(0), values)
            .unwrap();
        let entity = owned.entity();
        let _ = owned.forget();

        let empty = ();
        world.change_archetype(entity, empty, &[dropped]).unwrap();
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert_eq!(world.locate(entity).unwrap().archetype(), target_table);
        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();
    }

    #[cfg(feature = "rc")]
    #[test]
    fn spawn_appends_added_and_changed_notices() {
        let mut world = World::new().unwrap();
        let velocity = world
            .register_component::<Velocity>(StoragePolicy::TableEligible)
            .unwrap();
        let table = world.register_table(&[velocity]).unwrap();
        world.finish_registration().unwrap();
        let mut added_reader = world
            .notifications
            .subscribe(velocity, crate::notification::NoticeKind::Added)
            .unwrap();
        let mut changed_reader = world
            .notifications
            .subscribe(velocity, crate::notification::NoticeKind::Changed)
            .unwrap();

        let values = (crate::bundle::ComponentValue(Velocity(3)),);
        let owned = world
            .spawn(crate::entity::AllocationDomain(0), values)
            .unwrap();
        let entity = owned.entity();
        let _ = owned.forget();

        let added_batch = world.notifications.read(&mut added_reader).unwrap();
        assert_eq!(added_batch.into_iter().next().unwrap().entity, entity);
        let changed_batch = world.notifications.read(&mut changed_reader).unwrap();
        assert_eq!(changed_batch.into_iter().next().unwrap().entity, entity);
        world.notifications.unsubscribe(added_reader).unwrap();
        world.notifications.unsubscribe(changed_reader).unwrap();
        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();
    }

    #[cfg(feature = "rc")]
    #[test]
    fn despawn_appends_removed_notice() {
        let mut world = World::new().unwrap();
        let velocity = world
            .register_component::<Velocity>(StoragePolicy::TableEligible)
            .unwrap();
        let table = world.register_table(&[velocity]).unwrap();
        world.finish_registration().unwrap();
        let values = (crate::bundle::ComponentValue(Velocity(2)),);
        let owned = world
            .spawn(crate::entity::AllocationDomain(0), values)
            .unwrap();
        let entity = owned.entity();
        let _ = owned.forget();

        let mut reader = world
            .notifications
            .subscribe(velocity, crate::notification::NoticeKind::Removed)
            .unwrap();
        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();
        let streams = &world.notifications.components[velocity.0 as usize];
        let removed = streams.removed.as_ref().unwrap().borrow();
        assert_eq!(removed.notices.range.end.0, 1);
        assert_eq!(removed.values.len(), 1);
        drop(removed);
        world.notifications.unsubscribe(reader).unwrap();
        world.reclaim_notifications().unwrap();
        let streams = &world.notifications.components[velocity.0 as usize];
        assert_eq!(streams.removed.as_ref().unwrap().borrow().values.len(), 0);
    }

    #[cfg(feature = "rc")]
    #[test]
    fn spawn_sparse_component_and_flush() {
        let mut world = World::new().unwrap();
        let position = world
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let tag = world
            .register_component::<SparseTag>(StoragePolicy::SparseOnly)
            .unwrap();
        world.register_table(&[position]).unwrap();
        world.finish_registration().unwrap();
        let archetype = world.register_archetype(&[position, tag]).unwrap();

        let values = (
            crate::bundle::ComponentValue(Position(3)),
            crate::bundle::ComponentValue(SparseTag(42)),
        );
        let owned = world
            .spawn(crate::entity::AllocationDomain(0), values)
            .unwrap();
        let entity = owned.entity();
        let _ = owned.forget();

        let mut query = world.query::<&SparseTag>().unwrap();
        // SAFETY: 借用仅用于读取，且在销毁前已经结束。
        assert_eq!(unsafe { query.get(&mut world, entity) }.unwrap().0, 42);

        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();
        assert!(world.locate(entity).is_none());
    }

    #[test]
    fn builds_global_storage_before_freeze() {
        let mut registry = crate::registry::TypeRegistry::new();
        let position = registry
            .register_component::<Position>(StoragePolicy::TableEligible)
            .unwrap();
        let velocity = registry
            .register_component::<Velocity>(StoragePolicy::TableEligible)
            .unwrap();
        let table = registry.register_table(&[position]).unwrap();
        let storage = build_world_storage_inner(&registry).unwrap();
        drop(storage);

        registry.freeze(2).unwrap();
        assert!(registry.table(table).is_some());
        assert!(registry.archetype(table).is_some());

        let plan = registry.plan_archetype(&[position, velocity]).unwrap();
        assert!(plan.is_new);
        let variant = registry.commit_archetype(plan);
        assert_eq!(variant.table, table.table);
        assert!(variant.variant > 0);
        assert!(registry.archetype(variant).is_some());

        assert!(registry
            .register_component::<Other>(StoragePolicy::TableEligible)
            .is_err());
    }
}
