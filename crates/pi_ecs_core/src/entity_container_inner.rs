//! 容器专用访问配对芯。
#[cfg(all(test, feature = "rc"))]
mod container_deque_batch {
    use crate::{
        change_detection::*, destruction::PendingDestruction, entity_container::*, entity_store::*,
        owned_entity::OwnedEntity, world::*,
    };
    use std::{
        cell::{Cell, RefCell},
        marker::PhantomData,
    };
    struct Fields {
        nodes: EntityNodePools,
        entities: EntityIdentityStore,
        pending: PendingDestruction,
        access: AccessState,
    }
    impl Fields {
        fn new() -> Self {
            Self {
                nodes: EntityNodePools {
                    links: RefCell::new(NodePool {
                        slots: pi_slotmap::SlotMap::with_key(),
                    }),
                    deque: RefCell::new(Default::default()),
                },
                entities: RefCell::new(EntityStore::new(1).unwrap()),
                pending: PendingDestruction {
                    pending: Default::default(),
                    reclamation: Default::default(),
                },
                access: AccessState {
                    mode: Cell::new(AccessMode::Idle),
                },
            }
        }
        fn allocate(&self) -> OwnedEntity<'_> {
            let entity = self
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
            OwnedEntity {
                world_identity: 17,
                entity,
                entities: &self.entities,
                pending_destruction: &self.pending,
                access: &self.access,
            }
        }
        fn view<'a>(
            &'a self,
            head: &'a mut EntityDeque<()>,
            ticks: &'a mut crate::resource::ResourceTicks,
        ) -> EntityDequeMut<'a, 'a, ()> {
            EntityDequeMut {
                value: head,
                nodes: &self.nodes,
                world_identity: 17,
                entities: &self.entities,
                pending_destruction: &self.pending,
                access: &self.access,
                marker: ChangeMarker {
                    tick: tick(9),
                    target: ChangeTarget::Resource(ticks),
                },
                marked: false,
            }
        }
    }
    fn tick(n: u64) -> ChangeTick {
        ChangeTick::new(std::num::NonZeroU64::new(n).unwrap())
    }
    fn head() -> EntityDeque<()> {
        EntityDeque {
            deque: Default::default(),
            marker: PhantomData,
        }
    }
    fn ticks() -> crate::resource::ResourceTicks {
        crate::resource::ResourceTicks {
            added: tick(1),
            changed: tick(1),
        }
    }
    #[test]
    fn container_deque_batch_abort_child() {
        let Ok(case) = std::env::var("PI_ECS_DEQUE_ABORT") else {
            return;
        };
        std::panic::set_hook(Box::new(|_| std::process::exit(91)));
        if case.starts_with("release/") {
            release_failure(&case);
        }
        if case == "not-ready" {
            std::process::abort();
        }
        if case == "control" {
            eprintln!("deque-ready:{case}");
            std::process::abort();
        }
        if ["panic", "wrong-exit", "other-termination"].contains(&case.as_str()) {
            eprintln!("deque-ready:{case}");
            match case.as_str() {
                "panic" => panic!("not abort"),
                "wrong-exit" => std::process::exit(101),
                _ => std::process::exit(0xc0000005_u32 as i32),
            }
        }
        if case.starts_with("release/") {
            use crate::{
                component::*, component_policy::ReleaseMode, destruction::*, ptr::OwningPtr,
                unsafe_world_cell::UnsafeWorldCell,
            };
            let parts: Vec<_> = case.split('/').collect();
            let (kind, fault, mode) = (parts[1], parts[2], parts[3]);
            let mode = if mode == "normal" {
                ReleaseMode::Normal
            } else {
                ReleaseMode::WholeState
            };
            // 子进程在已验证的释放入口终止；World::new 初始化全部真实字段。
            // ManuallyDrop 避免测试失败路径意外触及未实现 World::drop。
            let mut world = std::mem::ManuallyDrop::new(World::new().unwrap());
            macro_rules! release_case {
                ($container:ident, $policy:ident, $push:ident, $head:expr, $pool:ident, $corrupt:expr) => {{
                    let id = world
                        .registry
                        .register_component::<$container<String>>(StoragePolicy::SparseOnly)
                        .unwrap();
                    let entity = world
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
                    let owned = OwnedEntity {
                        world_identity: world.identity,
                        entity,
                        entities: &world.entities,
                        pending_destruction: &world.pending_destruction,
                        access: &world.access,
                    };
                    let mut head = std::mem::ManuallyDrop::new($head);
                    let mut ticks = ticks();
                    let cell = UnsafeWorldCell::from_ref(&world);
                    let mut view = unsafe {
                        <$policy as ComponentPolicy<$container<String>>>::write(
                            &mut head,
                            cell,
                            ChangeMarker {
                                tick: tick(9),
                                target: ChangeTarget::Resource(&mut ticks),
                            },
                        )
                    };
                    view.$push(owned).ok().unwrap();
                    drop(view);
                    if fault == "corrupt" {
                        ($corrupt)(&mut *head);
                    }
                    let _old_pool = (fault == "corrupt" && kind == "deque")
                        .then(|| std::mem::take(&mut *world.entity_nodes.deque.borrow_mut()));
                    let _pool = (fault == "pool-borrow").then(|| world.entity_nodes.$pool.borrow());
                    let _entities = (fault == "entity-borrow").then(|| world.entities.borrow());
                    OBSERVED.with(|slot| slot.set(&world.access));
                    let address = std::ptr::NonNull::from(&mut *head).cast();
                    let context = ReleaseContext { world: cell, mode };
                    eprintln!("deque-ready:{case}");
                    unsafe {
                        release_value(
                            &context,
                            world.registry.component(id).unwrap(),
                            OwningPtr::from_raw(address),
                        );
                    }
                }};
            }
            match kind {
                "list" => release_case!(
                    EntityList,
                    ListAccess,
                    push_front,
                    EntityList::<String> {
                        head: None,
                        tail: None,
                        len: 0,
                        marker: PhantomData
                    },
                    links,
                    |head: &mut EntityList<String>| head.len += 1
                ),
                "queue" => release_case!(
                    EntityQueue,
                    QueueAccess,
                    push_back,
                    EntityQueue::<String> {
                        head: None,
                        tail: None,
                        len: 0,
                        marker: PhantomData
                    },
                    links,
                    |head: &mut EntityQueue<String>| head.len += 1
                ),
                "deque" => release_case!(
                    EntityDeque,
                    DequeAccess,
                    push_back,
                    EntityDeque::<String> {
                        deque: Default::default(),
                        marker: PhantomData
                    },
                    deque,
                    |_head: &mut EntityDeque<String>| {}
                ),
                _ => std::process::exit(94),
            }
            std::process::exit(92);
        }
        if case.starts_with("release/") {
            release_failure(&case);
        }
        let f = Fields::new();
        let mut h = head();
        let mut t = ticks();
        {
            let mut v = f.view(&mut h, &mut t);
            v.push_back(f.allocate()).ok().unwrap();
        }
        OBSERVED.with(|slot| slot.set(&f.access));
        let parts: Vec<_> = case.split('/').collect();
        let (kind, method, fault) = (parts[0], parts[1], parts[2]);
        let _original_pool = (fault == "corrupt" && method != "owned-next" && method != "next")
            .then(|| std::mem::take(&mut *f.nodes.deque.borrow_mut()));
        macro_rules! read {
            ($view:expr) => {{
                let view = $view;
                if method == "owned-next" || method == "next" {
                    let mut iter = view.iter_owned().unwrap();
                    if fault == "corrupt" {
                        iter.chain.remaining = 2;
                    }
                    let _held = (fault == "borrow").then(|| f.nodes.deque.borrow_mut());
                    eprintln!("deque-ready:{case}");
                    if method == "owned-next" {
                        let _value = std::mem::ManuallyDrop::new(iter.next());
                    } else {
                        let _value = iter.chain.next();
                    }
                    std::process::exit(92);
                }
                let _held = (fault == "borrow").then(|| f.nodes.deque.borrow_mut());
                eprintln!("deque-ready:{case}");
                match method {
                    "front" => {
                        let _value = view.front();
                    }
                    "iter" => {
                        let _value = view.iter();
                    }
                    "front-owned" => {
                        let _value = std::mem::ManuallyDrop::new(view.front_owned());
                    }
                    "back-owned" => {
                        let _value = std::mem::ManuallyDrop::new(view.back_owned());
                    }
                    "get-owned" => {
                        let _value =
                            std::mem::ManuallyDrop::new(view.get_owned(view.value.deque.head()));
                    }
                    "iter-owned" => {
                        let _value = view.iter_owned();
                    }
                    _ => std::process::exit(94),
                }
            }};
        }
        if kind == "ref" {
            read!(EntityDequeRef {
                value: &h,
                nodes: &f.nodes,
                world_identity: 17,
                entities: &f.entities,
                pending_destruction: &f.pending,
                access: &f.access
            });
        } else {
            let mut v = f.view(&mut h, &mut t);
            if [
                "push-front",
                "push-back",
                "insert-before",
                "insert-after",
                "pop-front",
                "pop-back",
                "remove",
                "clear",
            ]
            .contains(&method)
            {
                let input = method.starts_with("push") || method.starts_with("insert");
                let input = input.then(|| f.allocate());
                let key = v.value.deque.head();
                let _held = (fault == "borrow").then(|| f.nodes.deque.borrow());
                eprintln!("deque-ready:{case}");
                match method {
                    "push-front" => {
                        let _value = std::mem::ManuallyDrop::new(v.push_front(input.unwrap()));
                    }
                    "push-back" => {
                        let _value = std::mem::ManuallyDrop::new(v.push_back(input.unwrap()));
                    }
                    "insert-before" => {
                        let _value =
                            std::mem::ManuallyDrop::new(v.insert_before(key, input.unwrap()));
                    }
                    "insert-after" => {
                        let _value =
                            std::mem::ManuallyDrop::new(v.insert_after(key, input.unwrap()));
                    }
                    "pop-front" => {
                        let _value = std::mem::ManuallyDrop::new(v.pop_front());
                    }
                    "pop-back" => {
                        let _value = std::mem::ManuallyDrop::new(v.pop_back());
                    }
                    "remove" => {
                        let _value = std::mem::ManuallyDrop::new(v.remove(key));
                    }
                    _ => {
                        let _value = v.clear();
                    }
                }
            } else {
                read!(v);
            }
        }
        std::process::exit(92);
    }
    std::thread_local! { static OBSERVED: Cell<*const AccessState> = const { Cell::new(std::ptr::null()) }; }
    pub(super) fn observe_fatal() {
        OBSERVED.with(|slot| {
            let access = slot.get();
            if !access.is_null() {
                // Set only by the subprocess fixture; that stack remains live until abort.
                eprintln!("deque-stopped:{}", super::stopped(unsafe { &*access }));
            }
        });
    }
    fn release_failure(case: &str) -> ! {
        use crate::{
            component::*, component_policy::ReleaseMode, destruction::*, ptr::OwningPtr,
            unsafe_world_cell::UnsafeWorldCell,
        };
        let parts: Vec<_> = case.split('/').collect();
        let (kind, fault) = (parts[1], parts[2]);
        let mode = if parts[3] == "normal" {
            ReleaseMode::Normal
        } else {
            ReleaseMode::WholeState
        };
        // 全字段合法初始化；进程必定 abort/exit，无未实现 World Drop 的干扰。
        let mut world = std::mem::ManuallyDrop::new(World::new().unwrap());
        macro_rules! run {
            ($container:ident, $policy:ident, $push:ident, $head:expr, $pool:ident, $empty_pool:expr) => {{
                let id = world
                    .registry
                    .register_component::<$container<String>>(StoragePolicy::SparseOnly)
                    .unwrap();
                let entity = world
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
                let owned = OwnedEntity {
                    world_identity: world.identity,
                    entity,
                    entities: &world.entities,
                    pending_destruction: &world.pending_destruction,
                    access: &world.access,
                };
                let mut head = std::mem::ManuallyDrop::new($head);
                let mut ticks = ticks();
                let cell = UnsafeWorldCell::from_ref(&world);
                let mut view = unsafe {
                    <$policy as ComponentPolicy<$container<String>>>::write(
                        &mut head,
                        cell,
                        ChangeMarker {
                            tick: tick(9),
                            target: ChangeTarget::Resource(&mut ticks),
                        },
                    )
                };
                view.$push(owned).ok().unwrap();
                drop(view);
                // 坏链：头保留原键，原池保活至终止。
                let _original = (fault == "corrupt").then(|| {
                    std::mem::replace(&mut *world.entity_nodes.$pool.borrow_mut(), $empty_pool)
                });
                let _held = (fault == "pool-borrow").then(|| world.entity_nodes.$pool.borrow());
                let _entities = (fault == "entity-borrow").then(|| world.entities.borrow());
                OBSERVED.with(|slot| slot.set(&world.access));
                let address = std::ptr::NonNull::from(&mut *head).cast();
                let context = ReleaseContext { world: cell, mode };
                eprintln!("deque-ready:{case}");
                unsafe {
                    release_value(
                        &context,
                        world.registry.component(id).unwrap(),
                        OwningPtr::from_raw(address),
                    );
                }
            }};
        }
        match kind {
            "list" => run!(
                EntityList,
                ListAccess,
                push_front,
                EntityList::<String> {
                    head: None,
                    tail: None,
                    len: 0,
                    marker: PhantomData
                },
                links,
                NodePool {
                    slots: pi_slotmap::SlotMap::with_key()
                }
            ),
            "queue" => run!(
                EntityQueue,
                QueueAccess,
                push_back,
                EntityQueue::<String> {
                    head: None,
                    tail: None,
                    len: 0,
                    marker: PhantomData
                },
                links,
                NodePool {
                    slots: pi_slotmap::SlotMap::with_key()
                }
            ),
            "deque" => run!(
                EntityDeque,
                DequeAccess,
                push_back,
                EntityDeque::<String> {
                    deque: Default::default(),
                    marker: PhantomData
                },
                deque,
                Default::default()
            ),
            _ => std::process::exit(94),
        }
        std::process::exit(92)
    }
    #[test]
    fn container_release_batch_failures_stop_before_exact_abort() {
        let control = child("control");
        assert!(target_abort(&control, control.status, "control"));
        for case in ["not-ready", "panic", "wrong-exit", "other-termination"] {
            let output = child(case);
            assert!(
                !target_abort(&output, control.status, case),
                "{case}: {output:?}"
            );
        }
        for kind in ["list", "queue", "deque"] {
            for fault in ["corrupt", "pool-borrow", "entity-borrow"] {
                for mode in ["normal", "whole"] {
                    let case = format!("release/{kind}/{fault}/{mode}");
                    let output = child(&case);
                    assert!(
                        target_abort(&output, control.status, &case),
                        "{case}: {output:?}"
                    );
                    assert!(
                        String::from_utf8_lossy(&output.stderr)
                            .lines()
                            .any(|line| line == "deque-stopped:true"),
                        "{case}: {output:?}"
                    );
                }
            }
        }
    }
    fn child(case: &str) -> std::process::Output {
        std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact","entity_container::entity_container_inner::container_deque_batch::container_deque_batch_abort_child","--nocapture"]).env("PI_ECS_DEQUE_ABORT",case).output().unwrap()
    }
    fn target_abort(
        output: &std::process::Output,
        control: std::process::ExitStatus,
        case: &str,
    ) -> bool {
        if !String::from_utf8_lossy(&output.stderr)
            .lines()
            .any(|line| line == format!("deque-ready:{case}"))
        {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            let _ = control;
            output.status.signal() == Some(6)
        }
        #[cfg(windows)]
        {
            !output.status.success() && output.status == control
        }
    }
    #[test]
    fn container_deque_batch_abort_classifier_and_access_matrix() {
        let control = child("control");
        assert!(target_abort(&control, control.status, "control"));
        for case in ["not-ready", "panic", "wrong-exit", "other-termination"] {
            let output = child(case);
            assert!(
                !target_abort(&output, control.status, case),
                "{case}: {output:?}"
            );
        }
        for kind in ["ref", "mut"] {
            for method in [
                "front",
                "iter",
                "front-owned",
                "back-owned",
                "get-owned",
                "iter-owned",
                "owned-next",
                "next",
                "push-front",
                "push-back",
                "insert-before",
                "insert-after",
                "pop-front",
                "pop-back",
                "remove",
                "clear",
            ] {
                if kind == "ref"
                    && [
                        "push-front",
                        "push-back",
                        "insert-before",
                        "insert-after",
                        "pop-front",
                        "pop-back",
                        "remove",
                        "clear",
                    ]
                    .contains(&method)
                {
                    continue;
                }
                for fault in ["borrow", "corrupt"] {
                    let case = format!("{kind}/{method}/{fault}");
                    let output = child(&case);
                    assert!(
                        target_abort(&output, control.status, &case),
                        "{case}: {output:?}"
                    );
                    if method != "next" {
                        assert!(
                            String::from_utf8_lossy(&output.stderr)
                                .lines()
                                .any(|line| line == "deque-stopped:true"),
                            "{case}: {output:?}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn container_deque_batch_notice_dedup_and_panic_remainder() {
        use crate::notification::*;
        let f = Fields::new();
        let first = f.allocate();
        let first_id = first.entity();
        let last = f.allocate();
        let last_id = last.entity();
        let mut h = head();
        let mut t = ticks();
        {
            let mut v = f.view(&mut h, &mut t);
            v.push_back(first).ok().unwrap();
            v.push_back(last).ok().unwrap();
        }
        let component = crate::component::ComponentId(0);
        let mut subscriptions = pi_slotmap::SlotMap::with_key();
        subscriptions.insert(SubscriptionState {
            stream: (component, NoticeKind::Changed),
            next: Cell::new(0),
        });
        let notices = Notifications {
            subscriptions,
            components: vec![ComponentNoticeStreams {
                added: None,
                removed: None,
                changed: Some(RefCell::new(NoticeStream {
                    headers: Default::default(),
                    range: NoticeStreamRange {
                        first: NoticeSequence(0),
                        end: NoticeSequence(0),
                    },
                })),
            }],
            layouts: vec![],
            type_ids: vec![],
        };
        let demand = TrackingDemand {
            changed_notice: true,
            ..Default::default()
        };
        let mut v = EntityDequeMut {
            value: &mut h,
            nodes: &f.nodes,
            world_identity: 17,
            entities: &f.entities,
            pending_destruction: &f.pending,
            access: &f.access,
            marker: ChangeMarker {
                tick: tick(9),
                target: ChangeTarget::Component {
                    ticks: pi_ecs_types::change_detection::ComponentTicksMut {
                        added: None,
                        changed: None,
                    },
                    demand: &demand,
                    notice: Some(NoticeWrite {
                        notifications: &notices,
                        access: &f.access,
                        component,
                        entity: first_id,
                        borrow: PhantomData,
                    }),
                },
            },
            marked: false,
        };
        let held = notices.components[0].changed.as_ref().unwrap().borrow();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            v.clear().unwrap();
        }))
        .is_err());
        drop(held);
        assert!(super::stopped(&f.access));
        assert_eq!(f.entities.borrow().counts(first_id), Some((0, 0)));
        assert_eq!(f.entities.borrow().counts(last_id), Some((1, 0)));
        assert_eq!(v.len(), 1);
        assert_eq!(v.front().unwrap(), Some(last_id));
        v.clear().unwrap();
        assert_eq!(f.entities.borrow().counts(last_id), Some((0, 0)));
        drop(v);
        f.access.mode.set(AccessMode::Idle);
        let mut v = EntityDequeMut {
            value: &mut h,
            nodes: &f.nodes,
            world_identity: 17,
            entities: &f.entities,
            pending_destruction: &f.pending,
            access: &f.access,
            marker: ChangeMarker {
                tick: tick(9),
                target: ChangeTarget::Component {
                    ticks: pi_ecs_types::change_detection::ComponentTicksMut {
                        added: None,
                        changed: None,
                    },
                    demand: &demand,
                    notice: Some(NoticeWrite {
                        notifications: &notices,
                        access: &f.access,
                        component,
                        entity: first_id,
                        borrow: PhantomData,
                    }),
                },
            },
            marked: false,
        };
        v.clear().unwrap();
        assert_eq!(
            notices.components[0]
                .changed
                .as_ref()
                .unwrap()
                .borrow()
                .range
                .end
                .0,
            1
        );
        v.push_back(f.allocate()).ok().unwrap();
        v.push_front(f.allocate()).ok().unwrap();
        v.clear().unwrap();
        let stream = notices.components[0].changed.as_ref().unwrap().borrow();
        assert_eq!(stream.headers.len(), 2);
        assert_eq!(stream.range.end.0, 2);
        assert_eq!(stream.headers[1].entity, first_id);
        assert_eq!(stream.headers[1].tick, tick(9));
    }
    #[test]
    fn container_deque_batch_keys_insertions_and_rejections() {
        let f = Fields::new();
        let other = Fields::new();
        let mut a = head();
        let mut b = head();
        let mut empty = head();
        let mut ta = ticks();
        let mut tb = ticks();
        let mut te = ticks();
        let first = f.allocate();
        let id = first.entity();
        let duplicate = first.try_clone().unwrap();
        let mut va = f.view(&mut a, &mut ta);
        let mut vb = f.view(&mut b, &mut tb);
        let mut ve = f.view(&mut empty, &mut te);
        let key = va.push_front(first).ok().unwrap();
        let foreign = vb.push_back(f.allocate()).ok().unwrap();
        let before = va.insert_before(key, duplicate).ok().unwrap();
        let last = f.allocate();
        let last_id = last.entity();
        let after = va.insert_after(key, last).ok().unwrap();
        assert_eq!(
            va.iter().unwrap().collect::<Vec<_>>(),
            vec![id, id, last_id]
        );
        assert!(va.get_owned(foreign).is_err());
        assert!(vb.get_owned(key).is_err());
        assert!(ve.get_owned(key).is_err());
        assert!(ve.remove(key).is_err());
        assert!(!ve.marked);
        let rejected = f.allocate();
        let rejected_id = rejected.entity();
        let error = ve.insert_before(key, rejected).err().unwrap();
        assert_eq!(error.value.entity(), rejected_id);
        assert!(!ve.marked);
        assert_eq!(f.entities.borrow().counts(rejected_id), Some((1, 0)));
        drop(error);
        assert!(va.remove(foreign).is_err());
        assert!(vb.remove(key).is_err());
        let input = other.allocate();
        let input_id = input.entity();
        let error = ve.push_front(input).err().unwrap();
        assert_eq!(error.value.entity(), input_id);
        assert!(!ve.marked);
        assert_eq!(other.entities.borrow().counts(input_id), Some((1, 0)));
        drop(error);
        let value = va.remove(before).unwrap();
        assert_eq!(value.entity(), id);
        assert_eq!(f.entities.borrow().counts(id), Some((2, 0)));
        drop(value);
        assert!(va.remove(before).is_err());
        let replacement = va.insert_after(key, f.allocate()).ok().unwrap();
        assert_ne!(replacement, before);
        assert!(va.get_owned(before).is_err());
        assert_eq!(va.remove(after).unwrap().entity(), last_id);
        assert_eq!(vb.len(), 1);
        va.clear().unwrap();
        vb.clear().unwrap();
        ve.clear().unwrap();
        assert_eq!(f.entities.borrow().counts(id), Some((0, 0)));
        assert_eq!(f.nodes.deque.borrow().len(), 0);
    }
    #[test]
    fn container_deque_batch_owned_retry_and_stopped_cleanup() {
        let f = Fields::new();
        let mut h = head();
        let mut t = ticks();
        let input = f.allocate();
        let id = input.entity();
        let duplicate = input.try_clone().unwrap();
        let mut v = f.view(&mut h, &mut t);
        let key = v.push_front(input).ok().unwrap();
        v.push_back(duplicate).ok().unwrap();
        let r = EntityDequeRef {
            value: &*v.value,
            nodes: &f.nodes,
            world_identity: 17,
            entities: &f.entities,
            pending_destruction: &f.pending,
            access: &f.access,
        };
        assert_eq!(r.front().unwrap(), Some(id));
        assert_eq!(r.iter().unwrap().collect::<Vec<_>>(), vec![id, id]);
        assert_eq!(r.front_owned().unwrap().unwrap().entity(), id);
        assert_eq!(r.back_owned().unwrap().unwrap().entity(), id);
        assert_eq!(r.get_owned(key).unwrap().entity(), id);
        let mut owned = r.iter_owned().unwrap();
        assert_eq!(f.entities.borrow().counts(id), Some((2, 0)));
        f.entities.borrow_mut().slots[0].get_mut(id).unwrap().strong = u32::MAX;
        assert_eq!(
            *owned.next().unwrap().err().unwrap().current_context(),
            crate::error::ErrorKind::ResourceExhausted
        );
        f.entities.borrow_mut().slots[0].get_mut(id).unwrap().strong = 2;
        let copy = owned.next().unwrap().unwrap();
        assert_eq!(copy.entity(), id);
        assert_eq!(f.entities.borrow().counts(id), Some((3, 0)));
        drop(copy);
        f.access.mode.set(AccessMode::Stopped);
        assert!(owned.next().unwrap().is_err());
        drop(owned);
        assert!(v.front_owned().is_err());
        assert!(v.back_owned().is_err());
        assert!(v.get_owned(key).is_err());
        let rejected = f.allocate();
        let rejected_id = rejected.entity();
        let error = v.push_back(rejected).err().unwrap();
        assert_eq!(error.value.entity(), rejected_id);
        drop(error);
        let transferred = v.remove(key).unwrap();
        assert_eq!(transferred.entity(), id);
        assert_eq!(f.entities.borrow().counts(id), Some((2, 0)));
        drop(transferred);
        v.clear().unwrap();
        assert_eq!(f.entities.borrow().counts(id), Some((0, 0)));
        assert_eq!(f.nodes.deque.borrow().len(), 0);
    }
    #[test]
    fn container_deque_batch_interleaved_ends_and_transfers() {
        let f = Fields::new();
        let mut a = head();
        let mut b = head();
        let mut ta = ticks();
        let mut tb = ticks();
        let first = f.allocate();
        let id = first.entity();
        let duplicate = first.try_clone().unwrap();
        let last = f.allocate();
        let last_id = last.entity();
        let mut va = f.view(&mut a, &mut ta);
        let mut vb = f.view(&mut b, &mut tb);
        let key = va.push_back(first).ok().unwrap();
        vb.push_front(last).ok().unwrap();
        va.push_front(duplicate).ok().unwrap();
        assert_eq!(va.front().unwrap(), Some(id));
        assert_eq!(va.len(), 2);
        let mut iter = va.iter().unwrap();
        vb.push_back(f.allocate()).ok().unwrap();
        assert_eq!(iter.next(), Some(id));
        assert_eq!(iter.next(), Some(id));
        assert_eq!(iter.next(), None);
        drop(iter);
        assert_eq!(va.get_owned(key).unwrap().entity(), id);
        assert_eq!(va.back_owned().unwrap().unwrap().entity(), id);
        assert_eq!(f.entities.borrow().counts(id), Some((2, 0)));
        let popped = va.pop_back().unwrap().unwrap();
        assert_eq!(popped.entity(), id);
        assert_eq!(f.entities.borrow().counts(id), Some((2, 0)));
        drop(popped);
        assert!(va.get_owned(key).is_err());
        drop(va.pop_front().unwrap());
        assert!(va.is_empty());
        assert_eq!(vb.pop_front().unwrap().unwrap().entity(), last_id);
        va.clear().unwrap();
        vb.clear().unwrap();
        assert_eq!(f.entities.borrow().counts(id), Some((0, 0)));
        assert_eq!(f.entities.borrow().counts(last_id), Some((0, 0)));
        assert_eq!(f.nodes.deque.borrow().len(), 0);
        drop(va);
        drop(vb);
        assert_eq!(ta.added, tick(1));
        assert_eq!(ta.changed, tick(9));
    }
    #[test]
    fn container_deque_batch_empty_views_do_not_mark() {
        let f = Fields::new();
        let mut h = head();
        let mut t = ticks();
        let mut v = f.view(&mut h, &mut t);
        assert_eq!(v.len(), 0);
        assert!(v.is_empty());
        assert_eq!(v.front().unwrap(), None);
        assert!(v.iter().unwrap().next().is_none());
        assert!(v.front_owned().unwrap().is_none());
        assert!(v.back_owned().unwrap().is_none());
        assert!(v.iter_owned().unwrap().next().is_none());
        assert!(v.pop_front().unwrap().is_none());
        assert!(v.pop_back().unwrap().is_none());
        v.clear().unwrap();
        assert!(!v.marked);
        let r = EntityDequeRef {
            value: &*v.value,
            nodes: &f.nodes,
            world_identity: 17,
            entities: &f.entities,
            pending_destruction: &f.pending,
            access: &f.access,
        };
        assert_eq!(r.len(), 0);
        assert!(r.is_empty());
        assert_eq!(r.front().unwrap(), None);
        assert!(r.iter().unwrap().next().is_none());
        assert!(r.front_owned().unwrap().is_none());
        assert!(r.back_owned().unwrap().is_none());
        assert!(r.iter_owned().unwrap().next().is_none());
    }
}

