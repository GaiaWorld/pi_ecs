#[cfg(feature = "rc")]
impl<C: Send + 'static> Sealed for &CommandSender<C> {}
// SAFETY: 仅借用本位置 State 中的句柄，RC 限所属线程；队列内部按操作短借，不取 World 业务引用。
#[cfg(feature = "rc")]
unsafe impl<C: Send + 'static> SystemParam for &CommandSender<C> {
    type Config = ();
    type State = CommandSenderParamState<C>;
    type Pending = ();
    type Item<'run> = &'run CommandSender<C>;
    /// # 用途
    /// config 无额外配置；registration 仅领取预登记句柄，不创建队列或选择容量。
    /// Receiver 资格由 State 唯一拥有，组合失败 Drop 归还，不清 payload。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// # 用途
    /// state 须属于 world 且服务开放；返回空 Together，不访问组件/资源，不制造生产消费排序边。
    /// RC 执行受所属线程限制；即时可见性取决于实际运行顺序，有先后需求由宿主阶段安排。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// # 用途
    /// 核对 state/world 同源后返回空 Together；句柄能力来自初始化而非 World 取值权限。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// state 为初始化状态，logical 为潜在原型全集；命令参数不贡献原型访问或隐式排序边。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// state 保持原句柄/资格；无本次工作区，不领取新 Receiver，不移动 payload。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// # 用途
    /// 核对 state/context 同源，只短借 State 句柄；不重建 World 引用或申请运行门禁。
    /// pending 为单位工作区；Receiver 及 read batch 不得越过本次 state 借用。
    /// # Safety
    /// 调用者保证本位置 State 独占借用及 RC 所属线程；同位置不能重复兑现或延长结果寿命。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// # 用途
    /// state/pending/context 同次执行且结果已释放；无延迟提交，不回退 send/取消/next 已发生的变化。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// # 用途
    /// registration 须同源且无活 Item/batch；消费 state，Receiver Drop 归还资格，Sender Drop 释放句柄。
    /// 不删除队列、清空 payload 或撤销其他 Sender；组合仍清理所有成员后报告首诊断。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

#[cfg(feature = "rc")]
impl<C: Send + 'static> Sealed for &mut CommandReceiver<C> {}
// SAFETY: 仅借用本位置 State 中的句柄，RC 限所属线程；队列内部按操作短借，不取 World 业务引用。
#[cfg(feature = "rc")]
unsafe impl<C: Send + 'static> SystemParam for &mut CommandReceiver<C> {
    type Config = ();
    type State = CommandReceiverParamState<C>;
    type Pending = ();
    type Item<'run> = &'run mut CommandReceiver<C>;
    /// # 用途
    /// config 无额外配置；registration 仅领取预登记句柄，不创建队列或选择容量。
    /// Receiver 资格由 State 唯一拥有，组合失败 Drop 归还，不清 payload。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// # 用途
    /// state 须属于 world 且服务开放；返回空 Together，不访问组件/资源，不制造生产消费排序边。
    /// RC 执行受所属线程限制；即时可见性取决于实际运行顺序，有先后需求由宿主阶段安排。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// # 用途
    /// 核对 state/world 同源后返回空 Together；句柄能力来自初始化而非 World 取值权限。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// state 为初始化状态，logical 为潜在原型全集；命令参数不贡献原型访问或隐式排序边。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// state 保持原句柄/资格；无本次工作区，不领取新 Receiver，不移动 payload。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// # 用途
    /// 核对 state/context 同源，只短借 State 句柄；不重建 World 引用或申请运行门禁。
    /// pending 为单位工作区；Receiver 及 read batch 不得越过本次 state 借用。
    /// # Safety
    /// 调用者保证本位置 State 独占借用及 RC 所属线程；同位置不能重复兑现或延长结果寿命。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// # 用途
    /// state/pending/context 同次执行且结果已释放；无延迟提交，不回退 send/取消/next 已发生的变化。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// # 用途
    /// registration 须同源且无活 Item/batch；消费 state，Receiver Drop 归还资格，Sender Drop 释放句柄。
    /// 不删除队列、清空 payload 或撤销其他 Sender；组合仍清理所有成员后报告首诊断。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

