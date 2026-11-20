//! 单系统生命周期配对芯。
#[cfg(all(test, feature = "rc"))]
mod context_tests {
    use crate::system::SystemRunContext;
    use pi_ecs_core::{
        access::AccessTree, change_detection::ChangeWindow, unsafe_world_cell::UnsafeWorldCell,
        world::World,
    };

    // Seam: 公共 child 只接受 Together；派生描述不借临时父上下文，保留原窗口及来源。
    #[test]
    fn rc_context_together_child_retains_authority_and_rejects_other_nodes() {
        let mut world = World::new().unwrap();
        world.finish_registration().unwrap();
        let tree = AccessTree::Together(vec![AccessTree::Exclusive(vec![AccessTree::Together(
            Vec::new(),
        )])]);
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                let tick = unsafe { world.next_system_tick()? };
                // SAFETY: 只派生描述，不兑现任何业务引用，原会话覆盖全部描述。
                unsafe {
                    world.system_scope(&tree, version, |access| {
                        let source = access.world_cell().id();
                        let window = ChangeWindow {
                            last_run: Some(tick),
                            this_run: tick,
                        };
                        let child = {
                            let parent = SystemRunContext::new(access, window).unwrap();
                            assert!(parent.child(1).is_err());
                            parent.child(0).unwrap()
                        };
                        assert!(child.child(0).is_err());
                        let (access, actual) = child.into_parts();
                        assert_eq!(access.world_cell().id(), source);
                        assert_eq!(actual.last_run, Some(tick));
                        assert_eq!(actual.this_run, tick);
                    })?;
                }
                Ok(())
            })
            .unwrap();
        world.close().unwrap();
    }

    // Seam: ParamSet 的内部 Exclusive 派生必须拒绝 Together、叶子与越界。
    #[test]
    fn rc_context_exclusive_child_preserves_window_and_node_kind() {
        let mut world = World::new().unwrap();
        let id = world.register_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        world.insert_resource(23u32).unwrap();
        let leaf = || {
            AccessTree::Leaf(pi_ecs_core::access::AccessDeclaration {
                resources: vec![(id, pi_ecs_core::access::AccessMode::Read)].into_boxed_slice(),
                multi_resources: Box::new([]),
                components: Box::new([]),
                tick_reads: Box::new([]),
                reads_entities: false,
                scope: pi_ecs_core::access::EntityScope {
                    with: Box::new([]),
                    without: Box::new([]),
                },
                structure: pi_ecs_core::access::StructuralAccess::None,
                observations: Box::new([]),
            })
        };
        let exclusive = AccessTree::Exclusive(vec![leaf()]);
        let together = AccessTree::Together(vec![leaf()]);
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                let tick = unsafe { world.next_system_tick()? };
                // SAFETY: 成员结果仅在该次独占短借内使用；无兄弟结果或缓存。
                unsafe {
                    world.system_scope(&exclusive, version, |access| {
                        let parent = SystemRunContext::new(
                            access,
                            ChangeWindow {
                                last_run: None,
                                this_run: tick,
                            },
                        )
                        .unwrap();
                        assert!(parent.exclusive_child(1).is_err());
                        let child = parent.exclusive_child(0).unwrap();
                        assert!(child.exclusive_child(0).is_err());
                        let (access, window) = child.into_parts();
                        assert_eq!(window.this_run, tick);
                        assert!(window.last_run.is_none());
                        assert_eq!(*access.resource_parts::<u32>(id).unwrap().unwrap().0, 23);
                    })?;
                    world.system_scope(&together, version, |access| {
                        let parent = SystemRunContext::new(
                            access,
                            ChangeWindow {
                                last_run: None,
                                this_run: tick,
                            },
                        )
                        .unwrap();
                        assert!(parent.exclusive_child(0).is_err());
                    })?;
                }
                Ok(())
            })
            .unwrap();
        world.close().unwrap();
    }

    // Seam: 真实运行授权构造上下文，并由公共 into_parts 交回原授权和窗口。
    #[test]
    fn rc_context_roundtrip_retains_source_and_window() {
        let mut world = World::new().unwrap();
        world.finish_registration().unwrap();
        let tree = AccessTree::Together(Vec::new());
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                let tick = unsafe { world.next_system_tick()? };
                // SAFETY: 空声明不取业务引用；上下文及返回描述均不逃出会话。
                unsafe {
                    world.system_scope(&tree, version, |access| {
                        let source = access.world_cell().id();
                        let context = SystemRunContext::new(
                            access,
                            ChangeWindow {
                                last_run: None,
                                this_run: tick,
                            },
                        )
                        .unwrap();
                        let (access, window) = context.into_parts();
                        assert_eq!(access.world_cell().id(), source);
                        assert!(window.last_run.is_none());
                        assert_eq!(window.this_run, tick);
                        assert!(access.child(0).is_err());
                        let context = SystemRunContext::new(
                            access,
                            ChangeWindow {
                                last_run: Some(tick),
                                this_run: tick,
                            },
                        )
                        .unwrap();
                        let (_, window) = context.into_parts();
                        assert_eq!(window.last_run, Some(tick));
                        assert_eq!(window.this_run, tick);
                    })?;
                }
                Ok(())
            })
            .unwrap();
        world.close().unwrap();
    }
}