#[cfg(all(test, feature = "rc"))]
mod container_link_batch {
    use crate::{
        change_detection::*, destruction::PendingDestruction, entity_container::*, entity_store::*,
        owned_entity::OwnedEntity, world::*,
    };
    use std::{
        cell::{Cell, RefCell},
        marker::PhantomData,
    };
    struct Fields {
        nodes: EntityNodePools,
        entities: EntityIdentityStore,
        pending: PendingDestruction,
        access: AccessState,
    }
    impl Fields {
        fn new() -> Self {
            Self {
                nodes: EntityNodePools {
                    links: RefCell::new(NodePool {
                        slots: pi_slotmap::SlotMap::with_key(),
                    }),
                    deque: RefCell::new(Default::default()),
                },
                entities: RefCell::new(EntityStore::new(1).unwrap()),
                pending: PendingDestruction {
                    pending: Default::default(),
                    reclamation: Default::default(),
                },
                access: AccessState {
                    mode: Cell::new(AccessMode::Idle),
                },
            }
        }
        fn allocate(&self) -> OwnedEntity<'_> {
            let entity = self
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
            OwnedEntity {
                world_identity: 17,
                entity,
                entities: &self.entities,
                pending_destruction: &self.pending,
                access: &self.access,
            }
        }
        fn list<'a>(
            &'a self,
            head: &'a mut EntityList<()>,
            ticks: &'a mut crate::resource::ResourceTicks,
        ) -> EntityListMut<'a, 'a, ()> {
            EntityListMut {
                value: head,
                nodes: &self.nodes,
                world_identity: 17,
                entities: &self.entities,
                pending_destruction: &self.pending,
                access: &self.access,
                marker: ChangeMarker {
                    tick: tick(9),
                    target: ChangeTarget::Resource(ticks),
                },
                marked: false,
            }
        }
    }
    #[test]
    fn container_link_batch_cross_world_and_read_views() {
        let f = Fields::new();
        let other = Fields::new();
        let mut head = EntityList {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        };
        let mut ticks = crate::resource::ResourceTicks {
            added: tick(1),
            changed: tick(1),
        };
        let mut v = f.list(&mut head, &mut ticks);
        let mut foreign = other.allocate();
        foreign.world_identity = 18;
        let foreign_id = foreign.entity();
        let error = v.push_front(foreign).err().unwrap();
        assert_eq!(error.value.entity(), foreign_id);
        assert_eq!(error.value.world_identity, 18);
        assert!(!v.marked);
        assert!(v.is_empty());
        assert_eq!(other.entities.borrow().counts(foreign_id), Some((1, 0)));
        drop(error);
        let value = f.allocate();
        let id = value.entity();
        v.push_front(value).ok().unwrap();
        let read = EntityListRef {
            value: &*v.value,
            nodes: &f.nodes,
            world_identity: 17,
            entities: &f.entities,
            pending_destruction: &f.pending,
            access: &f.access,
        };
        assert_eq!(read.len(), 1);
        assert!(!read.is_empty());
        assert_eq!(read.front().unwrap(), Some(id));
        assert_eq!(read.iter().unwrap().collect::<Vec<_>>(), vec![id]);
        assert_eq!(read.front_owned().unwrap().unwrap().entity(), id);
        assert_eq!(
            read.iter_owned().unwrap().next().unwrap().unwrap().entity(),
            id
        );
        let copy = v.iter_owned().unwrap().next().unwrap().unwrap();
        assert_eq!(f.entities.borrow().counts(id), Some((2, 0)));
        drop(copy);
        v.clear().unwrap();
        let q = EntityQueue::<()> {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        };
        let read = EntityQueueRef {
            value: &q,
            nodes: &f.nodes,
            world_identity: 17,
            entities: &f.entities,
            pending_destruction: &f.pending,
            access: &f.access,
        };
        assert_eq!(read.len(), 0);
        assert!(read.is_empty());
        assert_eq!(read.front().unwrap(), None);
        assert!(read.iter().unwrap().next().is_none());
        assert!(read.front_owned().unwrap().is_none());
        assert!(read.iter_owned().unwrap().next().is_none());
    }
    #[test]
    fn container_link_batch_queue_owned_retry_and_stopped_clear() {
        let f = Fields::new();
        let mut head = EntityQueue::<()> {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        };
        let mut ticks = crate::resource::ResourceTicks {
            added: tick(1),
            changed: tick(1),
        };
        let mut v = EntityQueueMut {
            value: &mut head,
            nodes: &f.nodes,
            world_identity: 17,
            entities: &f.entities,
            pending_destruction: &f.pending,
            access: &f.access,
            marker: ChangeMarker {
                tick: tick(9),
                target: ChangeTarget::Resource(&mut ticks),
            },
            marked: false,
        };
        assert!(v.pop_front().unwrap().is_none());
        v.clear().unwrap();
        assert!(!v.marked);
        let first = f.allocate();
        let id = first.entity();
        let duplicate = first.try_clone().unwrap();
        let last = f.allocate();
        let last_id = last.entity();
        v.push_back(first).ok().unwrap();
        v.push_back(last).ok().unwrap();
        v.push_back(duplicate).ok().unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v.front().unwrap(), Some(id));
        assert_eq!(v.iter().unwrap().collect::<Vec<_>>(), vec![id, last_id, id]);
        let copy = v.front_owned().unwrap().unwrap();
        assert_eq!(f.entities.borrow().counts(id), Some((3, 0)));
        drop(copy);
        let mut owned = v.iter_owned().unwrap();
        assert_eq!(f.entities.borrow().counts(id), Some((2, 0)));
        f.entities.borrow_mut().slots[0].get_mut(id).unwrap().strong = u32::MAX;
        assert_eq!(
            *owned.next().unwrap().err().unwrap().current_context(),
            crate::error::ErrorKind::ResourceExhausted
        );
        f.entities.borrow_mut().slots[0].get_mut(id).unwrap().strong = 2;
        assert_eq!(owned.next().unwrap().unwrap().entity(), id);
        f.access.mode.set(AccessMode::Stopped);
        assert!(owned.next().unwrap().is_err());
        drop(owned);
        let rejected = f.allocate();
        let rejected_id = rejected.entity();
        let error = v.push_back(rejected).err().unwrap();
        assert_eq!(error.value.entity(), rejected_id);
        drop(error);
        let transferred = v.pop_front().unwrap().unwrap();
        assert_eq!(transferred.entity(), id);
        drop(transferred);
        v.clear().unwrap();
        assert!(v.is_empty());
        assert_eq!(f.entities.borrow().counts(id), Some((0, 0)));
        assert_eq!(f.entities.borrow().counts(last_id), Some((0, 0)));
    }
    #[test]
    fn container_link_batch_mark_panic_cleans_transferred_value_and_keeps_remainder() {
        use crate::notification::*;
        let f = Fields::new();
        let first = f.allocate();
        let first_id = first.entity();
        let last = f.allocate();
        let last_id = last.entity();
        let mut head = EntityList::<()> {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        };
        let mut ticks = crate::resource::ResourceTicks {
            added: tick(1),
            changed: tick(1),
        };
        {
            let mut v = f.list(&mut head, &mut ticks);
            v.push_front(last).ok().unwrap();
            v.push_front(first).ok().unwrap();
        }
        let component = crate::component::ComponentId(0);
        let mut subscriptions = pi_slotmap::SlotMap::with_key();
        subscriptions.insert(SubscriptionState {
            stream: (component, NoticeKind::Changed),
            next: Cell::new(0),
        });
        let notices = Notifications {
            subscriptions,
            components: vec![ComponentNoticeStreams {
                added: None,
                changed: Some(RefCell::new(NoticeStream {
                    headers: Default::default(),
                    range: NoticeStreamRange {
                        first: NoticeSequence(0),
                        end: NoticeSequence(0),
                    },
                })),
                removed: None,
            }],
            layouts: vec![],
            type_ids: vec![],
        };
        let demand = TrackingDemand {
            changed_notice: true,
            ..Default::default()
        };
        let mut v = EntityListMut {
            value: &mut head,
            nodes: &f.nodes,
            world_identity: 17,
            entities: &f.entities,
            pending_destruction: &f.pending,
            access: &f.access,
            marker: ChangeMarker {
                tick: tick(9),
                target: ChangeTarget::Component {
                    ticks: pi_ecs_types::change_detection::ComponentTicksMut {
                        added: None,
                        changed: None,
                    },
                    demand: &demand,
                    notice: Some(NoticeWrite {
                        notifications: &notices,
                        access: &f.access,
                        component,
                        entity: first_id,
                        borrow: PhantomData,
                    }),
                },
            },
            marked: false,
        };
        let held = notices.components[0].changed.as_ref().unwrap().borrow();
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            v.clear().unwrap();
        }))
        .is_err());
        drop(held);
        assert!(super::stopped(&f.access));
        assert_eq!(f.entities.borrow().counts(first_id), Some((0, 0)));
        assert_eq!(f.entities.borrow().counts(last_id), Some((1, 0)));
        assert_eq!(v.len(), 1);
        assert_eq!(v.front().unwrap(), Some(last_id));
        v.clear().unwrap();
        assert_eq!(f.entities.borrow().counts(last_id), Some((0, 0)));
    }
    #[test]
    fn container_link_batch_component_ticks_preserve_added() {
        let demand = TrackingDemand {
            added_tick: true,
            changed_tick: true,
            ..Default::default()
        };
        let mut added = Some(tick(2));
        let mut changed = Some(tick(3));
        let mut marker = ChangeMarker {
            tick: tick(9),
            target: ChangeTarget::Component {
                ticks: pi_ecs_types::change_detection::ComponentTicksMut {
                    added: Some(&mut added),
                    changed: Some(&mut changed),
                },
                demand: &demand,
                notice: None,
            },
        };
        let mut marked = false;
        marker.set_changed(&mut marked);
        assert!(marked);
        marker.set_changed(&mut marked);
        drop(marker);
        assert_eq!(added, Some(tick(2)));
        assert_eq!(changed, Some(tick(9)));
    }
    std::thread_local! {
        static OBSERVED_ACCESS: Cell<Option<&'static AccessState>> = const { Cell::new(None) };
    }
    pub(super) fn observe_fatal() {
        OBSERVED_ACCESS.with(|slot| {
            if let Some(access) = slot.get() {
                eprintln!("container-stopped:{}", super::stopped(access));
            }
        });
    }
    fn run_access_case(case: &str) -> ! {
        let parts: Vec<_> = case.split('/').collect();
        let (kind, method, fault) = (parts[0], parts[1], parts[2]);
        let f = Box::leak(Box::new(Fields::new()));
        let mut list = EntityList::<()> {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        };
        let mut queue = EntityQueue::<()> {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        };
        let mut ticks = crate::resource::ResourceTicks {
            added: tick(1),
            changed: tick(1),
        };
        let mut initial = f.list(&mut list, &mut ticks);
        initial.push_front(f.allocate()).ok().unwrap();
        drop(initial);
        queue.head = list.head;
        queue.tail = list.tail;
        queue.len = list.len;
        OBSERVED_ACCESS.with(|slot| slot.set(Some(&f.access)));
        macro_rules! exercise {
            ($view:expr) => {{
                let view = $view;
                if method == "owned-next" {
                    let mut iter = view.iter_owned().unwrap();
                    if fault == "corrupt" {
                        iter.chain.remaining = 2;
                    }
                    let _held = (fault == "borrow").then(|| f.nodes.links.borrow_mut());
                    ready(case);
                    let _result = std::mem::ManuallyDrop::new(iter.next());
                    std::process::exit(92);
                }
                let _held = (fault == "borrow").then(|| f.nodes.links.borrow_mut());
                ready(case);
                match method {
                    "front" => {
                        let _result = std::mem::ManuallyDrop::new(view.front());
                    }
                    "iter" => {
                        let _result = std::mem::ManuallyDrop::new(view.iter());
                    }
                    "front-owned" => {
                        let _result = std::mem::ManuallyDrop::new(view.front_owned());
                    }
                    "iter-owned" => {
                        let _result = std::mem::ManuallyDrop::new(view.iter_owned());
                    }
                    _ => {}
                }
            }};
        }
        if fault == "corrupt" && method != "owned-next" {
            list.len = 2;
            queue.len = 2;
        }
        if kind == "list-ref" {
            exercise!(EntityListRef {
                value: &list,
                nodes: &f.nodes,
                world_identity: 17,
                entities: &f.entities,
                pending_destruction: &f.pending,
                access: &f.access
            });
        } else if kind == "queue-ref" {
            exercise!(EntityQueueRef {
                value: &queue,
                nodes: &f.nodes,
                world_identity: 17,
                entities: &f.entities,
                pending_destruction: &f.pending,
                access: &f.access
            });
        } else {
            macro_rules! mutate {
                ($view:expr, $push:ident) => {{
                    let mut view = $view;
                    if ["push", "pop", "clear"].contains(&method) {
                        let input = (method == "push").then(|| f.allocate());
                        let _held = (fault == "borrow").then(|| f.nodes.links.borrow());
                        ready(case);
                        match method {
                            "push" => {
                                let _result =
                                    std::mem::ManuallyDrop::new(view.$push(input.unwrap()));
                            }
                            "pop" => {
                                let _result = std::mem::ManuallyDrop::new(view.pop_front());
                            }
                            _ => {
                                let _result = std::mem::ManuallyDrop::new(view.clear());
                            }
                        }
                    } else {
                        exercise!(view);
                    }
                }};
            }
            if kind == "list-mut" {
                mutate!(f.list(&mut list, &mut ticks), push_front);
            } else {
                mutate!(
                    EntityQueueMut {
                        value: &mut queue,
                        nodes: &f.nodes,
                        world_identity: 17,
                        entities: &f.entities,
                        pending_destruction: &f.pending,
                        access: &f.access,
                        marker: ChangeMarker {
                            tick: tick(9),
                            target: ChangeTarget::Resource(&mut ticks)
                        },
                        marked: false
                    },
                    push_back
                );
            }
        }
        std::process::exit(92);
    }
    fn ready(case: &str) {
        eprintln!("container-ready:{case}");
    }
    fn child(case: &str) -> std::process::Output {
        std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "entity_container::entity_container_inner::container_link_batch::container_link_batch_abort_child", "--nocapture"])
            .env("PI_ECS_CONTAINER_ABORT", case).output().unwrap()
    }
    fn target_abort(
        output: &std::process::Output,
        control: std::process::ExitStatus,
        case: &str,
    ) -> bool {
        if !String::from_utf8_lossy(&output.stderr)
            .lines()
            .any(|line| line == format!("container-ready:{case}"))
        {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            let _ = control;
            output.status.signal() == Some(6)
        }
        #[cfg(windows)]
        {
            !output.status.success() && output.status == control
        }
    }
    #[test]
    fn container_link_batch_abort_classifier_rejects_non_targets() {
        let control = child("control");
        assert!(target_abort(&control, control.status, "control"));
        for case in [
            "panic",
            "wrong-exit",
            "test-failure",
            "other-termination",
            "not-ready",
        ] {
            let output = child(case);
            assert!(
                !target_abort(&output, control.status, case),
                "{case}: {output:?}"
            );
        }
    }
    #[test]
    fn container_link_batch_all_access_paths_stop_before_abort() {
        let control = child("control").status;
        for kind in ["list-mut", "list-ref", "queue-mut", "queue-ref"] {
            let methods: &[&str] = if kind.ends_with("mut") {
                &[
                    "front",
                    "iter",
                    "front-owned",
                    "iter-owned",
                    "push",
                    "pop",
                    "clear",
                    "owned-next",
                ]
            } else {
                &["front", "iter", "front-owned", "iter-owned", "owned-next"]
            };
            for method in methods {
                for fault in ["corrupt", "borrow"] {
                    let case = format!("{kind}/{method}/{fault}");
                    let output = child(&case);
                    assert!(target_abort(&output, control, &case), "{case}: {output:?}");
                    assert!(
                        String::from_utf8_lossy(&output.stderr)
                            .lines()
                            .any(|line| line == "container-stopped:true"),
                        "{case}: missing Stopped before abort: {output:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn container_link_batch_abort_child() {
        let Ok(case) = std::env::var("PI_ECS_CONTAINER_ABORT") else {
            return;
        };
        std::panic::set_hook(Box::new(|_| std::process::exit(91)));
        if case == "not-ready" {
            std::process::abort();
        }
        if case == "control" {
            ready(&case);
            std::process::abort();
        }
        if ["panic", "wrong-exit", "test-failure", "other-termination"].contains(&case.as_str()) {
            ready(&case);
            match case.as_str() {
                "panic" => panic!("not abort"),
                "wrong-exit" => std::process::exit(93),
                "test-failure" => std::process::exit(101),
                _ => {
                    #[cfg(unix)]
                    unsafe {
                        unsafe extern "C" {
                            fn raise(signal: i32) -> i32;
                        }
                        raise(15);
                    }
                    std::process::exit(0xc0000005_u32 as i32);
                }
            }
        }
        if case.contains('/') {
            run_access_case(&case);
        }
        let f = Fields::new();
        let mut head = EntityList {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        };
        let mut ticks = crate::resource::ResourceTicks {
            added: tick(1),
            changed: tick(1),
        };
        let mut v = f.list(&mut head, &mut ticks);
        let owned = f.allocate();
        v.push_front(owned).ok().unwrap();
        ready(&case);
        if case == "corrupt" {
            v.value.len = 2;
            let _ = v.iter();
        } else if case == "borrow" {
            let _borrow = f.nodes.links.borrow_mut();
            let _ = v.front();
        } else if case == "identity" {
            let id = v.front().unwrap().unwrap();
            f.entities.borrow_mut().slots[0].get_mut(id).unwrap().strong = 0;
            let _ = v.front_owned();
        }
        std::process::exit(92);
    }
    #[test]
    fn container_link_batch_corrupt_and_conflicting_borrows_abort() {
        let control = child("control");
        assert!(target_abort(&control, control.status, "control"));
        for case in ["corrupt", "borrow", "identity"] {
            let output = child(case);
            assert!(
                target_abort(&output, control.status, case),
                "{case}: {output:?}"
            );
        }
    }
    #[test]
    fn container_link_batch_changed_notice_only_and_dedup() {
        use crate::notification::*;
        let f = Fields::new();
        let owned = f.allocate();
        let id = owned.entity();
        let component = crate::component::ComponentId(0);
        let mut subscriptions = pi_slotmap::SlotMap::with_key();
        subscriptions.insert(SubscriptionState {
            stream: (component, NoticeKind::Changed),
            next: Cell::new(0),
        });
        let notices = Notifications {
            subscriptions,
            components: vec![ComponentNoticeStreams {
                added: None,
                changed: Some(RefCell::new(NoticeStream {
                    headers: Default::default(),
                    range: NoticeStreamRange {
                        first: NoticeSequence(0),
                        end: NoticeSequence(0),
                    },
                })),
                removed: None,
            }],
            layouts: vec![],
            type_ids: vec![],
        };
        let demand = TrackingDemand {
            changed_notice: true,
            ..Default::default()
        };
        let mut head = EntityList::<()> {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        };
        let mut v = EntityListMut {
            value: &mut head,
            nodes: &f.nodes,
            world_identity: 17,
            entities: &f.entities,
            pending_destruction: &f.pending,
            access: &f.access,
            marker: ChangeMarker {
                tick: tick(9),
                target: ChangeTarget::Component {
                    ticks: pi_ecs_types::change_detection::ComponentTicksMut {
                        added: None,
                        changed: None,
                    },
                    demand: &demand,
                    notice: Some(NoticeWrite {
                        notifications: &notices,
                        access: &f.access,
                        component,
                        entity: id,
                        borrow: PhantomData,
                    }),
                },
            },
            marked: false,
        };
        v.clear().unwrap();
        assert_eq!(
            notices.components[0]
                .changed
                .as_ref()
                .unwrap()
                .borrow()
                .range
                .end
                .0,
            0
        );
        v.push_front(owned).ok().unwrap();
        v.clear().unwrap();
        let stream = notices.components[0].changed.as_ref().unwrap().borrow();
        assert_eq!(stream.headers.len(), 1);
        assert_eq!(stream.range.end.0, 1);
        assert_eq!(stream.headers[0].entity, id);
        assert_eq!(stream.headers[0].tick, tick(9));
    }
    fn tick(n: u64) -> ChangeTick {
        ChangeTick::new(std::num::NonZeroU64::new(n).unwrap())
    }
    #[test]
    fn container_link_batch_list_transfers_and_interleaves() {
        let f = Fields::new();
        let mut a = EntityList {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        };
        let mut b = EntityList {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        };
        let mut ta = crate::resource::ResourceTicks {
            added: tick(1),
            changed: tick(1),
        };
        let mut tb = crate::resource::ResourceTicks {
            added: tick(1),
            changed: tick(1),
        };
        let first = f.allocate();
        let id = first.entity();
        let duplicate = first.try_clone().unwrap();
        let other = f.allocate();
        let other_id = other.entity();
        let mut va = f.list(&mut a, &mut ta);
        let mut vb = f.list(&mut b, &mut tb);
        assert!(va.is_empty());
        assert!(va.pop_front().unwrap().is_none());
        va.push_front(first).ok().unwrap();
        vb.push_front(other).ok().unwrap();
        let mut iter = va.iter().unwrap();
        vb.push_front(duplicate).ok().unwrap();
        assert_eq!(iter.next(), Some(id));
        assert_eq!(iter.next(), None);
        assert_eq!(vb.iter().unwrap().collect::<Vec<_>>(), vec![id, other_id]);
        assert_eq!(f.entities.borrow().counts(id), Some((2, 0)));
        let popped = va.pop_front().unwrap().unwrap();
        assert_eq!(f.entities.borrow().counts(id), Some((2, 0)));
        drop(popped);
        vb.clear().unwrap();
        assert_eq!(f.entities.borrow().counts(id), Some((0, 0)));
        assert_eq!(f.entities.borrow().counts(other_id), Some((0, 0)));
        assert!(va.is_empty());
        assert!(vb.is_empty());
        drop(va);
        drop(vb);
        assert_eq!(ta.added, tick(1));
        assert_eq!(ta.changed, tick(9));
        assert_eq!(tb.added, tick(1));
        assert_eq!(tb.changed, tick(9));
    }
}

impl<T: 'static> crate::entity_container::EntityListRef<'_, '_, T> {
    pub(crate) fn list_ref_front_owned<'w>(
        _v: &crate::entity_container::EntityListRef<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            match link_front(
                _v.nodes,
                _v.value.head,
                _v.value.tail,
                _v.value.len,
                _v.access,
            )? {
                Some(entity) => acquire_member(
                    entity,
                    _v.world_identity,
                    _v.entities,
                    _v.pending_destruction,
                    _v.access,
                )
                .map(Some),
                None => Ok(None),
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T11，尚未实现， 非 RC 成员增持延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityListRef<'_, '_, T> {
    pub(crate) fn list_ref_iter_owned<'a, 'w>(
        _v: &'a crate::entity_container::EntityListRef<'_, 'w, T>,
    ) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
        Ok(crate::entity_container::EntityOwnedIter {
            chain: link_iter(
                _v.nodes,
                _v.value.head,
                _v.value.tail,
                _v.value.len,
                _v.access,
            )?,
            world_identity: _v.world_identity,
            entities: _v.entities,
            pending_destruction: _v.pending_destruction,
            access: _v.access,
        })
    }
}
impl<T: 'static> crate::entity_container::EntityListMut<'_, '_, T> {
    pub(crate) fn list_mut_front_owned<'w>(
        _v: &crate::entity_container::EntityListMut<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            match link_front(
                _v.nodes,
                _v.value.head,
                _v.value.tail,
                _v.value.len,
                _v.access,
            )? {
                Some(entity) => acquire_member(
                    entity,
                    _v.world_identity,
                    _v.entities,
                    _v.pending_destruction,
                    _v.access,
                )
                .map(Some),
                None => Ok(None),
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T12，尚未实现， 非 RC 成员增持延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityListMut<'_, '_, T> {
    pub(crate) fn list_mut_iter_owned<'a, 'w>(
        _v: &'a crate::entity_container::EntityListMut<'_, 'w, T>,
    ) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
        Ok(crate::entity_container::EntityOwnedIter {
            chain: link_iter(
                _v.nodes,
                _v.value.head,
                _v.value.tail,
                _v.value.len,
                _v.access,
            )?,
            world_identity: _v.world_identity,
            entities: _v.entities,
            pending_destruction: _v.pending_destruction,
            access: _v.access,
        })
    }
}
impl<T: 'static> crate::entity_container::EntityQueueRef<'_, '_, T> {
    pub(crate) fn queue_ref_front_owned<'w>(
        _v: &crate::entity_container::EntityQueueRef<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            match link_front(
                _v.nodes,
                _v.value.head,
                _v.value.tail,
                _v.value.len,
                _v.access,
            )? {
                Some(entity) => acquire_member(
                    entity,
                    _v.world_identity,
                    _v.entities,
                    _v.pending_destruction,
                    _v.access,
                )
                .map(Some),
                None => Ok(None),
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T13，尚未实现， 非 RC 成员增持延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityQueueRef<'_, '_, T> {
    pub(crate) fn queue_ref_iter_owned<'a, 'w>(
        _v: &'a crate::entity_container::EntityQueueRef<'_, 'w, T>,
    ) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
        Ok(crate::entity_container::EntityOwnedIter {
            chain: link_iter(
                _v.nodes,
                _v.value.head,
                _v.value.tail,
                _v.value.len,
                _v.access,
            )?,
            world_identity: _v.world_identity,
            entities: _v.entities,
            pending_destruction: _v.pending_destruction,
            access: _v.access,
        })
    }
}
impl<T: 'static> crate::entity_container::EntityQueueMut<'_, '_, T> {
    pub(crate) fn queue_mut_front_owned<'w>(
        _v: &crate::entity_container::EntityQueueMut<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            match link_front(
                _v.nodes,
                _v.value.head,
                _v.value.tail,
                _v.value.len,
                _v.access,
            )? {
                Some(entity) => acquire_member(
                    entity,
                    _v.world_identity,
                    _v.entities,
                    _v.pending_destruction,
                    _v.access,
                )
                .map(Some),
                None => Ok(None),
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T14，尚未实现， 非 RC 成员增持延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityQueueMut<'_, '_, T> {
    pub(crate) fn queue_mut_iter_owned<'a, 'w>(
        _v: &'a crate::entity_container::EntityQueueMut<'_, 'w, T>,
    ) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
        Ok(crate::entity_container::EntityOwnedIter {
            chain: link_iter(
                _v.nodes,
                _v.value.head,
                _v.value.tail,
                _v.value.len,
                _v.access,
            )?,
            world_identity: _v.world_identity,
            entities: _v.entities,
            pending_destruction: _v.pending_destruction,
            access: _v.access,
        })
    }
}
impl<T: 'static> crate::entity_container::EntityDequeRef<'_, '_, T> {
    pub(crate) fn deque_ref_front_owned<'w>(
        _v: &crate::entity_container::EntityDequeRef<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            let entity = deque_end(_v.nodes, &_v.value.deque, _v.access, false)?;
            entity
                .map(|entity| {
                    acquire_member(
                        entity,
                        _v.world_identity,
                        _v.entities,
                        _v.pending_destruction,
                        _v.access,
                    )
                })
                .transpose()
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T15，尚未实现， 非 RC Deque 增持延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeRef<'_, '_, T> {
    pub(crate) fn deque_ref_iter_owned<'a, 'w>(
        _v: &'a crate::entity_container::EntityDequeRef<'_, 'w, T>,
    ) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
        Ok(crate::entity_container::EntityOwnedIter {
            chain: deque_iter(_v.nodes, &_v.value.deque, _v.access)?,
            world_identity: _v.world_identity,
            entities: _v.entities,
            pending_destruction: _v.pending_destruction,
            access: _v.access,
        })
    }
}
impl<T: 'static> crate::entity_container::EntityDequeRef<'_, '_, T> {
    pub(crate) fn deque_ref_back_owned<'w>(
        _v: &crate::entity_container::EntityDequeRef<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            let entity = deque_end(_v.nodes, &_v.value.deque, _v.access, true)?;
            entity
                .map(|entity| {
                    acquire_member(
                        entity,
                        _v.world_identity,
                        _v.entities,
                        _v.pending_destruction,
                        _v.access,
                    )
                })
                .transpose()
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T16，尚未实现， 非 RC Deque 增持延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeRef<'_, '_, T> {
    pub(crate) fn deque_ref_get_owned<'w>(
        _v: &crate::entity_container::EntityDequeRef<'_, 'w, T>,
        _key: crate::entity_container::DequeKey,
    ) -> Result<crate::owned_entity::OwnedEntity<'w>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            let entity = {
                let pool = _v
                    .nodes
                    .deque
                    .try_borrow()
                    .unwrap_or_else(|_| broken_with_access(Some(_v.access)));
                if !validate_deque(&pool, &_v.value.deque, _v.access, Some(_key)) {
                    return Err(invalid_deque_key());
                }
                pool.get(_key)
                    .unwrap_or_else(|| broken_with_access(Some(_v.access)))
                    .value
            };
            acquire_member(
                entity,
                _v.world_identity,
                _v.entities,
                _v.pending_destruction,
                _v.access,
            )
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T17，尚未实现， 非 RC Deque 增持延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_mut_front_owned<'w>(
        _v: &crate::entity_container::EntityDequeMut<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            let entity = deque_end(_v.nodes, &_v.value.deque, _v.access, false)?;
            entity
                .map(|entity| {
                    acquire_member(
                        entity,
                        _v.world_identity,
                        _v.entities,
                        _v.pending_destruction,
                        _v.access,
                    )
                })
                .transpose()
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T18，尚未实现， 非 RC Deque 增持延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_mut_iter_owned<'a, 'w>(
        _v: &'a crate::entity_container::EntityDequeMut<'_, 'w, T>,
    ) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
        Ok(crate::entity_container::EntityOwnedIter {
            chain: deque_iter(_v.nodes, &_v.value.deque, _v.access)?,
            world_identity: _v.world_identity,
            entities: _v.entities,
            pending_destruction: _v.pending_destruction,
            access: _v.access,
        })
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_mut_back_owned<'w>(
        _v: &crate::entity_container::EntityDequeMut<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            let entity = deque_end(_v.nodes, &_v.value.deque, _v.access, true)?;
            entity
                .map(|entity| {
                    acquire_member(
                        entity,
                        _v.world_identity,
                        _v.entities,
                        _v.pending_destruction,
                        _v.access,
                    )
                })
                .transpose()
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T19，尚未实现， 非 RC Deque 增持延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_mut_get_owned<'w>(
        _v: &crate::entity_container::EntityDequeMut<'_, 'w, T>,
        _key: crate::entity_container::DequeKey,
    ) -> Result<crate::owned_entity::OwnedEntity<'w>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            let entity = {
                let pool = _v
                    .nodes
                    .deque
                    .try_borrow()
                    .unwrap_or_else(|_| broken_with_access(Some(_v.access)));
                if !validate_deque(&pool, &_v.value.deque, _v.access, Some(_key)) {
                    return Err(invalid_deque_key());
                }
                pool.get(_key)
                    .unwrap_or_else(|| broken_with_access(Some(_v.access)))
                    .value
            };
            acquire_member(
                entity,
                _v.world_identity,
                _v.entities,
                _v.pending_destruction,
                _v.access,
            )
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T20，尚未实现， 非 RC Deque 增持延期")
        }
    }
}
impl crate::entity_container::EntityOwnedIter<'_, '_> {
    pub(crate) fn owned_next<'w>(
        _iter: &mut crate::entity_container::EntityOwnedIter<'_, 'w>,
    ) -> Option<Result<crate::owned_entity::OwnedEntity<'w>, crate::error::EcsError>> {
        #[cfg(feature = "rc")]
        {
            let (entity, next) = chain_peek(&_iter.chain, Some(_iter.access))?;
            match acquire_member(
                entity,
                _iter.world_identity,
                _iter.entities,
                _iter.pending_destruction,
                _iter.access,
            ) {
                Ok(value) => {
                    _iter.chain.next = next;
                    _iter.chain.remaining -= 1;
                    Some(Ok(value))
                }
                Err(error) => Some(Err(error)),
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T21，尚未实现， 非 RC 后端延期")
        }
    }
}