use crate::{
    local::{Local, LocalConfig, LocalState},
    notification_reader::*,
    param_composition::{
        NamedParam, ParamAccess, ParamSet, ParamSetItem, ParamView, SequentialParam,
    },
    param_state::*,
    resource_param::*,
    spawn::SpawnConfig,
    system::SystemRunContext,
    system_param::{private::Sealed, ParamRegistration, SystemParam},
};
#[cfg(feature = "rc")]
use pi_ecs_core::command::{CommandReceiver, CommandSender};
use pi_ecs_core::{
    alter::{QueryAlter, QueryAlterState},
    bundle::{Bundle, SystemBundle},
    error::EcsError,
    lookup::Lookup,
    query::{Query, QueryData, QueryFilter, QueryMut, QueryState, ReadOnlyQueryData},
    spawn::{Spawn, SpawnState},
    world::WorldRef,
};
#[path = "system_param_impls_inner.rs"]
mod system_param_impls_inner;
use system_param_impls_inner::{ParamImpl, RebindImpl};

impl<Q: ReadOnlyQueryData + 'static, F: QueryFilter + 'static> Sealed for Query<'_, Q, F> {}
unsafe impl<Q: ReadOnlyQueryData + 'static, F: QueryFilter + 'static> SystemParam
    for Query<'_, Q, F>
{
    type Config = ();
    type State = QueryState<Q, F>;
    type Pending = ();
    type Item<'run> = Query<'run, Q, F>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}
// SAFETY: 结果仅短借成员，重绑不扩权；不保留跨切换物理视图。
unsafe impl<Q: ReadOnlyQueryData + 'static, F: QueryFilter + 'static> SequentialParam
    for Query<'_, Q, F>
{
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

impl<Q: QueryData + 'static, F: QueryFilter + 'static> Sealed for QueryMut<'_, Q, F> {}
unsafe impl<Q: QueryData + 'static, F: QueryFilter + 'static> SystemParam for QueryMut<'_, Q, F> {
    type Config = ();
    type State = QueryState<Q, F>;
    type Pending = ();
    type Item<'run> = QueryMut<'run, Q, F>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}
// SAFETY: 结果仅短借成员，重绑不扩权；不保留跨切换物理视图。
unsafe impl<Q: QueryData + 'static, F: QueryFilter + 'static> SequentialParam
    for QueryMut<'_, Q, F>
{
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

impl<Q: QueryData + 'static, F: QueryFilter + 'static> Sealed for Lookup<'_, Q, F> {}
unsafe impl<Q: QueryData + 'static, F: QueryFilter + 'static> SystemParam for Lookup<'_, Q, F> {
    type Config = ();
    type State = QueryState<Q, F>;
    type Pending = ();
    type Item<'run> = Lookup<'run, Q, F>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}
// SAFETY: 结果仅短借成员，重绑不扩权；不保留跨切换物理视图。
unsafe impl<Q: QueryData + 'static, F: QueryFilter + 'static> SequentialParam for Lookup<'_, Q, F> {
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

