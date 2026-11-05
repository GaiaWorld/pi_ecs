//! 资源参数值访问配对芯；设计期失败桩。
pub(crate) fn multi_next<'a, T: Send + Sync + 'static>(
    _iter: &mut crate::resource_param::MultiResIter<'a, T>,
) -> Option<crate::resource_param::Res<'a, T>> {
    todo!("尚未实现逐项只读包装")
}
pub(crate) fn multi_len<T: Send + Sync + 'static>(
    _value: &crate::resource_param::MultiRes<'_, T>,
) -> usize {
    todo!("尚未实现多例数量")
}
pub(crate) fn multi_empty<T: Send + Sync + 'static>(
    _value: &crate::resource_param::MultiRes<'_, T>,
) -> bool {
    todo!("尚未实现多例空检查")
}
pub(crate) fn multi_iter<'a, T: Send + Sync + 'static>(
    _value: &'a crate::resource_param::MultiRes<'_, T>,
) -> crate::resource_param::MultiResIter<'a, T> {
    todo!("尚未实现多例遍历")
}
pub(crate) fn multi_structure<T: Send + Sync + 'static>(
    _value: &crate::resource_param::MultiRes<'_, T>,
) -> bool {
    todo!("尚未实现结构版本检查")
}

pub(crate) fn res_deref<'a, T: Send + Sync + 'static>(
    _value: &'a crate::resource_param::Res<'_, T>,
) -> &'a T {
    todo!("尚未实现参数只读访问")
}
pub(crate) fn res_is_added<T: Send + Sync + 'static>(
    _value: &crate::resource_param::Res<'_, T>,
) -> bool {
    todo!("尚未实现资源窗口检查")
}
pub(crate) fn res_is_changed<T: Send + Sync + 'static>(
    _value: &crate::resource_param::Res<'_, T>,
) -> bool {
    todo!("尚未实现资源窗口检查")
}
pub(crate) fn resmut_deref<'a, T: Send + Sync + 'static>(
    _value: &'a crate::resource_param::ResMut<'_, T>,
) -> &'a T {
    todo!("尚未实现参数只读访问")
}
pub(crate) fn resmut_deref_mut<'a, T: Send + Sync + 'static>(
    _value: &'a mut crate::resource_param::ResMut<'_, T>,
) -> &'a mut T {
    todo!("尚未实现参数可写访问")
}
pub(crate) fn resmut_is_added<T: Send + Sync + 'static>(
    _value: &crate::resource_param::ResMut<'_, T>,
) -> bool {
    todo!("尚未实现资源窗口检查")
}
pub(crate) fn resmut_is_changed<T: Send + Sync + 'static>(
    _value: &crate::resource_param::ResMut<'_, T>,
) -> bool {
    todo!("尚未实现资源窗口检查")
}
pub(crate) fn resmut_set_changed<T: Send + Sync + 'static>(
    _value: &mut crate::resource_param::ResMut<'_, T>,
) {
    todo!("尚未实现显式资源标记")
}
pub(crate) fn multiresmut_deref<'a, T: Send + Sync + 'static>(
    _value: &'a crate::resource_param::MultiResMut<'_, T>,
) -> &'a T {
    todo!("尚未实现参数只读访问")
}
pub(crate) fn multiresmut_deref_mut<'a, T: Send + Sync + 'static>(
    _value: &'a mut crate::resource_param::MultiResMut<'_, T>,
) -> &'a mut T {
    todo!("尚未实现参数可写访问")
}
pub(crate) fn multiresmut_is_added<T: Send + Sync + 'static>(
    _value: &crate::resource_param::MultiResMut<'_, T>,
) -> bool {
    todo!("尚未实现资源窗口检查")
}
pub(crate) fn multiresmut_is_changed<T: Send + Sync + 'static>(
    _value: &crate::resource_param::MultiResMut<'_, T>,
) -> bool {
    todo!("尚未实现资源窗口检查")
}
pub(crate) fn multiresmut_set_changed<T: Send + Sync + 'static>(
    _value: &mut crate::resource_param::MultiResMut<'_, T>,
) {
    todo!("尚未实现显式资源标记")
}