impl<T: 'static> crate::entity_container::EntityListRef<'_, '_, T> {
    pub(crate) fn list_read_len<'a>(
        _v: &'a crate::entity_container::EntityListRef<'_, '_, T>,
    ) -> usize {
        _v.value.len
    }
}
impl<T: 'static> crate::entity_container::EntityListRef<'_, '_, T> {
    pub(crate) fn list_read_empty<'a>(
        _v: &'a crate::entity_container::EntityListRef<'_, '_, T>,
    ) -> bool {
        _v.value.len == 0
    }
}
impl<T: 'static> crate::entity_container::EntityListRef<'_, '_, T> {
    pub(crate) fn list_read_front<'a>(
        _v: &'a crate::entity_container::EntityListRef<'_, '_, T>,
    ) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
        link_front(
            _v.nodes,
            _v.value.head,
            _v.value.tail,
            _v.value.len,
            _v.access,
        )
    }
}
impl<T: 'static> crate::entity_container::EntityListRef<'_, '_, T> {
    pub(crate) fn list_read_iter<'a>(
        _v: &'a crate::entity_container::EntityListRef<'_, '_, T>,
    ) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
        link_iter(
            _v.nodes,
            _v.value.head,
            _v.value.tail,
            _v.value.len,
            _v.access,
        )
    }
}
impl<T: 'static> crate::entity_container::EntityListMut<'_, '_, T> {
    pub(crate) fn list_write_empty(_v: &crate::entity_container::EntityListMut<'_, '_, T>) -> bool {
        _v.value.len == 0
    }
}
impl<T: 'static> crate::entity_container::EntityListMut<'_, '_, T> {
    pub(crate) fn list_write_iter<'a>(
        _v: &'a crate::entity_container::EntityListMut<'_, '_, T>,
    ) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
        link_iter(
            _v.nodes,
            _v.value.head,
            _v.value.tail,
            _v.value.len,
            _v.access,
        )
    }
}
impl<T: 'static> crate::entity_container::EntityQueueRef<'_, '_, T> {
    pub(crate) fn queue_read_len<'a>(
        _v: &'a crate::entity_container::EntityQueueRef<'_, '_, T>,
    ) -> usize {
        _v.value.len
    }
}
impl<T: 'static> crate::entity_container::EntityQueueRef<'_, '_, T> {
    pub(crate) fn queue_read_empty<'a>(
        _v: &'a crate::entity_container::EntityQueueRef<'_, '_, T>,
    ) -> bool {
        _v.value.len == 0
    }
}
impl<T: 'static> crate::entity_container::EntityQueueRef<'_, '_, T> {
    pub(crate) fn queue_read_front<'a>(
        _v: &'a crate::entity_container::EntityQueueRef<'_, '_, T>,
    ) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
        link_front(
            _v.nodes,
            _v.value.head,
            _v.value.tail,
            _v.value.len,
            _v.access,
        )
    }
}
impl<T: 'static> crate::entity_container::EntityQueueRef<'_, '_, T> {
    pub(crate) fn queue_read_iter<'a>(
        _v: &'a crate::entity_container::EntityQueueRef<'_, '_, T>,
    ) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
        link_iter(
            _v.nodes,
            _v.value.head,
            _v.value.tail,
            _v.value.len,
            _v.access,
        )
    }
}
impl<T: 'static> crate::entity_container::EntityQueueMut<'_, '_, T> {
    pub(crate) fn queue_write_empty(
        _v: &crate::entity_container::EntityQueueMut<'_, '_, T>,
    ) -> bool {
        _v.value.len == 0
    }
}
impl<T: 'static> crate::entity_container::EntityQueueMut<'_, '_, T> {
    pub(crate) fn queue_write_iter<'a>(
        _v: &'a crate::entity_container::EntityQueueMut<'_, '_, T>,
    ) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
        link_iter(
            _v.nodes,
            _v.value.head,
            _v.value.tail,
            _v.value.len,
            _v.access,
        )
    }
}
impl<T: 'static> crate::entity_container::EntityDequeRef<'_, '_, T> {
    pub(crate) fn deque_read_len<'a>(
        _v: &'a crate::entity_container::EntityDequeRef<'_, '_, T>,
    ) -> usize {
        _v.value.deque.len()
    }
}
impl<T: 'static> crate::entity_container::EntityDequeRef<'_, '_, T> {
    pub(crate) fn deque_read_empty<'a>(
        _v: &'a crate::entity_container::EntityDequeRef<'_, '_, T>,
    ) -> bool {
        _v.value.deque.len() == 0
    }
}
impl<T: 'static> crate::entity_container::EntityDequeRef<'_, '_, T> {
    pub(crate) fn deque_read_front<'a>(
        _v: &'a crate::entity_container::EntityDequeRef<'_, '_, T>,
    ) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
        deque_front_value(_v.nodes, &_v.value.deque, _v.access)
    }
}
impl<T: 'static> crate::entity_container::EntityDequeRef<'_, '_, T> {
    pub(crate) fn deque_read_iter<'a>(
        _v: &'a crate::entity_container::EntityDequeRef<'_, '_, T>,
    ) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
        deque_iter(_v.nodes, &_v.value.deque, _v.access)
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_write_empty(
        _v: &crate::entity_container::EntityDequeMut<'_, '_, T>,
    ) -> bool {
        _v.value.deque.len() == 0
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_write_iter<'a>(
        _v: &'a crate::entity_container::EntityDequeMut<'_, '_, T>,
    ) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
        deque_iter(_v.nodes, &_v.value.deque, _v.access)
    }
}
impl crate::entity_container::EntityChainIter<'_> {
    pub(crate) fn chain_next(
        _iter: &mut crate::entity_container::EntityChainIter<'_>,
    ) -> Option<crate::entity::Entity> {
        #[cfg(feature = "rc")]
        {
            let (entity, next) = chain_peek(_iter, None)?;
            _iter.next = next;
            _iter.remaining -= 1;
            Some(entity)
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T22，尚未实现， 非 RC 后端延期")
        }
    }
}