impl<Q, F, Add, Remove> Sealed for QueryAlter<'_, Q, F, Add, Remove>
where
    Q: QueryData + 'static,
    F: QueryFilter + 'static,
    Add: SystemBundle<Shape = Add> + 'static,
    Remove: Bundle + 'static,
{
}
unsafe impl<Q, F, Add, Remove> SystemParam for QueryAlter<'_, Q, F, Add, Remove>
where
    Q: QueryData + 'static,
    F: QueryFilter + 'static,
    Add: SystemBundle<Shape = Add> + 'static,
    Remove: Bundle + 'static,
{
    type Config = ();
    type State = QueryAlterState<Q, F>;
    type Pending = ();
    type Item<'run> = QueryAlter<'run, Q, F, Add, Remove>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<B: SystemBundle<Shape = B> + 'static> Sealed for Spawn<'_, B> {}
// SAFETY: the ParamSet short borrow excludes sibling results; Spawn retains no physical cache.
unsafe impl<B: SystemBundle<Shape = B> + 'static> SequentialParam for Spawn<'_, B> {
    /// # 用途
    /// state 为固定目标登记，context 为 Spawn 位置短授权；没有物理地址缓存可刷新。
    /// 实际绑定仍检查同源及完整目标，不重新申请门禁、分配 tick 或创建原型。
    /// # Safety
    /// 上一成员所有派生结果均已离开；不得在子位置活跃时使用父授权。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}
// SAFETY: fetch 仅兑现当前 Spawn 位置的同源结构叶子，不交付组件借用。
unsafe impl<B: SystemBundle<Shape = B> + 'static> SystemParam for Spawn<'_, B> {
    type Config = SpawnConfig;
    type State = SpawnState<B>;
    type Pending = ();
    type Item<'run> = Spawn<'run, B>;
    /// # 用途
    /// config 给出本位置固定分区，registration 只查冻结 World 的分区和 B 目标；失败不登记状态。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// # 用途
    /// state 须属于 world；返回真实目标的展开结构写。目标未发布时，保留结构写独占和潜在流权限。
    /// 不创建原型、绑定业务引用或另取运行门禁；失败不交付本次准备。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// # 用途
    /// state 须属于 world；返回声明固定分区和完整原型的单个叶子，供阶段逻辑种子与取参授权使用。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// state 是固定目标，logical 是本阶段闭包；只展开目标成员写，无源，无实体读取。
    /// 若找不到目标，返回错误，不补造原型或授权。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// state 是本参数位置的登记状态；无延迟工作，返回 ()。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// # 用途
    /// 使用 state、空 pending 及 context 的本位置结构授权与 this_run 兑现创建参数。
    /// 绑定失败按 SystemParam 协议 panic；成功参数及派生句柄不超过本次授权。
    /// # Safety
    /// 调用方须排除本系统及并行任务的冲突值/成员借用，且同一次位置授权只兑现一次。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// # 用途
    /// state、空 pending 和 context 属同一运行；结果已结束，所有成功创建早已发布，本步无副作用。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// # 用途
    /// 在框架同源 registration 下消费 state；无订阅、追踪或实体责任需要清偿。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<T: Send + Sync + 'static> Sealed for Res<'_, T> {}
unsafe impl<T: Send + Sync + 'static> SystemParam for Res<'_, T> {
    type Config = ();
    type State = ResourceParamState;
    type Pending = ();
    type Item<'run> = Res<'run, T>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}
// SAFETY: 结果仅短借成员，重绑不扩权；不保留跨切换物理视图。
unsafe impl<T: Send + Sync + 'static> SequentialParam for Res<'_, T> {
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

impl<T: Send + Sync + 'static> Sealed for Option<Res<'_, T>> {}
unsafe impl<T: Send + Sync + 'static> SystemParam for Option<Res<'_, T>> {
    type Config = ();
    type State = ResourceParamState;
    type Pending = ();
    type Item<'run> = Option<Res<'run, T>>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}
// SAFETY: 结果仅短借成员，重绑不扩权；不保留跨切换物理视图。
unsafe impl<T: Send + Sync + 'static> SequentialParam for Option<Res<'_, T>> {
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

impl<T: Send + Sync + 'static> Sealed for ResMut<'_, T> {}
unsafe impl<T: Send + Sync + 'static> SystemParam for ResMut<'_, T> {
    type Config = ();
    type State = ResourceParamState;
    type Pending = ();
    type Item<'run> = ResMut<'run, T>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}
// SAFETY: 结果仅短借成员，重绑不扩权；不保留跨切换物理视图。
unsafe impl<T: Send + Sync + 'static> SequentialParam for ResMut<'_, T> {
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

impl<T: Send + Sync + 'static> Sealed for Option<ResMut<'_, T>> {}
unsafe impl<T: Send + Sync + 'static> SystemParam for Option<ResMut<'_, T>> {
    type Config = ();
    type State = ResourceParamState;
    type Pending = ();
    type Item<'run> = Option<ResMut<'run, T>>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}
// SAFETY: 结果仅短借成员，重绑不扩权；不保留跨切换物理视图。
unsafe impl<T: Send + Sync + 'static> SequentialParam for Option<ResMut<'_, T>> {
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

impl<T: Send + Sync + 'static> Sealed for MultiRes<'_, T> {}
unsafe impl<T: Send + Sync + 'static> SystemParam for MultiRes<'_, T> {
    type Config = ();
    type State = MultiResourceParamState;
    type Pending = MultiResourcePending;
    type Item<'run> = MultiRes<'run, T>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}
// SAFETY: 结果仅短借成员，重绑不扩权；不保留跨切换物理视图。
unsafe impl<T: Send + Sync + 'static> SequentialParam for MultiRes<'_, T> {
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

impl<T: Send + Sync + 'static> Sealed for MultiResMut<'_, T> {}
unsafe impl<T: Send + Sync + 'static> SystemParam for MultiResMut<'_, T> {
    type Config = MultiResConfig<T>;
    type State = MultiResourceProducerState;
    type Pending = ();
    type Item<'run> = MultiResMut<'run, T>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}
// SAFETY: 结果仅短借成员，重绑不扩权；不保留跨切换物理视图。
unsafe impl<T: Send + Sync + 'static> SequentialParam for MultiResMut<'_, T> {
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

impl<T: Send + 'static> Sealed for Local<'_, T> {}
unsafe impl<T: Send + 'static> SystemParam for Local<'_, T> {
    type Config = LocalConfig<T>;
    type State = LocalState<T>;
    type Pending = ();
    type Item<'run> = Local<'run, T>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<T: pi_ecs_core::component::Component> Sealed for AddedReader<'_, T> {}
#[cfg(feature = "rc")]
unsafe impl<T: pi_ecs_core::component::Component> SystemParam for AddedReader<'_, T> {
    type Config = ();
    type State = ReaderParamState;
    type Pending = ();
    type Item<'run> = AddedReader<'run, T>;
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        _config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        ReaderParamState::added_param_initialize::<T>(registration)
    }
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        ReaderParamState::added_param_prepare(state, world)
    }
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        Self::reader_declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        Self::reader_ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        ReaderParamState::added_param_begin(state)
    }
    /// # Safety
    /// 调用者保证通知观察权限与写入冲突已排除，且授权覆盖读取结果。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        // SAFETY: 观察权限由 prepare 声明，context 提供本次只读通知访问。
        unsafe { ReaderParamState::added_param_fetch::<T>(state, pending, context) }
    }
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        ReaderParamState::added_param_finish(state, pending, context)
    }
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        ReaderParamState::added_param_unregister::<T>(state, registration)
    }
}

impl<T: pi_ecs_core::component::Component> Sealed for ChangedReader<'_, T> {}
#[cfg(feature = "rc")]
unsafe impl<T: pi_ecs_core::component::Component> SystemParam for ChangedReader<'_, T> {
    type Config = ();
    type State = ReaderParamState;
    type Pending = ();
    type Item<'run> = ChangedReader<'run, T>;
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        _config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        ReaderParamState::changed_param_initialize::<T>(registration)
    }
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        ReaderParamState::changed_param_prepare(state, world)
    }
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        Self::reader_declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        Self::reader_ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        ReaderParamState::changed_param_begin(state)
    }
    /// # Safety
    /// 调用者保证通知观察权限与写入冲突已排除，且授权覆盖读取结果。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        // SAFETY: 观察权限由 prepare 声明，context 提供本次只读通知访问。
        unsafe { ReaderParamState::changed_param_fetch::<T>(state, pending, context) }
    }
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        ReaderParamState::changed_param_finish(state, pending, context)
    }
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        ReaderParamState::changed_param_unregister::<T>(state, registration)
    }
}

