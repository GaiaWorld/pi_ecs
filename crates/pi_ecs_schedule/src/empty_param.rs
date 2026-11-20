//! 零参数系统的空参数协议，不访问 World 或登记责任。
#[path = "empty_param_inner.rs"]
mod empty_param_inner;
use crate::{
    param_composition::ParamAccess,
    system::SystemRunContext,
    system_param::{ParamRegistration, SystemParam},
};
use pi_ecs_core::{error::EcsError, world::WorldRef};
// SAFETY: 空协议不产生任何 World 数据访问；各行为设计芯仍为失败桩。
unsafe impl SystemParam for () {
    type Config = ();
    type State = ();
    type Pending = ();
    type Item<'run> = ();
    /// # 用途
    /// config/registration 属于初始化调用；无登记和状态，仅建立空协议。
    fn initialize(config: (), registration: &mut ParamRegistration<'_>) -> Result<(), EcsError> {
        ParamRegistration::empty_initialize(config, registration)
    }
    /// # 用途
    /// state/world 属于本次准备；空访问，不改变 World。
    fn prepare(state: &mut (), world: WorldRef<'_>) -> Result<ParamAccess, EcsError> {
        SystemRunContext::empty_prepare(state, world)
    }
    /// # 用途
    /// state 无运行数据，工作区为空。
    fn declaration(
        state: &Self::State,
        world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        SystemRunContext::empty_declaration(state, world)
    }
    /// # 用途
    /// 空参数不访问任何逻辑原型，返回空权限。
    fn ordering_access(
        state: &Self::State,
        logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, EcsError> {
        SystemRunContext::empty_ordering_access(state, logical)
    }
    /// # 用途
    /// 为本参数位置建立本次工作区；不推进观察点，不取得业务引用。
    fn begin(state: &mut ()) {
        SystemRunContext::empty_begin(state)
    }
    /// # 用途
    /// 返回空参数，不构造业务引用。
    /// # Safety
    /// 调用者遵守同次执行生命周期，context 不逃逸。
    unsafe fn fetch<'run>(
        state: &'run mut (),
        pending: &'run mut (),
        context: SystemRunContext<'run>,
    ) {
        SystemRunContext::empty_fetch(state, pending, context)
    }
    /// # 用途
    /// state/pending/context 属于同次执行，无提交责任。
    fn finish(state: &mut (), pending: (), context: &SystemRunContext<'_>) {
        SystemRunContext::empty_finish(state, pending, context)
    }
    /// # 用途
    /// 消费空 state，无订阅或生产者需要解除。
    fn unregister(state: (), registration: &mut ParamRegistration<'_>) -> Result<(), EcsError> {
        ParamRegistration::empty_unregister(state, registration)
    }
}