#[cfg(all(test, feature = "rc"))]
mod lifecycle_tests {
    use super::*;
    use pi_ecs_core::{parameter_registration::ProducerRegistration, world::World};
    struct Producer;
    // SAFETY: 本测试参数仅登记/注销生产者，不兑现任何业务引用。
    unsafe impl SystemParam for Producer {
        type Config = u32;
        type State = ProducerRegistration;
        type Pending = ();
        type Item<'r> = ();
        fn initialize(
            value: u32,
            registration: &mut ParamRegistration<'_>,
        ) -> Result<Self::State, EcsError> {
            registration.access().register_producer(value)
        }
        fn prepare(_: &mut Self::State, _: &mut World) -> Result<ParamAccess, EcsError> {
            Ok(ParamAccess::Together(Vec::new()))
        }
        fn declaration(
            _: &Self::State,
            _: &mut World,
        ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
            Ok(pi_ecs_core::access::AccessTree::Together(Vec::new()))
        }
        fn ordering_access(
            _: &Self::State,
            _: &pi_ecs_core::access::OrderingArchetypes,
        ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
            Ok(Vec::new())
        }
        fn begin(_: &mut Self::State) {}
        unsafe fn fetch<'r>(_: &'r mut Self::State, _: &'r mut (), _: SystemRunContext<'r>) {}
        fn finish(_: &mut Self::State, _: (), _: &SystemRunContext<'_>) {}
        fn unregister(
            state: Self::State,
            registration: &mut ParamRegistration<'_>,
        ) -> Result<(), EcsError> {
            registration.access().unregister_producer::<u32>(state)
        }
    }
    struct ObservedFunction(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl SystemFunction<()> for ObservedFunction {
        type Param = Producer;
        fn call<'r>(&mut self, _: ()) {
            let previous = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_ne!(previous, usize::MAX, "用户函数失败");
        }
    }
    // Seam: 真实 System::prepare，异源拒绝不破坏同源准备，注销仍清偿生产者。
    #[test]
    fn rc_system_prepare_preserves_identity_and_refreshes_revision() {
        use crate::system::System;
        let mut world = World::new().unwrap();
        world.register_multi_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        let mut other = World::new().unwrap();
        other.finish_registration().unwrap();
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut system = FunctionSystem {
            matched_revision: None,
            prepared_access: None,
            signature: std::marker::PhantomData::<fn()>,
            id: SystemId(7),
            function: ObservedFunction(calls.clone()),
            state: FunctionSystemState::Uninitialized(crate::system_builder::ParamConfig::new(23)),
            last_run: None,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            world.schedule_scope(|world| {
                assert!(system.prepare(world).is_err());
                world.parameter_registration_scope(|access| {
                    system.initialize(&mut ParamRegistration::new(access))
                })?;
                assert!(system.prepare(world)?.is_empty());
                assert!(system.prepared_access.is_some());
                assert!(matches!(system.access_tree()?, pi_ecs_core::access::AccessTree::Together(items) if items.is_empty()));
                let logical = pi_ecs_core::access::OrderingArchetypes { components: Box::new([]) };
                assert!(system.ordering_access(&logical)?.is_empty());
                let revision = system.matched_revision;
                assert!(system.prepare(&mut other).is_err());
                assert_eq!(system.matched_revision, revision);
                assert!(system.prepared_access.is_some());
                assert!(system.prepare(world)?.is_empty());
                assert!(system.last_run.is_none());
                let tick = unsafe { world.next_system_tick()? };
                assert!(unsafe { system.run_prepared(&mut other, tick) }.is_err());
                assert!(system.last_run.is_none());
                unsafe { system.run_prepared(world, tick)?; }
                assert_eq!(system.last_run, Some(tick));
                let next = unsafe { world.next_system_tick()? };
                unsafe { system.run_prepared(world, next)?; }
                assert_eq!(system.last_run, Some(next));
                assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
                let declaration = pi_ecs_core::access::AccessTree::Together(Vec::new());
                let version = pi_ecs_core::unsafe_world_cell::UnsafeWorldCell::from_ref(world).archetype_revision().0;
                unsafe {
                    world.system_scope(&declaration, version, |access| {
                        system.run(SystemRunContext::new(access, pi_ecs_core::change_detection::ChangeWindow {
                            last_run: None,
                            this_run: next,
                        }).unwrap());
                    })?;
                }
                assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
                assert_eq!(system.last_run, Some(next));
                Ok(())
            }).unwrap();
        }));
        if result.is_ok() {
            let previous_run = system.last_run;
            calls.store(usize::MAX, std::sync::atomic::Ordering::SeqCst);
            let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                world
                    .schedule_scope(|world| {
                        let tick = unsafe { world.next_system_tick()? };
                        unsafe { system.run_prepared(world, tick) }
                    })
                    .unwrap();
            }));
            assert!(failed.is_err());
            assert_eq!(system.last_run, previous_run);
            assert!(world.schedule_scope(|_| Ok(())).is_err());
        }
        world
            .parameter_unregistration_scope(|access| {
                system.unregister(&mut ParamRegistration::new(access))
            })
            .unwrap();
        assert!(system.access_tree().is_err());
        world.close().unwrap();
        other.close().unwrap();
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }
    struct Function;
    impl SystemFunction<()> for Function {
        type Param = Producer;
        fn call<'r>(&mut self, _: ()) {}
    }
    #[test]
    fn schedule_unregister_releases_registered_producers_before_world_close() {
        use crate::system::System;
        let mut world = World::new().unwrap();
        world.register_multi_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        let mut system = FunctionSystem {
            matched_revision: None,
            prepared_access: None,
            signature: std::marker::PhantomData::<fn()>,
            id: SystemId(2),
            function: Function,
            state: FunctionSystemState::Uninitialized(crate::system_builder::ParamConfig::new(19)),
            last_run: None,
        };
        world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|access| {
                    system.initialize(&mut ParamRegistration::new(access))
                })
            })
            .unwrap();
        let mut stage =
            crate::stage::Stage::new(crate::stage::StageId(1), crate::executor::ExecutorId(0));
        stage.add_system(system).unwrap();
        let mut schedule = crate::schedule::Schedule::new();
        schedule
            .add_executor(
                crate::executor::Executor::new(crate::executor::ExecutionMode::Serial).unwrap(),
            )
            .unwrap();
        schedule.add_stage(stage).unwrap();
        let mut other = World::new().unwrap();
        other.finish_registration().unwrap();
        assert!(schedule
            .stage_mut(crate::stage::StageId(1))
            .unwrap()
            .unregister_system(SystemId(2), &mut other)
            .is_err());
        assert!(schedule.unregister(&mut other).is_err());
        assert_eq!(
            world
                .read_scope(|pin| pin.multi_resource::<u32>().unwrap().len())
                .unwrap(),
            1
        );
        other.close().unwrap();
        schedule.unregister(&mut world).unwrap();
        schedule.unregister(&mut world).unwrap();
        assert!(world
            .read_scope(|pin| pin.multi_resource::<u32>().unwrap().is_empty())
            .unwrap());
        world.close().unwrap();
    }
    #[test]
    fn system_unregister_cross_world_preserves_producer_and_same_world_is_idempotent() {
        use crate::system::System;
        let mut world = World::new().unwrap();
        world.register_multi_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        let mut other = World::new().unwrap();
        other.finish_registration().unwrap();
        let mut system = FunctionSystem {
            matched_revision: None,
            prepared_access: None,
            signature: std::marker::PhantomData::<fn()>,
            id: SystemId(1),
            function: Function,
            state: FunctionSystemState::Uninitialized(crate::system_builder::ParamConfig::new(7)),
            last_run: None,
        };
        world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|access| {
                    let mut registration = ParamRegistration::new(access);
                    system.initialize(&mut registration)?;
                    system.initialize(&mut registration)
                })
            })
            .unwrap();
        assert!(other
            .parameter_unregistration_scope(
                |access| system.unregister(&mut ParamRegistration::new(access))
            )
            .is_err());
        assert_eq!(
            world
                .read_scope(|pin| pin
                    .multi_resource::<u32>()
                    .unwrap()
                    .iter()
                    .copied()
                    .collect::<Vec<_>>())
                .unwrap(),
            [7]
        );
        world
            .parameter_unregistration_scope(|access| {
                system.unregister(&mut ParamRegistration::new(access))
            })
            .unwrap();
        world
            .parameter_unregistration_scope(|access| {
                system.unregister(&mut ParamRegistration::new(access))
            })
            .unwrap();
        assert!(world
            .read_scope(|pin| pin.multi_resource::<u32>().unwrap().is_empty())
            .unwrap());
        world.close().unwrap();
        other.close().unwrap();
    }
}

