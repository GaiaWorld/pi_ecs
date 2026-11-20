//! RC 整体关闭配对芯。
#[cfg(all(test, feature = "rc"))]
mod shutdown_lifecycle_batch {
    use crate::world::{AccessMode, World};
    use crate::{
        component::{Component, PlainAccess, StoragePolicy},
        entity::{ArchetypeId, Entity},
        ptr::OwningPtr,
    };
    use std::{
        mem::ManuallyDrop,
        sync::{Arc, Mutex},
    };
    struct Payload(u32, Arc<Mutex<Vec<u32>>>);
    impl Component for Payload {
        type Access = PlainAccess;
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.1.lock().unwrap().push(self.0);
        }
    }
    // Fixture 仅用已实现的登记/物理安装接口，构造真实 World；验收调用 close/Drop。
    fn install(world: &mut World, ty: ArchetypeId) -> Entity {
        let row = world
            .storage
            .as_mut()
            .unwrap()
            .table_mut(ty.table)
            .unwrap()
            .claim_row()
            .unwrap();
        let entity = world
            .entities
            .borrow_mut()
            .allocate(
                crate::entity::AllocationDomain(0),
                crate::entity::EntityLocation::from_parts(0, 0, row.index(), ty.table, ty.variant),
            )
            .unwrap();
        let member = world
            .storage
            .as_mut()
            .unwrap()
            .append_member(ty, entity, row)
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
                    ty.table,
                    ty.variant,
                ),
            )
            .unwrap();
        world
            .storage
            .as_mut()
            .unwrap()
            .table_mut(ty.table)
            .unwrap()
            .publish_row(row, entity)
            .unwrap();
        entity
    }
    fn sparse<C: Component>(
        world: &mut World,
        id: crate::component::ComponentId,
        entity: Entity,
        value: C,
    ) {
        world
            .storage
            .as_mut()
            .unwrap()
            .prepare_sparse(id, entity)
            .unwrap();
        let mut value = ManuallyDrop::new(value);
        let pointer = unsafe { OwningPtr::from_raw(std::ptr::NonNull::from(&mut *value).cast()) };
        world
            .storage
            .as_mut()
            .unwrap()
            .insert_sparse(id, entity, pointer);
    }
    #[test]
    fn shutdown_lifecycle_batch_values_once() {
        for explicit in [false, true] {
            for policy in [StoragePolicy::SparseOnly, StoragePolicy::TableEligible] {
                let log = Arc::new(Mutex::new(Vec::new()));
                let mut world = World::new().unwrap();
                let id = world
                    .registry
                    .register_component::<Payload>(policy)
                    .unwrap();
                world.notifications.layouts = vec![Some(std::alloc::Layout::new::<Payload>())];
                world.notifications.type_ids = vec![Some(std::any::TypeId::of::<Payload>())];
                let table = if matches!(policy, StoragePolicy::TableEligible) {
                    Some(world.registry.register_table(&[id]).unwrap())
                } else {
                    None
                };
                // 宿主 insert_resource 仍是 T263；夹具按已审布局安装真实资源 owner。
                let tick =
                    crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(1).unwrap());
                world
                    .resources
                    .entries
                    .push(Some(crate::resource::ResourceEntry {
                        value: Box::new(crate::resource::ResourceValue {
                            value: std::cell::UnsafeCell::new(Payload(4, log.clone())),
                            ticks: std::cell::UnsafeCell::new(crate::resource::ResourceTicks {
                                added: tick,
                                changed: tick,
                            }),
                        }),
                        recovery: crate::resource::StateRecovery::Preserve,
                    }));
                // 多例/默认目录的真实擦除 owner；独立于 Component 释放路径。
                let mut values = vec![Payload(5, log.clone()), Payload(6, log.clone())];
                let mut ticks = vec![
                    crate::resource::ResourceTicks {
                        added: tick,
                        changed: tick,
                    },
                    crate::resource::ResourceTicks {
                        added: tick,
                        changed: tick,
                    },
                ];
                world
                    .multi_resources
                    .entries
                    .push(Box::new(crate::resource::MultiResourceCore {
                        next_producer: 2,
                        structure_version: 2,
                        producer_offsets: [
                            (crate::resource::ProducerId(0), 0),
                            (crate::resource::ProducerId(1), 1),
                        ]
                        .into(),
                        values_root: std::ptr::NonNull::new(values.as_mut_ptr()).unwrap(),
                        ticks_root: std::ptr::NonNull::new(ticks.as_mut_ptr()).unwrap(),
                        values: std::cell::UnsafeCell::new(values),
                        ticks: std::cell::UnsafeCell::new(ticks),
                        producers: vec![
                            crate::resource::ProducerId(0),
                            crate::resource::ProducerId(1),
                        ],
                    }));
                world.query_defaults.values.insert(
                    std::any::TypeId::of::<Payload>(),
                    Box::new(Payload(7, log.clone())),
                );
                world.finish_registration().unwrap();
                let ty = table.unwrap_or_else(|| {
                    let plan = world.registry.plan_archetype(&[id]).unwrap();
                    let ty = world.registry.commit_archetype(plan);
                    world
                        .storage
                        .as_mut()
                        .unwrap()
                        .prepare_archetype(ty)
                        .unwrap();
                    ty
                });
                let entity = install(&mut world, ty);
                if table.is_some() {
                    let row = world.entities.borrow().locate(entity).unwrap().table_row();
                    let mut value = ManuallyDrop::new(Payload(1, log.clone()));
                    unsafe {
                        world
                            .storage
                            .as_mut()
                            .unwrap()
                            .table_mut(ty.table)
                            .unwrap()
                            .column_mut(crate::table::ColumnId(0))
                            .unwrap()
                            .insert_raw(
                                row as usize,
                                std::ptr::from_mut(&mut *value).cast(),
                                std::alloc::Layout::new::<Payload>(),
                            )
                    };
                } else {
                    sparse(&mut world, id, entity, Payload(1, log.clone()));
                }
                let reader = world
                    .notifications
                    .subscribe(id, crate::notification::NoticeKind::Removed)
                    .unwrap();
                let tick =
                    crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(1).unwrap());
                let mut old = ManuallyDrop::new(Payload(2, log.clone()));
                assert!(world
                    .notifications
                    .append_removed(id, entity, tick, unsafe {
                        OwningPtr::from_raw(std::ptr::NonNull::from(&mut *old).cast())
                    })
                    .is_ok());
                // 模拟已移入而未发布头的尾值，不能按 end/headers/读者游标枚举。
                let mut tail = ManuallyDrop::new(Payload(3, log.clone()));
                unsafe {
                    world.notifications.components[id.0 as usize]
                        .removed
                        .as_mut()
                        .unwrap()
                        .get_mut()
                        .values
                        .push_back(OwningPtr::from_raw(
                            std::ptr::NonNull::from(&mut *tail).cast(),
                        ))
                };
                drop(reader);
                // 只有 prepare、无活值，不能读取或析构业务字节。
                world
                    .storage
                    .as_mut()
                    .unwrap()
                    .prepare_sparse(id, Entity::default())
                    .unwrap();
                let tick = world.change_tick;
                if explicit {
                    world.close().unwrap();
                    world.close().unwrap();
                    assert_eq!(world.change_tick, tick);
                    assert!(world.storage.is_none());
                }
                drop(world);
                let mut actual = log.lock().unwrap().clone();
                actual.sort_unstable();
                assert_eq!(actual, vec![1, 2, 3, 4, 5, 6, 7]);
                assert_eq!(Arc::strong_count(&log), 1);
            }
        }
    }
    #[test]
    fn shutdown_lifecycle_batch_cycles_and_all_containers() {
        use crate::{
            entity_container::*,
            owned_entity::{StrongEntities, WeakEntities},
        };
        use std::marker::PhantomData;
        struct Check {
            root: usize,
            entities: [Entity; 2],
            log: Arc<Mutex<Vec<u32>>>,
        }
        impl Drop for Check {
            fn drop(&mut self) {
                let world = unsafe { &*(self.root as *const World) };
                for entity in self.entities {
                    assert_eq!(world.entities.borrow().counts(entity), Some((0, 0)));
                }
                assert!(world.pending_destruction.pending.is_empty());
                assert!(world.pending_destruction.reclamation.is_empty());
                assert!(world.storage.is_some());
                assert!(!world.registry.components.is_empty());
                self.log.lock().unwrap().push(99);
            }
        }
        for explicit in [true, false] {
            let log = Arc::new(Mutex::new(Vec::new()));
            let mut world = Box::new(World::new().unwrap());
            let strong = world
                .registry
                .register_component::<StrongEntities<1, String>>(StoragePolicy::SparseOnly)
                .unwrap();
            let weak = world
                .registry
                .register_component::<WeakEntities<2, String>>(StoragePolicy::SparseOnly)
                .unwrap();
            let list = world
                .registry
                .register_component::<EntityList<String>>(StoragePolicy::SparseOnly)
                .unwrap();
            let queue = world
                .registry
                .register_component::<EntityQueue<String>>(StoragePolicy::SparseOnly)
                .unwrap();
            let deque = world
                .registry
                .register_component::<EntityDeque<String>>(StoragePolicy::SparseOnly)
                .unwrap();
            world.finish_registration().unwrap();
            let empty = ArchetypeId {
                table: 0,
                variant: 0,
            };
            let a = install(&mut world, empty);
            let b = install(&mut world, empty);
            // 两个初始 strong 责任转给互相指向的真实头，形成不归零强环。
            sparse(
                &mut world,
                strong,
                a,
                StrongEntities::<1, String> {
                    entities: [b],
                    marker: PhantomData,
                },
            );
            sparse(
                &mut world,
                strong,
                b,
                StrongEntities::<1, String> {
                    entities: [a],
                    marker: PhantomData,
                },
            );
            for _ in 0..2 {
                world.entities.borrow_mut().acquire_weak(b).unwrap();
            }
            sparse(
                &mut world,
                weak,
                a,
                WeakEntities::<2, String> {
                    entities: [b, b],
                    marker: PhantomData,
                },
            );
            let mut keys = Vec::new();
            for _ in 0..4 {
                world.entities.borrow_mut().acquire_strong(b).unwrap();
                keys.push(world.entity_nodes.links.borrow_mut().insert(EntityLink {
                    value: b,
                    next: None,
                }));
            }
            world
                .entity_nodes
                .links
                .borrow_mut()
                .get_mut(keys[0])
                .unwrap()
                .next = Some(keys[1]);
            world
                .entity_nodes
                .links
                .borrow_mut()
                .get_mut(keys[2])
                .unwrap()
                .next = Some(keys[3]);
            sparse(
                &mut world,
                list,
                a,
                EntityList::<String> {
                    head: Some(keys[0]),
                    tail: Some(keys[1]),
                    len: 2,
                    marker: PhantomData,
                },
            );
            sparse(
                &mut world,
                queue,
                a,
                EntityQueue::<String> {
                    head: Some(keys[2]),
                    tail: Some(keys[3]),
                    len: 2,
                    marker: PhantomData,
                },
            );
            let mut head = pi_slot_deque::Deque::new();
            for _ in 0..2 {
                world.entities.borrow_mut().acquire_strong(b).unwrap();
                head.ops(&mut world.entity_nodes.deque.borrow_mut())
                    .push_back(EntityNode { value: b });
            }
            // Deque 的头责任保存在 Removed 中，确保节点余责检查晚于历史释放。
            let info = world.registry.component(deque).unwrap();
            world
                .notifications
                .layouts
                .resize(world.registry.components.len(), None);
            world
                .notifications
                .type_ids
                .resize(world.registry.components.len(), None);
            world.notifications.layouts[deque.0 as usize] = Some(info.layout);
            world.notifications.type_ids[deque.0 as usize] = Some(info.type_id);
            let _reader = world
                .notifications
                .subscribe(deque, crate::notification::NoticeKind::Removed)
                .unwrap();
            let mut value = ManuallyDrop::new(EntityDeque::<String> {
                deque: head,
                marker: PhantomData,
            });
            unsafe {
                world.notifications.components[deque.0 as usize]
                    .removed
                    .as_mut()
                    .unwrap()
                    .get_mut()
                    .values
                    .push_back(OwningPtr::from_raw(
                        std::ptr::NonNull::from(&mut *value).cast(),
                    ))
            };
            assert_eq!(world.entities.borrow().counts(a), Some((1, 0)));
            assert_eq!(world.entities.borrow().counts(b), Some((7, 2)));
            let root = std::ptr::from_ref(&*world) as usize;
            world
                .resources
                .entries
                .push(Some(crate::resource::ResourceEntry {
                    value: Box::new(Check {
                        root,
                        entities: [a, b],
                        log: log.clone(),
                    }),
                    recovery: crate::resource::StateRecovery::Preserve,
                }));
            if explicit {
                world.close().unwrap();
                world.close().unwrap();
            }
            drop(world);
            assert_eq!(*log.lock().unwrap(), vec![99]);
        }
    }
    #[test]
    fn shutdown_lifecycle_batch_commands_once() {
        use crate::command::{CommandCapacity, SimulationTick};
        for explicit in [true, false] {
            let log = Arc::new(Mutex::new(Vec::new()));
            let mut world = World::new().unwrap();
            world
                .commands
                .register::<Payload>(CommandCapacity::Unbounded)
                .unwrap();
            let sender = world.commands.sender::<Payload>(world.epoch).unwrap();
            let mut receiver = world.commands.receiver::<Payload>(world.epoch).unwrap();
            assert!(sender.send(Payload(1, log.clone())).is_ok());
            assert!(sender
                .schedule_at(SimulationTick(100), Payload(2, log.clone()))
                .is_ok());
            if explicit {
                world.close().unwrap();
                world.close().unwrap();
            }
            drop(world);
            let rejected = sender.send(Payload(3, log.clone())).err().unwrap();
            assert!(receiver.read().is_err());
            assert_eq!(log.lock().unwrap().len(), 2);
            drop(rejected);
            drop((sender, receiver));
            let mut actual = log.lock().unwrap().clone();
            actual.sort_unstable();
            assert_eq!(actual, vec![1, 2, 3]);
            assert_eq!(Arc::strong_count(&log), 1);
        }
    }
    static ZST_DROPS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    struct Zst;
    impl Component for Zst {
        type Access = PlainAccess;
    }
    impl Drop for Zst {
        fn drop(&mut self) {
            ZST_DROPS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    #[test]
    fn shutdown_lifecycle_batch_zst_each_responsibility() {
        ZST_DROPS.store(0, std::sync::atomic::Ordering::SeqCst);
        let mut world = World::new().unwrap();
        let id = world
            .registry
            .register_component::<Zst>(StoragePolicy::SparseOnly)
            .unwrap();
        world.notifications.layouts = vec![Some(std::alloc::Layout::new::<Zst>())];
        world.notifications.type_ids = vec![Some(std::any::TypeId::of::<Zst>())];
        world.finish_registration().unwrap();
        for _ in 0..3 {
            let entity = install(
                &mut world,
                ArchetypeId {
                    table: 0,
                    variant: 0,
                },
            );
            sparse(&mut world, id, entity, Zst);
        }
        let _reader = world
            .notifications
            .subscribe(id, crate::notification::NoticeKind::Removed)
            .unwrap();
        for _ in 0..4 {
            let mut value = ManuallyDrop::new(Zst);
            unsafe {
                world.notifications.components[id.0 as usize]
                    .removed
                    .as_mut()
                    .unwrap()
                    .get_mut()
                    .values
                    .push_back(OwningPtr::from_raw(
                        std::ptr::NonNull::from(&mut *value).cast(),
                    ))
            };
        }
        world.close().unwrap();
        world.close().unwrap();
        drop(world);
        assert_eq!(ZST_DROPS.load(std::sync::atomic::Ordering::SeqCst), 7);
    }
    fn abort_child(case: &str) -> std::process::Output {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "shutdown::shutdown_inner::shutdown_lifecycle_batch::shutdown_lifecycle_batch_abort_driver", "--nocapture"])
            .env("PI_ECS_SHUTDOWN_ABORT", case).output().unwrap()
    }
    fn exact_abort(output: &std::process::Output, control: std::process::ExitStatus) -> bool {
        let ready = String::from_utf8_lossy(&output.stderr)
            .lines()
            .any(|line| line == "shutdown-ready");
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ready && output.status.signal() == Some(6)
        }
        #[cfg(not(unix))]
        {
            ready && output.status == control && !output.status.success()
        }
    }
    #[test]
    fn shutdown_lifecycle_batch_abort_driver() {
        let Ok(case) = std::env::var("PI_ECS_SHUTDOWN_ABORT") else {
            return;
        };
        if case != "not-ready" {
            eprintln!("shutdown-ready");
        }
        match case.as_str() {
            "control" => std::process::abort(),
            "not-ready" => std::process::abort(),
            "wrong-exit" => std::process::exit(91),
            "panic" => panic!("negative control"),
            _ => {}
        }
        struct Panics;
        impl Component for Panics {
            type Access = PlainAccess;
        }
        impl Drop for Panics {
            fn drop(&mut self) {
                panic!("shutdown payload panic");
            }
        }
        let mut world = Box::new(World::new().unwrap());
        // Hook 不持 World 引用；每次业务 panic 都报告实际门禁/关闭状态。
        let access = std::ptr::addr_of!(world.access) as usize;
        let shutdown = std::ptr::addr_of!(world.shutdown) as usize;
        std::panic::set_hook(Box::new(move |_| {
            let access = unsafe { &*(access as *const crate::world::AccessState) };
            let old = access.mode.replace(AccessMode::Stopped);
            let stopped = matches!(old, AccessMode::Stopped);
            access.mode.set(old);
            let closing = unsafe { *(shutdown as *const crate::shutdown::ShutdownState) }
                == crate::shutdown::ShutdownState::Closing;
            eprintln!("shutdown-state:{stopped}:{closing}");
        }));
        match case.as_str() {
            "component" | "component-drop" | "removed" => {
                let id = world
                    .registry
                    .register_component::<Panics>(StoragePolicy::SparseOnly)
                    .unwrap();
                world.finish_registration().unwrap();
                let entity = install(
                    &mut world,
                    ArchetypeId {
                        table: 0,
                        variant: 0,
                    },
                );
                if case == "removed" {
                    world.notifications.layouts = vec![Some(std::alloc::Layout::new::<Panics>())];
                    world.notifications.type_ids = vec![Some(std::any::TypeId::of::<Panics>())];
                    let _reader = world
                        .notifications
                        .subscribe(id, crate::notification::NoticeKind::Removed)
                        .unwrap();
                    let mut value = ManuallyDrop::new(Panics);
                    unsafe {
                        world.notifications.components[id.0 as usize]
                            .removed
                            .as_mut()
                            .unwrap()
                            .get_mut()
                            .values
                            .push_back(OwningPtr::from_raw(
                                std::ptr::NonNull::from(&mut *value).cast(),
                            ))
                    };
                } else {
                    sparse(&mut world, id, entity, Panics);
                }
            }
            "command" => {
                world
                    .commands
                    .register::<Panics>(crate::command::CommandCapacity::Unbounded)
                    .unwrap();
                assert!(world
                    .commands
                    .sender::<Panics>(world.epoch)
                    .unwrap()
                    .send(Panics)
                    .is_ok());
            }
            "double" => {
                world
                    .multi_resources
                    .entries
                    .push(Box::new(vec![Panics, Panics]));
            }
            "resource" | "outer" => {
                world
                    .resources
                    .entries
                    .push(Some(crate::resource::ResourceEntry {
                        value: Box::new(Panics),
                        recovery: crate::resource::StateRecovery::Preserve,
                    }));
            }
            "running" => world.access.mode.set(AccessMode::Running),
            "closing" => world.shutdown = crate::shutdown::ShutdownState::Closing,
            "nodes" => {
                world
                    .entity_nodes
                    .links
                    .borrow_mut()
                    .insert(crate::entity_container::EntityLink {
                        value: Entity::default(),
                        next: None,
                    });
            }
            _ => std::process::exit(92),
        }
        if case == "outer" {
            panic!("outer panic");
        }
        if matches!(case.as_str(), "component-drop" | "running" | "closing") {
            drop(world);
        } else {
            world.close().unwrap();
        }
        std::process::exit(93)
    }
    #[test]
    fn shutdown_lifecycle_batch_abort_boundaries() {
        let control = abort_child("control");
        assert!(exact_abort(&control, control.status));
        for case in ["not-ready", "wrong-exit", "panic"] {
            let output = abort_child(case);
            assert!(
                !exact_abort(&output, control.status),
                "negative {case}: {output:?}"
            );
        }
        for case in [
            "component",
            "component-drop",
            "removed",
            "command",
            "resource",
            "double",
            "outer",
            "running",
            "closing",
            "nodes",
        ] {
            let output = abort_child(case);
            assert!(exact_abort(&output, control.status), "{case}: {output:?}");
            if !matches!(case, "running" | "closing") {
                assert!(
                    String::from_utf8_lossy(&output.stderr)
                        .lines()
                        .any(|line| line == "shutdown-state:true:true"),
                    "{case}: {output:?}"
                );
            }
        }
    }
    #[test]
    fn shutdown_lifecycle_batch_gates() {
        for (expected, mode) in [
            AccessMode::Running,
            AccessMode::Pinning(1),
            AccessMode::Maintaining,
        ]
        .into_iter()
        .enumerate()
        {
            let mut world = World::new().unwrap();
            world.access.mode.set(mode);
            assert!(world.close().is_err());
            assert_eq!(world.shutdown, crate::shutdown::ShutdownState::Open);
            let unchanged = world.access.mode.replace(AccessMode::Stopped);
            assert!(matches!(
                (expected, unchanged),
                (0, AccessMode::Running)
                    | (1, AccessMode::Pinning(1))
                    | (2, AccessMode::Maintaining)
            ));
            world.close().unwrap();
        }
        let mut world = World::new().unwrap();
        world.shutdown = crate::shutdown::ShutdownState::Closing;
        assert!(world.close().is_err());
        assert!(matches!(
            world.access.mode.replace(AccessMode::Stopped),
            AccessMode::Idle
        ));
        assert_eq!(world.shutdown, crate::shutdown::ShutdownState::Closing);
        // 注入的前置状态恢复，仅为使夹具能够正常关闭。
        world.shutdown = crate::shutdown::ShutdownState::Open;
        world.close().unwrap();
    }
    #[test]
    fn shutdown_lifecycle_batch_empty_world() {
        const CHILD: &str = "PI_ECS_SHUTDOWN_EMPTY";
        if std::env::var_os(CHILD).is_some() {
            let mut world = World::new().unwrap();
            let tick = world.change_tick;
            world.close().unwrap();
            world.close().unwrap();
            assert_eq!(world.change_tick, tick);
            assert!(matches!(
                world.access.mode.replace(AccessMode::Stopped),
                AccessMode::Stopped
            ));
            drop(world);
            drop(World::new().unwrap());
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "shutdown::shutdown_inner::shutdown_lifecycle_batch::shutdown_lifecycle_batch_empty_world", "--nocapture"])
            .env(CHILD, "1").output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[cfg(feature = "rc")]
