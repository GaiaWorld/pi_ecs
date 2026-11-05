//! 单系统长期状态及本次执行边界，不负责执行图定向或阶段推进。
use crate::system_param::{ParamRegistration, SystemParam};
use pi_ecs_core::{
    access::{AccessDeclaration, RunAccess},
    change_detection::{ChangeTick, ChangeWindow},
    error::EcsError,
};
/// Schedule 内稳定系统身份，不使用函数地址作为身份。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SystemId(
    /// 调度局部编号，复用及代际策略待定。
    pub u64,
);
/// 本次系统上下文，不能脱离授权保留组件引用。
pub struct SystemRunContext<'a> {
    /// 参数组合校验后的受限运行权限，持续到 finish 完成。
    pub(crate) access: RunAccess<'a>,
    /// 上次成功执行和本次 tick；首次 None 无下界，其余左开右闭，当前写入立即可观察。
    pub(crate) window: ChangeWindow,
}
/// 对参数签名的函数调用适配；不允许输出携带本次参数借用。
pub trait SystemFunction<P: SystemParam> {
    /// item 是本次参数元组；正常返回 ()，不在首版混入业务 Result 适配。
    fn call<'run>(&mut self, item: P::Item<'run>);
}
/// 调度持有的系统接口，首版只允许框架适配实现。
pub trait System: private::Sealed {
    /// 返回本调度系统身份，不改变状态。
    fn id(&self) -> SystemId;
    /// 阶段前初始化/刷新参数计划，返回尚未丢失参数边界的访问声明。
    /// 失败不执行系统；失效必需订阅不得被悄悄自动重新订阅。
    fn prepare(
        &mut self,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Vec<AccessDeclaration>, EcsError>;
    /// context 来自本次已准备系统，参数线程能力、World 和计划版本均匹配。
    /// 正常完成提交后更新 last_run；panic 不继续模拟，不回退通知游标。
    /// # Safety
    /// 调用者证明 context 覆盖整个执行与完成过程，且不存在冲突并行系统。
    unsafe fn run(&mut self, context: SystemRunContext<'_>) -> Result<(), EcsError>;
    /// 仅阶段前静止点调用，解除所有参数订阅责任；调用后系统不可运行。
    fn unregister(&mut self, registration: &mut ParamRegistration<'_>) -> Result<(), EcsError>;
}
/// 函数及参数长期状态；不保存本次 Query/Reader、组件引用或授权。
pub struct FunctionSystem<F, P: SystemParam> {
    /// 系统身份。
    pub(crate) id: SystemId,
    /// 用户函数或手写函数适配。
    pub(crate) function: F,
    /// 未初始化及注销后为 None，运行前必须准备成功。
    pub(crate) state: Option<P::State>,
    /// 构建时接收、初始化时消费的参数初值及恢复配置。
    pub(crate) config: Option<crate::system_builder::ParamConfig<P>>,
    /// 仅成功执行及完成提交后推进；None 表示尚未成功运行。
    pub(crate) last_run: Option<ChangeTick>,
    // @to_design 未初始化与已注销的状态区分、运行失败毒化及线程能力表示待闭合。
}
impl<F, P: SystemParam> private::Sealed for FunctionSystem<F, P> {}
impl<F: SystemFunction<P>, P: SystemParam> System for FunctionSystem<F, P> {
    fn id(&self) -> SystemId {
        crate::system_inner::id_inner(self)
    }
    fn prepare(
        &mut self,
        registration: &mut ParamRegistration<'_>,
    ) -> Result<Vec<AccessDeclaration>, EcsError> {
        crate::system_inner::prepare_inner(self, registration)
    }
    unsafe fn run(&mut self, context: SystemRunContext<'_>) -> Result<(), EcsError> {
        /* 设计已确认：
        检查已准备状态，使用本次 last_run/this_run 和受限授权。
        begin 建立 Pending，fetch 本次参数，调用用户函数。
        用户正常返回后释放参数及派生引用，finish 显式提交已接受修改。
        授权持续到 finish/guard 收束，再更新 last_run 并返回。
        panic 不回滚、不继续模拟；执行图负责停止派发和任务收束，本模块不构图。
        */
        crate::system_inner::run_inner(self, context)
    }
    fn unregister(&mut self, registration: &mut ParamRegistration<'_>) -> Result<(), EcsError> {
        crate::system_inner::unregister_inner(self, registration)
    }
}
pub(crate) mod private {
    pub trait Sealed {}
}
// @to_design 构造/函数签名适配、Query/Reader/Alter 注入、系统控制入口及 World 运行门禁待闭合。
// World 仅作为参数准备目标，不向用户函数注入任意共享可写 World。