impl<T: pi_ecs_core::component::Component> Sealed for RemovedReader<'_, T> {}
#[cfg(feature = "rc")]
unsafe impl<T: pi_ecs_core::component::Component> SystemParam for RemovedReader<'_, T> {
    type Config = ();
    type State = ReaderParamState;
    type Pending = ();
    type Item<'run> = RemovedReader<'run, T>;
    /// 解析组件、建立独立 Removed 订阅；同类型不同参数位置不合并。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        _config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        ReaderParamState::removed_param_initialize::<T>(registration)
    }
    /// 声明 Removed 观察权限；不读取组件值或推进读者游标。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        ReaderParamState::removed_param_prepare::<T>(state, world)
    }
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        Self::reader_declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        Self::reader_ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        ReaderParamState::removed_param_begin::<T>(state)
    }
    /// # Safety
    /// 调用者保证通知观察权限与结构/写入冲突已排除，且授权覆盖旧值读取。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        // SAFETY: 通知观察权限由 prepare 声明，context 保持只读 World 授权。
        unsafe { ReaderParamState::removed_param_fetch::<T>(state, pending, context) }
    }
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        ReaderParamState::removed_param_finish::<T>(state, pending, context)
    }
    /// 静止点注销独立订阅；成功或失败都不保留旧句柄。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        ReaderParamState::removed_param_unregister::<T>(state, registration)
    }
}