impl<T: 'static> crate::entity_container::EntityListMut<'_, '_, T> {
    pub(crate) fn list_len(_v: &crate::entity_container::EntityListMut<'_, '_, T>) -> usize {
        _v.value.len
    }
}
impl<T: 'static> crate::entity_container::EntityListMut<'_, '_, T> {
    pub(crate) fn list_front(
        _v: &crate::entity_container::EntityListMut<'_, '_, T>,
    ) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
        link_front(
            _v.nodes,
            _v.value.head,
            _v.value.tail,
            _v.value.len,
            _v.access,
        )
    }
}
impl<T: 'static> crate::entity_container::EntityListMut<'_, '_, T> {
    pub(crate) fn list_clear(
        _v: &mut crate::entity_container::EntityListMut<'_, '_, T>,
    ) -> Result<(), crate::error::EcsError> {
        while let Some(value) = Self::list_pop_front(_v)? {
            drop(value);
        }
        Ok(())
    }
}
impl<T: 'static> crate::entity_container::EntityListMut<'_, '_, T> {
    pub(crate) fn list_push_front<'w>(
        _v: &mut crate::entity_container::EntityListMut<'_, 'w, T>,
        _value: crate::owned_entity::OwnedEntity<'w>,
    ) -> Result<(), crate::entity_container::EntityInsertError<'w>> {
        #[cfg(feature = "rc")]
        {
            push_link(
                _v.value.head,
                &mut _v.value.head,
                &mut _v.value.tail,
                &mut _v.value.len,
                _v.nodes,
                _v.world_identity,
                _v.access,
                _value,
                false,
            )?;
            mark_changed(&mut _v.marker, &mut _v.marked, _v.access);
            Ok(())
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T23，尚未实现， 非 RC 后端延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityListMut<'_, '_, T> {
    pub(crate) fn list_pop_front<'w>(
        _v: &mut crate::entity_container::EntityListMut<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            let value = pop_link(
                &mut _v.value.head,
                &mut _v.value.tail,
                &mut _v.value.len,
                _v.nodes,
                _v.world_identity,
                _v.entities,
                _v.pending_destruction,
                _v.access,
            );
            if value.is_some() {
                mark_changed(&mut _v.marker, &mut _v.marked, _v.access);
            }
            Ok(value)
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T24，尚未实现， 非 RC 后端延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityQueueMut<'_, '_, T> {
    pub(crate) fn queue_len(_v: &crate::entity_container::EntityQueueMut<'_, '_, T>) -> usize {
        _v.value.len
    }
}
impl<T: 'static> crate::entity_container::EntityQueueMut<'_, '_, T> {
    pub(crate) fn queue_front(
        _v: &crate::entity_container::EntityQueueMut<'_, '_, T>,
    ) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
        link_front(
            _v.nodes,
            _v.value.head,
            _v.value.tail,
            _v.value.len,
            _v.access,
        )
    }
}
impl<T: 'static> crate::entity_container::EntityQueueMut<'_, '_, T> {
    pub(crate) fn queue_clear(
        _v: &mut crate::entity_container::EntityQueueMut<'_, '_, T>,
    ) -> Result<(), crate::error::EcsError> {
        while let Some(value) = Self::queue_pop_front(_v)? {
            drop(value);
        }
        Ok(())
    }
}
impl<T: 'static> crate::entity_container::EntityQueueMut<'_, '_, T> {
    pub(crate) fn queue_push_back<'w>(
        _v: &mut crate::entity_container::EntityQueueMut<'_, 'w, T>,
        _value: crate::owned_entity::OwnedEntity<'w>,
    ) -> Result<(), crate::entity_container::EntityInsertError<'w>> {
        #[cfg(feature = "rc")]
        {
            push_link(
                _v.value.head,
                &mut _v.value.head,
                &mut _v.value.tail,
                &mut _v.value.len,
                _v.nodes,
                _v.world_identity,
                _v.access,
                _value,
                true,
            )?;
            mark_changed(&mut _v.marker, &mut _v.marked, _v.access);
            Ok(())
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T25，尚未实现， 非 RC 后端延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityQueueMut<'_, '_, T> {
    pub(crate) fn queue_pop_front<'w>(
        _v: &mut crate::entity_container::EntityQueueMut<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            let value = pop_link(
                &mut _v.value.head,
                &mut _v.value.tail,
                &mut _v.value.len,
                _v.nodes,
                _v.world_identity,
                _v.entities,
                _v.pending_destruction,
                _v.access,
            );
            if value.is_some() {
                mark_changed(&mut _v.marker, &mut _v.marked, _v.access);
            }
            Ok(value)
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T26，尚未实现， 非 RC 后端延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_len(_v: &crate::entity_container::EntityDequeMut<'_, '_, T>) -> usize {
        _v.value.deque.len()
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_front(
        _v: &crate::entity_container::EntityDequeMut<'_, '_, T>,
    ) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
        deque_front_value(_v.nodes, &_v.value.deque, _v.access)
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_clear(
        _v: &mut crate::entity_container::EntityDequeMut<'_, '_, T>,
    ) -> Result<(), crate::error::EcsError> {
        while let Some(value) = Self::deque_pop_front(_v)? {
            drop(value);
        }
        Ok(())
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_push_front<'w>(
        _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
        _value: crate::owned_entity::OwnedEntity<'w>,
    ) -> Result<crate::entity_container::DequeKey, crate::entity_container::EntityInsertError<'w>>
    {
        #[cfg(feature = "rc")]
        {
            deque_insert(_v, _value, DequeInsert::Front)
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T27，尚未实现， 非 RC Deque 后端延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_push_back<'w>(
        _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
        _value: crate::owned_entity::OwnedEntity<'w>,
    ) -> Result<crate::entity_container::DequeKey, crate::entity_container::EntityInsertError<'w>>
    {
        #[cfg(feature = "rc")]
        {
            deque_insert(_v, _value, DequeInsert::Back)
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T28，尚未实现， 非 RC Deque 后端延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_pop_front<'w>(
        _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            deque_take(_v, None, false)
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T29，尚未实现， 非 RC Deque 后端延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_pop_back<'w>(
        _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            deque_take(_v, None, true)
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T30，尚未实现， 非 RC Deque 后端延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_insert_before<'w>(
        _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
        _key: crate::entity_container::DequeKey,
        _value: crate::owned_entity::OwnedEntity<'w>,
    ) -> Result<crate::entity_container::DequeKey, crate::entity_container::EntityInsertError<'w>>
    {
        #[cfg(feature = "rc")]
        {
            deque_insert(_v, _value, DequeInsert::Before(_key))
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T31，尚未实现， 非 RC Deque 后端延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_insert_after<'w>(
        _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
        _key: crate::entity_container::DequeKey,
        _value: crate::owned_entity::OwnedEntity<'w>,
    ) -> Result<crate::entity_container::DequeKey, crate::entity_container::EntityInsertError<'w>>
    {
        #[cfg(feature = "rc")]
        {
            deque_insert(_v, _value, DequeInsert::After(_key))
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T32，尚未实现， 非 RC Deque 后端延期")
        }
    }
}
impl<T: 'static> crate::entity_container::EntityDequeMut<'_, '_, T> {
    pub(crate) fn deque_remove<'w>(
        _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
        _key: crate::entity_container::DequeKey,
    ) -> Result<crate::owned_entity::OwnedEntity<'w>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            Ok(deque_take(_v, Some(_key), false)?
                .unwrap_or_else(|| broken_with_access(Some(_v.access))))
        }
        #[cfg(not(feature = "rc"))]
        {
            todo!("编号：T33，尚未实现， 非 RC Deque 后端延期")
        }
    }
}

