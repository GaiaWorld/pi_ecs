//! 资源宿主配对芯，仅失败桩。
use crate::{change_detection::Mut, error::EcsError, resource::ResourceId, world::World};
pub(crate) fn register_resource_inner<T: Send + Sync + 'static>(
    _world: &mut World,
) -> Result<ResourceId, EcsError> {
    todo!("@to_design 资源存储与授权接线")
}
pub(crate) fn insert_resource_inner<T: Send + Sync + 'static>(
    _world: &World,
    _value: T,
) -> Result<Option<T>, EcsError> {
    todo!("@to_design 资源存储与授权接线")
}
pub(crate) fn remove_resource_inner<T: Send + Sync + 'static>(
    _world: &World,
) -> Result<Option<T>, EcsError> {
    todo!("@to_design 资源存储与授权接线")
}
pub(crate) fn pin_resource_inner<'a, T: Send + Sync + 'static>(
    _access: &'a crate::world::WorldPin<'_>,
) -> Option<&'a T> {
    todo!("尚未实现授权资源读取")
}
pub(crate) fn write_resource_inner<'a, T: Send + Sync + 'static>(
    _access: &'a crate::world::WorldWrite<'_>,
) -> Option<&'a T> {
    todo!("尚未实现授权资源读取")
}
pub(crate) fn write_resource_mut_inner<'a, T: Send + Sync + 'static>(
    _access: &'a mut crate::world::WorldWrite<'_>,
) -> Option<Mut<'a, T>> {
    todo!("尚未实现授权资源写入")
}
pub(crate) fn pin_multi_inner<'a, T: Send + Sync + 'static>(
    _access: &'a crate::world::WorldPin<'_>,
) -> Option<crate::resource::MultiResourceRead<'a, T>> {
    todo!("尚未实现授权多例视图")
}
pub(crate) fn write_multi_inner<'a, T: Send + Sync + 'static>(
    _access: &'a crate::world::WorldWrite<'_>,
) -> Option<crate::resource::MultiResourceRead<'a, T>> {
    todo!("尚未实现授权多例视图")
}
pub(crate) fn multi_len_inner<T>(_view: &crate::resource::MultiResourceRead<'_, T>) -> usize {
    todo!("尚未实现视图长度")
}
pub(crate) fn multi_empty_inner<T>(_view: &crate::resource::MultiResourceRead<'_, T>) -> bool {
    todo!("尚未实现视图空检查")
}
pub(crate) fn multi_iter_inner<'a, T>(
    _view: &'a crate::resource::MultiResourceRead<'_, T>,
) -> std::slice::Iter<'a, T> {
    todo!("尚未实现视图遍历")
}
