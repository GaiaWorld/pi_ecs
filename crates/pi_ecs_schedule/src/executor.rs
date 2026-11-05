//! 执行器只决定如何运行已准备系统，不定义阶段顺序或自行同步原型。
use pi_ecs_core::{error::EcsError, world::World};
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
    Parallel,
}
/// 可以服务多个顺序阶段的执行器，不持有第二个 World。
pub struct Executor {
    /// 本执行器的运行策略。
    pub(crate) mode: ExecutionMode,
    // @to_design 线程资源、任务提交和运行系统表示在 System 契约确定后补齐。
}
impl Executor {
    /// mode 选择运行策略；创建失败返回错误，不开始任何阶段。
    pub fn new(mode: ExecutionMode) -> Result<Self, EcsError> {
        crate::executor_inner::new_inner(mode)
    }
    /// prepared 是 Stage::prepare 产出的本阶段计划，world 必须与准备时一致。
    /// 不重新同步原型；运行后必须等待全部任务结束、Alter 发布及参数/guard 释放才返回。
    /// 普通错误不得遗留后台访问；执行 panic 必须先收束任务再传播，不自动开始下一阶段。
    /// @to_design 工作线程 panic 的收束及系统返回错误的分类待系统执行协议闭合。
    pub(crate) fn execute(
        &mut self,
        world: &World,
        prepared: &mut crate::stage::PreparedStage<'_>,
    ) -> Result<(), EcsError> {
        crate::executor_inner::execute_inner(self, world, prepared)
    }
}
