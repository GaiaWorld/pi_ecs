//! Stage 是完整收束的动作节点，独占其 System 实例及运行状态。
//! 已确认不做跨 Stage System 实例共享；函数可复用，实例状态独立。
//! 当前由 Schedule 拥有并按登记顺序运行，Stage 自行计算冲突与顺序，再交绑定 Executor 执行。
#[path = "stage_inner.rs"]
mod stage_inner;
use crate::executor::{Executor, ExecutorId};
use crate::system::System;
use pi_ecs_core::{
    access::ResolvedAccess,
    error::EcsError,
    world::{World, WorldRef},
};
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
    /// 系统槽按登记顺序排列；下标即执行计划中的系统序号，运行期不重排或移除。
    pub(crate) systems: Vec<StageSystem>,
    /// 排序缓存所属 (World 身份, 布局替换代次, 已发布原型总量)，与系统匹配扫描进度职责不同。
    /// 首次/登记变化/注销时 None；仅潜在权限展开及排序全部成功才更新，运行中不推进。
    pub(crate) ordering_snapshot: Option<(u64, u64, usize)>,
    /// 对应 ordering_snapshot 的排序结果；覆盖本阶段 Alter 潜在目标闭包，阶段内固定。
    pub(crate) ordering: Option<crate::stage_plan::StagePlan>,
    // 顺序约束：同组件先写后读；多个写者按登记顺序，最终由 stage_plan 计算。
}
/// 非 rc 构建的系统对象：可跨线程移动，供阶段计划把互不冲突的系统派发到工作线程。
#[cfg(not(feature = "rc"))]
pub(crate) type SystemObject = Box<dyn System + Send>;
/// rc 单线程构建的系统对象：不要求 Send；Parallel 已在 Executor::new 拒绝。
#[cfg(feature = "rc")]
pub(crate) type SystemObject = Box<dyn System>;
/// 可登记到 Stage 的系统的编译期门槛，随构建模式统一，不由调用者逐系统选择。
#[cfg(not(feature = "rc"))]
pub trait StageSystemBound: System + Send {}
#[cfg(not(feature = "rc"))]
impl<T: System + Send> StageSystemBound for T {}
/// rc 单线程构建不附加 Send；并行与否由阶段准备的计划决定。
#[cfg(feature = "rc")]
pub trait StageSystemBound: System {}
#[cfg(feature = "rc")]
impl<T: System> StageSystemBound for T {}
/// 阶段内一个系统槽：实例及最近一次准备的展开权限。
pub(crate) struct StageSystem {
    /// 本阶段已使用身份；注销后仍保留，不允许重新登记同一身份。
    pub(crate) id: crate::system::SystemId,
    /// 活动系统实例；None 为已注销空槽，永不复用或压缩。初始化失败的实例仍为 Some 并拒绝准备。
    /// 不跨阶段共享；注销完整清偿后释放函数/配置，panic 时停止，不能把空槽误判为已安全清偿。
    pub(crate) system: Option<SystemObject>,
    /// 最近一次成功准备展开的权限；准备开始即清空，失败保持为空。
    pub(crate) resolved: Vec<ResolvedAccess>,
}
impl StageSystem {
    /// 返回系统实例的可变借用；不改变构建模式能力，也不授予跨线程共享能力。
    pub(crate) fn system_mut(&mut self) -> Option<&mut (dyn System + '_)> {
        Self::system_mut_inner(self)
    }
}
/// 阶段入口准备完成后的借用，不能跨本阶段执行保存或移作另一 World 使用。
pub(crate) struct PreparedStage<'s> {
    /// 独占本阶段系统和长期状态，保证独占本次执行；启动前可增量刷新已初始化系统，不重复初始化或重排。
    pub(crate) stage: &'s mut Stage,
    // 计划单源保存在 stage.ordering，不另存副本；有效性由入口准备保证。
}
impl Stage {
    /// # 用途
    /// 只读检查组装身份/索引：系统历史身份唯一、活动对象 id 与槽身份一致，空槽不参与派发。
    /// 不访问 World，不 initialize/prepare、消费初值或构图；返回首个错误，不修改对象。
    /// 不保证参数组件/资源存在、别名合法或具有快照能力，这些由各自生命周期入口验证。
    pub(crate) fn validate_configuration(&self) -> Result<(), EcsError> {
        self.validate_configuration_inner()
    }

