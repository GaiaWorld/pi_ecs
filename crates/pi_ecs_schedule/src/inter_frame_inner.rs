//! 帧间和阶段入口数据流的登记、计划与串行执行。
use super::*;
use crate::system::SystemInputKind;
use pi_ecs_core::error::{classified_error, ErrorKind};
use std::sync::atomic::{AtomicU64, Ordering};

fn flow_error(message: &'static str) -> EcsError {
    classified_error(ErrorKind::InvalidState, message)
}

#[cfg(test)]
#[test]
fn schedule_unregister_stops_world_on_entry_configuration_drop_panic() {
    use crate::{
        executor::{ExecutionMode, Executor},
        schedule::Schedule,
        stage::{Stage, StageId},
        system_builder::SystemBuilder,
    };
    use std::{cell::Cell, rc::Rc};

    struct Capture(Rc<Cell<usize>>, bool);
    impl Drop for Capture {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            if self.1 {
                std::panic::panic_any("entry configuration drop");
            }
        }
    }

    let mut world = World::new().unwrap();
    world.finish_registration().unwrap();
    let mut schedule = Schedule::new();
    let executor = schedule
        .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
        .unwrap();
    schedule
        .add_stage(Stage::new(StageId(0), executor))
        .unwrap();
    let drops = Rc::new(Cell::new(0));
    let mut flow = SystemFlow::new();
    // 直接组装两个尚未初始化的系统，避免经过非空图的静态组图桩。
    for id in 0..2 {
        let capture = Capture(drops.clone(), id == 1);
        let system = SystemBuilder::new(move || {
            let _ = &capture;
        })
        .build(SystemId(id))
        .unwrap();
        flow.systems.push(RegisteredSystem {
            id: SystemId(id),
            system: Box::new(system),
        });
    }
    schedule.stages[0].entry_flow = Some(flow);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        schedule.unregister(&mut world)
    }))
    .unwrap_err();
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"entry configuration drop")
    );
    assert!(world.is_stopped());
    assert_eq!(drops.get(), 1);
    assert!(schedule.stages[0].entry_flow.is_some());
    let flow = schedule.stages[0].entry_flow.as_ref().unwrap();
    assert!(flow.validate_stage_install().is_err());
    let mut other = World::new().unwrap();
    other.finish_registration().unwrap();
    assert!(schedule.unregister(&mut other).is_err());
    assert_eq!(drops.get(), 1);
    assert!(!other.is_stopped());
    other.close().unwrap();
    schedule.unregister(&mut world).unwrap();
    assert_eq!(drops.get(), 2);
    assert!(schedule.stages[0].entry_flow.is_none());
    world.close().unwrap();
}

#[cfg(test)]
#[allow(dead_code)]
fn check_stage_batch_registration(stage: &mut crate::stage::Stage) -> Result<(), EcsError> {
    use crate::{
        resource_param::ResMut,
        system::{PortFunction, PortSignature, SharedInput, UnitInput},
        system_builder::SystemBuilder,
    };

    struct Expired(Vec<u32>);
    fn poll(_: (), mut timers: ResMut<'_, Vec<u32>>) -> Option<Expired> {
        if timers.is_empty() {
            None
        } else {
            Some(Expired(std::mem::take(&mut *timers)))
        }
    }
    fn consume(batch: &Expired, mut count: ResMut<'_, usize>) -> Option<()> {
        *count += batch.0.len();
        None
    }
    fn check_call<'a>(batch: &'a Expired, value: ResMut<'a, usize>) {
        use crate::system::{SystemFunction, SystemInput};
        let mut adapter = PortFunction::<_, SharedInput<Expired>, ()>::new(consume);
        let _ = <_ as SystemFunction<PortSignature<ResMut<'static, usize>>>>::call_with_ports(
            &mut adapter,
            SystemInput::Shared(batch),
            value,
        );
    }
    let mut flow = SystemFlow::new();
    flow.register::<(), _, _>(
        SystemBuilder::<_, PortSignature<ResMut<'static, Vec<u32>>>>::with_config(
            PortFunction::<_, UnitInput, Expired>::new(poll),
            (),
        ),
    )?;
    flow.register_shared(
        SystemBuilder::<_, PortSignature<ResMut<'static, usize>>>::with_config(
            PortFunction::<_, SharedInput<Expired>, ()>::new(consume),
            (),
        ),
    )?;
    stage
        .set_entry_flow(flow)
        .map_err(|rejected| rejected.error)
}

