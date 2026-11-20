//! 参数组合：同时访问和分时访问必须保留不同的权限结构。
#[path = "param_composition_inner.rs"]
mod param_composition_inner;
use crate::{system::SystemRunContext, system_param::SystemParam};
use pi_ecs_core::access::ResolvedAccess;
/// 参数访问树；检查不能过早扁平化而丢失 ParamSet 边界。
pub enum ParamAccess {
    /// 单个合法参数本次展开后的权限；同系统先按此检查别名。
    Leaf(Vec<ResolvedAccess>),
    /// 成员可以同时使用，逐对检查潜在冲突。
    Together(Vec<ParamAccess>),
    /// 成员分时使用，成员内部仍校验；对组外以全部成员权限并集检查。
    Exclusive(Vec<ParamAccess>),
}
/// # 用途
/// 可在 ParamSet 内反复短借的参数，包括即时 Spawn/Alter、查询、资源与通知 Reader。
/// # Safety
/// Item 及所有派生引用/物理视图必须受成员短借限制；State/Pending 不留跨切换缓冲引用。
/// 每次取参前 rebind 同步当前结构，不增加静态权限，不提交旧成员或重新初始化登记。
pub unsafe trait SequentialParam: SystemParam {
    /// # 用途
    /// state/context 同源；成员旧结果全部结束后，每次 p0/p1（含重复同成员）均调用。
    /// 查询/Lookup/Alter 按当前原型总量补扫后缀，重绑稳定根并刷新子授权版本；
    /// Reader 复核订阅后在 fetch/read 取得新的批次尾界，不复用旧批次或回退游标。
    /// 资源复核存在性/版本，不重新登记；静态声明和窗口/this_run 不变，不分配 tick。
    /// 返回仅更新结构快照的同一 Exclusive 子授权，不扩范围、不重排系统；失败不交付 Item。
    /// # Safety
    /// 调用方持 ParamSet 独占短借，覆盖本成员结果，排除兄弟结果和组外冲突引用；
    /// 不恢复整份 World 引用；同步只访问当前声明覆盖的元数据与本成员缓存。
    unsafe fn rebind<'r>(
        state: &mut Self::State,
        context: SystemRunContext<'r>,
    ) -> Result<SystemRunContext<'r>, pi_ecs_core::error::EcsError>;
}
/// 参数集描述：两项起步，后续元组扩展复用相同短借用规则。
pub struct ParamSet<P>(
    /// 类型描述，不预先构造全部成员引用。
    std::marker::PhantomData<fn() -> P>,
);
/// 本次双成员参数集，只保存状态/工作区和授权，p0/p1 时才 fetch。
pub struct ParamSetItem<'a, 'w, A: SequentialParam, B: SequentialParam> {
    /// 第一个参数长期状态的本次独占借用。
    pub(crate) first: &'a mut A::State,
    /// 第二个参数长期状态的本次独占借用。
    pub(crate) second: &'a mut B::State,
    /// 第一项独立工作区。
    pub(crate) first_pending: &'a mut A::Pending,
    /// 第二项独立工作区。
    pub(crate) second_pending: &'a mut B::Pending,
    /// 按值轻量描述，只引用长期声明/World，窗口值复制，不构造上下文树。
    pub(crate) context: SystemRunContext<'w>,
}
impl<A: SequentialParam, B: SequentialParam> ParamSetItem<'_, '_, A, B> {
    /// 短借用 self，所有结果使用结束前不能访问另一成员；每个成员内部仍拒绝可变别名。
    /// 状态/Pending 短借 self，按值派生 Exclusive 描述并缩短到该短借；不能借住局部描述。
    /// 直接返回成员参数，生命周期错误按 SystemParam::fetch 直接 panic，不增加错误标记协议。
    /// 每次派生 Exclusive 子描述后调用 SequentialParam::rebind，再以短借 fetch。
    /// Alter 每次操作返回前已发布，切换/Drop/finish 不补提交；旧结果结束即释放物理视图。
    /// 仅长期稳定容器地址与已扫描结构进度可留存；新成员必须观察先前已发布的结构和通知。
    /// rebind/fetch 失败直接 panic，由系统会话标停，不调用用户代码或继续切换。
    ///
    /// 先结束第一项结果，才可切换第二项；样例仅编译，不调用设计桩。
    /// ```no_run
    /// use pi_ecs_schedule::{param_composition::ParamSetItem, resource_param::ResMut};
    /// fn sequential(set: &mut ParamSetItem<'_, '_, ResMut<'static, u32>, ResMut<'static, u32>>) {
    ///     { let mut first = set.p0(); *first += 1; }
    ///     let mut second = set.p1();
    ///     *second += 1;
    /// }
    /// ```
    /// 活的第一项结果阻止借用兄弟，不能仅凭状态字段不同绕过互斥。
    /// ```compile_fail,E0499
    /// use pi_ecs_schedule::{param_composition::ParamSetItem, resource_param::ResMut};
    /// fn overlapping(set: &mut ParamSetItem<'_, '_, ResMut<'static, u32>, ResMut<'static, u32>>) {
    ///     let mut first = set.p0();
    ///     let mut second = set.p1();
    ///     *first += 1;
    ///     *second += 1;
    /// }
    /// ```
    /// 同一成员也不能在旧结果仍活跃时重复取出。
    /// ```compile_fail,E0499
    /// use pi_ecs_schedule::{param_composition::ParamSetItem, resource_param::ResMut};
    /// fn repeated(set: &mut ParamSetItem<'_, '_, ResMut<'static, u32>, ResMut<'static, u32>>) {
    ///     let mut first = set.p0();
    ///     let mut again = set.p0();
    ///     *first += 1;
    ///     *again += 1;
    /// }
    /// ```
    pub fn p0(&mut self) -> A::Item<'_> {
        /* 设计已确认：结束旧成员借用→派生短 Exclusive 子授权→A::rebind 同步当前结构→A::fetch。
        保留同一 Pending 和固定窗口；重绑失败展开标停，不延迟提交 Alter，不恢复整份 World。 */
        Self::p0_inner(self)
    }
    /// 与 p0 互斥的短借用，不构造可同时持有的冲突结果；生命周期错误与 p0 相同，直接 panic。
    pub fn p1(&mut self) -> B::Item<'_> {
        Self::p1_inner(self)
    }
}
/// 命名结果适配，只能从已有参数结果构造业务视图，不得制造新 World 权限。
/// 长期状态、初始化、finish 和注销全部属于 P，普通命名组合仍代表同时借用。
pub trait ParamView<P: SystemParam> {
    /// 命名结构体可包含 P 返回的借用，不能延长其生命周期。
    type Item<'a>;
    /// 消费 item 并重组字段；不得把短借用升级为 static。
    fn from_param<'a>(item: P::Item<'a>) -> Self::Item<'a>;
}
/// 以 M 为已有参数组合 P 提供命名结果，不开放外部 SystemParam 安全边界。
pub struct NamedParam<P: SystemParam, M: ParamView<P>>(
    /// 参数描述及纯结果重组类型。
    std::marker::PhantomData<fn() -> (P, M)>,
);
// ParamSet/NamedParam 与元组的关联 Item、配置/状态/工作区及生命周期委托见 system_param_impls.rs。
// p0/p1 的短借用约束由上述声明及文档编译检查表达，实际组合与清偿芯仍待实现。
// 参数集不提高跨系统并行度；对外仍占用全部成员权限，不允许跳过组外冲突检查。

