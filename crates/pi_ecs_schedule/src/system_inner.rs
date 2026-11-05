//! 单系统生命周期配对芯，仅设计失败桩。
use crate::{
    system::{FunctionSystem, SystemFunction, SystemId, SystemRunContext},
    system_param::{ParamRegistration, SystemParam},
};
use pi_ecs_core::{access::AccessDeclaration, error::EcsError};
pub(crate) fn id_inner<F, P: SystemParam>(_system: &FunctionSystem<F, P>) -> SystemId {
    todo!("尚未实现")
}
pub(crate) fn prepare_inner<F: SystemFunction<P>, P: SystemParam>(
    _system: &mut FunctionSystem<F, P>,
    _registration: &mut ParamRegistration<'_>,
) -> Result<Vec<AccessDeclaration>, EcsError> {
    todo!("@to_design 受限登记和参数适配")
}
pub(crate) fn run_inner<F: SystemFunction<P>, P: SystemParam>(
    _system: &mut FunctionSystem<F, P>,
    _context: SystemRunContext<'_>,
) -> Result<(), EcsError> {
    todo!("@to_design 参数适配、窗口接线和执行失败毒化")
}
pub(crate) fn unregister_inner<F: SystemFunction<P>, P: SystemParam>(
    _system: &mut FunctionSystem<F, P>,
    _registration: &mut ParamRegistration<'_>,
) -> Result<(), EcsError> {
    todo!("@to_design 受限注销与系统状态")
}
