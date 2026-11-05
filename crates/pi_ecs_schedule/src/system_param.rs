//! 参数长期状态与本次借用分离；首版封闭实现，不在此构建执行图。
use pi_ecs_core::{error::EcsError, world::World};
/// 参数登记上下文，仅阶段前静止点可构造，不授予业务值写权限。
pub struct ParamRegistration<'a> {
    /// 解析类型及计划所对应的 World。
    pub(crate) world: &'a World,
    // @to_design core 订阅/追踪需求的受限维护授权尚未接通，禁止强转 &mut World。
}
/// 框架参数的生命周期协议，具体 Query/Reader/Alter 适配随后接入。
pub trait SystemParam: private::Sealed {
    /// 每参数位置独立的拥有型初始化配置；组合配置与参数元组同形，不按类型合并。
    /// 普通 Query/Res 使用无额外配置，Local/多例提供初值及恢复能力；不得伪造 State 工厂。
    type Config;
    /// 长期状态，不得保存组件引用、运行授权或本次参数。
    type State;
    /// 本次待完成工作，独立于用户借用；Alter 请求由此保留到 finish。
    type Pending;
    /// 用户可见参数，生命周期不超过本次授权和长期状态借用。
    type Item<'run>;
    /// config 为本参数位置的配置，初始化消费其初值/工厂；registration 为阶段前维护授权。
    /// 元组逐位置初始化，不合并同类型 Local/订阅；任一失败清理本次已初始化成员。
    /// 失败须清理本次部分登记，不修改业务值，不撤销此前已成功的控制变更。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError>;
    /// state 属于 world；阶段前同步计划，返回保留参数边界的声明列表。
    /// 调用者先检查同参数及参数组合别名，再汇总权限；失败不运行系统。
    fn prepare(
        state: &mut Self::State,
        world: &World,
    ) -> Result<crate::param_composition::ParamAccess, EcsError>;
    /// 为本次执行准备独立工作区，不借用业务组件；失败不得留下半执行修改。
    fn begin(state: &mut Self::State) -> Result<Self::Pending, EcsError>;
    /// state/pending/context 属于同次执行；构造用户参数，不扩大权限。
    /// # Safety
    /// 调用者已检查全部参数组合和并发冲突，线程能力合法，授权覆盖所有返回引用及 finish。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: &'run crate::system::SystemRunContext<'_>,
    ) -> Result<Self::Item<'run>, EcsError>;
    /// 用户参数及所有派生引用已释放；context 仍持本次授权，pending 保存已接受修改。
    /// 正常返回前显式提交 Alter，不依靠 Drop；开始执行移动后失败 panic，不回滚。
    fn finish(
        state: &mut Self::State,
        pending: Self::Pending,
        context: &crate::system::SystemRunContext<'_>,
    );
    /// 阶段前注销 state 的订阅/需求；不得有本参数活借用，释放所有登记责任。
    /// 最后一个通知订阅取消后停止对应流推送，不取消其他过滤器的 tick 需求。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError>;
}
pub(crate) mod private {
    pub trait Sealed {}
}
// 条件跳过不执行 begin/fetch/finish，不推进 last_run 或通知游标；配置仅首次初始化消费。
// 普通元组同时可用，NamedParam 不扩权限，ParamSet 按需短借用且对外使用权限并集。
// @to_design Query/Reader/Alter 具体适配、元组 GAT 及线程能力证明待闭合；不提供假成功默认实现。