#[cfg(feature = "rc")]
impl<F: crate::system::SystemFunction<M, Param = P>, P: crate::system_param::SystemParam, M>
    crate::system::FunctionSystem<F, P, M>
{
    pub(super) fn run_prepared_inner(
        &mut self,
        _world: &mut pi_ecs_core::world::World,
        _this_run: pi_ecs_core::change_detection::ChangeTick,
    ) -> Result<(), pi_ecs_core::error::EcsError> {
        let FunctionSystemState::Initialized {
            world_identity,
            state,
            prepared: true,
        } = &mut self.state
        else {
            return Err(system_error("系统没有有效准备"));
        };
        let cell = pi_ecs_core::unsafe_world_cell::UnsafeWorldCell::from_ref(_world);
        if *world_identity != cell.id() {
            return Err(system_error("系统来自其他 World"));
        }
        let version = self
            .matched_revision
            .ok_or_else(|| system_error("系统缺少匹配版本"))?
            .0;
        let declaration = self
            .prepared_access
            .as_ref()
            .ok_or_else(|| system_error("系统缺少完整声明"))?;
        let window = pi_ecs_core::change_detection::ChangeWindow {
            last_run: self.last_run,
            this_run: _this_run,
        };
        let function = &mut self.function;
        // SAFETY: 调用方持唯一串行运行权；完整声明保活至参数及 Pending 清理结束。
        unsafe {
            _world.system_scope(declaration, version, |access| {
                let context = SystemRunContext::new(access, window).expect("系统上下文无效");
                execute_parameter::<F, P, M>(function, state, context);
            })?;
        }
        self.last_run = Some(_this_run);
        Ok(())
    }
}
impl<'w> crate::system::SystemRunContext<'w> {
    pub(super) fn into_parts_inner(
        self,
    ) -> (
        pi_ecs_core::access::RunAccess<'w>,
        pi_ecs_core::change_detection::ChangeWindow,
    ) {
        (self.access, self.window)
    }
    pub(super) fn child_inner(
        &self,
        _position: usize,
    ) -> Result<crate::system::SystemRunContext<'w>, pi_ecs_core::error::EcsError> {
        // 先由安全短借入口确认 Together 种类和位置，避免把 Exclusive 暴露给安全 child。
        self.access.child(_position)?;
        // SAFETY: 上面已确认 Together；这里只派生原声明上的描述，不兑现业务引用。
        // 实际取参仍受 SystemParam::fetch 的单位置/父子互斥协议约束。
        let access = unsafe { self.access.context_child(_position)? };
        Self::new(access, self.window)
    }
    pub(super) fn exclusive_child_inner(
        &self,
        _position: usize,
    ) -> Result<crate::system::SystemRunContext<'w>, pi_ecs_core::error::EcsError> {
        // SAFETY: 仅派生描述，调用者通过 exclusive_child 的 unsafe 契约
        // 保证 ParamSet 成员短借及兄弟结果/物理缓存已结束。
        let access = unsafe { self.access.context_child(_position)? };
        // context_child 已拒绝叶子和越界；相同位置能安全 child 则是 Together。
        if self.access.child(_position).is_ok() {
            return Err(system_error("Exclusive 子授权不能来自 Together"));
        }
        Self::new(access, self.window)
    }
}
use crate::{
    param_composition::ParamAccess,
    system::{FunctionSystem, FunctionSystemState, SystemFunction, SystemId, SystemRunContext},
    system_param::{ParamRegistration, SystemParam},
};
use pi_ecs_core::{access::ResolvedAccess, error::EcsError};
fn execute_parameter<F: SystemFunction<M, Param = P>, P: SystemParam, M>(
    function: &mut F,
    state: &mut P::State,
    context: SystemRunContext<'_>,
) {
    let mut pending = P::begin(state);
    // SAFETY: context 仅含非拥有的授权描述与 Copy 窗口；复制不另开会话。
    // 原描述只用于结果全部结束后的 finish，不重复 fetch，不与子结果同时兑现。
    // 两份描述均被外层同一系统会话保活，不复制声明树、World 或业务值。
    let fetch_context = unsafe { std::ptr::read(&context) };
    let item = unsafe { P::fetch(state, &mut pending, fetch_context) };
    function.call(item);
    P::finish(state, pending, &context);
}
#[derive(Debug)]
struct ForeignUnregistration;