    /// # 用途
    /// RC 宿主在两次运行之间注销 id；world 为绑定 World，Idle/Stopped，无任务/参数借用。
    /// 未登记返回 Err 且不失效缓存；已注销返回 Ok，幂等。已初始化实例通过 System::unregister 清偿。
    /// 已初始化实例先核对 World 身份，异源不撤销缓存/不消费实例；核对成功才撤销排序并消费状态。
    /// 未初始化只释放配置，正常诊断也完成清偿并留下 None 空槽。
    /// 保留身份/槽位，不复用；不 flush/settle/分配 tick，归零留待维护或关闭。
    /// 内部故障/析构 panic 停止 World，不承诺可继续；不能只丢系统对象跳过参数注销。
    #[cfg(feature = "rc")]
    pub fn unregister_system(
        &mut self,
        id: crate::system::SystemId,
        world: &mut World,
    ) -> Result<(), EcsError> {
        self.unregister_system_inner(id, world)
    }
    /// # 用途
    /// RC 静止点注销全部系统，按登记顺序清偿；world 是原绑定 World，Idle/Stopped 且无活参数。
    /// 各系统正常返回 Err 仍继续其余注销并返回首个诊断；已消费系统不重复注销，清空排序缓存。
    /// 内部故障/panic 停止，不假装完成；不关闭 World、不维护实体、不运行系统。
    #[cfg(feature = "rc")]
    pub(crate) fn unregister(&mut self, world: &mut World) -> Result<(), EcsError> {
        self.unregister_inner(world)
    }
    /// id 为本调度阶段编号，executor 为目标执行器编号；构造空阶段，不隐式添加系统。
    /// 编号归属和重复检查由 Schedule::add_stage 执行，不开始运行。
    pub fn new(id: StageId, executor: ExecutorId) -> Self {
        Self::new_inner(id, executor)
    }
    /// system 按登记顺序追加到本阶段，返回稳定槽位下标，不初始化或访问 World。
    /// 编译期门槛随构建模式统一：非 rc 要求 S: Send，rc 单线程不要求。
    /// 是否实际并行由阶段准备基于真实原型与 Alter 目标计算的计划决定，不在此登记。
    /// 系统身份曾在本阶段使用（包括已注销）即返回错误且不加入；只追加，不复用空槽或热替换。
    /// 登记成功撤销排序缓存，前置拒绝不改变缓存；宿主仅在两次运行之间调用，不提供运行中请求队列。
    /// 注销由 Stage 在静止点管理，先撤销缓存再消费状态；正常诊断也代表状态已消费。
    /// 注销槽仍占位但不再参与准备/排序/运行。
    pub fn add_system<S: StageSystemBound + 'static>(
        &mut self,
        system: S,
    ) -> Result<usize, EcsError> {
        Self::add_system_inner(self, Box::new(system))
    }
    /// # 用途
    /// world 不得同时由另一 Schedule 运行或被宿主结构修改；上一阶段已完全收束。
    /// 仅 Some 活动槽参与准备，None 空槽跳过；计划仍使用原稳定槽位，不把紧凑活动索引当作槽号。
    /// 第一遍按登记顺序调用全部活动 System::initialize，首错停止，之前成功登记保留，后续本次不尝试。
    /// RC 第一遍完整包在 World::parameter_registration_scope 中，以 ParamRegistration 包装受限能力；
    /// 不临时退回 Idle，不借整份 World 给参数，不在 prepare/启动前刷新中再次打开登记作用域。
    /// 初始化全部成功后登记集合稳定，RC 调用 World::apply_tracking 汇总全 World 而非仅本阶段需求。
    /// 应用失败本阶段不运行；第二遍逐系统 prepare，同步 Query/Alter 和权限。
    /// 两遍完成前不派发任何系统；第二遍不新增/注销生产者或订阅，启动前刷新同样不得初始化。
    /// 每次准备按当前已发布原型重新展开：Query/Lookup 使用本次匹配原型。
    /// Alter 额外纳入固定增删将发布的源/目标原型；原型或 Alter 目标变化会改变计划波次。
    /// 通过 StagePlan::ordering_archetypes 推导潜在闭包，再由 System::ordering_access 展开排序权限。
    /// 构造按登记顺序的活动权限与永久槽号映射，build_ordering 输出直接使用永久槽号。
    /// 以 StagePlan::build_ordering 更新缓存；系统登记/注销直接失效，绑定期声明固定；恢复重绑由布局代次失效。
    /// 一个系统准备失败只删除其本次声明并不运行，不回滚已成功的控制变更。
    /// 已成功控制变更不因后续准备失败回滚；专门执行图的驱动设计暂不展开。
    /// 准备失败不运行系统，不改变组件值；阶段内点查不承担全局同步。
    /// Alter::prepare 仅生成逻辑目标计划，阶段准备不创建原型组合闭包，不改变布局代次。
    /// 实际转换首次命中时才创建目标 variant；必须持排他结构能力，不能与活查询/组件借用重叠扩容。
    /// 未发布目标用 OrderingArchetypes/OrderingAccess 的逻辑序号，不冒充真实 ArchetypeId。
    /// rc 单线程在冲突结果结束后直接创建；每个系统启动前同步已发布原型，普通已绑定视图不自动刷新。
    /// ParamSet 每次成员重绑必须同步当前已发布结构，Alter 同时重建遍历快照并重证表顺序；有序后续系统也须看见前序目标及成员。
    /// 非 rc 首版含 Alter 的系统独占运行，持布局写授权直到所有参数结果/guard 结束。
    /// 普通系统持布局读授权，可按已有值权限并行；结构写与任何其他系统不在同波。
    /// 同系统参数共用一次布局授权，仍校验源/目标与值访问别名；不得在写授权内重取读锁。
    /// 阶段入口比较 ordering_snapshot；变化时调用系统展开权限并重算排序，包含 Alter 潜在目标闭包。
    /// 入口成功后顺序固定；执行中新原型同步后续系统匹配及当前系统 ParamSet 按需重绑，下一次进入阶段才据新总量重排。
    /// @to_design 第二轮非 RC：布局锁及受保护字段、授权 guard 与并行待运行子图刷新表示；RC 使用串行作用域与固定顺序。
    /// 并行启动的系统无前后可见性承诺；须明确同步顺序后才称后续系统，不能按线程启动时间推导。
    pub(crate) fn prepare(&mut self, world: WorldRef<'_>) -> Result<PreparedStage<'_>, EcsError> {
        Self::prepare_inner(self, world)
    }
    /// executor 由 Schedule 按编号选择；world 按构建模式为 &mut（rc）或共享 &（非 rc）。
    /// 正常返回时本阶段所有任务及作用域发布已完成，不自动执行 settle。
    /// 准备失败不执行，执行失败不继续下一阶段；引用和 guard 必须在阶段返回前释放。
    pub(crate) fn run(
        &mut self,
        world: WorldRef<'_>,
        executor: &mut Executor,
    ) -> Result<(), EcsError> {
        /* 设计已确认：
        上一阶段已结束，按活动槽顺序 initialize；首错终止，本轮不运行系统，不回滚之前成功登记。
        全部初始化成功形成登记屏障，汇总追踪需求，再按活动槽 prepare；任何错误不生成 PreparedStage。
        后续启动前刷新只 prepare，不初始化/增删登记；不复用跨屏障的运行视图、业务引用或授权。
        同源冻结容器根可按 QueryState 协议长期缓存，不得缓存可搬移值/tick 缓冲地址。
        比较 World 身份/布局代次/原型总量；登记/注销直接使缓存失效，不维护参数/系统排序版本。
        任一不同或无缓存：构造潜在闭包、展开权限、构图；全部成功后一次发布快照与排序。
        全部相同：复用排序，仍建立本次授权/视图；不复用失效缓冲指针或跳过资源/订阅有效性检查。
        多例结构版本独立于原型快照；prepare 核对生产者映射，变化只重绑本参数，不重排。
        失败保留旧缓存最多供下次比较，本次绝不可派发；不能提前把缓存快照更新为新值。
        入口按排序快照变化重建含潜在目标的计划；执行中只刷新系统匹配和授权，不重排剩余系统。
        执行器只消费当前有效波次；波内按执行器能力串行或并行，不独立猜测动态权限。
        Parallel 的可行性由构建模式与计划决定，不按单个系统登记降级；rc 已在构造执行器时拒绝。
        将 PreparedStage 交给 Executor::execute；只允许启动前刷新匹配/授权，不重复初始化或阶段排序。
        等待任务、Alter 作用域发布和全部参数借用/guard 结束。
        成功阶段调用 flush_destruction_in_run：先回收可释放通知旧值，再收束其与系统产生的全部归零级联。
        新 Removed 有读者则留待后续消费，无读者值直接释放；最后回收身份槽，完成后进入下一阶段。
        参数/准备错误及 panic 不调用正常阶段末收束，部分已发布结果保留。
        共享 World 的维护授权待接通，不能强转 &mut World；本轮不改造执行图驱动。
        本方法不启动其他阶段，不固定每阶段 settle，不支持另一 Schedule 同时运行同一 World。
        */
        Self::run_inner(self, world, executor)
    }
}
