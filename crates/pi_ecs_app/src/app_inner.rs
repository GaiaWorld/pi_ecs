//! App 生命周期配对芯。
#[cfg(all(test, feature = "rc"))]
mod close_tests {
    use crate::app::App;
    use pi_ecs_core::world::World;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Child(Arc<AtomicUsize>);

    #[test]
    fn app_rejects_live_receiver_but_accepts_a_retained_sender() {
        use pi_ecs_core::command::CommandCapacity;
        let mut world = World::new().unwrap();
        world
            .register_command::<u32>(CommandCapacity::Unbounded)
            .unwrap();
        let sender = world.command_sender::<u32>().unwrap();
        let receiver = world.command_receiver::<u32>().unwrap();
        let rejected = match App::new(world) {
            Ok(_) => panic!("持有外部接收资格时不能接管"),
            Err(rejected) => rejected,
        };
        drop(receiver);
        let mut app = App::new(rejected.value).unwrap();
        assert!(sender.send(9).is_ok());
        assert!(app.world().command_queue_status::<u32>().unwrap().ready);
        app.close().unwrap();
    }

    #[test]
    fn app_rejects_world_with_host_notice_and_returns_it_for_cleanup() {
        use pi_ecs_core::{
            component::{Component, PlainAccess, StoragePolicy},
            notification::NoticeKind,
        };
        struct Tag;
        impl Component for Tag {
            type Access = PlainAccess;
        }
        let mut world = World::new().unwrap();
        let component = world
            .register_component::<Tag>(StoragePolicy::SparseOnly)
            .unwrap();
        let subscription = world
            .subscribe_notice(component, NoticeKind::Changed)
            .unwrap();
        let mut world = match App::new(world) {
            Ok(_) => panic!("未注销的宿主订阅不能移交给 App"),
            Err(rejected) => rejected.value,
        };
        world.unsubscribe_notice(subscription).unwrap();
        App::new(world).unwrap().close().unwrap();
    }

