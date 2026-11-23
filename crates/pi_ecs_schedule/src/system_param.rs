//! 参数长期状态与本次借用分离；开放 unsafe 实现协议，不在此构建执行图。
#[path = "system_param_inner.rs"]
mod system_param_inner;
use pi_ecs_core::{error::EcsError, world::WorldRef};
/// 参数登记上下文，包装 core 受限能力，不授予业务值访问或整份 World 借用。
pub struct ParamRegistration<'a> {
    /// 阶段首遍登记或静止点清偿作用域交付；不可复制、不保存到长期 State。
    access: pi_ecs_core::parameter_registration::ParameterRegistration<'a>,
    #[cfg(feature = "rc")]
    system: Option<(crate::system::SystemId, usize)>,
}
/// 框架参数生命周期协议；具体关联类型和委托见 system_param_impls.rs。
/// # Safety
/// 实现者保证 prepare 声明完整覆盖 fetch/finish 的访问，Item 不越过授权，不能制造冲突引用。
/// 排序声明在绑定生命周期内固定：完整 Q/F、读写、通知、固定增删、资源及生产者身份不得动态改变。
/// 变更须注销旧实例并登记新实例；恢复重绑通过布局代次失效，新原型只改变匹配结果。
/// 初始化/注销责任、Pending 提交、失败停止和线程约束须保持框架协议；普通组合优先使用派生。
/// 从登记领取的 CommandReceiver 必须由本参数 State 独占持有，供本实例的 Item 使用；不可泄漏、移入全局或转交其他实例。
/// 在组合失败或注销时归还同一资格；此规则也约束公开的 access 入口及其调用者。
pub unsafe trait SystemParam {
    /// 每参数位置独立的拥有型初始化配置；组合配置与参数元组同形，不按类型合并。
    /// 普通 Query/Res 使用无额外配置，Local/多例提供初值及恢复能力；不得伪造 State 工厂。
    type Config;
    /// 长期状态，不得保存组件引用、运行授权或本次参数。
    type State: 'static;
    /// 本次待完成工作，独立于用户借用；即时 Alter 为 ()，不保存请求到 finish。
    /// 失败不执行补偿提交；部分构造、消费及移交责任必须可清理，不能靠普通 Drop 假设未知值已析构。
    type Pending: 'static;
    /// 用户可见参数，生命周期不超过本次授权和长期状态借用。
    type Item<'run>;
    /// config 为本参数位置的配置，初始化消费其初值/工厂；registration 为根 Running 阶段首遍的受限登记授权。
    /// 查询通过 registration 的 query_state，Reader 通过 subscribe_notice，多例写通过 register_producer；不调用宿主维护入口。
    /// 元组逐位置初始化，不合并同类型 Local/订阅；任一失败清理本次已初始化成员。
    /// 失败返回前须清理本次部分登记，不修改业务值，不撤销此前已成功的控制变更。
    /// 组合清理复用 unregister 的完整清偿契约，不因清理诊断跳过其余已初始化成员；返回最初初始化错误。
    /// 查询参数初始化经 QueryState::new 登记逐组件 tick 需求；QueryAlter/Lookup 同样归查询状态拥有。
    /// Reader 只登记真实 NoticeSubscription，不额外登记通知需求副本，不强制开启活值 tick。
    /// RC 命令借用参数通过 registration 取得已登记队列句柄，Receiver 为 State 的唯一消费资格。
    /// 清理无法完成或析构 panic 时停止 World，不把残留责任作为普通 Err 返回。
    fn initialize(
        config: Self::Config,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Self::State, EcsError>;
    /// state 属于 world；Stage 全部活动系统初始化完成、登记集合稳定后同步计划。
    /// 不初始化其他参数、不新增/注销订阅或生产者，不执行用户工厂；缺失必要登记返回 Err。
    /// 可缓存同源冻结容器根地址；不得保存跨运行的可搬移值/tick 缓冲、业务引用或本次授权。
    /// 启动前重新绑定运行视图但不可绕过登记屏障；返回保留参数边界的声明列表。
    /// 调用者先检查同参数及参数组合别名，再汇总权限；失败不运行系统。
    /// 冻结后首次绑定固定相关 Table；新增变体仅扫描各 Table 未检查后缀，失败不推进扫描完成记录。
    /// 系统总量未变可跳过结构匹配扫描，仍执行资源存在性等必要准备；不在这里修改系统执行顺序。
    /// 多例写按 ProducerRegistration 查询 ProducerBinding，首次/版本变化刷新本参数缓存；
    /// 不遍历其他 Stage，不改稳定 WriteSlot 声明，不重新消费初值，失败使本次准备失效。
    /// 多例读实际数组/结构版本于 fetch 绑定；prepare 不推进 previous_version。
    fn prepare(
        state: &mut Self::State,
        world: WorldRef<'_>,
    ) -> Result<crate::param_composition::ParamAccess, EcsError>;
    /// # 用途
    /// state 为已初始化且属于 world 的参数状态；返回完整声明树，不从 ResolvedAccess 反推。
    /// 只读取长期登记与冻结元数据；不依赖 prepare 写入的运行绑定，也不检查业务资源实例是否存在。
    /// 与 prepare 的结构/位置一致，包含原始范围、资源、通知和固定增删；无访问参数返回空 Together。
    /// 元组为 Together，NamedParam 保持底层 P 的原树形，ParamSet 为 Exclusive；错误不产生授权，不改变业务值。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError>;
    /// # 用途
    /// state 为已初始化状态，logical 为同一排序构建的潜在原型全集；不读取 prepare 的运行绑定。
    /// 用真实 Q/F 结构匹配逻辑组合（含 OR/Option），不能仅用简化 AccessTree.scope 反推。
    /// Alter 目标完整闭包在外层生成；本方法不创建原型、不改运行匹配缓存、不生成授权。
    /// 组合按原参数位置拼接，对外含 ParamSet 全部权限；错误不交付半结果。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError>;
    /// state 为本次运行独占借用的长期状态；准备独立工作区，不借用业务组件。
    /// 成功直接返回工作区，生命周期错误 panic；部分构造遵守 Rust 展开安全，不补调用 finish。
    fn begin(state: &mut Self::State) -> Self::Pending;
    /// state/pending/context 属于同次执行；构造用户参数，不扩大权限。
    /// 直接返回 Item，缺失必需资源等生命周期错误 panic；初次与 ParamSet 按需 fetch 规则一致。
    /// 部分构造遵守 Rust 展开安全，不提供捕获后继续模拟的保证；普通业务点查仍保留自身 Result。
    /// # Safety
    /// 调用者已检查全部参数组合和并发冲突，线程能力合法，授权覆盖所有返回引用及 finish。
    /// 组合 fetch 按位置派生并按值传递子描述，只引用原声明/World，禁止借用局部临时 context，叶子只接收自身授权；NamedParam 不改树形。
    /// RC 经 context.access 的统一 UnsafeWorldCell 取值，不重建整份 &World/&mut World。
    /// ParamSet 按值保存轻量描述，经 &mut self 短借 State/Pending 并缩短描述寿命，不构造上下文树。
    /// 句柄复制不放宽参数声明；ParamSet 轮流 fetch，结果和物理缓存不得跨切换保留。
    /// 父/子描述不是可独立兑现的线性授权：Together 每位置只 fetch 一次，不能重复同一位置，
    /// 不能在子项结果存活时再用父描述 fetch 整组；新 State/Pending 也不能消除同一 World 的值别名。
    /// Exclusive 每次取参必须由 ParamSetItem 的独占短借覆盖全部成员结果；原父描述只为
    /// 派生其他已证明兼容的位置及借用结束后的 finish 保留，不另开一份并发运行授权。
    unsafe fn fetch<'run>(
        state: &'run mut Self::State,
        pending: &'run mut Self::Pending,
        context: crate::system::SystemRunContext<'run>,
    ) -> Self::Item<'run>;
    /// 用户参数及所有派生引用已释放；context 仍持本次授权，pending 保存已接受修改。
    /// 仅用户调用正常返回且参数借用结束后调用；即时 Alter 的 Pending=()，不在 finish/Drop 发布。
    /// 组合按参数位置完成，中途 panic 停止 World，不重试或补交其余成员；已发布部分不回滚。
    /// 消费 pending 期间必须维持已初始化/已移交责任记录，panic 不能导致重复析构或丢失隐藏值清理责任。
    /// MultiRes 仅提交本次实际 fetch 写入 Pending 的 observed_version；未 fetch/跳过/调用失败不推进。
    /// 不读取稍后新增槽的版本冒充本次已观察版本；该观察点与写者位置缓存独立。
    fn finish(
        state: &mut Self::State,
        pending: Self::Pending,
        context: &crate::system::SystemRunContext<'_>,
    );
    /// registration 为所属 World 的受限清偿上下文；初始化失败清理允许在登记作用域内，正常注销使用静止点清偿作用域。
    /// state 为本参数拥有的长期状态，不得有活借用；不借整份 World，不调用宿主 Idle 入口。
    /// 消费 state 并解除全部订阅、追踪需求及生产者槽责任；Ok 和 Err 正常返回均保证完整清偿，不能重试该 state。
    /// Err 仅报告清偿过程的诊断；无法完成清理的内部故障或析构 panic 停止 World，不承诺继续模拟。
    /// 组合参数按配置位置依次清理全部成员，收集首个错误，不能用提前返回跳过其余成员。
    /// 最后一个通知订阅取消后停止对应流推送，不取消其他过滤器的 tick 需求。
    /// Query/Lookup/QueryAlter 通过 registration 的 unregister_query 清偿所属查询责任，不能只 Drop 长期状态遗留需求。
    /// 生产者槽移除按正常注销析构其值；通知历史旧值仍由通知维护回收，不在这里提前析构。
    /// 命令 Receiver State 由 Drop 归还资格，Sender State 释放句柄；均不清空队列或撤回已消费命令。
    fn unregister(
        state: Self::State,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError>;
}
impl<'a> ParamRegistration<'a> {
    /// # 用途
    /// 包装本次受限登记能力，系统游标在初始化具体系统时建立。
    pub(crate) fn new(
        access: pi_ecs_core::parameter_registration::ParameterRegistration<'a>,
    ) -> Self {
        Self {
            access,
            #[cfg(feature = "rc")]
            system: None,
        }
    }

