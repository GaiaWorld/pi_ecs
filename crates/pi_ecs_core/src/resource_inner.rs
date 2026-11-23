//! 资源宿主配对芯。
#[cfg(all(test, feature = "rc"))]
mod rc_run_resource_tests {
    use crate::{
        access::*, resource::ResourceId, unsafe_world_cell::UnsafeWorldCell, world::World,
    };
    fn leaf(
        resources: Vec<(ResourceId, AccessMode)>,
        multi_resources: Vec<(ResourceId, MultiResourceAccess)>,
    ) -> AccessTree {
        AccessTree::Leaf(AccessDeclaration {
            resources: resources.into_boxed_slice(),
            multi_resources: multi_resources.into_boxed_slice(),
            components: Box::new([]),
            tick_reads: Box::new([]),
            reads_entities: false,
            scope: EntityScope {
                with: Box::new([]),
                without: Box::new([]),
            },
            structure: StructuralAccess::None,
            observations: Box::new([]),
        })
    }
    // Seam: 真实生产者登记、准备快照、运行写槽及集合读；不同槽可同时借用。
    #[test]
    fn rc_run_resource_producers_disjoint_slots_and_stale_binding() {
        let mut world = World::new().unwrap();
        let id = world.register_multi_resource::<u32>().unwrap();
        let empty = world.register_multi_resource::<u64>().unwrap();
        world.finish_registration().unwrap();
        let (first, last) = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|mut scope| {
                    Ok((
                        scope.register_producer(7u32)?,
                        scope.register_producer(11u32)?,
                    ))
                })
            })
            .unwrap();
        let before = world.producer_binding(&last).unwrap();
        let writes = leaf(
            vec![],
            vec![
                (
                    id,
                    MultiResourceAccess::WriteSlot {
                        producer: first.producer_id(),
                    },
                ),
                (
                    id,
                    MultiResourceAccess::WriteSlot {
                        producer: last.producer_id(),
                    },
                ),
            ],
        );
        world
            .schedule_scope(|world| {
                let binding = world.producer_binding(&first)?;
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                let tick = unsafe { world.next_system_tick()? };
                unsafe {
                    world.system_scope(&writes, version, |access| {
                        assert!(access.multi_resource_parts::<u32>(id).is_err());
                        let mut a = access.producer_mut::<u32>(&first, &binding, tick).unwrap();
                        let mut b = access.producer_mut::<u32>(&last, &before, tick).unwrap();
                        *a = 19;
                        *b = 23;
                    })?;
                }
                Ok(())
            })
            .unwrap();
        world
            .parameter_unregistration_scope(|mut scope| scope.unregister_producer::<u32>(first))
            .unwrap();
        let reads = leaf(
            vec![],
            vec![
                (id, MultiResourceAccess::ReadAll),
                (empty, MultiResourceAccess::ReadAll),
            ],
        );
        let last_write = leaf(
            vec![],
            vec![(
                id,
                MultiResourceAccess::WriteSlot {
                    producer: last.producer_id(),
                },
            )],
        );
        world
            .schedule_scope(|world| {
                let binding = world.producer_binding(&last)?;
                assert_eq!(binding.offset, 0);
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                let tick = unsafe { world.next_system_tick()? };
                unsafe {
                    world.system_scope(&last_write, version, |access| {
                        assert!(access.producer_mut::<u32>(&last, &before, tick).is_err());
                        assert!(access.producer_mut::<u64>(&last, &binding, tick).is_err());
                        let mut value = access.producer_mut::<u32>(&last, &binding, tick).unwrap();
                        *value = 29;
                    })?;
                }
                unsafe {
                    world.system_scope(&reads, version, |access| {
                        let view = access.multi_resource_parts::<u32>(id).unwrap();
                        assert_eq!(view.values, [29]);
                        assert_eq!(view.ticks.len(), 1);
                        assert_eq!(view.ticks[0].changed, tick);
                        assert_eq!(view.structure_version, 3);
                        assert!(access
                            .multi_resource_parts::<u64>(empty)
                            .unwrap()
                            .values
                            .is_empty());
                        assert!(access.multi_resource_parts::<u64>(id).is_err());
                        assert!(access.producer_mut::<u32>(&last, &binding, tick).is_err());
                    })?;
                }
                Ok(())
            })
            .unwrap();
        world
            .parameter_unregistration_scope(|mut scope| scope.unregister_producer::<u32>(last))
            .unwrap();
        world.close().unwrap();
    }

    // Seam: 系统会话的唯一资源读取/Mut；类型、叶子权限、缺实例与变化标记。
    #[test]
    fn rc_run_resource_unique_permissions_and_ticks() {
        let mut world = World::new().unwrap();
        let id = world.register_resource::<u32>().unwrap();
        let missing = world.register_resource::<u64>().unwrap();
        world.finish_registration().unwrap();
        world.insert_resource(7u32).unwrap();
        let read = leaf(
            vec![(id, AccessMode::Read), (missing, AccessMode::Read)],
            vec![],
        );
        let write = leaf(vec![(id, AccessMode::Write)], vec![]);
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                let tick = unsafe { world.next_system_tick()? };
                unsafe {
                    world.system_scope(&read, version, |access| {
                        let (value, ticks) = access.resource_parts::<u32>(id).unwrap().unwrap();
                        assert_eq!(*value, 7);
                        assert!(ticks.changed.get() < tick.get());
                        assert!(access.resource_parts::<u64>(missing).unwrap().is_none());
                        assert!(access.resource_parts::<u64>(id).is_err());
                        assert!(access.resource_mut::<u32>(id, tick).is_err());
                    })?;
                }
                unsafe {
                    world.system_scope(&write, version, |access| {
                        let mut value = access.resource_mut::<u32>(id, tick).unwrap().unwrap();
                        let window = crate::change_detection::ChangeWindow {
                            last_run: Some(crate::change_detection::ChangeTick::new(
                                std::num::NonZeroU64::new(tick.get() - 1).unwrap(),
                            )),
                            this_run: tick,
                        };
                        assert!(!value.is_added(window));
                        assert!(!value.is_changed(window));
                        *value = 19;
                        assert!(value.is_changed(window));
                    })?;
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(
            world
                .read_scope(|pin| pin.resource::<u32>().copied())
                .unwrap(),
            Some(19)
        );
        world.close().unwrap();
    }
}

