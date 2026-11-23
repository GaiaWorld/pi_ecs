//! 当前 Schedule 拥有 Stage/Executor，按阶段登记顺序串行驱动；系统实例和排序缓存归 Stage。
//! 暂不引入行为图或迁移阶段归属；系统冲突图仍使用 pi_graph。
#[path = "schedule_inner.rs"]
mod schedule_inner;
use pi_ecs_core::{error::EcsError, world::WorldRef};
/// 持有系统及其运行状态，不拥有第二套世界数据。
pub struct Schedule {
    /// 以登记顺序运行的阶段；正常返回后才进入下一阶段，运行时不得修改。
    pub(crate) stages: Vec<crate::stage::Stage>,
    /// 可复用执行器；阶段引用此数组局部编号，不允许同时启动多个阶段。
    pub(crate) executors: Vec<crate::executor::Executor>,
    // RC 运行门禁单源为 World::schedule_scope；不在 Schedule 再存一份 World 运行状态。
}
/// 只允许追加调度配置，不交出已安装的 Schedule。
pub struct ScheduleConfig<'a> {
    /// 原调度保持在宿主的登记槽中。
    pub(crate) schedule: &'a mut Schedule,
}
impl ScheduleConfig<'_> {
    /// # 用途
    /// 追加执行器，返回同一调度内的编号。
    pub fn add_executor(
        &mut self,
        executor: crate::executor::Executor,
    ) -> Result<crate::executor::ExecutorId, EcsError> {
        self.schedule.add_executor(executor)
    }
    /// # 用途
    /// 追加一个阶段，保持已有阶段的身份和登记责任。
    pub fn add_stage(
        &mut self,
        stage: crate::stage::Stage,
    ) -> Result<(), crate::stage::InstallError<crate::stage::Stage>> {
        self.schedule.add_stage(stage)
    }
    /// # 用途
    /// 查找已登记阶段的受限配置视图。
    pub fn stage_mut(&mut self, id: crate::stage::StageId) -> Result<StageConfig<'_>, EcsError> {
        self.schedule.stage_mut(id)
    }
}
/// 只开放单阶段追加配置，不允许替换阶段或注销系统。
pub struct StageConfig<'a> {
    /// 由 Schedule 保持原槽所有权。
    pub(crate) stage: &'a mut crate::stage::Stage,
}
impl StageConfig<'_> {
    /// # 用途
    /// 在原阶段中追加普通系统。
    pub fn add_system<S: crate::stage::StageSystemBound + 'static>(
        &mut self,
        system: S,
    ) -> Result<usize, crate::stage::InstallError<S>> {
        self.stage.add_system(system)
    }
    /// # 用途
    /// 安装阶段入口图；拒绝时连同错误归还原图。
    #[cfg(feature = "rc")]
    pub fn set_entry_flow(
        &mut self,
        flow: crate::inter_frame::SystemFlow,
    ) -> Result<(), crate::stage::StageFlowInstallError> {
        self.stage.set_entry_flow(flow)
    }
}
impl Schedule {
    /// # 用途
    /// 按 stage 与 id 在给定的所属 World 注销，配置视图不提供此操作。
    #[cfg(feature = "rc")]
    pub fn unregister_system(
        &mut self,
        stage: crate::stage::StageId,
        id: crate::system::SystemId,
        world: &mut pi_ecs_core::world::World,
    ) -> Result<(), EcsError> {
        self.unregister_system_inner(stage, id, world)
    }
    /// # 用途
    /// 接管前检查所有阶段与活动实例均不持有已登记责任，同时校验结构。
    pub fn validate_install(&self) -> Result<(), EcsError> {
        self.validate_install_inner()
    }
    /// # 用途
    /// 借出受限配置视图，不允许替换整个调度。
    pub fn configuration(&mut self) -> ScheduleConfig<'_> {
        self.configuration_inner()
    }
    /// # 用途
    /// 按 stages 显式指定的阶段序列运行，允许重复同一阶段；进根授权前校验全部阶段编号与执行器。
    /// 每次访问都运行阶段入口图及主体，复用原实例与长期参数；不推进应用模拟时间。
    /// 同 run 一样首个阶段错误停止，退出同一根授权，不重放前序阶段。
    #[cfg(feature = "rc")]
    pub fn run_stages(
        &mut self,
        world: &mut pi_ecs_core::world::World,
        stages: &[crate::stage::StageId],
    ) -> Result<(), EcsError> {
        self.run_stages_inner(world, stages)
    }
    /// # 用途
    /// 只读检查组装身份/索引：阶段/执行器归属、重复阶段身份与 RC 执行模式，并遍历阶段验证。
    /// 不访问 World，不 initialize/prepare、消费初值或构图；返回首个错误，不修改对象。
    /// 不保证参数组件/资源存在、别名合法或具有快照能力，这些由各自生命周期入口验证。
    pub fn validate_configuration(&self) -> Result<(), EcsError> {
        self.validate_configuration_inner()
    }

    /// # 用途
    /// 宿主在两次运行之间按 id 借用现有阶段，用于登记系统；缺失 Err，不创建或替换阶段。
    /// 借用期间不能运行 Schedule，不授予 World，不允许重排/删除已登记阶段。
    pub fn stage_mut(&mut self, id: crate::stage::StageId) -> Result<StageConfig<'_>, EcsError> {
        self.stage_mut_inner(id)
    }
    /// # 用途
    /// RC 在所属 world 的 Idle/Stopped 静止点按阶段登记顺序注销全部系统；无活任务/参数。
    /// 正常诊断继续其他阶段并返回首个错误，已注销不重试；不关闭 World、不执行维护或调度。
    /// panic/内部故障停止 World，不承诺剩余注销已完成。
    #[cfg(feature = "rc")]
    /// 清偿后保留阶段与历史系统槽，已关闭的入口图解除关联；允许再次运行或追加新系统。
    /// 未完成清偿的入口图留在原阶段，以保留原 World 的清偿责任。
    pub fn unregister(&mut self, world: &mut pi_ecs_core::world::World) -> Result<(), EcsError> {
        self.unregister_inner(world)
    }
    /// 创建尚未运行的调度；不隐式安装应用阶段。
    pub fn new() -> Self {
        Self::new_inner()
    }
    /// executor 加入本调度，返回稳定局部编号；不运行系统，失败不改变登记。
    pub fn add_executor(
        &mut self,
        executor: crate::executor::Executor,
    ) -> Result<crate::executor::ExecutorId, EcsError> {
        Self::add_executor_inner(self, executor)
    }
    /// stage 按登记顺序追加；阶段 id 重复或执行器不属于本调度时返回错误且不追加。
    /// 已持有登记责任的阶段不接管；任何拒绝都归还原 stage。
    pub fn add_stage(
        &mut self,
        stage: crate::stage::Stage,
    ) -> Result<(), crate::stage::InstallError<crate::stage::Stage>> {
        Self::add_stage_inner(self, stage)
    }
    /// 在 world 上按登记顺序运行阶段；rc 为 &mut World，非 rc 为共享 &World。
    /// 非 rc 的共享签名不授予任意写入；两种模式都不支持同一 World 重叠运行。
    /// 阶段前同步原型并刷新计划/冲突关系，串行与并行使用相同访问协议。
    /// 普通初始化/准备错误在无活参数后释放本入口根授权并回 Idle，保留之前成功结果，返回 Err。
    /// 不自动 flush 失败阶段、不重跑系统或进入后续阶段；已成功阶段的维护结果保留。
    /// panic/内部故障与时钟耗尽沿 World 停止协议；停止态不能被根授权释放改回 Idle。
    /// 下一次显式 run 从第一个阶段开始新一轮，不承诺幂等或失败点续跑，不回退各系统 last_run。
    pub fn run(&mut self, world: WorldRef<'_>) -> Result<(), EcsError> {
        /* 设计已确认：
        验证阶段执行器绑定，RC 调用 World::schedule_scope 建立先于派生参数的根会话。
        正常返回（含普通 Err）收束全部访问后退出自有 Running 到 Idle，已有 Stopped 保持。
        panic 守卫只标停，不补 finish/维护，不撤销已发布结果，不获取跨执行器阶段授权。
        按 stages 登记顺序调用 Stage::run，并从 executors 选择对应可复用执行器。
        上一阶段未完全收束不进入下一阶段，错误停止本轮，不遗留后台访问。
        执行器的串行/并行只作用于阶段内部；阶段入口同步原型，阶段间不重叠。
        不自动每阶段 settle；指定静止点的维护编排接口仍待定义。
        */
        Self::run_inner(self, world)
    }
}