type DequeHead =
    pi_slot_deque::Deque<crate::entity_container::DequeKey, crate::entity_container::EntityNode>;
#[cfg(feature = "rc")]
use pi_key::__impl::Null;
#[cfg(feature = "rc")]
fn validate_deque(
    pool: &pi_slot_deque::DequeStorage<
        crate::entity_container::DequeKey,
        crate::entity_container::EntityNode,
    >,
    head: &DequeHead,
    access: &crate::world::AccessState,
    wanted: Option<crate::entity_container::DequeKey>,
) -> bool {
    let mut cursor = head.head();
    let mut previous = crate::entity_container::DequeKey::null();
    let mut found = false;
    for _ in 0..head.len() {
        if cursor.is_null() || pool.get(cursor).is_none() || pool.prev(cursor) != Some(previous) {
            broken_with_access(Some(access));
        }
        found |= wanted == Some(cursor);
        previous = cursor;
        cursor = pool
            .next(cursor)
            .unwrap_or_else(|| broken_with_access(Some(access)));
    }
    if !cursor.is_null() || previous != head.tail() || (head.len() == 0 && !head.head().is_null()) {
        broken_with_access(Some(access));
    }
    found
}
fn deque_iter<'a>(
    nodes: &'a crate::entity_container::EntityNodePools,
    head: &DequeHead,
    access: &crate::world::AccessState,
) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
    #[cfg(feature = "rc")]
    {
        let pool = nodes
            .deque
            .try_borrow()
            .unwrap_or_else(|_| broken_with_access(Some(access)));
        validate_deque(&pool, head, access, None);
        Ok(EntityChainIter {
            nodes,
            next: (!head.head().is_null()).then_some(ChainCursor::Deque(head.head())),
            remaining: head.len(),
        })
    }
    #[cfg(not(feature = "rc"))]
    {
        let _ = (nodes, head, access);
        todo!("编号：T34，尚未实现， 非 RC Deque 后端延期")
    }
}
fn deque_front_value(
    nodes: &crate::entity_container::EntityNodePools,
    head: &DequeHead,
    access: &crate::world::AccessState,
) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
    let mut iter = deque_iter(nodes, head, access)?;
    Ok(iter.next())
}
#[cfg(feature = "rc")]
fn deque_end(
    nodes: &EntityNodePools,
    head: &DequeHead,
    access: &crate::world::AccessState,
    back: bool,
) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
    let pool = nodes
        .deque
        .try_borrow()
        .unwrap_or_else(|_| broken_with_access(Some(access)));
    validate_deque(&pool, head, access, None);
    let key = if back { head.tail() } else { head.head() };
    Ok(if key.is_null() {
        None
    } else {
        Some(
            pool.get(key)
                .unwrap_or_else(|| broken_with_access(Some(access)))
                .value,
        )
    })
}
#[cfg(feature = "rc")]
fn deque_take<'w, T: 'static>(
    v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
    key: Option<crate::entity_container::DequeKey>,
    back: bool,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    let mut pool = v
        .nodes
        .deque
        .try_borrow_mut()
        .unwrap_or_else(|_| broken_with_access(Some(v.access)));
    let found = validate_deque(&pool, &v.value.deque, v.access, key);
    if key.is_some() && !found {
        return Err(invalid_deque_key());
    }
    if v.value.deque.len() == 0 {
        return Ok(None);
    }
    let mut ops = v.value.deque.ops(&mut pool);
    let node = match key {
        Some(key) => ops.remove(key),
        None if back => ops.pop_back(),
        None => ops.pop_front(),
    }
    .unwrap_or_else(|| broken_with_access(Some(v.access)));
    let value = crate::owned_entity::OwnedEntity {
        world_identity: v.world_identity,
        entity: node.value,
        entities: v.entities,
        pending_destruction: v.pending_destruction,
        access: v.access,
    };
    drop(pool);
    mark_changed(&mut v.marker, &mut v.marked, v.access);
    Ok(Some(value))
}
#[cfg(feature = "rc")]
enum DequeInsert {
    Front,
    Back,
    Before(crate::entity_container::DequeKey),
    After(crate::entity_container::DequeKey),
}
#[cfg(feature = "rc")]
fn deque_insert<'w, T: 'static>(
    v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
    value: crate::owned_entity::OwnedEntity<'w>,
    position: DequeInsert,
) -> Result<crate::entity_container::DequeKey, crate::entity_container::EntityInsertError<'w>> {
    if value.world_identity != v.world_identity
        || !std::ptr::eq(value.entities, v.entities)
        || !std::ptr::eq(value.pending_destruction, v.pending_destruction)
        || !std::ptr::eq(value.access, v.access)
        || stopped(v.access)
    {
        return Err(crate::entity_container::EntityInsertError {
            error: crate::error::classified_error(
                crate::error::ErrorKind::InvalidState,
                "跨 World 输入或 World 已停止",
            ),
            value,
        });
    }
    let mut pool = v
        .nodes
        .deque
        .try_borrow_mut()
        .unwrap_or_else(|_| broken_with_access(Some(v.access)));
    let anchor = match position {
        DequeInsert::Before(key) | DequeInsert::After(key) => Some(key),
        _ => None,
    };
    let found = validate_deque(&pool, &v.value.deque, v.access, anchor);
    if anchor.is_some() && !found {
        return Err(crate::entity_container::EntityInsertError {
            error: invalid_deque_key(),
            value,
        });
    }
    if v.value.deque.len().checked_add(1).is_none() {
        return Err(crate::entity_container::EntityInsertError {
            error: crate::error::classified_error(
                crate::error::ErrorKind::ResourceExhausted,
                "容器长度耗尽",
            ),
            value,
        });
    }
    // Allocation touches only a private empty head while the input still owns its strong.
    let mut prepared = DequeHead::new();
    let key = prepared
        .ops(&mut pool)
        .push_back(crate::entity_container::EntityNode {
            value: value.entity(),
        });
    // Validated links, unchanged pool borrow, no allocation/callback from publication to forget.
    let moved = match position {
        DequeInsert::Front => prepared.ops(&mut pool).move_after_to(
            key,
            &mut v.value.deque,
            crate::entity_container::DequeKey::null(),
        ),
        DequeInsert::Back => prepared.ops(&mut pool).move_before_to(
            key,
            &mut v.value.deque,
            crate::entity_container::DequeKey::null(),
        ),
        DequeInsert::Before(anchor) => {
            prepared
                .ops(&mut pool)
                .move_before_to(key, &mut v.value.deque, anchor)
        }
        DequeInsert::After(anchor) => {
            prepared
                .ops(&mut pool)
                .move_after_to(key, &mut v.value.deque, anchor)
        }
    };
    if moved != Some(key) {
        broken_with_access(Some(v.access));
    }
    std::mem::forget(value);
    drop(pool);
    mark_changed(&mut v.marker, &mut v.marked, v.access);
    Ok(key)
}
#[cfg(feature = "rc")]
fn invalid_deque_key() -> crate::error::EcsError {
    crate::error::classified_error(
        crate::error::ErrorKind::NotFound,
        "节点键不属于当前 Deque 或已失效",
    )
}
#[cfg(feature = "rc")]
use crate::entity_container::{LinkKey, NodePool};
#[cfg(feature = "rc")]
impl<N> crate::entity_container::NodePool<N> {
    pub(crate) fn node_pool_insert_inner(_pool: &mut NodePool<N>, _node: N) -> LinkKey {
        _pool.slots.insert(_node)
    }
}
#[cfg(feature = "rc")]
impl<N> crate::entity_container::NodePool<N> {
    pub(crate) fn node_pool_get_inner(_pool: &NodePool<N>, _key: LinkKey) -> Option<&N> {
        _pool.slots.get(_key)
    }
}
#[cfg(feature = "rc")]
impl<N> crate::entity_container::NodePool<N> {
    pub(crate) fn node_pool_get_mut_inner(
        _pool: &mut NodePool<N>,
        _key: LinkKey,
    ) -> Option<&mut N> {
        _pool.slots.get_mut(_key)
    }
}
#[cfg(feature = "rc")]
impl<N> crate::entity_container::NodePool<N> {
    pub(crate) fn node_pool_remove_inner(_pool: &mut NodePool<N>, _key: LinkKey) -> Option<N> {
        _pool.slots.remove(_key)
    }
}

