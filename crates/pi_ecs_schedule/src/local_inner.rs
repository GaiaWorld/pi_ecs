//! Local 借用配对芯；设计期失败桩。
pub(crate) fn local_deref<'a, T: Send + 'static>(_value: &'a crate::local::Local<'_, T>) -> &'a T {
    todo!("尚未实现参数只读访问")
}
pub(crate) fn local_deref_mut<'a, T: Send + 'static>(
    _value: &'a mut crate::local::Local<'_, T>,
) -> &'a mut T {
    todo!("尚未实现参数可写访问")
}