fn topological_order(
    included: &[bool],
    successors: &[Vec<usize>],
    cycle: &'static str,
) -> Result<Vec<usize>, EcsError> {
    use graph_core::{GraphInsert, NodeAccess, NodeRef};
    let mut graph = graph_storage::sparse::SparseGraph::<usize, ()>::new();
    let mut nodes = Vec::with_capacity(included.len());
    for (slot, &included) in included.iter().enumerate() {
        nodes.push(if included {
            Some(graph.insert_node(slot).expect("排序节点可登记"))
        } else {
            None
        });
    }
    for (source, successors) in successors.iter().enumerate() {
        if let Some(from) = nodes[source] {
            for &target in successors {
                if let Some(to) = nodes[target] {
                    graph.insert_edge(from, to, ()).expect("排序边可登记");
                }
            }
        }
    }
    let order = graph_algo::topo::toposort(&graph).map_err(|_| flow_error(cycle))?;
    Ok(order
        .into_iter()
        .map(|node| *graph.node(node).expect("排序节点存在").weight())
        .collect())
}

impl SegmentInputs {
    fn new(size: usize) -> Self {
        Self {
            pending: (0..size)
                .map(|_| std::collections::VecDeque::new())
                .collect(),
            notice_ready: vec![false; size],
        }
    }

    fn hand_off(&mut self, key: TargetKey, input: PendingInput) -> Result<(), EcsError> {
        self.pending
            .get_mut(key.slot)
            .ok_or_else(|| flow_error("输出计划包含无效系统槽"))?
            .push_back(input);
        Ok(())
    }
}

struct SegmentPlan<'a> {
    flow: Option<u64>,
    order: &'a [TargetKey],
    output: &'a [Option<OutputEdges>],
}

impl SegmentPlan<'_> {
    fn run(
        &self,
        world: &mut World,
        systems: &mut [RegisteredSystem],
        notices: &mut [RegisteredNotice],
        mut inputs: SegmentInputs,
        first: &mut Option<EcsError>,
    ) -> Result<(), EcsError> {
        use crate::system::SystemInput;
        let mut visited = vec![false; systems.len()];
        for key in self.order {
            if Some(key.flow) != self.flow
                || key.slot >= systems.len()
                || self.output.get(key.slot).is_none()
                || std::mem::replace(&mut visited[key.slot], true)
            {
                return Err(flow_error("入口执行计划包含无效系统槽"));
            }
        }
        for key in self.order {
            let slot = key.slot;
            let system = &mut systems[slot].system;
            if std::mem::take(&mut inputs.notice_ready[slot]) && inputs.pending[slot].is_empty() {
                inputs.pending[slot].push_back(PendingInput::Unit);
            }
            while let Some(input) = inputs.pending[slot].pop_front() {
                if let Err(error) = system.prepare(&mut *world) {
                    if world.is_stopped() {
                        return Err(error);
                    }
                    first.get_or_insert(error);
                    continue;
                }
                // SAFETY: this segment has the unique root driver and no live parameter references.
                let tick = unsafe { world.next_system_tick()? };
                let output = match input {
                    // SAFETY: prepare succeeded for this World at the current layout and tick.
                    PendingInput::Unit => unsafe {
                        system.run_with_input(world, tick, SystemInput::Unit)?
                    },
                    // SAFETY: same prepared system, with an owned payload from the segment.
                    PendingInput::Owned(value) => unsafe {
                        system.run_with_input(world, tick, SystemInput::Owned(value))?
                    },
                    // SAFETY: the upstream value is owned by this segment and stays live through the call.
                    PendingInput::Shared(value) => unsafe {
                        system.run_with_input(world, tick, SystemInput::Shared(value.as_ref()))?
                    },
                };
                if let Some(value) = output {
                    match &self.output[slot] {
                        Some(OutputEdges::Owned(target)) => {
                            inputs.hand_off(*target, PendingInput::Owned(value))?
                        }
                        Some(OutputEdges::Shared(targets)) => {
                            let shared: std::rc::Rc<dyn Any> = value.into();
                            for target in targets {
                                inputs.hand_off(*target, PendingInput::Shared(shared.clone()))?;
                            }
                        }
                        None => drop(value),
                    }
                }
                acknowledge(world, notices, Some(&mut inputs), false)?;
            }
        }
        if inputs.pending.iter().any(|queue| !queue.is_empty())
            || inputs.notice_ready.contains(&true)
        {
            return Err(flow_error("实际入口交付超出已发布计划"));
        }
        Ok(())
    }
}

