//! Schedule 的目标是命名执行图，调用 Stage 或子 Schedule，系统状态归 Stage。
//! @to_design 当前 stages/executors 与线性运行是旧临时壳，不是最终组合接口。
//! 正式设计采用 App 统一拥有 Stage，图仅引用；需对接执行图节点/调用/返回/暂停协议后替换。
//! 图所有者显式组合，插件提供动作/子图并通过扩展点接入，不由安装依赖推导执行顺序。
use pi_ecs_core::{error::EcsError, world::World};
/// 持有系统及其运行状态，不拥有第二套世界数据。
pub struct Schedule {
    /// 以登记顺序运行的阶段；正常返回后才进入下一阶段，运行时不得修改。
    pub(crate) stages: Vec<crate::stage::Stage>,
    /// 可复用执行器；阶段引用此数组局部编号，不允许同时启动多个阶段。
    pub(crate) executors: Vec<crate::executor::Executor>,
    // @to_design 防止同一 World 被多个 Schedule/宿主同时驱动的安全运行门禁待闭合。
}
impl Schedule {
    /// 创建尚未运行的调度；不隐式安装应用阶段。
    pub fn new() -> Self {
        crate::schedule_inner::new_inner()
    }
    /// executor 加入本调度，返回稳定局部编号；不运行系统，失败不改变登记。
    pub fn add_executor(
        &mut self,
        executor: crate::executor::Executor,
    ) -> Result<crate::executor::ExecutorId, EcsError> {
        crate::schedule_inner::add_executor_inner(self, executor)
    }
    /// stage 按登记顺序追加；阶段 id 重复或执行器不属于本调度时返回错误且不追加。
    pub fn add_stage(&mut self, stage: crate::stage::Stage) -> Result<(), EcsError> {
        crate::schedule_inner::add_stage_inner(self, stage)
    }
    /// 在共享 world 上按登记顺序运行阶段；同一 World 不支持重叠的 Schedule 运行。
    /// 共享签名不授予任意写入，安全入口须通过 World 运行门禁防止重叠驱动。
    /// 阶段前同步原型并刷新计划/冲突关系，串行与并行使用相同访问协议。
    /// @to_design 系统错误、延迟命令应用、确定先后和部分执行后的停止协议待定。
    pub fn run(&mut self, world: &World) -> Result<(), EcsError> {
        /* 设计已确认：
        验证阶段执行器绑定，进入唯一 World 运行会话，不获取跨执行器阶段授权。
        按 stages 登记顺序调用 Stage::run，并从 executors 选择对应可复用执行器。
        上一阶段未完全收束不进入下一阶段，错误停止本轮，不遗留后台访问。
        执行器的串行/并行只作用于阶段内部；阶段入口同步原型，阶段间不重叠。
        不自动每阶段 settle；指定静止点的维护编排接口仍待定义。
        */
        crate::schedule_inner::run_inner(self, world)
    }
}