    #[test]
    fn rejects_used_schedule_and_returns_its_receiver_responsibility() {
        use crate::app::ScheduleLabel;
        use pi_ecs_core::command::{CommandCapacity, CommandRead};
        use pi_ecs_schedule::{
            executor::{ExecutionMode, Executor},
            schedule::Schedule,
            stage::{Stage, StageId},
            system::SystemId,
            system_builder::SystemBuilder,
        };
        let mut source = World::new().unwrap();
        source
            .register_command::<u32>(CommandCapacity::Unbounded)
            .unwrap();
        source.finish_registration().unwrap();
        let mut schedule = Schedule::new();
        let executor = schedule
            .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
            .unwrap();
        let mut stage = Stage::new(StageId(0), executor);
        stage
            .add_system(
                SystemBuilder::new(|_: CommandRead<'_, u32>| {})
                    .build(SystemId(0))
                    .unwrap(),
            )
            .unwrap();
        schedule.add_stage(stage).unwrap();
        schedule.run(&mut source).unwrap();
        let mut app = App::new(World::new().unwrap()).unwrap();
        let mut rejected = app
            .add_schedule(ScheduleLabel::of::<Child>(), schedule)
            .unwrap_err();
        assert!(source.command_receiver::<u32>().is_err());
        rejected.value.unregister(&mut source).unwrap();
        assert!(source.command_receiver::<u32>().is_ok());
        app.close().unwrap();
        source.close().unwrap();
    }
    impl crate::plugin::Plugin for Child {
        fn build(&self, app: &mut App) -> Result<(), pi_ecs_core::error::EcsError> {
            assert!(app.require_plugin::<Self>().is_err());
            app.world_mut()?.register_resource::<u32>()?;
            Ok(())
        }
        fn finish(&self, app: &mut App) -> Result<(), pi_ecs_core::error::EcsError> {
            app.require_plugin::<Self>()?;
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn rc_app_runs_named_stage_and_unregisters_stable_slot() {
        use crate::app::ScheduleLabel;
        use pi_ecs_schedule::{
            executor::{ExecutionMode, Executor},
            schedule::Schedule,
            stage::{Stage, StageId},
            system::SystemId,
            system_builder::SystemBuilder,
        };
        let mut app = App::new(World::new().unwrap()).unwrap();
        let finishes = Arc::new(AtomicUsize::new(0));
        app.add_plugin(Child(finishes.clone())).unwrap();
        assert!(app.add_plugin(Child(finishes.clone())).is_err());
        let label = ScheduleLabel::of::<Child>();
        let stage = StageId(4);
        let mut schedule = Schedule::new();
        let executor = schedule
            .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
            .unwrap();
        app.add_schedule(label, schedule).unwrap();
        app.schedule_mut(label)
            .unwrap()
            .add_stage(Stage::new(stage, executor))
            .unwrap();
        app.finish().unwrap();
        app.finish().unwrap();
        assert_eq!(finishes.load(Ordering::SeqCst), 1);
        assert!(app.world().table_layouts().is_ok());
        assert!(app.world_mut().is_err());
        assert_eq!(app.insert_resource(9u32).unwrap(), None);
        assert_eq!(app.insert_resource(10u32).unwrap(), Some(9));
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let id = SystemId(6);
        let system = SystemBuilder::new(move || {
            observed.fetch_add(1, Ordering::SeqCst);
        })
        .build(id)
        .unwrap();
        assert_eq!(app.add_system(label, stage, system).unwrap(), 0);
        app.run(label).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        app.unregister_system(label, stage, id).unwrap();
        app.unregister_system(label, stage, id).unwrap();
        assert!(app
            .add_system(label, stage, SystemBuilder::new(|| {}).build(id).unwrap())
            .is_err());
        app.run(label).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn rc_app_system_panic_preserves_stopped_world_and_blocks_rerun() {
        use crate::app::ScheduleLabel;
        use pi_ecs_schedule::{
            executor::{ExecutionMode, Executor},
            schedule::Schedule,
            stage::{Stage, StageId},
            system::SystemId,
            system_builder::SystemBuilder,
        };
        let mut app = App::new(World::new().unwrap()).unwrap();
        let label = ScheduleLabel::of::<()>();
        let mut schedule = Schedule::new();
        let executor = schedule
            .add_executor(Executor::new(ExecutionMode::Serial).unwrap())
            .unwrap();
        let mut stage = Stage::new(StageId(0), executor);
        stage
            .add_system(
                SystemBuilder::new(|| panic!("system panic"))
                    .build(SystemId(0))
                    .unwrap(),
            )
            .unwrap();
        schedule.add_stage(stage).unwrap();
        app.add_schedule(label, schedule).unwrap();
        app.finish().unwrap();
        let panic =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.run(label))).unwrap_err();
        assert_eq!(panic.downcast_ref::<&str>(), Some(&"system panic"));
        assert!(app.world().is_stopped());
        assert!(app.run(label).is_err());
        app.close().unwrap();
    }

    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn app_close_and_drop_release_resources_once() {
        for explicit in [true, false] {
            let drops = Arc::new(AtomicUsize::new(0));
            let mut world = World::new().unwrap();
            world.register_resource::<Payload>().unwrap();
            world.insert_resource(Payload(drops.clone())).unwrap();
            let mut app = App::new(world).unwrap();
            let mut schedule = pi_ecs_schedule::schedule::Schedule::new();
            let executor = schedule
                .add_executor(
                    pi_ecs_schedule::executor::Executor::new(
                        pi_ecs_schedule::executor::ExecutionMode::Serial,
                    )
                    .unwrap(),
                )
                .unwrap();
            schedule
                .add_stage(pi_ecs_schedule::stage::Stage::new(
                    pi_ecs_schedule::stage::StageId(1),
                    executor,
                ))
                .unwrap();
            app.add_schedule(crate::app::ScheduleLabel::of::<Payload>(), schedule)
                .unwrap();
            if explicit {
                app.close().unwrap();
                app.close().unwrap();
                assert_eq!(drops.load(Ordering::SeqCst), 1);
            }
            drop(app);
            assert_eq!(drops.load(Ordering::SeqCst), 1);
        }
    }
}

#[cfg(feature = "rc")]
impl crate::app::AppRunGuard<'_> {
    pub(super) fn complete_inner(&mut self, stopped: bool) {
        if stopped {
            *self.state = crate::app::AppState::Failed;
        }
        self.completed = true;
    }
    pub(super) fn drop_inner(&mut self) {
        if !self.completed {
            *self.state = crate::app::AppState::Failed;
        }
    }
}
#[cfg(feature = "rc")]
impl crate::app::App {
    pub(super) fn insert_resource_inner<T: Send + Sync + 'static>(
        &mut self,
        value: T,
    ) -> Result<Option<T>, EcsError> {
        self.ready_idle()?;
        self.world.insert_resource(value)
    }
    pub(super) fn add_system_inner<S: pi_ecs_schedule::stage::StageSystemBound + 'static>(
        &mut self,
        label: ScheduleLabel,
        stage: pi_ecs_schedule::stage::StageId,
        system: S,
    ) -> Result<usize, pi_ecs_schedule::stage::InstallError<S>> {
        use pi_ecs_schedule::stage::InstallError;
        let schedule = match self.ready_idle().and_then(|()| self.schedule_index(label)) {
            Ok(index) => index,
            Err(error) => {
                return Err(InstallError {
                    value: system,
                    error,
                })
            }
        };
        match self.schedules[schedule].1.stage_mut(stage) {
            Ok(mut config) => config.add_system(system),
            Err(error) => Err(InstallError {
                value: system,
                error,
            }),
        }
    }
    pub(super) fn unregister_system_inner(
        &mut self,
        label: ScheduleLabel,
        stage: pi_ecs_schedule::stage::StageId,
        system: pi_ecs_schedule::system::SystemId,
    ) -> Result<(), EcsError> {
        self.ready_idle()?;
        let schedule = self.schedule_index(label)?;
        let mut guard = crate::app::AppRunGuard {
            state: &mut self.state,
            completed: false,
        };
        let result = self.schedules[schedule]
            .1
            .unregister_system(stage, system, &mut self.world);
        guard.complete(self.world.is_stopped());
        result
    }
    fn ready_idle(&mut self) -> Result<(), EcsError> {
        if !matches!(self.state, crate::app::AppState::Ready) {
            return Err(app_error("App 尚未就绪"));
        }
        if self.world.is_stopped() {
            self.state = crate::app::AppState::Failed;
            return Err(app_error("World 已停止"));
        }
        drop(self.world.pin()?);
        Ok(())
    }
    pub(super) fn close_inner(&mut self) -> Result<(), pi_ecs_core::error::EcsError> {
        use crate::app::AppState;
        if matches!(self.state, AppState::Closed) {
            return Ok(());
        }
        if matches!(self.state, AppState::Closing | AppState::Finishing) {
            return Err(app_error("App 当前不能关闭"));
        }
        // 真实清偿作用域只检查门禁；在改变 App 状态前拒绝运行/维护中的关闭。
        self.world.parameter_unregistration_scope(|_| Ok(()))?;
        self.state = AppState::Closing;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut first = self.inter_frame.close(&mut self.world).err();
            for (_, schedule) in &mut self.schedules {
                if let Err(error) = schedule.unregister(&mut self.world) {
                    if first.is_none() {
                        first = Some(error);
                    }
                }
            }
            self.schedules.clear();
            self.plugins.clear();
            self.plugin_types.clear();
            self.world.close()?;
            Ok::<_, EcsError>(first)
        }));
        match result {
            Ok(Ok(first)) => {
                self.state = AppState::Closed;
                first.map_or(Ok(()), Err)
            }
            Ok(Err(error)) => {
                self.state = AppState::Failed;
                Err(error)
            }
            Err(payload) => {
                self.state = AppState::Failed;
                // 将尚未关闭的 World 单向标停，不允许展开后继续运行。
                let mut payload = Some(payload);
                let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.world.parameter_unregistration_scope::<()>(|_| {
                        std::panic::resume_unwind(payload.take().expect("原始展开载荷"))
                    })
                }));
                match stopped {
                    Err(payload) => std::panic::resume_unwind(payload),
                    Ok(_) => std::panic::resume_unwind(payload.expect("门禁拒绝保留原始载荷")),
                }
            }
        }
    }
    pub(super) fn drop_inner(&mut self) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.close()));
        if result.is_err() || !matches!(self.state, crate::app::AppState::Closed) {
            std::process::abort();
        }
    }
}
impl App {
    pub(crate) fn capture_game_inner(
        _app: &mut crate::app::App,
    ) -> Result<pi_ecs_schedule::snapshot::GameSnapshot, pi_ecs_core::error::EcsError> {
        todo!("编号：T1，尚未实现， ：全 Stage 状态协调与维护授权接线")
    }
}
impl App {
    pub(crate) fn restore_game_inner(
        _app: &mut crate::app::App,
        _snapshot: &pi_ecs_schedule::snapshot::GameSnapshot,
    ) -> Result<(), pi_ecs_core::error::EcsError> {
        todo!("编号：T2，尚未实现， ：全 Stage 恢复与内部存储排他入口")
    }
}

