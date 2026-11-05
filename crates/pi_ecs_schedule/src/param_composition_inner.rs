//! 参数集借用配对芯，只有失败桩。
use crate::param_composition::{BorrowOnlyParam, ParamSetItem};
use pi_ecs_core::error::EcsError;
pub(crate) fn p0_inner<'a, A: BorrowOnlyParam, B: BorrowOnlyParam>(
    _set: &'a mut ParamSetItem<'_, '_, A, B>,
) -> Result<A::Item<'a>, EcsError> {
    todo!("@to_design 短借用授权重绑")
}
pub(crate) fn p1_inner<'a, A: BorrowOnlyParam, B: BorrowOnlyParam>(
    _set: &'a mut ParamSetItem<'_, '_, A, B>,
) -> Result<B::Item<'a>, EcsError> {
    todo!("@to_design 短借用授权重绑")
}