#[cfg(feature = "rc")]
fn run_resource_error() -> crate::error::EcsError {
    pi_ecs_types::error::classified_error(
        pi_ecs_types::error::ErrorKind::InvalidState,
        "资源叶子权限、类型或生产者绑定无效",
    )
}
// 类型擦除只擦除查元数据的单态函数；不缓存 World、值地址或生产者责任。
// 所有映射/版本仍以 MultiResourceCore<T> 为单源；非泛型入口经 TypeId 选择安全 downcast。
#[cfg(feature = "rc")]
static PRODUCER_LOOKUPS: std::sync::OnceLock<
    std::sync::Mutex<
        std::collections::HashMap<
            std::any::TypeId,
            fn(&dyn std::any::Any, u64) -> Option<crate::resource::ProducerBinding>,
        >,
    >,
> = std::sync::OnceLock::new();
#[cfg(feature = "rc")]
fn producer_lookup<T: Send + Sync + 'static>(
    entry: &dyn std::any::Any,
    producer: u64,
) -> Option<crate::resource::ProducerBinding> {
    let core = entry.downcast_ref::<super::MultiResourceCore<T>>()?;
    let offset = *core.producer_offsets.get(&super::ProducerId(producer))?;
    if core.producers.get(offset) != Some(&super::ProducerId(producer)) {
        return None;
    }
    Some(super::ProducerBinding {
        structure_version: core.structure_version,
        offset,
    })
}

#[cfg(all(test, feature = "rc"))]
mod multi_resource_producer_batch {
    use crate::world::World;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    struct Payload(u32, Arc<AtomicUsize>);

