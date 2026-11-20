//! 单系统长期状态及本次执行边界，不负责执行图定向或阶段推进。
#[path = "system_inner.rs"]
mod system_inner;
use crate::system_param::{ParamRegistration, SystemParam};
use pi_ecs_core::{
    access::{ResolvedAccess, RunAccess},
    change_detection::{ChangeTick, ChangeWindow},
    error::EcsError,
};
/// 所属 Stage 内稳定系统身份，不使用函数地址作为身份；跨 Stage 定位需同时带阶段身份。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SystemId(
    /// 阶段局部编号；曾登记即永久保留历史记录，注销后不复用，不另设代际。
    pub u64,
);
/// 本次系统上下文，不能脱离授权保留组件引用。
pub struct SystemRunContext<'a> {
    /// 参数组合校验后的 RunAccess，内部使用统一 UnsafeWorldCell；持续到 finish 完成。
    /// 外层 core 系统状态守卫保活到全部参数清理后；本上下文不能解锁会话或返回整个 World。
    pub(crate) access: RunAccess<'a>,
    /// 上次成功执行和本次 tick；首次 None 无下界，其余左开右闭，当前写入立即可观察。
    pub(crate) window: ChangeWindow,
}
impl<'w> SystemRunContext<'w> {
    /// # 用途
    /// 消费 self，按值交出原授权与本次窗口，不借临时上下文、不克隆长期声明或扩大权限。
    /// 供外部 unsafe SystemParam::fetch 使用；组合节点须先按原树派生正确子项，叶子绑定自行校验。
    /// 能取得描述不等于可重复兑现：仍遵守 fetch 的别名、原会话保活及父子互斥要求。
    pub fn into_parts(self) -> (RunAccess<'w>, ChangeWindow) {
        self.into_parts_inner()
    }
    /// # 用途
    /// access/window 属于同次系统运行，按值接管，不分配子节点或复制长期权限集合。
    /// 结果引用原声明及 World，不引用调用栈临时 context；不延长来源授权。
    pub(crate) fn new(access: RunAccess<'w>, window: ChangeWindow) -> Result<Self, EcsError> {
        Self::context_new(access, window)
    }
    /// # 用途
    /// position 为 Together 成员；返回按值子描述，只引用原长期声明，窗口按值复制，无堆分配。
    /// 越界/非 Together 返回 Err，不回退父权限，不产生额外 World 授权。
    /// 本方法只派生描述，不取业务引用；重复派生本身安全，实际 fetch 仍须遵守
    /// SystemParam::fetch 的 unsafe 协议，不能重复兑现同一位置或同时兑现父节点及其子节点。
    pub fn child(&self, position: usize) -> Result<SystemRunContext<'w>, EcsError> {
        self.child_inner(position)
    }
    /// # 用途
    /// position 为 Exclusive 成员，返回按值短授权描述，不引用临时 self。
    /// # Safety
    /// 框架须经 ParamSet 的独占短借把返回寿命缩至成员 State/Pending 借用，排除兄弟活结果/物理缓存。
    pub(crate) unsafe fn exclusive_child(
        &self,
        position: usize,
    ) -> Result<SystemRunContext<'w>, EcsError> {
        self.exclusive_child_inner(position)
    }
}
/// 对参数签名的函数调用适配；不允许输出携带本次参数借用。
pub trait SystemFunction<Marker> {
    /// 本函数的完整参数协议；Marker 仅用于签名推导和避免不同适配实现重叠。
    type Param: SystemParam;
    /// item 是本次参数元组；正常返回 ()，不在首版混入业务 Result 适配。
    fn call<'run>(&mut self, item: <Self::Param as SystemParam>::Item<'run>);
}
/// 调度持有的系统接口，首版只允许框架适配实现。
pub trait System: private::Sealed {
    /// # 用途
    /// 先成功 prepare，再对 logical 共享逻辑全集展开排序权限，不能仅看已发布命中。
    /// 包含潜在目标，并单独展开成员读取 entities；物理搬移不能冒充保留组件业务值写入。
    /// 不创建原型、不修改运行匹配缓存；失败不发布排序，不生成运行授权。
    fn ordering_access(
        &self,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError>;
    /// # 用途
    /// 在唯一根 Running 内运行已准备系统；world 是所属 World，this_run 是刚分配的本次 tick。
    /// 内部拆分借用声明缓存/函数/参数状态，调用 World::system_scope；不要求执行器借树后再可变调用系统。
    /// begin/fetch/call/finish 及所有参数清理均在会话内，窗口绑定本系统 last_run。
    /// 仅 system_scope 成功返回后推进 last_run；Err 不推进，panic 标停且不补 finish。
    /// 不重新排序/初始化/分配 tick；准备无效或来源不符拒绝，不调用用户函数。
    /// # Safety
    /// 调用者持唯一串行运行权，准备已成功，tick 属于本 World 本次派发，无其他活参数引用。
    #[cfg(feature = "rc")]
    unsafe fn run_prepared(
        &mut self,
        world: &mut pi_ecs_core::world::World,
        this_run: ChangeTick,
    ) -> Result<(), EcsError>;
    /// # 用途
    /// 成功 prepare 后借用已校验的完整声明树，不返回展开权限的合并结果。
    /// 未准备/准备失败/注销返回错误；借用结束前不能刷新声明。
    fn access_tree(&self) -> Result<&pi_ecs_core::access::AccessTree, EcsError>;
    /// 返回本调度系统身份，不改变状态。
    /// # 用途
    /// 返回稳定系统身份，不复用已注销身份。
    fn id(&self) -> SystemId;
    /// # 用途
    /// registration 是所属 World 的阶段入口登记上下文，无系统参数或任务借用。
    /// 未初始化时消费配置并调用参数 initialize，一次成功后重复调用仅确认同一 World，不重复登记。
    /// 失败清理本实例本次部分登记并进入 InitializationFailed，不回滚其他成功系统；失败/注销实例拒绝。
    /// 不绑定运行地址、不分配系统 tick、不执行用户函数；Stage 全部初始化成功后才开始 prepare。
    fn initialize(&mut self, registration: &mut ParamRegistration<'_>) -> Result<(), EcsError>;
    /// # 用途
    /// 阶段登记屏障之后刷新参数计划；world 属于已绑定 World，当前没有登记变更，不交付登记能力。
    /// 本系统必须已初始化；未初始化/初始化失败/注销返回 Err，不隐式初始化，不增删订阅或生产者。
    /// 已初始化后的计划刷新失败保留长期状态及登记责任，但本次 prepared 失效；修正条件后可重试刷新。
    /// 失败不执行系统；失效必需订阅不得被悄悄自动重新订阅。
    fn prepare(
        &mut self,
        world: pi_ecs_core::world::WorldRef<'_>,
    ) -> Result<Vec<ResolvedAccess>, EcsError>;
    /// context 来自本次已准备系统，参数线程能力、World 和计划版本均匹配。
    /// 生命周期错误直接 panic；本低层入口不推进 last_run，由 run_prepared 在会话成功后推进。
    /// panic 不回滚写入或通知游标、不补调用 finish；执行器收束任务后传播，不支持捕获后继续模拟。
    /// # Safety
    /// 调用者证明 context 覆盖整个执行与完成过程，且不存在冲突并行系统。
    /// # 用途
    /// 使用已准备状态和本次授权调用系统；正常返回后完成参数，展开不补交。
    unsafe fn run(&mut self, context: SystemRunContext<'_>);
    /// registration 为所属 World 的阶段前静止点上下文，调用前全部参数借用和 Pending 已收束。
    /// 消费已有参数状态并按 SystemParam::unregister 清偿；Ok/Err 正常返回都进入 Unregistered，不再 prepare/run。
    /// 已消费的状态不重试；无法完成清理或析构 panic 停止 World，不把部分注销当作正常返回。
    /// Initialized 先核对 World 身份；异源 Err 保留状态/准备及清偿责任，同源才消费并注销。
    /// 尚未初始化的配置只释放、不调用 initialize；无登记责任的状态不重复注销参数。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(&mut self, registration: &mut ParamRegistration<'_>) -> Result<(), EcsError>;
}
/// 参数初始化生命周期；配置与长期状态互斥拥有，失败实例不能隐式重建初值。
pub(crate) enum FunctionSystemState<P: SystemParam> {
    /// 尚未消费配置；只能在阶段前尝试初始化一次。
    Uninitialized(
        /// 构建时接收的完整参数配置，初始化消费其拥有权。
        crate::system_builder::ParamConfig<P>,
    ),
    /// 已初始化，计划刷新失败仍保留长期状态及登记责任。
    Initialized {
        /// 首次成功初始化所属 World 的非复用身份，包含零参数/纯 Local 系统；不是地址或布局代次。
        /// 后续初始化/准备/运行/注销先核对；异源拒绝不能消费状态或解除原登记。
        world_identity: u64,
        /// 每参数位置的长期状态，不包含本次运行借用。
        state: P::State,
        /// 最近一次阶段准备是否成功；准备开始即失效，成功后才允许运行。
        prepared: bool,
    },
    /// 初始化失败且本次部分登记已清理；prepare/run 拒绝，需重新构建系统实例。
    InitializationFailed,
    /// 已完成注销，无参数登记责任；不能重新初始化或运行。
    Unregistered,
}
/// 函数及参数长期状态；不保存本次 Query/Reader、组件引用或授权。
pub struct FunctionSystem<F, P: SystemParam, Marker = P> {
    /// 最近全部参数同步成功的 (布局替换代次, World 原型总量)，None 为首次，失败不推进。
    /// 启动前直接比较 World；与 Stage 排序快照职责独立，匹配同步不推进 last_run。
    pub(crate) matched_revision: Option<(u64, usize)>,
    /// 最近准备成功的完整声明；准备开始置 None，错误不保留旧授权，展开列表仅供排序。
    pub(crate) prepared_access: Option<pi_ecs_core::access::AccessTree>,
    /// 编译期签名标记，不拥有 Marker，不附加 Send/Sync 或运行数据。
    pub(crate) signature: std::marker::PhantomData<fn() -> Marker>,
    /// 系统身份。
    pub(crate) id: SystemId,
    /// 用户函数或手写函数适配。
    pub(crate) function: F,
    /// 单源表达初始化与注销状态；禁止配置与长期状态同时存在，不另设运行失败状态。
    pub(crate) state: FunctionSystemState<P>,
    /// 仅成功执行及完成提交后推进；None 表示尚未成功运行。
    pub(crate) last_run: Option<ChangeTick>,
    // RC 使用 core::unsafe_world_cell 的统一句柄与 SystemSession；普通参数不持独立字段能力包。
    // panic 按 Rust 展开释放拥有型工作区；擦除存储仍须记录初始化责任，不另造通用恢复状态机。
}
impl<F, P: SystemParam, Marker> private::Sealed for FunctionSystem<F, P, Marker> {}
impl<F: SystemFunction<Marker, Param = P>, P: SystemParam, Marker> System
    for FunctionSystem<F, P, Marker>
{
    #[cfg(feature = "rc")]
    /// # 用途
    /// 在根 Running 内执行已准备系统；会话成功后才推进上次运行观察点。
    /// # Safety
    /// 调用方保证状态已准备且权限同源、完整；结果存活期不得兑现冲突权限。
    unsafe fn run_prepared(
        &mut self,
        world: &mut pi_ecs_core::world::World,
        this_run: ChangeTick,
    ) -> Result<(), EcsError> {
        /* 设计已确认：
        先核对 Initialized.world_identity 与来源 World，再检查准备有效；拆分借用 prepared_access、function 与参数 state。
        window 使用 self.last_run 和 this_run；以完整树调用 World::system_scope。
        会话内构造按值轻量描述，组合参数按位置派生无分配子描述，执行 begin/fetch/call/finish 并清理。
        scope 返回 Err 或 panic 不推进 last_run；只有 scope 成功才写入 this_run。
        */
        self.run_prepared_inner(world, this_run)
    }
    /// # 用途
    /// 借用本次有效完整声明，不复制权限集合或扩大授权。
    fn access_tree(&self) -> Result<&pi_ecs_core::access::AccessTree, EcsError> {
        self.access_tree_inner()
    }
    /// # 用途
    /// 在给定逻辑原型全集上展开真实排序权限，不读取业务值。
    fn ordering_access(
        &self,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        self.ordering_access_inner(logical)
    }
    /// # 用途
    /// 返回稳定系统身份，不复用已注销身份。
    fn id(&self) -> SystemId {
        Self::id_inner(self)
    }
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(&mut self, registration: &mut ParamRegistration<'_>) -> Result<(), EcsError> {
        /* 设计已确认：
        已初始化先核对 world_identity，异源原样拒绝，同源返回且不重复登记。
        未初始化消费配置初始化，全部成功后保存 World 身份及 State，prepared=false。
        失败由参数清偿本次部分登记，保存 InitializationFailed，保留其他系统成功结果。
        */
        self.initialize_inner(registration)
    }
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(
        &mut self,
        world: pi_ecs_core::world::WorldRef<'_>,
    ) -> Result<Vec<ResolvedAccess>, EcsError> {
        /* 设计已确认：
        先核对 Initialized.world_identity，异源原样拒绝；同源才使本次 prepared 失效。
        登记屏障已经完成；Uninitialized 直接拒绝，不消费配置或新增登记。
        Initialized：先使 prepared 失效；调用参数 prepare 并校验组合，成功才恢复 prepared。
        刷新失败保留长期状态和登记责任，不推进 last_run，可在条件修正后重试。
        InitializationFailed/Unregistered：拒绝，不消费配置或自动初始化。
        */
        Self::prepare_inner(self, world)
    }
    /// # 用途
    /// 使用已准备状态和本次授权调用系统；正常返回后完成参数，展开不补交。
    /// # Safety
    /// 调用方保证状态已准备且权限同源、完整；结果存活期不得兑现冲突权限。
    unsafe fn run(&mut self, context: SystemRunContext<'_>) {
        /* 设计已确认：
        检查已准备状态及同次授权；以 self.last_run 覆盖 context.window.last_run，本次 tick 来自运行入口。
        begin 建立拥有型 Pending，fetch 参数，再调用用户函数。
        用户正常返回、参数及派生引用结束后，按参数位置 finish 显式提交。
        本低层入口不更新 last_run；run_prepared 在外层 scope 成功后推进。生命周期错误直接 panic。
        panic 按 Rust 展开，不补交剩余工作、不回滚已发生写入或游标；单次操作部分初始化值须安全清理。
        执行器收束已启动任务后传播 panic；不提供捕获后继续模拟的协议，不构图。
        */
        Self::run_inner(self, context)
    }
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(&mut self, registration: &mut ParamRegistration<'_>) -> Result<(), EcsError> {
        /* 设计已确认：
        先核对 Initialized.world_identity，异源不消费状态也不撤销准备；同源确认全部借用收束后撤销准备。
        Initialized：消费长期状态，委托 P::unregister；其正常返回保证登记全部清偿。
        Uninitialized：只释放未消费配置；InitializationFailed/Unregistered 没有参数登记可重复解除。
        正常清理结束后记 Unregistered，保留原 last_run，不推进变化窗口；向调用者传回清理诊断。
        内部故障或析构 panic 不视为正常注销，停止 World，由停止后的整体清理承接未清责任。
        */
        Self::unregister_inner(self, registration)
    }
}
pub(crate) mod private {
    /// # 用途
    /// 框架封闭适配边界，避免外部绕过参数声明与运行授权协议。
    pub trait Sealed {}
}
// RC 构造与函数签名适配见 SystemBuilder/FunctionSignature，参数注入见 SystemParam，门禁见 World 运行作用域。
// 运行期系统动态增删/启停请求协议延期；首轮只保留既有静止点登记、注销与关闭。
// World 仅作为参数准备目标，不向用户函数注入任意共享可写 World。