use crate::{
    app::{App, ScheduleLabel},
    plugin::Plugin,
};
use pi_ecs_core::{error::EcsError, world::World};
use pi_ecs_schedule::schedule::Schedule;
fn app_error(message: &'static str) -> EcsError {
    pi_ecs_core::error::classified_error(pi_ecs_core::error::ErrorKind::InvalidState, message)
}
impl ScheduleLabel {
    pub(crate) fn label_inner<T: 'static>() -> ScheduleLabel {
        ScheduleLabel {
            type_id: std::any::TypeId::of::<T>(),
        }
    }
}
impl App {
    pub(crate) fn new_inner(
        _world: World,
    ) -> Result<App, pi_ecs_schedule::stage::InstallError<World>> {
        if let Err(error) = _world.validate_building() {
            return Err(pi_ecs_schedule::stage::InstallError {
                error,
                value: _world,
            });
        }
        Ok(App {
            #[cfg(feature = "rc")]
            world: _world,
            #[cfg(not(feature = "rc"))]
            world: pi_share::Share::new(_world),
            schedules: Vec::new(),
            #[cfg(feature = "rc")]
            inter_frame: pi_ecs_schedule::inter_frame::SystemFlow::new(),
            plugins: Vec::new(),
            plugin_types: Default::default(),
            state: crate::app::AppState::Building,
        })
    }
}
#[cfg(feature = "rc")]
impl App {
    pub(super) fn inter_frame_mut_inner(
        &mut self,
    ) -> Result<pi_ecs_schedule::inter_frame::FlowConfig<'_>, EcsError> {
        if !matches!(
            self.state,
            crate::app::AppState::Building | crate::app::AppState::Finishing
        ) {
            return Err(app_error("当前不能配置帧间图"));
        }
        Ok(self.inter_frame.configuration())
    }
    pub(super) fn dispatch_inner<I: 'static>(
        &mut self,
        _target: &pi_ecs_schedule::inter_frame::InterFrameTarget<I>,
        _input: I,
    ) -> Result<(), EcsError> {
        self.run_guarded(|world, flow, _| flow.dispatch(world, _target, _input))
    }
    pub(super) fn post_inner<C: Send + 'static>(
        &mut self,
        _command: C,
    ) -> Result<(), pi_ecs_core::command::CommandSendError<C>> {
        match self.ready_idle().and_then(|()| self.world.command_sender::<C>()) {
            Ok(sender) => sender.send(_command),
            Err(error) => Err(pi_ecs_core::command::CommandSendError {
                error,
                command: _command,
            }),
        }
    }
    pub(super) fn set_queue_priority_inner<C: Send + 'static>(
        &mut self,
        _priority: pi_ecs_schedule::inter_frame::QueuePriority,
    ) -> Result<(), EcsError> {
        if !matches!(self.state, crate::app::AppState::Building | crate::app::AppState::Finishing) {
            self.ready_idle()?;
        }
        self.inter_frame.set_queue_priority::<C>(_priority)
    }
    pub(super) fn run_next_ready_inner(
        &mut self,
        _budget: pi_ecs_schedule::inter_frame::FrameBudget,
    ) -> Result<pi_ecs_schedule::inter_frame::ReadyRun, EcsError> {
        self.run_guarded(|world, flow, _| flow.run_next_ready(world, _budget))
    }
    pub(super) fn run_stages_inner(
        &mut self,
        _label: crate::app::ScheduleLabel,
        _stages: &[pi_ecs_schedule::stage::StageId],
    ) -> Result<(), EcsError> {
        let index = self.schedule_index(_label)?;
        self.run_guarded(|world, _, schedules| schedules[index].1.run_stages(world, _stages))
    }

    fn run_guarded<R>(
        &mut self,
        run: impl FnOnce(&mut World, &mut pi_ecs_schedule::inter_frame::SystemFlow, &mut [(ScheduleLabel, Schedule)]) -> Result<R, EcsError>,
    ) -> Result<R, EcsError> {
        self.ready_idle()?;
        let mut guard = crate::app::AppRunGuard { state: &mut self.state, completed: false };
        let result = run(&mut self.world, &mut self.inter_frame, &mut self.schedules);
        guard.complete(self.world.is_stopped());
        result
    }
}
impl App {
    pub(crate) fn add_plugin_inner<P: Plugin + 'static>(
        app: &mut App,
        plugin: P,
    ) -> Result<(), EcsError> {
        use crate::app::{AppState, InstalledPlugin, PluginInstallState};
        let type_id = std::any::TypeId::of::<P>();
        if !matches!(app.state, AppState::Building) || app.plugin_types.contains_key(&type_id) {
            return Err(app_error("当前不能安装此插件"));
        }
        app.plugin_types
            .insert(type_id, PluginInstallState::Installing);
        app.build_step(|app| {
            plugin.build(app)?;
            if !matches!(app.state, AppState::Building) {
                return Err(app_error("插件构建未保持构建状态"));
            }
            app.plugins.push(InstalledPlugin {
                type_id,
                plugin: Some(Box::new(plugin)),
            });
            app.plugin_types
                .insert(type_id, PluginInstallState::Installed);
            Ok(())
        })
    }
}
impl App {
    pub(crate) fn require_plugin_inner<P: Plugin + 'static>(app: &App) -> Result<(), EcsError> {
        if matches!(
            app.plugin_types.get(&std::any::TypeId::of::<P>()),
            Some(crate::app::PluginInstallState::Installed)
        ) {
            Ok(())
        } else {
            Err(app_error("所需插件尚未安装"))
        }
    }
}
impl App {
    pub(crate) fn add_schedule_inner(
        _app: &mut App,
        _label: ScheduleLabel,
        _schedule: Schedule,
    ) -> Result<(), pi_ecs_schedule::stage::InstallError<Schedule>> {
        use pi_ecs_schedule::stage::InstallError;
        if !matches!(
            _app.state,
            crate::app::AppState::Building | crate::app::AppState::Finishing
        ) {
            return Err(InstallError {
                value: _schedule,
                error: app_error("当前不允许登记调度"),
            });
        }
        if _app.schedules.iter().any(|(label, _)| *label == _label) {
            return Err(InstallError {
                value: _schedule,
                error: app_error("调度标签重复"),
            });
        }
        if let Err(error) = _schedule.validate_install() {
            return Err(InstallError {
                value: _schedule,
                error,
            });
        }
        _app.schedules.push((_label, _schedule));
        Ok(())
    }
}
impl App {
    pub(crate) fn schedule_mut_inner(
        app: &mut App,
        label: ScheduleLabel,
    ) -> Result<pi_ecs_schedule::schedule::ScheduleConfig<'_>, EcsError> {
        if !matches!(
            app.state,
            crate::app::AppState::Building | crate::app::AppState::Finishing
        ) {
            return Err(app_error("当前不能配置调度"));
        }
        let index = app.schedule_index(label)?;
        Ok(app.schedules[index].1.configuration())
    }
    fn schedule_index(&self, label: ScheduleLabel) -> Result<usize, EcsError> {
        self.schedules
            .iter()
            .position(|(key, _)| *key == label)
            .ok_or_else(|| {
                pi_ecs_core::error::classified_error(
                    pi_ecs_core::error::ErrorKind::NotFound,
                    "调度标签未登记",
                )
            })
    }
}
impl App {
    pub(crate) fn world_inner(app: &App) -> &World {
        &app.world
    }
}
impl App {
    pub(crate) fn world_mut_inner(app: &mut App) -> Result<crate::app::WorldConfig<'_>, EcsError> {
        if !matches!(
            app.state,
            crate::app::AppState::Building | crate::app::AppState::Finishing
        ) {
            return Err(app_error("当前不能独占配置 World"));
        }
        app.world.validate_building()?;
        #[cfg(feature = "rc")]
        {
            Ok(crate::app::WorldConfig {
                world: &mut app.world,
            })
        }
        #[cfg(not(feature = "rc"))]
        pi_share::Share::get_mut(&mut app.world)
            .map(|world| crate::app::WorldConfig { world })
            .ok_or_else(|| app_error("World 并非独占所有"))
    }
}
impl App {
    pub(crate) fn finish_inner(app: &mut App) -> Result<(), EcsError> {
        use crate::app::{AppState, PluginInstallState};
        match app.state {
            AppState::Ready => return Ok(()),
            AppState::Building
                if !app
                    .plugin_types
                    .values()
                    .any(|state| matches!(state, PluginInstallState::Installing)) => {}
            _ => return Err(app_error("当前不能收束 App 构建")),
        }
        app.state = AppState::Finishing;
        app.build_step(|app| {
            for index in 0..app.plugins.len() {
                let plugin = app.plugins[index]
                    .plugin
                    .take()
                    .expect("已安装插件槽不可为空");
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| plugin.finish(app)));
                app.plugins[index].plugin = Some(plugin);
                match result {
                    Ok(result) => result?,
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            }
            for (_, schedule) in &app.schedules {
                schedule.validate_configuration()?;
            }
            #[cfg(feature = "rc")]
            app.inter_frame.validate_configuration(false)?;
            #[cfg(feature = "rc")]
            app.world.finish_registration()?;
            #[cfg(not(feature = "rc"))]
            if app.world.table_layouts().is_err() {
                pi_share::Share::get_mut(&mut app.world)
                    .ok_or_else(|| app_error("World 并非独占所有"))?
                    .finish_registration()?;
            }
            app.state = AppState::Ready;
            Ok(())
        })
    }

    fn build_step(
        &mut self,
        step: impl FnOnce(&mut Self) -> Result<(), EcsError>,
    ) -> Result<(), EcsError> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| step(self))) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                if !matches!(self.state, crate::app::AppState::Closed) {
                    self.state = crate::app::AppState::Failed;
                }
                Err(error)
            }
            Err(payload) => {
                if !matches!(self.state, crate::app::AppState::Closed) {
                    self.state = crate::app::AppState::Failed;
                }
                std::panic::resume_unwind(payload)
            }
        }
    }
}
impl App {
    pub(crate) fn run_inner(app: &mut App, label: ScheduleLabel) -> Result<(), EcsError> {
        let index = app.schedule_index(label)?;
        #[cfg(feature = "rc")]
        {
            app.run_guarded(|world, _, schedules| schedules[index].1.run(world))
        }
        #[cfg(not(feature = "rc"))]
        {
            if !matches!(app.state, crate::app::AppState::Ready) {
                return Err(app_error("App 尚未就绪"));
            }
            app.schedules[index].1.run(&app.world)
        }
    }
}