fn acknowledge(
    world: &World,
    notices: &mut [RegisteredNotice],
    mut inputs: Option<&mut SegmentInputs>,
    removed_only: bool,
) -> Result<(), EcsError> {
    for notice in notices {
        let subscription = notice
            .subscription
            .as_mut()
            .ok_or_else(|| flow_error("通知路由没有已登记订阅"))?;
        if world.notifications().acknowledge_published(subscription)?
            && (!removed_only || notice.route.kind == NoticeKind::Removed)
        {
            if let Some(inputs) = inputs.as_deref_mut() {
                *inputs
                    .notice_ready
                    .get_mut(notice.target.slot)
                    .ok_or_else(|| flow_error("通知路由包含无效系统槽"))? = true;
            }
        }
    }
    Ok(())
}

impl QueueRoute {
    fn checked_status(
        &self,
        world: &World,
    ) -> Result<pi_ecs_core::command::CommandQueueStatus, EcsError> {
        let status = (self.status)(world)?;
        if self.receiver_claim.is_none() || status.receiver_claim != self.receiver_claim {
            return Err(flow_error("帧间队列的实际接收资格与绑定不符"));
        }
        Ok(status)
    }
}

#[cfg(test)]
#[test]
fn barrier_stop_preserves_its_diagnostic_over_prepare_and_root_errors() {
    use crate::{resource_param::Res, system_builder::SystemBuilder};

    let mut world = World::new().unwrap();
    world.register_resource::<u32>().unwrap();
    world.finish_registration().unwrap();
    let mut flow = SystemFlow::new();
    let entry = flow
        .register::<(), _, _>(SystemBuilder::new(|_: Res<'_, u32>| {
            panic!("missing resource must reject preparation");
        }))
        .unwrap();
    let prepare_error = flow.dispatch(&mut world, &entry, ()).unwrap_err();
    assert!(!world.is_stopped());
    let error = world
        .schedule_scope(|world| {
            flow.prepare_in_run(world)?;
            flow.run_round_with_barrier(world, entry.key, PendingInput::Unit, |world| {
                world.stop_in_run()?;
                Err(flow_error("injected destruction barrier diagnosis"))
            })
        })
        .unwrap_err();
    let diagnostic = format!("{error:?}");
    assert!(
        diagnostic.contains("injected destruction barrier diagnosis"),
        "{diagnostic}"
    );
    assert_ne!(diagnostic, format!("{prepare_error:?}"));
    assert!(world.is_stopped());
    assert!(matches!(flow.state, FlowState::Failed));
    flow.close(&mut world).unwrap();
    world.close().unwrap();
}

impl<I> InterFrameTarget<I> {
    pub(super) fn clone_inner(&self) -> Self {
        Self {
            key: self.key,
            input: PhantomData,
        }
    }
}