    /// # 用途
    /// 使用本系统的独立位置游标初始化全部参数，返回时还原外层游标。
    pub(crate) fn initialize_system<P: SystemParam>(
        &mut self,
        system: crate::system::SystemId,
        config: P::Config,
    ) -> Result<P::State, EcsError> {
        self.initialize_system_inner::<P>(system, config)
    }

    /// # 用途
    /// 按声明顺序初始化一个参数；若其未推进位置，计作一个普通叶子。
    pub(crate) fn initialize_parameter<P: SystemParam>(
        &mut self,
        config: P::Config,
    ) -> Result<P::State, EcsError> {
        self.initialize_parameter_inner::<P>(config)
    }

    /// # 用途
    /// 领取当前生产参数位置并前移游标；没有系统登记上下文时返回错误。
    #[cfg(feature = "rc")]
    pub(crate) fn producer_position(
        &mut self,
    ) -> Result<(crate::system::SystemId, usize), EcsError> {
        self.producer_position_inner()
    }

    /// # 用途
    /// 返回受限 core 能力短借，不暴露整份 World；允许操作由来源作用域模式检查。
    /// 外部 unsafe SystemParam 可复用本能力完成初始化或清偿，不能移出/保存该作用域。
    /// 短借期间不可重入登记上下文；不会解锁运行门禁或绕过只清偿模式。
    pub fn access(
        &mut self,
    ) -> &mut pi_ecs_core::parameter_registration::ParameterRegistration<'a> {
        &mut self.access
    }
}
pub(crate) mod private {
    /// # 用途
    /// 框架封闭适配边界，避免外部绕过参数声明与运行授权协议。
    pub trait Sealed {}
}
// 条件跳过不执行 begin/fetch/finish，不推进 last_run 或通知游标；配置仅首次初始化消费。
// 普通元组同时可用，NamedParam 不扩权限，ParamSet 按需短借用且对外使用权限并集。
// 具体参数、1..8 元组、ParamSet 和 NamedParam 的关联类型及委托见 system_param_impls.rs。
// State/Pending 为拥有型 static 状态，禁止借用运行上下文；非 rc 由系统对象 Send 传递要求其 Send，
// 不要求 Sync；rc 不要求 Send。不为并行伪造证明。
// RC 跨包绑定、登记授权与派生引用收束见各参数壳及 core 受限入口；配对芯明确失败，不提供假成功默认实现。