impl std::fmt::Display for ForeignUnregistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("注销前的 World 身份校验失败")
    }
}

#[cfg(feature = "rc")]
impl SystemRunContext<'_> {
    pub(crate) fn is_foreign_unregistration(error: &EcsError) -> bool {
        error.downcast_ref::<ForeignUnregistration>().is_some()
    }
}

fn system_error(message: &'static str) -> EcsError {
    pi_ecs_core::error::classified_error(pi_ecs_core::error::ErrorKind::InvalidState, message)
}

impl<F, P: SystemParam, Marker> FunctionSystem<F, P, Marker> {
    pub(crate) fn id_inner(system: &FunctionSystem<F, P, Marker>) -> SystemId {
        system.id
    }
}

impl<F: SystemFunction<Marker, Param = P>, P: SystemParam, Marker> FunctionSystem<F, P, Marker> {
    pub(crate) fn prepare_inner(
        system: &mut FunctionSystem<F, P, Marker>,
        world: pi_ecs_core::world::WorldRef<'_>,
    ) -> Result<Vec<ResolvedAccess>, EcsError> {
        let FunctionSystemState::Initialized {
            world_identity,
            state,
            prepared,
        } = &mut system.state
        else {
            return Err(system_error("系统尚未初始化或已注销"));
        };
        if *world_identity != pi_ecs_core::unsafe_world_cell::UnsafeWorldCell::from_ref(world).id()
        {
            return Err(system_error("系统来自其他 World"));
        }
        *prepared = false;
        system.prepared_access = None;
        #[cfg(feature = "rc")]
        let access = P::prepare(state, &mut *world)?;
        #[cfg(not(feature = "rc"))]
        let access = P::prepare(state, world)?;
        #[cfg(feature = "rc")]
        let declaration = P::declaration(state, &mut *world)?;
        #[cfg(not(feature = "rc"))]
        let declaration = P::declaration(state, world)?;
        fn leaves<'a>(
            tree: &'a pi_ecs_core::access::AccessTree,
            output: &mut Vec<&'a pi_ecs_core::access::AccessDeclaration>,
        ) {
            use pi_ecs_core::access::AccessTree;
            match tree {
                AccessTree::Leaf(leaf) => output.push(leaf),
                AccessTree::Together(items) | AccessTree::Exclusive(items) => {
                    for item in items {
                        leaves(item, output);
                    }
                }
            }
        }
        fn validate(tree: &pi_ecs_core::access::AccessTree) -> Result<(), EcsError> {
            use pi_ecs_core::access::AccessTree;
            match tree {
                AccessTree::Leaf(_) => Ok(()),
                AccessTree::Together(items) | AccessTree::Exclusive(items) => {
                    for item in items {
                        validate(item)?;
                    }
                    if matches!(tree, AccessTree::Together(_)) {
                        for (index, item) in items.iter().enumerate() {
                            let mut left = Vec::new();
                            leaves(item, &mut left);
                            for other in &items[index + 1..] {
                                let mut right = Vec::new();
                                leaves(other, &mut right);
                                if left.iter().any(|a| right.iter().any(|b| a.conflicts(b))) {
                                    return Err(system_error("同时使用的参数访问冲突"));
                                }
                            }
                        }
                    }
                    Ok(())
                }
            }
        }
        fn flatten(access: ParamAccess, output: &mut Vec<ResolvedAccess>) {
            match access {
                ParamAccess::Leaf(items) => output.extend(items),
                ParamAccess::Together(items) | ParamAccess::Exclusive(items) => {
                    for item in items {
                        flatten(item, output);
                    }
                }
            }
        }
        validate(&declaration)?;
        let mut resolved = Vec::new();
        flatten(access, &mut resolved);
        system.matched_revision = Some(
            pi_ecs_core::unsafe_world_cell::UnsafeWorldCell::from_ref(world).archetype_revision(),
        );
        system.prepared_access = Some(declaration);
        *prepared = true;
        Ok(resolved)
    }
}