impl SystemFlow {
    pub(super) fn configuration_inner(&mut self) -> FlowConfig<'_> {
        FlowConfig { flow: self }
    }
    pub(super) fn new_inner() -> Self {
        Self {
            identity: None,
            world_identity: None,
            systems: Vec::new(),
            queues: Vec::new(),
            notices: Vec::new(),
            state: FlowState::Building,
        }
    }

    pub(super) fn register_inner<I: 'static, F, M>(
        &mut self,
        _builder: crate::system_builder::SystemBuilder<F, M>,
    ) -> Result<InterFrameTarget<I>, EcsError>
    where
        F: crate::system::SystemFunction<M> + 'static,
        F::Param: 'static,
        <F::Param as crate::system_param::SystemParam>::Config: 'static,
        M: 'static,
    {
        let key = self.register_system(_builder, |input| match input {
            SystemInputKind::Unit => TypeId::of::<I>() == TypeId::of::<()>(),
            SystemInputKind::Owned(id) => id == TypeId::of::<I>(),
            SystemInputKind::Shared(_) => false,
        })?;
        Ok(InterFrameTarget {
            key,
            input: PhantomData,
        })
    }
    fn register_system<F, M>(
        &mut self,
        builder: crate::system_builder::SystemBuilder<F, M>,
        accepts: impl FnOnce(SystemInputKind) -> bool,
    ) -> Result<TargetKey, EcsError>
    where
        F: crate::system::SystemFunction<M> + 'static,
        F::Param: 'static,
        <F::Param as crate::system_param::SystemParam>::Config: 'static,
        M: 'static,
    {
        self.require_building()?;
        let slot = self.systems.len();
        let id = SystemId(u64::try_from(slot).map_err(|_| flow_error("入口槽号耗尽"))?);
        let system = builder.build(id)?;
        if !accepts(system.ports().input) {
            return Err(flow_error("登记入口的输入端口不匹配"));
        }
        static NEXT_FLOW: AtomicU64 = AtomicU64::new(1);
        let flow = match self.identity {
            Some(flow) => flow,
            None => NEXT_FLOW
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .map_err(|_| flow_error("入口图身份耗尽"))?,
        };
        self.identity = Some(flow);
        self.systems.push(RegisteredSystem {
            id,
            system: Box::new(system),
        });
        Ok(TargetKey { flow, slot })
    }

    fn require_building(&self) -> Result<(), EcsError> {
        if !matches!(self.state, FlowState::Building) {
            return Err(flow_error("入口图已结束组装"));
        }
        Ok(())
    }

    pub(super) fn register_shared_inner<F, M>(
        &mut self,
        _builder: crate::system_builder::SystemBuilder<F, M>,
    ) -> Result<(), EcsError>
    where
        F: crate::system::SystemFunction<M> + 'static,
        F::Param: 'static,
        <F::Param as crate::system_param::SystemParam>::Config: 'static,
        M: 'static,
    {
        self.register_system(_builder, |input| {
            matches!(input, SystemInputKind::Shared(_))
        })?;
        Ok(())
    }
    pub(super) fn bind_queue_inner<C: Send + 'static>(
        &mut self,
        _target: &InterFrameTarget<()>,
    ) -> Result<(), EcsError> {
        self.require_building()?;
        self.require_unit(_target.key)?;
        let command = TypeId::of::<C>();
        if self
            .queues
            .iter()
            .any(|queue| queue.command == command || queue.consumer == _target.key)
            || self
                .notices
                .iter()
                .any(|notice| notice.target == _target.key)
        {
            return Err(flow_error("队列类型或目标已绑定"));
        }
        self.queues.push(QueueRoute {
            command,
            consumer: _target.key,
            receiver_claim: None,
            priority: QueuePriority::default(),
            status: World::command_queue_status::<C>,
        });
        Ok(())
    }
    fn target(&self, key: TargetKey) -> Result<&RegisteredSystem, EcsError> {
        if self.identity != Some(key.flow) {
            return Err(flow_error("目标不属于此入口图"));
        }
        self.systems
            .get(key.slot)
            .ok_or_else(|| flow_error("入口目标已失效"))
    }

    fn require_unit(&self, key: TargetKey) -> Result<(), EcsError> {
        if self.target(key)?.system.ports().input != SystemInputKind::Unit {
            return Err(flow_error("路由要求单位输入端口"));
        }
        Ok(())
    }

    pub(super) fn set_queue_priority_inner<C: Send + 'static>(
        &mut self,
        _priority: QueuePriority,
    ) -> Result<(), EcsError> {
        if !matches!(self.state, FlowState::Building | FlowState::Ready(_)) {
            return Err(flow_error("当前不能更新队列优先级"));
        }
        let queue = self
            .queues
            .iter_mut()
            .find(|queue| queue.command == TypeId::of::<C>())
            .ok_or_else(|| flow_error("帧间队列尚未绑定"))?;
        queue.priority = _priority;
        Ok(())
    }
    pub(super) fn wake_on_inner(
        &mut self,
        _route: NoticeRoute,
        _target: &InterFrameTarget<()>,
    ) -> Result<(), EcsError> {
        self.require_building()?;
        self.require_unit(_target.key)?;
        if self
            .queues
            .iter()
            .any(|queue| queue.consumer == _target.key)
        {
            return Err(flow_error("不能用通知启动已绑定队列入口"));
        }
        if !self
            .notices
            .iter()
            .any(|notice| notice.route == _route && notice.target == _target.key)
        {
            self.notices.push(RegisteredNotice {
                route: _route,
                target: _target.key,
                subscription: None,
            });
        }
        Ok(())
    }
    pub(super) fn validate_configuration_inner(&self, _stage_entry: bool) -> Result<(), EcsError> {
        if _stage_entry && (!self.queues.is_empty() || !self.notices.is_empty()) {
            return Err(flow_error("阶段入口不允许队列或通知路由"));
        }
        let (_, triggers) = self.output_edges()?;
        topological_order(
            &vec![true; self.systems.len()],
            &triggers,
            "入口图存在输出触发环",
        )?;
        Ok(())
    }

    fn output_edges(&self) -> Result<(Vec<Option<OutputEdges>>, Vec<Vec<usize>>), EcsError> {
        use std::collections::HashMap;
        let mut producers: HashMap<TypeId, Vec<usize>> = HashMap::new();
        for (slot, entry) in self.systems.iter().enumerate() {
            if let Some(output) = entry
                .system
                .ports()
                .output
                .filter(|id| *id != TypeId::of::<()>())
            {
                producers.entry(output).or_default().push(slot);
            }
        }
        let mut edges: Vec<Option<OutputEdges>> = (0..self.systems.len()).map(|_| None).collect();
        let mut triggers = vec![Vec::new(); self.systems.len()];
        for (slot, entry) in self.systems.iter().enumerate() {
            let (id, shared) = match entry.system.ports().input {
                SystemInputKind::Unit => continue,
                SystemInputKind::Owned(id) => (id, false),
                SystemInputKind::Shared(id) => (id, true),
            };
            let Some(sources) = producers.get(&id) else {
                continue;
            };
            let &[source] = sources.as_slice() else {
                return Err(flow_error("同一输入负载存在多个生产者"));
            };
            let target = TargetKey {
                flow: self.identity.expect("已登记图身份"),
                slot,
            };
            edges[source] = Some(match (edges[source].take(), shared) {
                (None, true) => OutputEdges::Shared(Box::new([target])),
                (None, false) => OutputEdges::Owned(target),
                (Some(OutputEdges::Shared(previous)), true) => {
                    let mut targets = previous.into_vec();
                    targets.push(target);
                    OutputEdges::Shared(targets.into_boxed_slice())
                }
                _ => return Err(flow_error("输出不能连接多个拥有端口或混用拥有与只读端口")),
            });
            triggers[source].push(slot);
        }
        Ok((edges, triggers))
    }

    pub(super) fn validate_stage_install_inner(&self) -> Result<(), EcsError> {
        if !matches!(self.state, FlowState::Building)
            || self.world_identity.is_some()
            || !self.queues.is_empty()
            || !self.notices.is_empty()
        {
            return Err(pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::InvalidState,
                "阶段入口只接受未绑定且无队列或通知路由的构建中图",
            ));
        }
        self.validate_configuration(true)
    }

    pub(super) fn prepare_in_run_inner(&mut self, _world: &mut World) -> Result<(), EcsError> {
        self.prepare(_world, false)
    }

    fn prepare(&mut self, world: &mut World, stage_entry: bool) -> Result<(), EcsError> {
        use crate::system_param::ParamRegistration;
        use pi_ecs_core::unsafe_world_cell::UnsafeWorldCell;
        let identity = UnsafeWorldCell::from_ref(&mut *world).id();
        if self.world_identity.is_some_and(|owner| owner != identity) {
            return Err(flow_error("入口图来自其他 World"));
        }
        match &self.state {
            FlowState::Building => {
                self.world_identity = Some(identity);
                self.state = FlowState::Failed;
                self.validate_configuration(stage_entry)?;
                for (slot, entry) in self.systems.iter_mut().enumerate() {
                    let queue = self
                        .queues
                        .iter_mut()
                        .find(|queue| queue.consumer.slot == slot);
                    let command = queue.as_ref().map(|queue| queue.command);
                    let claim = world.parameter_registration_scope(|mut access| {
                        access.restrict_receivers(command)?;
                        let mut registration = ParamRegistration::new(access);
                        entry.system.initialize(&mut registration)?;
                        Ok(registration.access().receiver_claim())
                    })?;
                    if let Some(queue) = queue {
                        queue.receiver_claim = claim;
                    }
                }
                world.parameter_registration_scope(|mut access| {
                    for notice in &mut self.notices {
                        notice.subscription = Some(
                            access.subscribe_notice(notice.route.component, notice.route.kind)?,
                        );
                    }
                    Ok(())
                })?;
                world.apply_tracking()?;
                for queue in &self.queues {
                    queue.checked_status(world)?;
                }
            }
            FlowState::Ready(plans) => {
                for queue in &self.queues {
                    if let Err(error) = queue.checked_status(world) {
                        return self.stop(world, error);
                    }
                }
                if plans.layout_revision
                    == UnsafeWorldCell::from_ref(&mut *world).archetype_revision()
                {
                    return Ok(());
                }
                self.state = FlowState::Failed;
            }
            _ => return Err(flow_error("入口图不可运行")),
        }
        self.state = FlowState::Ready(self.compile(world, stage_entry)?);
        Ok(())
    }

    fn compile(&self, world: &mut World, stage_entry: bool) -> Result<FlowPlans, EcsError> {
        use crate::stage_plan::{OrderingRelation, StagePlan};
        use pi_ecs_core::{
            access::{AccessMode, AccessTree},
            unsafe_world_cell::UnsafeWorldCell,
        };
        let (output_edges, mut triggers) = self.output_edges()?;
        let trees = self
            .systems
            .iter()
            .map(|entry| entry.system.planning_declaration(&mut *world))
            .collect::<Result<Vec<_>, _>>()?;
        fn visit(tree: &AccessTree, f: &mut impl FnMut(&pi_ecs_core::access::AccessDeclaration)) {
            match tree {
                AccessTree::Leaf(declaration) => f(declaration),
                AccessTree::Together(children) | AccessTree::Exclusive(children) => {
                    for child in children {
                        visit(child, f);
                    }
                }
            }
        }
        for (slot, tree) in trees.iter().enumerate() {
            visit(tree, &mut |declaration| {
                declaration.visit_notice_access(|access| {
                    if access.mode == AccessMode::Write {
                        for notice in &self.notices {
                            if notice.route.component == access.component
                                && notice.route.kind == access.kind
                                && !triggers[slot].contains(&notice.target.slot)
                            {
                                triggers[slot].push(notice.target.slot);
                            }
                        }
                    }
                });
            });
        }
        topological_order(
            &vec![true; self.systems.len()],
            &triggers,
            "入口图存在通知或输出触发环",
        )?;
        let tree_refs = trees.iter().collect::<Vec<_>>();
        let logical = StagePlan::ordering_archetypes(&mut *world, &tree_refs)?;
        let accesses = self
            .systems
            .iter()
            .map(|entry| entry.system.planning_ordering_access(&logical))
            .collect::<Result<Vec<_>, _>>()?;
        let plan = |roots: &[usize]| -> Result<Box<[TargetKey]>, EcsError> {
            let mut included = vec![false; self.systems.len()];
            let mut search = roots.to_vec();
            while let Some(slot) = search.pop() {
                if !std::mem::replace(&mut included[slot], true) {
                    search.extend(&triggers[slot]);
                }
            }
            let mut ordering = triggers.clone();
            for earlier in 0..included.len() {
                if !included[earlier] {
                    continue;
                }
                for later in earlier + 1..included.len() {
                    if !included[later] {
                        continue;
                    }
                    let relation =
                        StagePlan::strict_relation(&logical, &accesses[earlier], &accesses[later])
                            .map_err(|error| {
                                error.attach(format!(
                                    "系统 {:?} 与 {:?}",
                                    self.systems[earlier].id, self.systems[later].id
                                ))
                            })?;
                    let (from, to) = match relation {
                        OrderingRelation::Independent => continue,
                        OrderingRelation::EarlierFirst => (earlier, later),
                        OrderingRelation::LaterFirst => (later, earlier),
                    };
                    if !ordering[from].contains(&to) {
                        ordering[from].push(to);
                    }
                }
            }
            let order = topological_order(&included, &ordering, "入口触发与访问顺序无法同时满足")?;
            Ok(order
                .into_iter()
                .map(|slot| TargetKey {
                    flow: self.identity.expect("非空计划属于已登记图"),
                    slot,
                })
                .collect())
        };
        let mut by_entry = Vec::new();
        let mut stage = Box::<[TargetKey]>::default();
        let mut after_destruction = Box::<[TargetKey]>::default();
        if stage_entry {
            let roots = self
                .systems
                .iter()
                .enumerate()
                .filter_map(|(slot, entry)| {
                    (entry.system.ports().input == SystemInputKind::Unit).then_some(slot)
                })
                .collect::<Vec<_>>();
            stage = plan(&roots)?;
        } else {
            for (slot, entry) in self.systems.iter().enumerate() {
                by_entry.push(
                    if matches!(entry.system.ports().input, SystemInputKind::Shared(_)) {
                        Box::new([])
                    } else {
                        plan(&[slot])?
                    },
                );
            }
            let roots = self
                .notices
                .iter()
                .filter_map(|notice| {
                    (notice.route.kind == NoticeKind::Removed).then_some(notice.target.slot)
                })
                .collect::<Vec<_>>();
            after_destruction = plan(&roots)?;
        }
        Ok(FlowPlans {
            by_entry,
            stage_entry: stage,
            after_destruction,
            output_edges,
            layout_revision: UnsafeWorldCell::from_ref(world).archetype_revision(),
        })
    }

    fn stop<T>(&mut self, world: &mut World, error: EcsError) -> Result<T, EcsError> {
        self.state = FlowState::Failed;
        let _ = world.stop_in_run();
        Err(error)
    }
    pub(super) fn dispatch_inner<I: 'static>(
        &mut self,
        _world: &mut World,
        _target: &InterFrameTarget<I>,
        _input: I,
    ) -> Result<(), EcsError> {
        self.require_idle_world(_world)?;
        let key = _target.key;
        let input = match self.target(key)?.system.ports().input {
            SystemInputKind::Unit if TypeId::of::<I>() == TypeId::of::<()>() => PendingInput::Unit,
            SystemInputKind::Owned(id) if id == TypeId::of::<I>() => {
                PendingInput::Owned(Box::new(_input))
            }
            _ => return Err(flow_error("立即派发输入不匹配")),
        };
        if self.queues.iter().any(|queue| queue.consumer == key) {
            return Err(flow_error("不能立即派发已绑定队列入口"));
        }
        _world.schedule_scope(|world| {
            self.prepare_in_run(world)?;
            self.run_round(world, key, input)
        })
    }

    fn require_idle_world(&self, world: &mut World) -> Result<(), EcsError> {
        if self.world_identity.is_some_and(|owner| {
            owner != pi_ecs_core::unsafe_world_cell::UnsafeWorldCell::from_ref(&mut *world).id()
        }) {
            return Err(flow_error("入口图来自其他 World"));
        }
        if !matches!(self.state, FlowState::Building | FlowState::Ready(_)) {
            return Err(flow_error("入口图不可运行"));
        }
        drop(world.pin()?);
        Ok(())
    }

    fn run_round(
        &mut self,
        world: &mut World,
        key: TargetKey,
        input: PendingInput,
    ) -> Result<(), EcsError> {
        self.run_round_with_barrier(world, key, input, |world| {
            // SAFETY: the root is Running and all system and segment parameter borrows ended.
            unsafe { world.flush_destruction_in_run() }
        })
    }

    fn run_round_with_barrier(
        &mut self,
        world: &mut World,
        key: TargetKey,
        input: PendingInput,
        barrier: impl FnOnce(&mut World) -> Result<(), EcsError>,
    ) -> Result<(), EcsError> {
        let mut first = None;
        match self.run_round_segments(world, key, input, &mut first, barrier) {
            Err(error) => self.stop(world, error),
            Ok(()) => first.map_or(Ok(()), Err),
        }
    }

    fn run_round_segments(
        &mut self,
        world: &mut World,
        key: TargetKey,
        input: PendingInput,
        first: &mut Option<EcsError>,
        barrier: impl FnOnce(&mut World) -> Result<(), EcsError>,
    ) -> Result<(), EcsError> {
        let FlowState::Ready(plans) = &self.state else {
            return Err(flow_error("帧间入口图没有可用计划"));
        };
        let order = plans
            .by_entry
            .get(key.slot)
            .ok_or_else(|| flow_error("帧间入口计划槽无效"))?;
        acknowledge(world, &mut self.notices, None, false)?;
        let mut inputs = SegmentInputs::new(self.systems.len());
        inputs.hand_off(key, input)?;
        let segment = SegmentPlan {
            flow: self.identity,
            order,
            output: &plans.output_edges,
        };
        segment.run(world, &mut self.systems, &mut self.notices, inputs, first)?;
        barrier(world)?;
        let mut inputs = SegmentInputs::new(self.systems.len());
        acknowledge(world, &mut self.notices, Some(&mut inputs), true)?;
        let segment = SegmentPlan {
            flow: self.identity,
            order: &plans.after_destruction,
            output: &plans.output_edges,
        };
        segment.run(world, &mut self.systems, &mut self.notices, inputs, first)
    }
    pub(super) fn run_next_ready_inner(
        &mut self,
        _world: &mut World,
        _budget: FrameBudget,
    ) -> Result<ReadyRun, EcsError> {
        self.require_idle_world(_world)?;
        if _budget.frame_at.saturating_duration_since(_budget.now) <= _budget.reserve {
            return Ok(ReadyRun::YieldToFrame);
        }
        _world.schedule_scope(|world| {
            self.prepare_in_run(world)?;
            let mut selected: Option<(QueuePriority, TargetKey)> = None;
            for queue in &self.queues {
                let status = match queue.checked_status(world) {
                    Ok(status) => status,
                    Err(error) => return self.stop(world, error),
                };
                if status.ready && selected.is_none_or(|(priority, _)| queue.priority > priority) {
                    selected = Some((queue.priority, queue.consumer));
                }
            }
            let Some((_, key)) = selected else {
                return Ok(ReadyRun::Idle);
            };
            self.run_round(world, key, PendingInput::Unit)?;
            Ok(ReadyRun::Ran)
        })
    }
    pub(super) fn run_stage_entry_inner(&mut self, _world: &mut World) -> Result<(), EcsError> {
        if matches!(self.state, FlowState::Building)
            && self.world_identity.is_none()
            && self.systems.is_empty()
            && self.queues.is_empty()
            && self.notices.is_empty()
        {
            return Ok(());
        }
        self.prepare(_world, true)?;
        let FlowState::Ready(plans) = &self.state else {
            return Err(flow_error("阶段入口图没有可用计划"));
        };
        let mut inputs = SegmentInputs::new(self.systems.len());
        for (slot, entry) in self.systems.iter().enumerate() {
            if entry.system.ports().input == SystemInputKind::Unit {
                inputs.pending[slot].push_back(PendingInput::Unit);
            }
        }
        let mut first = None;
        let segment = SegmentPlan {
            flow: self.identity,
            order: &plans.stage_entry,
            output: &plans.output_edges,
        };
        if let Err(error) = segment.run(_world, &mut self.systems, &mut [], inputs, &mut first) {
            return self.stop(_world, error);
        }
        first.map_or(Ok(()), Err)
    }
    pub(super) fn is_closed_inner(&self) -> bool {
        matches!(self.state, FlowState::Closed)
    }

    pub(super) fn close_inner(&mut self, world: &mut World) -> Result<(), EcsError> {
        if self.is_closed() {
            return Ok(());
        }
        let identity = pi_ecs_core::unsafe_world_cell::UnsafeWorldCell::from_ref(&mut *world).id();
        if self.world_identity.is_some_and(|owner| owner != identity) {
            return Err(pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::InvalidState,
                "关闭入口图必须使用所属 World",
            ));
        }
        world.parameter_unregistration_scope(|_access| {
            if matches!(
                self.state,
                FlowState::Building | FlowState::ClosingUnregistered
            ) {
                self.world_identity = Some(identity);
                self.state = FlowState::ClosingUnregistered;
                while let Some(system) = self.systems.pop() {
                    drop(system);
                }
                self.queues.clear();
                self.notices.clear();
                self.state = FlowState::Closed;
                return Ok(());
            }
            self.state = FlowState::Failed;
            let mut registration = crate::system_param::ParamRegistration::new(_access);
            let mut first = None;
            while let Some(mut entry) = self.systems.pop() {
                if let Err(error) = entry.system.unregister(&mut registration) {
                    first.get_or_insert(error);
                }
            }
            while let Some(mut notice) = self.notices.pop() {
                if let Some(subscription) = notice.subscription.take() {
                    if let Err(error) = registration.access().unsubscribe_notice(subscription) {
                        first.get_or_insert(error);
                    }
                }
            }
            self.queues.clear();
            self.state = FlowState::Closed;
            first.map_or(Ok(()), Err)
        })
    }
}