unsafe fn release_whole_state(world: &mut crate::world::World) {
    use crate::{
        destruction::{ReleaseContext, ReleaseMode},
        ptr::OwningPtr,
        table::TableRow,
    };
    let cell = crate::unsafe_world_cell::UnsafeWorldCell::from_mut(world);
    let root = cell.pointer.as_ptr();
    let context = ReleaseContext {
        world: cell,
        mode: ReleaseMode::WholeState,
    };
    // 每个投影只借相应字段；回调可以短借身份和节点，不与存储/流借用重叠。
    let table_count = unsafe { &(*root).registry }.tables.len();
    for table_id in 0..table_count {
        let table_id = u16::try_from(table_id).expect("关闭 Table 编号损坏");
        let rows = match unsafe { &(*root).storage }.as_ref() {
            Some(storage) => storage.published_rows(table_id).expect("关闭 Table 缺失"),
            None => break,
        };
        for index in 0..rows {
            let row = TableRow::new(index);
            let entity = unsafe { &(*root).storage }
                .as_ref()
                .unwrap()
                .row_entity(table_id, row);
            let Some(entity) = entity else { continue };
            let location = unsafe { &(*root).entities }
                .borrow()
                .locate(entity)
                .expect("关闭活行身份缺失");
            assert_eq!(
                location.archetype().table,
                table_id,
                "关闭行身份 Table 错配"
            );
            assert_eq!(location.table_row(), index, "关闭行身份位置错配");
            // 退休前复制完整列责任游标；仅 Live 完整行有逐列责任。
            let columns: Vec<_> = unsafe { &(*root).registry }.tables[table_id as usize]
                .columns()
                .to_vec();
            let mut values = Vec::with_capacity(columns.len());
            {
                let storage = unsafe { &mut (*root).storage }.as_mut().unwrap();
                let mut table = storage.table_mut(table_id).expect("关闭 Table 缺失");
                for (column, component) in columns.into_iter().enumerate() {
                    let pointer = unsafe {
                        table
                            .column_mut(crate::table::ColumnId(column as u32))
                            .expect("关闭列缺失")
                            .remove_raw(index as usize)
                    };
                    values.push((
                        component,
                        std::ptr::NonNull::new(pointer).expect("关闭空值地址"),
                    ));
                }
                table.retire_row(row);
            }
            for (component, pointer) in values {
                let info = unsafe { &(*root).registry }
                    .component(component)
                    .expect("关闭组件元数据缺失");
                unsafe { (info.release)(&context, OwningPtr::from_raw(pointer)) };
            }
        }
    }
    loop {
        let value = unsafe { &mut (*root).storage }
            .as_mut()
            .and_then(|storage| unsafe { storage.take_one_sparse_for_shutdown_raw() });
        let Some((component, pointer)) = value else {
            break;
        };
        let info = unsafe { &(*root).registry }
            .component(component)
            .expect("关闭稀疏元数据缺失");
        unsafe { (info.release)(&context, OwningPtr::from_raw(pointer)) };
    }
    let count = unsafe { &(*root).notifications }.components.len();
    for component in 0..count {
        loop {
            let pointer = {
                let streams = &mut unsafe { &mut (*root).notifications }.components[component];
                streams.removed.as_mut().and_then(|stream| {
                    stream
                        .get_mut()
                        .values
                        .pop_front()
                        .map(|value| value.into_raw())
                })
            };
            let Some(pointer) = pointer else { break };
            let info = unsafe { &(*root).registry }
                .component(crate::component::ComponentId(component as u32))
                .expect("关闭 Removed 元数据缺失");
            unsafe {
                (info.release)(
                    &context,
                    OwningPtr::from_raw(std::ptr::NonNull::new(pointer).unwrap()),
                )
            };
        }
    }
    assert!(
        unsafe { &(*root).entity_nodes }
            .links
            .borrow()
            .slots
            .is_empty(),
        "关闭单链节点余责"
    );
    assert_eq!(
        unsafe { &(*root).entity_nodes }.deque.borrow().len(),
        0,
        "关闭双链节点余责"
    );
    // 先从目录移出唯一 owner，再在借用外普通析构；非组件不应用组件策略。
    let resources = std::mem::take(&mut unsafe { &mut (*root).resources }.entries);
    drop(resources);
    let multi = std::mem::take(&mut unsafe { &mut (*root).multi_resources }.entries);
    drop(multi);
    let defaults = std::mem::take(&mut unsafe { &mut (*root).query_defaults }.values);
    drop(defaults);
    // 所有托管值完成后才允许释放源分配和身份目录。
    unsafe { &mut (*root).storage }.take();
    let notices = unsafe { &mut (*root).notifications };
    notices.components.clear();
    notices.subscriptions.clear();
    notices.layouts.clear();
    notices.type_ids.clear();
    for slots in unsafe { &mut (*root).entities }.get_mut().slots.iter_mut() {
        slots.clear();
    }
    unsafe { &mut (*root).pending_destruction }.pending.clear(0);
    unsafe { &mut (*root).pending_destruction }
        .reclamation
        .clear(0);
    unsafe { &mut (*root).tracking }.queries.clear();
    unsafe { &mut (*root).resources }.ids.clear();
    unsafe { &mut (*root).multi_resources }.ids.clear();
    let registry = unsafe { &mut (*root).registry };
    registry.archetypes.clear();
    registry.archetype_ids.clear();
    registry.tables.clear();
    registry.table_ids.clear();
    registry.components.clear();
    registry.component_ids.clear();
}
#[cfg(feature = "rc")]
impl crate::world::World {
    pub(super) fn drop_shutdown_inner(&mut self) {
        // Drop 即使发生在外层展开中也不得传播清理异常。
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.close_inner()));
        if !matches!(result, Ok(Ok(()))) {
            self.access.mode.set(crate::world::AccessMode::Stopped);
            std::process::abort();
        }
    }
    pub(super) fn close_inner(&mut self) -> Result<(), crate::error::EcsError> {
        use crate::{shutdown::ShutdownState, world::AccessMode};
        if self.shutdown == ShutdownState::Closed {
            return Ok(());
        }
        let mode = self.access.mode.replace(AccessMode::Stopped);
        let allowed = matches!(mode, AccessMode::Idle | AccessMode::Stopped)
            && self.shutdown == ShutdownState::Open;
        self.access.mode.set(mode);
        if !allowed {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "World 当前不可关闭",
            ));
        }
        self.shutdown = ShutdownState::Closing;
        self.access.mode.set(AccessMode::Stopped);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Hub 先关闭所有入口，再在队列借用外销毁 payload。
            self.commands.close();
            unsafe { release_whole_state(self) };
        }));
        if result.is_err() {
            // 不析构 panic payload（其 Drop 也可能 panic），不重试半清理状态。
            self.access.mode.set(AccessMode::Stopped);
            std::process::abort();
        }
        self.shutdown = ShutdownState::Closed;
        Ok(())
    }
}