impl<P0: SystemParam> Sealed for (P0,) {}
unsafe impl<P0: SystemParam> SystemParam for (P0,) {
    type Config = (P0::Config,);
    type State = (P0::State,);
    type Pending = (P0::Pending,);
    type Item<'run> = (P0::Item<'run>,);
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<P0: SystemParam, P1: SystemParam> Sealed for (P0, P1) {}
unsafe impl<P0: SystemParam, P1: SystemParam> SystemParam for (P0, P1) {
    type Config = (P0::Config, P1::Config);
    type State = (P0::State, P1::State);
    type Pending = (P0::Pending, P1::Pending);
    type Item<'run> = (P0::Item<'run>, P1::Item<'run>);
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<P0: SystemParam, P1: SystemParam, P2: SystemParam> Sealed for (P0, P1, P2) {}
unsafe impl<P0: SystemParam, P1: SystemParam, P2: SystemParam> SystemParam for (P0, P1, P2) {
    type Config = (P0::Config, P1::Config, P2::Config);
    type State = (P0::State, P1::State, P2::State);
    type Pending = (P0::Pending, P1::Pending, P2::Pending);
    type Item<'run> = (P0::Item<'run>, P1::Item<'run>, P2::Item<'run>);
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<P0: SystemParam, P1: SystemParam, P2: SystemParam, P3: SystemParam> Sealed
    for (P0, P1, P2, P3)
{
}
unsafe impl<P0: SystemParam, P1: SystemParam, P2: SystemParam, P3: SystemParam> SystemParam
    for (P0, P1, P2, P3)
{
    type Config = (P0::Config, P1::Config, P2::Config, P3::Config);
    type State = (P0::State, P1::State, P2::State, P3::State);
    type Pending = (P0::Pending, P1::Pending, P2::Pending, P3::Pending);
    type Item<'run> = (
        P0::Item<'run>,
        P1::Item<'run>,
        P2::Item<'run>,
        P3::Item<'run>,
    );
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<P0: SystemParam, P1: SystemParam, P2: SystemParam, P3: SystemParam, P4: SystemParam> Sealed
    for (P0, P1, P2, P3, P4)
{
}
unsafe impl<P0: SystemParam, P1: SystemParam, P2: SystemParam, P3: SystemParam, P4: SystemParam>
    SystemParam for (P0, P1, P2, P3, P4)
{
    type Config = (P0::Config, P1::Config, P2::Config, P3::Config, P4::Config);
    type State = (P0::State, P1::State, P2::State, P3::State, P4::State);
    type Pending = (
        P0::Pending,
        P1::Pending,
        P2::Pending,
        P3::Pending,
        P4::Pending,
    );
    type Item<'run> = (
        P0::Item<'run>,
        P1::Item<'run>,
        P2::Item<'run>,
        P3::Item<'run>,
        P4::Item<'run>,
    );
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<
        P0: SystemParam,
        P1: SystemParam,
        P2: SystemParam,
        P3: SystemParam,
        P4: SystemParam,
        P5: SystemParam,
    > Sealed for (P0, P1, P2, P3, P4, P5)
{
}
unsafe impl<
        P0: SystemParam,
        P1: SystemParam,
        P2: SystemParam,
        P3: SystemParam,
        P4: SystemParam,
        P5: SystemParam,
    > SystemParam for (P0, P1, P2, P3, P4, P5)
{
    type Config = (
        P0::Config,
        P1::Config,
        P2::Config,
        P3::Config,
        P4::Config,
        P5::Config,
    );
    type State = (
        P0::State,
        P1::State,
        P2::State,
        P3::State,
        P4::State,
        P5::State,
    );
    type Pending = (
        P0::Pending,
        P1::Pending,
        P2::Pending,
        P3::Pending,
        P4::Pending,
        P5::Pending,
    );
    type Item<'run> = (
        P0::Item<'run>,
        P1::Item<'run>,
        P2::Item<'run>,
        P3::Item<'run>,
        P4::Item<'run>,
        P5::Item<'run>,
    );
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<
        P0: SystemParam,
        P1: SystemParam,
        P2: SystemParam,
        P3: SystemParam,
        P4: SystemParam,
        P5: SystemParam,
        P6: SystemParam,
    > Sealed for (P0, P1, P2, P3, P4, P5, P6)
{
}
unsafe impl<
        P0: SystemParam,
        P1: SystemParam,
        P2: SystemParam,
        P3: SystemParam,
        P4: SystemParam,
        P5: SystemParam,
        P6: SystemParam,
    > SystemParam for (P0, P1, P2, P3, P4, P5, P6)
{
    type Config = (
        P0::Config,
        P1::Config,
        P2::Config,
        P3::Config,
        P4::Config,
        P5::Config,
        P6::Config,
    );
    type State = (
        P0::State,
        P1::State,
        P2::State,
        P3::State,
        P4::State,
        P5::State,
        P6::State,
    );
    type Pending = (
        P0::Pending,
        P1::Pending,
        P2::Pending,
        P3::Pending,
        P4::Pending,
        P5::Pending,
        P6::Pending,
    );
    type Item<'run> = (
        P0::Item<'run>,
        P1::Item<'run>,
        P2::Item<'run>,
        P3::Item<'run>,
        P4::Item<'run>,
        P5::Item<'run>,
        P6::Item<'run>,
    );
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<
        P0: SystemParam,
        P1: SystemParam,
        P2: SystemParam,
        P3: SystemParam,
        P4: SystemParam,
        P5: SystemParam,
        P6: SystemParam,
        P7: SystemParam,
    > Sealed for (P0, P1, P2, P3, P4, P5, P6, P7)
{
}
unsafe impl<
        P0: SystemParam,
        P1: SystemParam,
        P2: SystemParam,
        P3: SystemParam,
        P4: SystemParam,
        P5: SystemParam,
        P6: SystemParam,
        P7: SystemParam,
    > SystemParam for (P0, P1, P2, P3, P4, P5, P6, P7)
{
    type Config = (
        P0::Config,
        P1::Config,
        P2::Config,
        P3::Config,
        P4::Config,
        P5::Config,
        P6::Config,
        P7::Config,
    );
    type State = (
        P0::State,
        P1::State,
        P2::State,
        P3::State,
        P4::State,
        P5::State,
        P6::State,
        P7::State,
    );
    type Pending = (
        P0::Pending,
        P1::Pending,
        P2::Pending,
        P3::Pending,
        P4::Pending,
        P5::Pending,
        P6::Pending,
        P7::Pending,
    );
    type Item<'run> = (
        P0::Item<'run>,
        P1::Item<'run>,
        P2::Item<'run>,
        P3::Item<'run>,
        P4::Item<'run>,
        P5::Item<'run>,
        P6::Item<'run>,
        P7::Item<'run>,
    );
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<A: SequentialParam, B: SequentialParam> Sealed for ParamSet<(A, B)> {}
unsafe impl<A: SequentialParam, B: SequentialParam> SystemParam for ParamSet<(A, B)> {
    type Config = (A::Config, B::Config);
    type State = (A::State, B::State);
    type Pending = (A::Pending, B::Pending);
    type Item<'run> = ParamSetItem<'run, 'run, A, B>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

impl<P: SystemParam, M: ParamView<P>> Sealed for NamedParam<P, M> {}
unsafe impl<P: SystemParam, M: ParamView<P>> SystemParam for NamedParam<P, M> {
    type Config = P::Config;
    type State = P::State;
    type Pending = P::Pending;
    type Item<'run> = M::Item<'run>;
    /// 消费每位置配置，解析状态/登记需求；组合部分失败清理本次登记，不修改业务值。
    /// # 用途
    /// 消费完整配置并登记本参数责任；失败按参数协议清偿本次部分登记。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError> {
        <Self as ParamImpl>::initialize_inner(config, registration)
    }
    /// 刷新同 World 计划并保留参数边界；不绑定业务引用、不取得新运行门禁。
    /// # 用途
    /// 核对同源 World 并刷新访问计划；失败撤销本次准备，不推进观察点。
    fn prepare(state: &mut Self::State, world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        <Self as ParamImpl>::prepare_inner(state, world)
    }
    /// 创建本次独立工作区；普通参数为单位类型，Alter 无延迟工作区，集合读者保留版本提交记录。
    /// # 用途
    /// 保留参数边界并报告完整潜在权限；不构造业务引用或新增运行授权。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        <Self as ParamImpl>::declaration_inner(state, world)
    }
    /// # 用途
    /// 保留真实结构过滤语义，展开潜在原型上的排序权限，不绑定业务引用。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        <Self as ParamImpl>::ordering_inner(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut Self::State) -> Self::Pending {
        <Self as ParamImpl>::begin_inner(state)
    }
    /// 借用本次 state/pending/context，复用系统 tick/窗口与授权；不复制长期业务地址。
    /// # Safety
    /// 调用者保证全部参数别名/线程能力已校验且授权覆盖所有结果和 finish，不能通过该入口扩大权限。
    /// # 用途
    /// 以本次同源授权绑定参数；返回结果仅活到运行借用结束，不新增权限。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: SystemRunContext<'run>,
    ) -> Self::Item<'run> {
        unsafe { <Self as ParamImpl>::fetch_inner(state, pending, context) }
    }
    /// Item 及派生引用结束后完成 pending；Alter 已在每次调用内发布，多例读者提交版本，Reader 不回退已消费游标。
    /// # 用途
    /// 结果借用结束后提交本次工作区；仅成功执行提交观察点，不回退已消费通知。
    fn finish(state: &mut Self::State, pending: Self::Pending, context: &SystemRunContext<'_>) {
        <Self as ParamImpl>::finish_inner(state, pending, context)
    }
    /// 静止点消费 state，使用所属 registration 清偿全部登记；Ok/Err 返回均无残留，不撤销组外参数登记。
    /// 组合按 SystemParam::unregister 逐位置清理全部成员后报告首错，不能提前返回；不可完成清理则停止。
    /// # 用途
    /// 在静止点清偿同源登记责任；全部成员清理后报告诊断，异源拒绝保留责任。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        <Self as ParamImpl>::unregister_inner(state, registration)
    }
}

