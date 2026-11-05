//! 参数组合：同时访问和分时访问必须保留不同的权限结构。
use crate::{system::SystemRunContext, system_param::SystemParam};
use pi_ecs_core::{access::AccessDeclaration, error::EcsError};
/// 参数访问树；检查不能过早扁平化而丢失 ParamSet 边界。
pub enum ParamAccess {
    /// 单个合法参数的实际声明。
    Leaf(Vec<AccessDeclaration>),
    /// 成员可以同时使用，逐对检查潜在冲突。
    Together(Vec<ParamAccess>),
    /// 成员分时使用，成员内部仍校验；对组外以全部成员权限并集检查。
    Exclusive(Vec<ParamAccess>),
}
/// 可在参数集中分时借用的参数，首版仅开放无结构提交的 Query/资源适配。
/// Alter 不实现此标记，避免切换成员时隐含结构提交。
pub trait BorrowOnlyParam: SystemParam {}
/// 参数集描述：两项起步，后续元组扩展复用相同短借用规则。
pub struct ParamSet<P>(
    /// 类型描述，不预先构造全部成员引用。
    std::marker::PhantomData<fn() -> P>,
);
/// 本次双成员参数集，只保存状态/工作区和授权，p0/p1 时才 fetch。
pub struct ParamSetItem<'a, 'w, A: BorrowOnlyParam, B: BorrowOnlyParam> {
    /// 第一个参数长期状态的本次独占借用。
    pub(crate) first: &'a mut A::State,
    /// 第二个参数长期状态的本次独占借用。
    pub(crate) second: &'a mut B::State,
    /// 第一项独立工作区。
    pub(crate) first_pending: &'a mut A::Pending,
    /// 第二项独立工作区。
    pub(crate) second_pending: &'a mut B::Pending,
    /// 不扩大本次系统已取得的权限。
    pub(crate) context: &'a SystemRunContext<'w>,
}
impl<A: BorrowOnlyParam, B: BorrowOnlyParam> ParamSetItem<'_, '_, A, B> {
    /// 短借用 self，所有结果使用结束前不能访问另一成员；每个成员内部仍拒绝可变别名。
    pub fn p0(&mut self) -> Result<A::Item<'_>, EcsError> {
        crate::param_composition_inner::p0_inner(self)
    }
    /// 与 p0 互斥的短借用，不构造可同时持有的冲突结果。
    pub fn p1(&mut self) -> Result<B::Item<'_>, EcsError> {
        crate::param_composition_inner::p1_inner(self)
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
// @to_design ParamSet/NamedParam 的 SystemParam 关联 Item 生命周期适配、元组组合及 BorrowOnlyParam 框架实现待闭合。
// 参数集不提高跨系统并行度；对外仍占用全部成员权限，不允许跳过组外冲突检查。
