//! Stage 是完整收束的动作节点，独占其 System 实例及运行状态。
//! 已确认不做跨 Stage System 实例共享；函数可复用，实例状态独立。
//! @to_design 阶段内条件系统组/访问计划与外部执行图协议待接入；现有 Executor 绑定是临时壳。
use crate::executor::{Executor, ExecutorId};
use pi_ecs_core::{error::EcsError, world::World};
/// 所属 Schedule 内唯一的阶段标识。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct StageId(
    /// 本调度局部编号。
    pub u32,
);
/// 一组系统、先后依赖及访问关系；Schedule 顺序执行，阶段之间不重叠。
pub struct Stage {
    /// 所属调度内唯一标识。
    pub(crate) id: StageId,
    /// 运行本阶段的执行器；多个阶段允许复用同一编号。
    pub(crate) executor: ExecutorId,
    // @to_design 系统集合的擦除/线程能力表示待闭合。执行图及全阶段推进由后续专门模块承担，本轮不定义。
    // 顺序约束：同组件先写后读；多个写者按登记顺序，不按登记顺序统一定向所有冲突。
}
/// 阶段入口准备完成后的借用，不能跨本阶段执行保存或移作另一 World 使用。
pub(crate) struct PreparedStage<'s> {
    /// 独占本阶段系统和长期状态，保证执行中不重复准备。
    pub(crate) stage: &'s mut Stage,
    // @to_design 已同步原型版本、访问图与系统参数准备结果字段待闭合。
}
impl Stage {
    /// id 为本调度阶段编号，executor 为目标执行器编号；构造空阶段，不隐式添加系统。
    /// 编号归属和重复检查由 Schedule::add_stage 执行，不开始运行。
    pub fn new(id: StageId, executor: ExecutorId) -> Self {
        crate::stage_inner::new_inner(id, executor)
    }
    /// world 不得同时由另一 Schedule 运行或被宿主结构修改；上一阶段已完全收束。
    /// 先应用阶段前订阅取消/系统控制变更并汇总追踪需求，再同步原型和 Query/Alter 计划，检查参数权限。
    /// 已成功控制变更不因后续准备失败回滚；专门执行图的驱动设计暂不展开。
    /// 准备失败不运行系统，不改变组件值；阶段内点查不承担全局同步。
    /// @to_design 具体原型快照和系统准备结果表示待闭合；阶段中登记新原型规则尚未确认。
    pub(crate) fn prepare(&mut self, world: &World) -> Result<PreparedStage<'_>, EcsError> {
        crate::stage_inner::prepare_inner(self, world)
    }
    /// executor 由 Schedule 按编号选择，world 为共享系统访问基础，不代表任意可写。
    /// 正常返回时本阶段所有任务及作用域发布已完成，不自动执行 settle。
    /// 准备失败不执行，执行失败不继续下一阶段；引用和 guard 必须在阶段返回前释放。
    pub(crate) fn run(&mut self, world: &World, executor: &mut Executor) -> Result<(), EcsError> {
        /* 设计已确认：
        上一阶段已结束，按 Stage::prepare 同步原型及更新访问计划。
        将 PreparedStage 交给选定 Executor::execute，不让执行器重复准备。
        等待任务、Alter 作用域发布和全部参数借用/guard 结束。
        在静止点收束阶段待销毁实体及析构引发的新归零，完成后才允许进入下一阶段。
        共享 World 的维护授权待接通，不能强转 &mut World；本轮不改造执行图驱动。
        本方法不启动其他阶段，不固定每阶段 settle，不支持另一 Schedule 同时运行同一 World。
        */
        crate::stage_inner::run_inner(self, world, executor)
    }
}