impl<F: SystemFunction<Marker, Param = P>, P: SystemParam, Marker> FunctionSystem<F, P, Marker> {
    pub(crate) fn run_inner(
        system: &mut FunctionSystem<F, P, Marker>,
        context: SystemRunContext<'_>,
    ) {
        let FunctionSystemState::Initialized {
            world_identity,
            state,
            prepared: true,
        } = &mut system.state
        else {
            panic!("系统没有有效准备");
        };
        assert_eq!(
            *world_identity,
            context.access.world_cell().id(),
            "系统来自其他 World"
        );
        assert!(system.prepared_access.is_some(), "系统缺少完整声明");
        let context = SystemRunContext {
            access: context.access,
            window: pi_ecs_core::change_detection::ChangeWindow {
                last_run: system.last_run,
                this_run: context.window.this_run,
            },
        };
        execute_parameter::<F, P, Marker>(&mut system.function, state, context);
    }
}

impl<F: SystemFunction<Marker, Param = P>, P: SystemParam, Marker> FunctionSystem<F, P, Marker> {
    pub(crate) fn unregister_inner(
        system: &mut FunctionSystem<F, P, Marker>,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        if let FunctionSystemState::Initialized { world_identity, .. } = &system.state {
            if *world_identity != registration.access().world_id() {
                return Err(system_error("系统来自其他 World").attach(ForeignUnregistration));
            }
        }
        system.prepared_access = None;
        system.matched_revision = None;
        let previous = std::mem::replace(&mut system.state, FunctionSystemState::Unregistered);
        match previous {
            FunctionSystemState::Initialized { state, .. } => P::unregister(state, registration),
            _ => Ok(()),
        }
    }
}

