//! 列操作配对芯，设计期显式失败桩。
use crate::{
    column::Column,
    ptr::{OwningPtr, Ptr, PtrMut},
    settle::SettlePlan,
};
use pi_ecs_types::error::EcsError;
pub(crate) fn new_inner(
    _component: pi_ecs_types::component::ComponentId,
    _layout: std::alloc::Layout,
) -> Result<Column, EcsError> {
    todo!("尚未实现按布局构造空列")
}
pub(crate) fn get_inner(_column: &Column, _row: usize) -> Ptr<'_> {
    todo!("尚未实现")
}
pub(crate) fn get_mut_inner(_column: &mut Column, _row: usize) -> PtrMut<'_> {
    todo!("尚未实现")
}
pub(crate) fn prepare_inner(_column: &mut Column, _row: usize) -> Result<(), EcsError> {
    todo!("@to_design pi_arr 擦除缓冲与对齐")
}
pub(crate) fn insert_inner(_column: &mut Column, _row: usize, _value: OwningPtr<'_>) {
    todo!("@to_design OwningPtr 拥有权移交")
}
pub(crate) fn remove_inner(_column: &mut Column, _row: usize) -> OwningPtr<'_> {
    todo!("@to_design OwningPtr 移出内存归属")
}
pub(crate) fn settle_inner(_column: &mut Column, _plan: &SettlePlan) {
    todo!("@to_design 擦除缓冲整理")
}