    impl Drop for Payload {
        fn drop(&mut self) {
            self.1.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn multi_resource_producer_batch_reinitialize_rollback_and_type_isolation() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new().unwrap();
        let id = world.register_multi_resource::<Payload>().unwrap();
        world.register_multi_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        let world_id = crate::unsafe_world_cell::UnsafeWorldCell::from_ref(&world).id();
        let first = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|mut scope| {
                    assert_eq!(scope.world_id(), world_id);
                    assert_eq!(scope.multi_resource_id::<Payload>(), Some(id));
                    let first = scope.register_producer(Payload(7, drops.clone()))?;
                    let rollback = scope.register_producer(Payload(11, drops.clone()))?;
                    scope.unregister_producer::<Payload>(rollback)?;
                    assert!(scope
                        .register_producer(PayloadMissing(drops.clone()))
                        .is_err());
                    Ok(first)
                })
            })
            .unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 2);
        let old = first.producer_id();
        world
            .parameter_unregistration_scope(|mut scope| {
                assert!(scope.register_producer(Payload(13, drops.clone())).is_err());
                scope.unregister_producer::<Payload>(first)
            })
            .unwrap();
        let replacement = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|mut scope| {
                    let replacement = scope.register_producer(Payload(19, drops.clone()))?;
                    let other = scope.register_producer(23u32)?;
                    Ok((replacement, other))
                })
            })
            .unwrap();
        assert!(replacement.0.producer_id() > old);
        world
            .read_scope(|pin| {
                assert_eq!(
                    pin.multi_resource::<Payload>()
                        .unwrap()
                        .iter()
                        .map(|v| v.0)
                        .collect::<Vec<_>>(),
                    [19]
                );
                assert_eq!(
                    pin.multi_resource::<u32>()
                        .unwrap()
                        .iter()
                        .copied()
                        .collect::<Vec<_>>(),
                    [23]
                );
            })
            .unwrap();
        world.close().unwrap();
        world.close().unwrap();
        drop(replacement);
        drop(world);
        assert_eq!(drops.load(Ordering::SeqCst), 5);
    }
    #[test]
    fn multi_resource_producer_batch_scope_gates_errors_and_unwind() {
        let mut world = World::new().unwrap();
        assert!(world
            .schedule_scope::<()>(|_| panic!("未冻结不得调用"))
            .is_err());
        assert!(world
            .parameter_registration_scope::<()>(|_| panic!("无根不得登记"))
            .is_err());
        world.finish_registration().unwrap();
        let pin = world.pin().unwrap();
        drop(pin);
        assert!(world
            .schedule_scope(|world| {
                assert!(world
                    .schedule_scope::<()>(|_| panic!("根不可重入"))
                    .is_err());
                assert!(world
                    .parameter_unregistration_scope::<()>(|_| panic!("Running 不可宿主清偿"))
                    .is_err());
                assert!(world.pin().is_err());
                world.parameter_registration_scope::<()>(|_| {
                    Err(pi_ecs_types::error::classified_error(
                        pi_ecs_types::error::ErrorKind::InvalidState,
                        "正常初始化失败",
                    ))
                })
            })
            .is_err());
        assert!(!world.is_stopped());
        world.schedule_scope(|_| Ok(())).unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = world.schedule_scope::<()>(|world| {
                world.parameter_registration_scope::<()>(|_| panic!("登记展开"))
            });
        }))
        .is_err());
        assert!(world.is_stopped());
        assert!(world
            .schedule_scope::<()>(|_| panic!("Stopped 不运行"))
            .is_err());
        world.parameter_unregistration_scope(|_| Ok(())).unwrap();
        assert!(world.is_stopped());
        world.close().unwrap();
        assert!(world
            .parameter_unregistration_scope::<()>(|_| panic!("Closed 不清偿"))
            .is_err());
        assert!(world
            .parameter_registration_scope::<()>(|_| panic!("Closed 不登记"))
            .is_err());
    }
    #[test]
    fn multi_resource_producer_batch_cross_world_token_rejected() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut first = World::new().unwrap();
        let mut other = World::new().unwrap();
        first.register_multi_resource::<Payload>().unwrap();
        other.register_multi_resource::<Payload>().unwrap();
        first.finish_registration().unwrap();
        other.finish_registration().unwrap();
        let token = first
            .schedule_scope(|world| {
                world.parameter_registration_scope(|mut scope| {
                    scope.register_producer(Payload(7, drops.clone()))
                })
            })
            .unwrap();
        assert!(other
            .parameter_unregistration_scope(|mut scope| scope.unregister_producer::<Payload>(token))
            .is_err());
        first
            .read_scope(|pin| assert_eq!(pin.multi_resource::<Payload>().unwrap().len(), 1))
            .unwrap();
        other
            .read_scope(|pin| assert!(pin.multi_resource::<Payload>().unwrap().is_empty()))
            .unwrap();
        first.close().unwrap();
        other.close().unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn multi_resource_producer_batch_identity_and_version_exhaustion() {
        for exhaust_version in [false, true] {
            let drops = Arc::new(AtomicUsize::new(0));
            let mut world = World::new().unwrap();
            let id = world.register_multi_resource::<Payload>().unwrap();
            world.finish_registration().unwrap();
            let token = world
                .schedule_scope(|world| {
                    world.parameter_registration_scope(|mut scope| {
                        scope.register_producer(Payload(7, drops.clone()))
                    })
                })
                .unwrap();
            // 本芯定义的真实集合仅注入不可实际遍历到达的计数上界，不伪造授权/责任。
            let core = world.multi_resources.entries[id.0 as usize]
                .downcast_mut::<super::super::MultiResourceCore<Payload>>()
                .unwrap();
            if exhaust_version {
                core.structure_version = u64::MAX;
            } else {
                core.next_producer = u64::MAX;
            }
            assert!(world
                .schedule_scope(|world| world.parameter_registration_scope(
                    |mut scope| scope.register_producer(Payload(11, drops.clone()))
                ))
                .is_err());
            assert!(!world.is_stopped());
            world
                .read_scope(|pin| {
                    let view = pin.multi_resource::<Payload>().unwrap();
                    assert_eq!(view.len(), 1);
                    assert_eq!(view.iter().next().unwrap().0, 7);
                })
                .unwrap();
            assert_eq!(drops.load(Ordering::SeqCst), 1);
            let result = world.parameter_unregistration_scope(|mut scope| {
                scope.unregister_producer::<Payload>(token)
            });
            assert_eq!(result.is_err(), exhaust_version);
            assert_eq!(world.is_stopped(), exhaust_version);
            assert_eq!(drops.load(Ordering::SeqCst), 2);
            world.close().unwrap();
            drop(world);
            assert_eq!(drops.load(Ordering::SeqCst), 2);
        }
    }
    #[test]
    fn multi_resource_producer_batch_destructor_unwind_consumes_once() {
        struct Panics(Arc<AtomicUsize>);
        impl Drop for Panics {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
                panic!("生产者析构失败");
            }
        }
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new().unwrap();
        world.register_multi_resource::<Panics>().unwrap();
        world.finish_registration().unwrap();
        let token = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|mut scope| {
                    scope.register_producer(Panics(drops.clone()))
                })
            })
            .unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = world.parameter_unregistration_scope(|mut scope| {
                scope.unregister_producer::<Panics>(token)
            });
        }))
        .is_err());
        assert!(world.is_stopped());
        world.close().unwrap();
        drop(world);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
    struct PayloadMissing(Arc<AtomicUsize>);
    impl Drop for PayloadMissing {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    // Seam: 公开根调度、参数登记、宿主聚合读取、静止点注销及关闭。
    // 不直接构造登记能力、生产者责任或 Running 状态。
    #[test]
    fn multi_resource_producer_batch_registration_unregister_and_close() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut world = World::new().unwrap();
        world.register_multi_resource::<Payload>().unwrap();
        world.finish_registration().unwrap();
        let registrations = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|mut registration| {
                    let first = registration.register_producer(Payload(7, drops.clone()))?;
                    let middle = registration.register_producer(Payload(11, drops.clone()))?;
                    let last = registration.register_producer(Payload(19, drops.clone()))?;
                    Ok((first, middle, last))
                })
            })
            .unwrap();
        let (first, middle, last) = registrations;
        assert_ne!(first.producer_id(), middle.producer_id());
        assert_ne!(middle.producer_id(), last.producer_id());
        {
            let pin = world.pin().unwrap();
            let view = pin.multi_resource::<Payload>().unwrap();
            assert_eq!(view.len(), 3);
            let mut values: Vec<_> = view.iter().map(|value| value.0).collect();
            values.sort_unstable();
            assert_eq!(values, [7, 11, 19]);
        }
        world
            .parameter_unregistration_scope(|mut registration| {
                registration.unregister_producer::<Payload>(middle)
            })
            .unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        {
            let pin = world.pin().unwrap();
            let view = pin.multi_resource::<Payload>().unwrap();
            assert_eq!(view.len(), 2);
            let mut values: Vec<_> = view.iter().map(|value| value.0).collect();
            values.sort_unstable();
            assert_eq!(values, [7, 19]);
        }
        world
            .parameter_unregistration_scope(|mut registration| {
                registration.unregister_producer::<Payload>(first)?;
                registration.unregister_producer::<Payload>(last)
            })
            .unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 3);
        assert!(world
            .read_scope(|pin| pin.multi_resource::<Payload>().unwrap().is_empty())
            .unwrap());
        world.close().unwrap();
        world.close().unwrap();
        drop(world);
        assert_eq!(drops.load(Ordering::SeqCst), 3);
    }
}
#[cfg(all(test, feature = "rc"))]
mod host_registry_resource_batch {
    use crate::world::World;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Payload(u32, Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.1.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn host_registry_resource_batch_lifecycle_gates_and_exhaustion() {
        use crate::world::AccessMode;
        let mut world = World::new().unwrap();
        world.register_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        world.insert_resource(7u32).unwrap();
        let before = world.read_change_tick();
        for mode in [
            AccessMode::Pinning(1),
            AccessMode::Running,
            AccessMode::Maintaining,
            AccessMode::Stopped,
        ] {
            world.access.mode.set(mode);
            assert!(world.register_resource::<u32>().is_err());
            assert!(world.register_multi_resource::<u32>().is_err());
            assert!(world.insert_resource(9u32).is_err());
            assert!(world.remove_resource::<u32>().is_err());
            assert!(world.write().is_err());
            assert!(world.pin().is_err());
            assert!(world.finish_registration().is_err());
            assert_eq!(world.read_change_tick(), before);
        }
        world.access.mode.set(AccessMode::Idle);
        assert_eq!(
            world
                .read_scope(|pin| pin.resource::<u32>().copied())
                .unwrap(),
            Some(7)
        );
        world.change_tick = u64::MAX;
        assert!(world.write().is_err());
        assert!(world.insert_resource(11u32).is_err());
        assert_eq!(world.read_change_tick().get(), u64::MAX);
        assert_eq!(world.remove_resource::<u32>().unwrap(), Some(7));
    }
    #[test]
    fn host_registry_resource_batch_write_unwind_stops() {
        let mut world = World::new().unwrap();
        world.register_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        world.insert_resource(7u32).unwrap();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _access = world.write().unwrap();
            panic!("host write callback");
        }))
        .is_err());
        assert!(world.pin().is_err());
        assert!(world.insert_resource(9u32).is_err());
        world.close().unwrap();
    }
    // 此真实写入验收保留：范围外 Mut::deref_mut 的失败桩不得以类型构造代替通过。
    #[test]
    fn host_registry_resource_batch_actual_mutation_blocked_by_change_detection() {
        let mut world = World::new().unwrap();
        world.register_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        world.insert_resource(7u32).unwrap();
        {
            let mut access = world.write().unwrap();
            let mut value = access.resource_mut::<u32>().unwrap();
            *value = 9;
        }
        assert_eq!(
            world
                .read_scope(|pin| pin.resource::<u32>().copied())
                .unwrap(),
            Some(9)
        );
    }
    // Seam: 独立多例目录及宿主空集合视图，不创建生产者。
    #[test]
    fn host_registry_resource_batch_multi_registration_and_empty_view() {
        let mut world = World::new().unwrap();
        let unique = world.register_resource::<u32>().unwrap();
        let multi = world.register_multi_resource::<u32>().unwrap();
        assert_eq!(world.register_multi_resource::<u32>().unwrap(), multi);
        world.finish_registration().unwrap();
        assert_eq!(world.register_resource::<u32>().unwrap(), unique);
        assert_eq!(world.register_multi_resource::<u32>().unwrap(), multi);
        assert!(world.register_multi_resource::<u64>().is_err());
        world.insert_resource(7u32).unwrap();
        {
            let pin = world.pin().unwrap();
            assert_eq!(pin.resource::<u32>(), Some(&7));
            let items = pin.multi_resource::<u32>().unwrap();
            assert!(items.is_empty());
            assert_eq!(items.len(), 0);
            assert_eq!(items.iter().count(), 0);
            assert!(pin.multi_resource::<u64>().is_none());
        }
        {
            let access = world.write().unwrap();
            assert!(access.multi_resource::<u32>().unwrap().is_empty());
            assert!(access.multi_resource::<u64>().is_none());
        }
        world.close().unwrap();
        assert!(world.register_multi_resource::<u32>().is_err());
    }
    // Seam: 公开 pin/write 读取不同 World 独立资源；写包装不提前标记。
    #[test]
    fn host_registry_resource_batch_authorized_reads() {
        let mut world = World::new().unwrap();
        let mut other = World::new().unwrap();
        world.register_resource::<u32>().unwrap();
        other.register_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        other.finish_registration().unwrap();
        world.insert_resource(7u32).unwrap();
        other.insert_resource(19u32).unwrap();
        assert_eq!(
            world
                .read_scope(|pin| pin.resource::<u32>().copied())
                .unwrap(),
            Some(7)
        );
        assert_eq!(
            other
                .read_scope(|pin| pin.resource::<u32>().copied())
                .unwrap(),
            Some(19)
        );
        let before = world.read_change_tick();
        {
            let mut access = world.write().unwrap();
            assert_eq!(access.resource::<u32>(), Some(&7));
            assert!(access.resource::<u64>().is_none());
            assert!(access.resource_mut::<u32>().is_some());
            assert!(access.resource_mut::<u64>().is_none());
        }
        assert_eq!(world.read_change_tick().get(), before.get() + 1);
        world.remove_resource::<u32>().unwrap();
        assert_eq!(
            world
                .read_scope(|pin| pin.resource::<u32>().copied())
                .unwrap(),
            None
        );
        world.close().unwrap();
        assert!(world.pin().is_err());
        assert!(world.write().is_err());
    }
    // Seam: World 资源登记、插入、替换、移出和关闭；Payload 并非 Component。
    #[test]
    fn host_registry_resource_batch_ownership_and_frozen_types() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut world = World::new().unwrap();
        let id = world.register_resource::<Payload>().unwrap();
        assert_eq!(world.register_resource::<Payload>().unwrap(), id);
        world.finish_registration().unwrap();
        assert_eq!(world.register_resource::<Payload>().unwrap(), id);
        assert!(world.register_resource::<u64>().is_err());
        assert!(world
            .insert_resource(Payload(7, count.clone()))
            .unwrap()
            .is_none());
        let old = world
            .insert_resource(Payload(9, count.clone()))
            .unwrap()
            .unwrap();
        assert_eq!(old.0, 7);
        assert_eq!(count.load(Ordering::SeqCst), 0);
        drop(old);
        let removed = world.remove_resource::<Payload>().unwrap().unwrap();
        assert_eq!(removed.0, 9);
        assert!(world.remove_resource::<Payload>().unwrap().is_none());
        drop(removed);
        assert_eq!(count.load(Ordering::SeqCst), 2);
        assert!(world
            .insert_resource(Payload(11, count.clone()))
            .unwrap()
            .is_none());
        world.close().unwrap();
        world.close().unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 3);
        assert!(world.register_resource::<Payload>().is_err());
        assert!(world.remove_resource::<Payload>().is_err());
        assert!(world.insert_resource(Payload(13, count.clone())).is_err());
        assert_eq!(count.load(Ordering::SeqCst), 4);
        drop(world);
        assert_eq!(count.load(Ordering::SeqCst), 4);
    }
}
#[cfg(feature = "rc")]
impl crate::world::World {
    pub(super) fn register_multi_resource_inner<T: Send + Sync + 'static>(
        &mut self,
    ) -> Result<crate::resource::ResourceId, crate::error::EcsError> {
        idle(self)?;
        let ty = std::any::TypeId::of::<T>();
        PRODUCER_LOOKUPS
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(ty, producer_lookup::<T>);
        if let Some(&id) = self.multi_resources.ids.get(&ty) {
            return Ok(id);
        }
        if self.registry.frozen {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "冻结后不能新增多例类型",
            ));
        }
        let index = u32::try_from(self.multi_resources.entries.len()).map_err(|_| {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "多例类型编号耗尽",
            )
        })?;
        let id = super::ResourceId(index);
        self.multi_resources
            .entries
            .push(Box::new(super::MultiResourceCore::<T> {
                next_producer: 0,
                structure_version: 0,
                producer_offsets: Default::default(),
                values: std::cell::UnsafeCell::new(Vec::new()),
                values_root: std::ptr::NonNull::dangling(),
                ticks: std::cell::UnsafeCell::new(Vec::new()),
                ticks_root: std::ptr::NonNull::dangling(),
                producers: Vec::new(),
            }));
        self.multi_resources.ids.insert(ty, id);
        Ok(id)
    }
    pub(super) fn producer_binding_inner(
        &self,
        _registration: &crate::parameter_registration::ProducerRegistration,
    ) -> Result<crate::resource::ProducerBinding, crate::error::EcsError> {
        let invalid = || {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "生产者绑定来源或责任无效",
            )
        };
        let mode = self.access.mode.replace(crate::world::AccessMode::Stopped);
        let allowed = matches!(
            mode,
            crate::world::AccessMode::Idle | crate::world::AccessMode::Running
        );
        self.access.mode.set(mode);
        if !allowed
            || self.shutdown != crate::shutdown::ShutdownState::Open
            || !self.registry.frozen
            || _registration.world_identity != self.identity
        {
            return Err(invalid());
        }
        let ty = self
            .multi_resources
            .ids
            .iter()
            .find_map(|(ty, id)| (*id == _registration.resource).then_some(*ty))
            .ok_or_else(invalid)?;
        let lookup = PRODUCER_LOOKUPS
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&ty)
            .copied()
            .ok_or_else(invalid)?;
        let entry = self
            .multi_resources
            .entries
            .get(_registration.resource.0 as usize)
            .ok_or_else(invalid)?;
        lookup(entry.as_ref(), _registration.producer).ok_or_else(invalid)
    }
}
use crate::{change_detection::Mut, error::EcsError, resource::ResourceId, world::World};
#[cfg(feature = "rc")]
fn multi_read<T: Send + Sync + 'static>(world: &World) -> Option<super::MultiResourceRead<'_, T>> {
    let id = world
        .multi_resources
        .ids
        .get(&std::any::TypeId::of::<T>())?;
    let core = world.multi_resources.entries[id.0 as usize]
        .downcast_ref::<super::MultiResourceCore<T>>()
        .expect("多例目录类型一致");
    // SAFETY: 原宿主授权排除全部生产槽写及结构维护；只使用缓存根，不重借 Vec 所有者。
    let values =
        unsafe { std::slice::from_raw_parts(core.values_root.as_ptr(), core.producers.len()) };
    Some(super::MultiResourceRead { values })
}
#[cfg(feature = "rc")]
fn idle(world: &World) -> Result<(), EcsError> {
    let mode = world.access.mode.replace(crate::world::AccessMode::Stopped);
    let allowed = matches!(mode, crate::world::AccessMode::Idle);
    world.access.mode.set(mode);
    if !allowed || !matches!(world.shutdown, crate::shutdown::ShutdownState::Open) {
        return Err(pi_ecs_types::error::classified_error(
            pi_ecs_types::error::ErrorKind::InvalidState,
            "资源维护要求未关闭且 Idle",
        ));
    }
    Ok(())
}
#[cfg(feature = "rc")]
fn next_tick(world: &mut World) -> Result<crate::change_detection::ChangeTick, EcsError> {
    let next = world.change_tick.checked_add(1).ok_or_else(|| {
        pi_ecs_types::error::classified_error(
            pi_ecs_types::error::ErrorKind::ResourceExhausted,
            "变化时钟耗尽",
        )
    })?;
    world.change_tick = next;
    Ok(crate::change_detection::ChangeTick::new(
        std::num::NonZeroU64::new(next).expect("非零时钟"),
    ))
}
impl World {
    pub(crate) fn register_resource_inner<T: Send + Sync + 'static>(
        _world: &mut World,
    ) -> Result<ResourceId, EcsError> {
        #[cfg(feature = "rc")]
        {
            idle(_world)?;
            let ty = std::any::TypeId::of::<T>();
            if let Some(&id) = _world.resources.ids.get(&ty) {
                return Ok(id);
            }
            if _world.registry.frozen {
                return Err(pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::InvalidState,
                    "冻结后不能新增资源类型",
                ));
            }
            let index = u32::try_from(_world.resources.entries.len()).map_err(|_| {
                pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::ResourceExhausted,
                    "资源编号耗尽",
                )
            })?;
            let id = ResourceId(index);
            _world.resources.entries.push(None);
            _world.resources.ids.insert(ty, id);
            Ok(id)
        }
        #[cfg(not(feature = "rc"))]
        todo!("编号：T59，尚未实现， 非 RC 资源存储与授权接线")
    }
}
#[cfg(feature = "rc")]
impl World {
    pub(crate) fn insert_resource_inner<T: Send + Sync + 'static>(
        _world: &mut World,
        _value: T,
    ) -> Result<Option<T>, EcsError> {
        idle(_world)?;
        let id = _world
            .resources
            .ids
            .get(&std::any::TypeId::of::<T>())
            .copied()
            .ok_or_else(|| {
                pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::NotFound,
                    "资源类型未登记",
                )
            })?;
        let tick = next_tick(_world)?;
        _world
            .access
            .mode
            .set(crate::world::AccessMode::Maintaining);
        let slot = &mut _world.resources.entries[id.0 as usize];
        let added = slot
            .as_ref()
            .map(|entry| {
                let value = entry
                    .value
                    .downcast_ref::<super::ResourceValue<T>>()
                    .expect("资源目录类型一致");
                // SAFETY: 独占维护，无资源值/tick 引用存活。
                unsafe { (*value.ticks.get()).added }
            })
            .unwrap_or(tick);
        let replacement = super::ResourceEntry {
            value: Box::new(super::ResourceValue {
                value: std::cell::UnsafeCell::new(_value),
                ticks: std::cell::UnsafeCell::new(super::ResourceTicks {
                    added,
                    changed: tick,
                }),
            }),
            recovery: super::StateRecovery::Preserve,
        };
        let old = slot.replace(replacement).map(|entry| {
            let value = entry
                .value
                .downcast::<super::ResourceValue<T>>()
                .unwrap_or_else(|_| panic!("资源目录类型一致"));
            value.value.into_inner()
        });
        _world.access.mode.set(crate::world::AccessMode::Idle);
        Ok(old)
    }
}
#[cfg(not(feature = "rc"))]
impl World {
    pub(crate) fn insert_resource_inner<T: Send + Sync + 'static>(
        _world: &World,
        _value: T,
    ) -> Result<Option<T>, EcsError> {
        todo!("编号：T60，尚未实现， 非 rc 资源插入")
    }
}
#[cfg(feature = "rc")]
impl World {
    pub(crate) fn remove_resource_inner<T: Send + Sync + 'static>(
        _world: &mut World,
    ) -> Result<Option<T>, EcsError> {
        idle(_world)?;
        let Some(&id) = _world.resources.ids.get(&std::any::TypeId::of::<T>()) else {
            return Ok(None);
        };
        _world
            .access
            .mode
            .set(crate::world::AccessMode::Maintaining);
        let old = _world.resources.entries[id.0 as usize].take().map(|entry| {
            let value = entry
                .value
                .downcast::<super::ResourceValue<T>>()
                .unwrap_or_else(|_| panic!("资源目录类型一致"));
            value.value.into_inner()
        });
        _world.access.mode.set(crate::world::AccessMode::Idle);
        Ok(old)
    }
}
#[cfg(not(feature = "rc"))]
impl World {
    pub(crate) fn remove_resource_inner<T: Send + Sync + 'static>(
        _world: &World,
    ) -> Result<Option<T>, EcsError> {
        todo!("编号：T61，尚未实现， 非 rc 资源移出")
    }
}
impl crate::world::WorldPin<'_> {
    pub(crate) fn pin_resource_inner<'a, T: Send + Sync + 'static>(
        _access: &'a crate::world::WorldPin<'_>,
    ) -> Option<&'a T> {
        #[cfg(feature = "rc")]
        {
            let world = _access.world;
            let id = world.resources.ids.get(&std::any::TypeId::of::<T>())?;
            let value = world.resources.entries[id.0 as usize]
                .as_ref()?
                .value
                .downcast_ref::<super::ResourceValue<T>>()
                .expect("资源类型一致");
            // SAFETY: pin 排除资源写与替换；引用只借本访问对象。
            Some(unsafe { &*value.value.get() })
        }
        #[cfg(not(feature = "rc"))]
        todo!("编号：T62，尚未实现， 非 RC 授权资源读取")
    }
}
impl crate::world::WorldWrite<'_> {
    pub(crate) fn write_resource_inner<'a, T: Send + Sync + 'static>(
        _access: &'a crate::world::WorldWrite<'_>,
    ) -> Option<&'a T> {
        #[cfg(feature = "rc")]
        {
            _access.require_active();
            let world = &*_access.world;
            let id = world.resources.ids.get(&std::any::TypeId::of::<T>())?;
            let value = world.resources.entries[id.0 as usize]
                .as_ref()?
                .value
                .downcast_ref::<super::ResourceValue<T>>()
                .expect("资源类型一致");
            // SAFETY: 独占授权的共享短借，期间不能取资源 Mut。
            Some(unsafe { &*value.value.get() })
        }
        #[cfg(not(feature = "rc"))]
        todo!("编号：T63，尚未实现， 非 RC 授权资源读取")
    }
}
impl crate::world::WorldWrite<'_> {
    pub(crate) fn write_resource_mut_inner<'a, T: Send + Sync + 'static>(
        _access: &'a mut crate::world::WorldWrite<'_>,
    ) -> Option<Mut<'a, T>> {
        #[cfg(feature = "rc")]
        {
            _access.require_active();
            let id = _access
                .world
                .resources
                .ids
                .get(&std::any::TypeId::of::<T>())?;
            let entry = _access.world.resources.entries[id.0 as usize].as_mut()?;
            let value = entry
                .value
                .downcast_mut::<super::ResourceValue<T>>()
                .expect("资源类型一致");
            Some(Mut {
                value: value.value.get_mut(),
                marker: crate::change_detection::ChangeMarker {
                    tick: _access.tick,
                    target: crate::change_detection::ChangeTarget::Resource(value.ticks.get_mut()),
                },
                marked: false,
            })
        }
        #[cfg(not(feature = "rc"))]
        todo!("编号：T64，尚未实现， 非 RC 授权资源写入")
    }
}
impl crate::world::WorldPin<'_> {
    pub(crate) fn pin_multi_inner<'a, T: Send + Sync + 'static>(
        _access: &'a crate::world::WorldPin<'_>,
    ) -> Option<crate::resource::MultiResourceRead<'a, T>> {
        #[cfg(feature = "rc")]
        {
            multi_read::<T>(_access.world)
        }
        #[cfg(not(feature = "rc"))]
        todo!("编号：T65，尚未实现， 非 RC 授权多例视图")
    }
}
impl crate::world::WorldWrite<'_> {
    pub(crate) fn write_multi_inner<'a, T: Send + Sync + 'static>(
        _access: &'a crate::world::WorldWrite<'_>,
    ) -> Option<crate::resource::MultiResourceRead<'a, T>> {
        #[cfg(feature = "rc")]
        {
            _access.require_active();
            multi_read::<T>(&*_access.world)
        }
        #[cfg(not(feature = "rc"))]
        todo!("编号：T66，尚未实现， 非 RC 授权多例视图")
    }
}
impl<T> crate::resource::MultiResourceRead<'_, T> {
    pub(crate) fn multi_len_inner(_view: &crate::resource::MultiResourceRead<'_, T>) -> usize {
        _view.values.len()
    }
}
impl<T> crate::resource::MultiResourceRead<'_, T> {
    pub(crate) fn multi_empty_inner(_view: &crate::resource::MultiResourceRead<'_, T>) -> bool {
        _view.values.is_empty()
    }
}
impl<T> crate::resource::MultiResourceRead<'_, T> {
    pub(crate) fn multi_iter_inner<'a>(
        _view: &'a crate::resource::MultiResourceRead<'_, T>,
    ) -> std::slice::Iter<'a, T> {
        _view.values.iter()
    }
}