impl<F, P: crate::system_param::SystemParam, M> FunctionSystem<F, P, M> {
    pub(super) fn access_tree_inner(&self) -> Result<&pi_ecs_core::access::AccessTree, EcsError> {
        if !matches!(
            self.state,
            FunctionSystemState::Initialized { prepared: true, .. }
        ) {
            return Err(system_error("系统没有有效准备"));
        }
        self.prepared_access
            .as_ref()
            .ok_or_else(|| system_error("系统缺少完整声明"))
    }
}

impl<F, P: crate::system_param::SystemParam, M> FunctionSystem<F, P, M> {
    pub(super) fn ordering_access_inner(
        &self,
        _logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        self.access_tree_inner()?;
        let FunctionSystemState::Initialized { state, .. } = &self.state else {
            return Err(system_error("系统尚未初始化或已注销"));
        };
        P::ordering_access(state, _logical)
    }
}

impl<F: SystemFunction<M, Param = P>, P: SystemParam, M> FunctionSystem<F, P, M> {
    pub(super) fn initialize_inner(
        &mut self,
        _registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        let world_identity = _registration.access().world_id();
        match &self.state {
            FunctionSystemState::Initialized {
                world_identity: bound,
                ..
            } => {
                return if *bound == world_identity {
                    Ok(())
                } else {
                    Err(system_error("系统来自其他 World"))
                }
            }
            FunctionSystemState::Uninitialized(_) => {}
            _ => return Err(system_error("系统初始化已失败或已注销")),
        }
        let previous =
            std::mem::replace(&mut self.state, FunctionSystemState::InitializationFailed);
        let FunctionSystemState::Uninitialized(config) = previous else {
            unreachable!()
        };
        let state = _registration.initialize_system::<P>(self.id, config.value)?;
        self.state = FunctionSystemState::Initialized {
            world_identity,
            state,
            prepared: false,
        };
        Ok(())
    }
}

impl<'w> SystemRunContext<'w> {
    pub(crate) fn context_new(
        _access: pi_ecs_core::access::RunAccess<'w>,
        _window: pi_ecs_core::change_detection::ChangeWindow,
    ) -> Result<crate::system::SystemRunContext<'w>, EcsError> {
        Ok(SystemRunContext {
            access: _access,
            window: _window,
        })
    }
}