#[cfg(test)]
mod value_context_compile_checks {
    use super::*;
    // 仅编译：真实调用 trait fetch，返回 Item 不引用本函数的局部 context 对象。
    unsafe fn fetch_local<'r, P: SystemParam>(
        state: &'r mut P::State,
        pending: &'r mut P::Pending,
        context: SystemRunContext<'r>,
    ) -> P::Item<'r> {
        unsafe { P::fetch(state, pending, context) }
    }
    // 同时借用两个互不重叠成员，子描述局部值移入 fetch；不需要稳定运行树。
    unsafe fn pair<'r, A: SystemParam, B: SystemParam>(
        state: &'r mut (A::State, B::State),
        pending: &'r mut (A::Pending, B::Pending),
        context: SystemRunContext<'r>,
    ) -> Result<(A::Item<'r>, B::Item<'r>), pi_ecs_core::error::EcsError> {
        let first = context.child(0)?;
        let second = context.child(1)?;
        Ok((
            unsafe { fetch_local::<A>(&mut state.0, &mut pending.0, first) },
            unsafe { fetch_local::<B>(&mut state.1, &mut pending.1, second) },
        ))
    }
    // ParamSet 成员描述来自 self，但按值交出；返回仍由 self 的独占短借约束。
    unsafe fn first<'r, A: SequentialParam, B: SequentialParam>(
        set: &'r mut ParamSetItem<'_, '_, A, B>,
    ) -> Result<A::Item<'r>, pi_ecs_core::error::EcsError> {
        let context = unsafe { set.context.exclusive_child(0)? };
        Ok(unsafe { A::fetch(&mut *set.first, &mut *set.first_pending, context) })
    }
}