#[cfg(feature = "rc")]
fn release_member(
    entity: crate::entity::Entity,
    entities: &crate::entity_store::EntityIdentityStore,
    pending: &crate::destruction::PendingDestruction,
    access: &crate::world::AccessState,
    mode: crate::component_policy::ReleaseMode,
) {
    let released = match entities.try_borrow_mut() {
        Ok(mut store) => store.release_strong(entity),
        Err(_) => broken_with_access(Some(access)),
    };
    // 身份短借已结束；WholeState 绝不构造 OwnedEntity 或登记业务队列。
    match released {
        Ok(true) if mode == crate::component_policy::ReleaseMode::Normal => pending.enqueue(entity),
        Ok(_) => {}
        Err(_) => broken_with_access(Some(access)),
    }
}

#[cfg(feature = "rc")]
#[allow(clippy::too_many_arguments)]
fn release_links(
    head: &mut Option<LinkKey>,
    tail: &mut Option<LinkKey>,
    len: &mut usize,
    nodes: &EntityNodePools,
    entities: &crate::entity_store::EntityIdentityStore,
    pending: &crate::destruction::PendingDestruction,
    access: &crate::world::AccessState,
    mode: crate::component_policy::ReleaseMode,
) {
    loop {
        let value = {
            let mut pool = nodes
                .links
                .try_borrow_mut()
                .unwrap_or_else(|_| broken_with_access(Some(access)));
            validate(&pool, *head, *tail, *len, access);
            let Some(key) = *head else {
                break;
            };
            let node = pool
                .remove(key)
                .unwrap_or_else(|| broken_with_access(Some(access)));
            *head = node.next;
            *len -= 1;
            if *len == 0 {
                *tail = None;
            }
            node.value
        };
        // 每步只摘一项，余链仍由有效头持有；池借用结束才清偿计数。
        release_member(value, entities, pending, access, mode);
    }
}

