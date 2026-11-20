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
impl Schedule {
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
    pub fn stage_mut(
        &mut self,
        id: crate::stage::StageId,
    ) -> Result<&mut crate::stage::Stage, EcsError> {
        self.stage_mut_inner(id)
    }
    /// # 用途
    /// RC 在所属 world 的 Idle/Stopped 静止点按阶段登记顺序注销全部系统；无活任务/参数。
    /// 正常诊断继续其他阶段并返回首个错误，已注销不重试；不关闭 World、不执行维护或调度。
    /// panic/内部故障停止 World，不承诺剩余注销已完成。
    #[cfg(feature = "rc")]
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
    pub fn add_stage(&mut self, stage: crate::stage::Stage) -> Result<(), EcsError> {
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
