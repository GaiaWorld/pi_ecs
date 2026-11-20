//! 执行器消费阶段固定计划，启动前经 System::prepare 刷新匹配；不定义排序或自行推导原型权限。
#[path = "executor_inner.rs"]
mod executor_inner;
use pi_ecs_core::{error::EcsError, world::WorldRef};
/// 调度内可复用的执行器编号；不同编号不意味着并发访问同一 World。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ExecutorId(
    /// 所属 Schedule 中的局部下标。
    pub usize,
);
/// 执行策略，串行也执行同样的权限检查。
pub enum ExecutionMode {
    /// 按依赖及确定顺序逐系统运行。
    Serial,
    /// 仅启动依赖已满足且与正在运行系统不冲突的系统。
    /// rc 构建保留此配置值以返回明确错误，但禁止创建/启动并行执行器，不自动降级。
    Parallel,
}
/// 可以服务多个顺序阶段的执行器，不持有第二个 World。
pub struct Executor {
    /// 本执行器的运行策略。
    pub(crate) mode: ExecutionMode,
    // @to_design 第二轮非 RC：线程资源、任务提交和并行运行系统表示。
}
impl Executor {
    /// mode 选择运行策略；创建失败返回错误，不开始任何阶段。
    /// rc 下 Parallel 在创建线程资源、登记执行器或准备/执行系统前返回配置错误。
    /// 不自动转 Serial；Serial 正常路径仍执行完整权限检查，非 rc 可显式选择两种模式。
    pub fn new(mode: ExecutionMode) -> Result<Self, EcsError> {
        Self::new_inner(mode)
    }
    /// prepared 是 Stage::prepare 产出的本阶段计划，world 必须与准备时一致。
    /// RC 按阶段入口已固定的 plan 顺序，逐系统重新 prepare/校验完整声明，再分配 next_system_tick。
    /// 然后调用 System::run_prepared，等待会话及参数清理成功后才启动下一系统；不借声明树跨可变系统调用。
    /// 新原型只增量同步匹配，不重排计划、不重复已运行系统。准备或 tick 错误不调用本系统，停止后续派发。
    /// 阶段全部成功后由阶段/调度驱动执行 flush_destruction_in_run，panic/错误不执行正常阶段末维护。
    /// 非 RC 按 prepared.stage.ordering 的波次顺序执行，波内按执行器能力串行或并行，不重复检查冲突。
    /// 串行也使用同一波次序列，保证与并行计划一致。
    /// 非 rc 的系统对象在登记时已统一要求 Send，Parallel 直接消费计划波次，不逐系统降级；
    /// rc 的 Parallel 已在构造时拒绝。派发只移动互不相交的系统独占借用，不共享 &dyn System；
    /// 世界级共享能力另行核对。
    /// 在 Stage 提供的刷新边界取得有效的待运行计划，不自行推导查询或反转依赖方向。
    /// 非 rc 普通系统共用布局读授权，含 Alter 系统独占布局写授权，覆盖全部参数派生借用。
    /// 系统内共用授权，不嵌套取锁；结构系统所在波次仅包含自身。
    /// 成功时等待任务、即时 Alter 调用及参数/guard 全部结束才返回。
    /// 系统生命周期错误直接 panic；停止本次运行的后续派发，已启动任务必须收束。
    /// 不补调用 panic 系统的 finish，不回滚已发布结果，不提供捕获后继续模拟的恢复协议。
    /// 返回 Err 前也不得遗留后台访问；panic 必须先收束任务再传播，不自动开始下一阶段。
    /// rc 不得创建工作线程或派发跨线程任务；外部反序列化/未来配置变更同样不能绕过 Parallel 前置拒绝。
    /// @to_design 第二轮非 RC：工作线程停止请求与 join/收束；不以取消任务代替借用结束。
    pub(crate) fn execute(
        &mut self,
        world: WorldRef<'_>,
        prepared: &mut crate::stage::PreparedStage<'_>,
    ) -> Result<(), EcsError> {
        Self::execute_inner(self, world, prepared)
    }
}