pub(crate) unsafe fn release_list_inner<'a, 'w, T: 'static>(
    _context: &crate::destruction::ReleaseContext<'w>,
    _head: crate::ptr::OwningPtr<'a>,
) {
    #[cfg(feature = "rc")]
    {
        // SAFETY: 登记的真实 EntityList<T>，唯一清偿且来源分配保持有效。
        let head = unsafe {
            &mut *_head
                .into_raw()
                .cast::<crate::entity_container::EntityList<T>>()
        };
        let world = _context.world.pointer.as_ptr();
        // SAFETY: 只投影节点、身份、停止状态与归零登记字段，无整 World 引用。
        let (nodes, entities, pending, access) = unsafe {
            (
                &*std::ptr::addr_of!((*world).entity_nodes),
                &*std::ptr::addr_of!((*world).entities),
                &*std::ptr::addr_of!((*world).pending_destruction),
                &*std::ptr::addr_of!((*world).access),
            )
        };
        release_links(
            &mut head.head,
            &mut head.tail,
            &mut head.len,
            nodes,
            entities,
            pending,
            access,
            _context.mode,
        );
        // 头仅含非拥有链接及 PhantomData；节点责任已逐份清偿，不再普通 Drop 补刀。
    }
    #[cfg(not(feature = "rc"))]
    {
        let _ = (_context, _head);
        todo!("编号：T35，尚未实现， 非 RC 整头释放后端延期")
    }
}
pub(crate) unsafe fn release_queue_inner<'a, 'w, T: 'static>(
    _context: &crate::destruction::ReleaseContext<'w>,
    _head: crate::ptr::OwningPtr<'a>,
) {
    #[cfg(feature = "rc")]
    {
        // SAFETY: 登记的真实 EntityQueue<T>，唯一清偿且来源分配保持有效。
        let head = unsafe {
            &mut *_head
                .into_raw()
                .cast::<crate::entity_container::EntityQueue<T>>()
        };
        let world = _context.world.pointer.as_ptr();
        // SAFETY: 只投影节点、身份、停止状态与归零登记字段，无整 World 引用。
        let (nodes, entities, pending, access) = unsafe {
            (
                &*std::ptr::addr_of!((*world).entity_nodes),
                &*std::ptr::addr_of!((*world).entities),
                &*std::ptr::addr_of!((*world).pending_destruction),
                &*std::ptr::addr_of!((*world).access),
            )
        };
        release_links(
            &mut head.head,
            &mut head.tail,
            &mut head.len,
            nodes,
            entities,
            pending,
            access,
            _context.mode,
        );
        // 头仅含非拥有链接及 PhantomData；节点责任已逐份清偿，不再普通 Drop 补刀。
    }
    #[cfg(not(feature = "rc"))]
    {
        let _ = (_context, _head);
        todo!("编号：T36，尚未实现， 非 RC 整头释放后端延期")
    }
}
pub(crate) unsafe fn release_deque_inner<'a, 'w, T: 'static>(
    _context: &crate::destruction::ReleaseContext<'w>,
    _head: crate::ptr::OwningPtr<'a>,
) {
    #[cfg(feature = "rc")]
    {
        // SAFETY: 登记的真实 EntityDeque<T>，唯一清偿且来源分配保持有效。
        let head = unsafe {
            &mut *_head
                .into_raw()
                .cast::<crate::entity_container::EntityDeque<T>>()
        };
        let world = _context.world.pointer.as_ptr();
        // SAFETY: 只投影节点、身份、停止状态与归零登记字段，无整 World 引用。
        let (nodes, entities, pending, access) = unsafe {
            (
                &*std::ptr::addr_of!((*world).entity_nodes),
                &*std::ptr::addr_of!((*world).entities),
                &*std::ptr::addr_of!((*world).pending_destruction),
                &*std::ptr::addr_of!((*world).access),
            )
        };
        loop {
            let value = {
                let mut pool = nodes
                    .deque
                    .try_borrow_mut()
                    .unwrap_or_else(|_| broken_with_access(Some(access)));
                validate_deque(&pool, &head.deque, access, None);
                if head.deque.len() == 0 {
                    head.deque.ops(&mut pool).clear();
                    break;
                }
                // 将单项转到栈上临时头；余链始终有效。clear 实际回收这一项，
                // 不把整条链摘空后才尝试逐份清偿。
                let key = head.deque.head();
                let value = pool
                    .get(key)
                    .unwrap_or_else(|| broken_with_access(Some(access)))
                    .value;
                let mut single = DequeHead::new();
                if head.deque.ops(&mut pool).move_front_to(&mut single) != Some(key) {
                    broken_with_access(Some(access));
                }
                single.ops(&mut pool).clear();
                value
            };
            release_member(value, entities, pending, access, _context.mode);
        }
        // 头仅含非拥有链接及 PhantomData；节点责任已逐份清偿，不再普通 Drop 补刀。
    }
    #[cfg(not(feature = "rc"))]
    {
        let _ = (_context, _head);
        todo!("编号：T37，尚未实现， 非 RC 整头释放后端延期")
    }
}