// SAFETY: 成员结果及批次受 ParamSet 短借限制；Alter 即时发布，Reader 不保留旧批次。
unsafe impl<Q, F, Add, Remove> SequentialParam for QueryAlter<'_, Q, F, Add, Remove>
where
    Q: QueryData + 'static,
    F: QueryFilter + 'static,
    Add: SystemBundle<Shape = Add> + 'static,
    Remove: Bundle + 'static,
{
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

// SAFETY: 成员结果及批次受 ParamSet 短借限制；Alter 即时发布，Reader 不保留旧批次。
#[cfg(feature = "rc")]
unsafe impl<T: pi_ecs_core::component::Component> SequentialParam for AddedReader<'_, T> {
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

// SAFETY: 成员结果及批次受 ParamSet 短借限制；Alter 即时发布，Reader 不保留旧批次。
#[cfg(feature = "rc")]
unsafe impl<T: pi_ecs_core::component::Component> SequentialParam for ChangedReader<'_, T> {
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

// SAFETY: 成员结果及批次受 ParamSet 短借限制；Alter 即时发布，Reader 不保留旧批次。
#[cfg(feature = "rc")]
unsafe impl<T: pi_ecs_core::component::Component> SequentialParam for RemovedReader<'_, T> {
    /// # 用途
    /// 每次成员短借前核对同源并同步当前结构；不重新登记、分配 tick 或扩大声明。
    /// # Safety
    /// 旧成员所有结果/视图已结束，context 是该 Exclusive 位置的原授权短借。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        unsafe { <Self as RebindImpl>::rebind_inner(state, context) }
    }
}

// SAFETY: Local borrows only its own state and keeps no view across member switches.
unsafe impl<T: Send + 'static> SequentialParam for Local<'_, T> {
    /// # 用途
    /// Local 只借自身状态；保留当前成员的短借描述，无需刷新 World 视图。
    /// # Safety
    /// 上一个成员的结果均已结束，context 来自此 Exclusive 位置的独占短借。
    unsafe fn rebind<'r>(
        _: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, EcsError> {
        Self::local_rebind(context)
    }
}
