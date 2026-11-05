//! 变化追踪配对芯，仅失败桩。
use crate::change_detection::Mut;
pub(crate) fn set_changed_inner<T>(_value: &mut Mut<'_, T>) {
    todo!("@to_design tick 槽和通知写通道")
}
pub(crate) fn deref_inner<'a, T>(_value: &'a Mut<'_, T>) -> &'a T {
    todo!("尚未实现")
}
pub(crate) fn deref_mut_inner<'a, T>(_value: &'a mut Mut<'_, T>) -> &'a mut T {
    todo!("@to_design 按需标记和观察权限")
}