#[cfg(feature = "rc")]
use crate::entity_container::{ChainCursor, EntityChainIter, EntityLink, EntityNodePools};
#[cfg(feature = "rc")]
fn mark_changed(
    marker: &mut crate::change_detection::ChangeMarker<'_>,
    marked: &mut bool,
    access: &crate::world::AccessState,
) {
    struct StopOnPanic<'a>(&'a crate::world::AccessState);
    impl Drop for StopOnPanic<'_> {
        fn drop(&mut self) {
            if std::thread::panicking() {
                self.0.mode.set(crate::world::AccessMode::Stopped);
            }
        }
    }
    let _guard = StopOnPanic(access);
    marker.set_changed(marked);
}
#[cfg(feature = "rc")]
fn broken_with_access(access: Option<&crate::world::AccessState>) -> ! {
    if let Some(access) = access {
        access.mode.set(crate::world::AccessMode::Stopped);
    }
    #[cfg(test)]
    {
        container_link_batch::observe_fatal();
        container_deque_batch::observe_fatal();
    }
    std::process::abort()
}
#[cfg(feature = "rc")]
fn stopped(access: &crate::world::AccessState) -> bool {
    let mode = access.mode.replace(crate::world::AccessMode::Idle);
    let stopped = matches!(mode, crate::world::AccessMode::Stopped);
    access.mode.set(mode);
    stopped
}
#[cfg(feature = "rc")]
fn validate(
    pool: &NodePool<EntityLink>,
    head: Option<LinkKey>,
    tail: Option<LinkKey>,
    len: usize,
    access: &crate::world::AccessState,
) {
    let mut cursor = head;
    let mut last = None;
    for _ in 0..len {
        let key = cursor.unwrap_or_else(|| broken_with_access(Some(access)));
        let node = pool
            .get(key)
            .unwrap_or_else(|| broken_with_access(Some(access)));
        last = Some(key);
        cursor = node.next;
    }
    if cursor.is_some() || last != tail || (len == 0 && head.is_some()) {
        broken_with_access(Some(access));
    }
}
fn link_iter<'a>(
    nodes: &'a crate::entity_container::EntityNodePools,
    head: Option<crate::entity_container::LinkKey>,
    tail: Option<crate::entity_container::LinkKey>,
    len: usize,
    access: &crate::world::AccessState,
) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
    #[cfg(feature = "rc")]
    {
        {
            let pool = nodes
                .links
                .try_borrow()
                .unwrap_or_else(|_| broken_with_access(Some(access)));
            validate(&pool, head, tail, len, access);
        }
        Ok(EntityChainIter {
            nodes,
            next: head.map(ChainCursor::Link),
            remaining: len,
        })
    }
    #[cfg(not(feature = "rc"))]
    {
        let _ = (nodes, head, tail, len, access);
        todo!("编号：T38，尚未实现， 非 RC 单链后端延期")
    }
}
fn link_front(
    nodes: &crate::entity_container::EntityNodePools,
    head: Option<crate::entity_container::LinkKey>,
    tail: Option<crate::entity_container::LinkKey>,
    len: usize,
    access: &crate::world::AccessState,
) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
    let iter = link_iter(nodes, head, tail, len, access)?;
    #[cfg(feature = "rc")]
    {
        Ok(chain_peek(&iter, Some(access)).map(|(entity, _)| entity))
    }
    #[cfg(not(feature = "rc"))]
    {
        Ok(iter.into_iter().next())
    }
}
#[cfg(feature = "rc")]
fn chain_peek(
    iter: &EntityChainIter<'_>,
    access: Option<&crate::world::AccessState>,
) -> Option<(crate::entity::Entity, Option<ChainCursor>)> {
    if iter.remaining == 0 {
        if iter.next.is_some() {
            broken_with_access(access);
        }
        return None;
    }
    let key = match iter.next {
        Some(ChainCursor::Link(key)) => key,
        Some(ChainCursor::Deque(key)) => {
            let pool = iter
                .nodes
                .deque
                .try_borrow()
                .unwrap_or_else(|_| broken_with_access(access));
            let node = pool.get(key).unwrap_or_else(|| broken_with_access(access));
            let next = pool.next(key).unwrap_or_else(|| broken_with_access(access));
            if (iter.remaining == 1) != next.is_null() {
                broken_with_access(access);
            }
            return Some((
                node.value,
                (!next.is_null()).then_some(ChainCursor::Deque(next)),
            ));
        }
        None => broken_with_access(access),
    };
    let pool = iter
        .nodes
        .links
        .try_borrow()
        .unwrap_or_else(|_| broken_with_access(access));
    let node = pool.get(key).unwrap_or_else(|| broken_with_access(access));
    if (iter.remaining == 1) != node.next.is_none() {
        broken_with_access(access);
    }
    Some((node.value, node.next.map(ChainCursor::Link)))
}
#[cfg(feature = "rc")]
fn acquire_member<'w>(
    entity: crate::entity::Entity,
    world_identity: u64,
    entities: &'w crate::entity_store::EntityIdentityStore,
    pending: &'w crate::destruction::PendingDestruction,
    access: &'w crate::world::AccessState,
) -> Result<crate::owned_entity::OwnedEntity<'w>, crate::error::EcsError> {
    if stopped(access) {
        return Err(crate::error::classified_error(
            crate::error::ErrorKind::InvalidState,
            "World 已停止",
        ));
    }
    let result = match entities.try_borrow_mut() {
        Ok(mut store) => store.acquire_strong(entity),
        Err(_) => broken_with_access(Some(access)),
    };
    if let Err(error) = result {
        if *error.current_context() == crate::error::ErrorKind::ResourceExhausted {
            return Err(error);
        }
        broken_with_access(Some(access));
    }
    Ok(crate::owned_entity::OwnedEntity {
        world_identity,
        entity,
        entities,
        pending_destruction: pending,
        access,
    })
}
#[cfg(feature = "rc")]
#[allow(clippy::too_many_arguments)]
fn push_link<'w>(
    old_head: Option<LinkKey>,
    head: &mut Option<LinkKey>,
    tail: &mut Option<LinkKey>,
    len: &mut usize,
    nodes: &EntityNodePools,
    world_identity: u64,
    access: &crate::world::AccessState,
    value: crate::owned_entity::OwnedEntity<'w>,
    back: bool,
) -> Result<(), crate::entity_container::EntityInsertError<'w>> {
    if value.world_identity != world_identity || stopped(access) {
        return Err(crate::entity_container::EntityInsertError {
            error: crate::error::classified_error(
                crate::error::ErrorKind::InvalidState,
                "跨 World 输入或 World 已停止",
            ),
            value,
        });
    }
    let new_len = len.checked_add(1).expect("容器长度耗尽");
    {
        let mut pool = nodes
            .links
            .try_borrow_mut()
            .unwrap_or_else(|_| broken_with_access(Some(access)));
        validate(&pool, *head, *tail, *len, access);
        let key = pool.insert(EntityLink {
            value: value.entity(),
            next: if back { None } else { old_head },
        });
        // Final allocation is complete; publication through forget cannot unwind.
        if back {
            if let Some(old_tail) = *tail {
                match pool.get_mut(old_tail) {
                    Some(node) => node.next = Some(key),
                    None => broken_with_access(Some(access)),
                }
            } else {
                *head = Some(key);
            }
            *tail = Some(key);
        } else {
            *head = Some(key);
            if *len == 0 {
                *tail = Some(key);
            }
        }
        *len = new_len;
        std::mem::forget(value);
    }
    Ok(())
}
#[cfg(feature = "rc")]
#[allow(clippy::too_many_arguments)]
fn pop_link<'w>(
    head: &mut Option<LinkKey>,
    tail: &mut Option<LinkKey>,
    len: &mut usize,
    nodes: &EntityNodePools,
    world_identity: u64,
    entities: &'w crate::entity_store::EntityIdentityStore,
    pending: &'w crate::destruction::PendingDestruction,
    access: &'w crate::world::AccessState,
) -> Option<crate::owned_entity::OwnedEntity<'w>> {
    let mut pool = nodes
        .links
        .try_borrow_mut()
        .unwrap_or_else(|_| broken_with_access(Some(access)));
    validate(&pool, *head, *tail, *len, access);
    let key = (*head)?;
    let node = pool
        .remove(key)
        .unwrap_or_else(|| broken_with_access(Some(access)));
    *head = node.next;
    *len -= 1;
    if *len == 0 {
        *tail = None;
    }
    let value = crate::owned_entity::OwnedEntity {
        world_identity,
        entity: node.value,
        entities,
        pending_destruction: pending,
        access,
    };
    drop(pool);
    Some(value)
}