#[cfg(feature = "rc")]
impl<'w> crate::access::RunAccess<'w> {
    fn multi_core<T: Send + Sync + 'static>(
        &self,
        resource: ResourceId,
    ) -> Result<&'w super::MultiResourceCore<T>, EcsError> {
        // SAFETY: 共享借用固定目录及类型化元数据，不借 values/ticks Vec 所有者。
        let store = unsafe { &*std::ptr::addr_of!((*self.world.pointer.as_ptr()).multi_resources) };
        if store.ids.get(&std::any::TypeId::of::<T>()) != Some(&resource) {
            return Err(run_resource_error());
        }
        store
            .entries
            .get(resource.0 as usize)
            .and_then(|entry| entry.downcast_ref::<super::MultiResourceCore<T>>())
            .ok_or_else(run_resource_error)
    }
    fn unique_value<T: Send + Sync + 'static>(
        &self,
        resource: ResourceId,
        write: bool,
    ) -> Result<Option<&'w super::ResourceValue<T>>, EcsError> {
        let invalid = || {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "资源类型、叶子或权限无效",
            )
        };
        let crate::access::AccessTree::Leaf(declaration) = self.declaration else {
            return Err(invalid());
        };
        if !declaration.resources.iter().any(|(id, mode)| {
            *id == resource && (!write || *mode == crate::access::AccessMode::Write)
        }) || (write && !self.world.mutable)
        {
            return Err(invalid());
        }
        // SAFETY: 仅共享冻结目录；不还原 World、不覆盖其他获准资源的 UnsafeCell 内容。
        let store = unsafe { &*std::ptr::addr_of!((*self.world.pointer.as_ptr()).resources) };
        if store.ids.get(&std::any::TypeId::of::<T>()) != Some(&resource) {
            return Err(invalid());
        }
        let entry = store.entries.get(resource.0 as usize).ok_or_else(invalid)?;
        entry
            .as_ref()
            .map(|entry| {
                entry
                    .value
                    .downcast_ref::<super::ResourceValue<T>>()
                    .ok_or_else(invalid)
            })
            .transpose()
    }

    pub(super) fn resource_parts_inner<T: Send + Sync + 'static>(
        &self,
        _resource: super::ResourceId,
    ) -> Result<Option<(&'w T, &'w super::ResourceTicks)>, crate::error::EcsError> {
        let value = self.unique_value::<T>(_resource, false)?;
        // SAFETY: 原叶子读/写授权及 fetch 协议排除冲突；字段 UnsafeCell 根保活至 'w。
        Ok(value.map(|value| unsafe { (&*value.value.get(), &*value.ticks.get()) }))
    }
    pub(super) fn resource_mut_inner<T: Send + Sync + 'static>(
        &self,
        _resource: super::ResourceId,
        _tick: crate::change_detection::ChangeTick,
    ) -> Result<Option<crate::change_detection::Mut<'w, T>>, crate::error::EcsError> {
        let value = self.unique_value::<T>(_resource, true)?;
        // SAFETY: 调用方兑现本叶子一次；两个独立 UnsafeCell 字段仅覆盖获准单槽。
        Ok(value.map(|value| unsafe {
            Mut {
                value: &mut *value.value.get(),
                marker: crate::change_detection::ChangeMarker {
                    tick: _tick,
                    target: crate::change_detection::ChangeTarget::Resource(
                        &mut *value.ticks.get(),
                    ),
                },
                marked: false,
            }
        }))
    }
    pub(super) fn multi_resource_parts_inner<T: Send + Sync + 'static>(
        &self,
        _resource: super::ResourceId,
    ) -> Result<super::MultiResourceView<'w, T>, crate::error::EcsError> {
        let crate::access::AccessTree::Leaf(declaration) = self.declaration else {
            return Err(run_resource_error());
        };
        if !declaration.multi_resources.iter().any(|(id, mode)| {
            *id == _resource && matches!(mode, crate::access::MultiResourceAccess::ReadAll)
        }) {
            return Err(run_resource_error());
        }
        let core = self.multi_core::<T>(_resource)?;
        // SAFETY: ReadAll 排除所有写槽；运行期结构固定，两个缓存根由维护期同时发布。
        Ok(super::MultiResourceView {
            values: unsafe {
                std::slice::from_raw_parts(core.values_root.as_ptr(), core.producers.len())
            },
            ticks: unsafe {
                std::slice::from_raw_parts(core.ticks_root.as_ptr(), core.producers.len())
            },
            structure_version: core.structure_version,
        })
    }
    pub(super) fn producer_mut_inner<T: Send + Sync + 'static>(
        &self,
        _registration: &crate::parameter_registration::ProducerRegistration,
        _binding: &super::ProducerBinding,
        _tick: crate::change_detection::ChangeTick,
    ) -> Result<crate::change_detection::Mut<'w, T>, crate::error::EcsError> {
        let crate::access::AccessTree::Leaf(declaration) = self.declaration else {
            return Err(run_resource_error());
        };
        if !self.world.mutable
            || self.world.id() != _registration.world_identity
            || !declaration.multi_resources.iter().any(|(id, mode)| {
                *id == _registration.resource
                    && matches!(mode,
                    crate::access::MultiResourceAccess::WriteSlot { producer }
                    if *producer == _registration.producer)
            })
        {
            return Err(run_resource_error());
        }
        let core = self.multi_core::<T>(_registration.resource)?;
        if core.structure_version != _binding.structure_version
            || core.producers.get(_binding.offset)
                != Some(&super::ProducerId(_registration.producer))
            || core
                .producer_offsets
                .get(&super::ProducerId(_registration.producer))
                != Some(&_binding.offset)
        {
            return Err(run_resource_error());
        }
        // SAFETY: 稳定生产者、当前版本和偏移均校验；只从独占来源缓存根投影获准单槽。
        // 不触碰 Vec 所有者或形成整个集合可变切片，故不同生产者的结果可同时存活。
        Ok(unsafe {
            Mut {
                value: &mut *core.values_root.as_ptr().add(_binding.offset),
                marker: crate::change_detection::ChangeMarker {
                    tick: _tick,
                    target: crate::change_detection::ChangeTarget::Multi(
                        &mut *core.ticks_root.as_ptr().add(_binding.offset),
                    ),
                },
                marked: false,
            }
        })
    }
}
