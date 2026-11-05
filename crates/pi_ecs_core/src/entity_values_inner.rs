//! 值集合配对芯；设计失败桩。
pub(crate) fn strong_entities_inner<'w, const N: usize, T: 'static>(
    _input: &mut EntityValues<'w>,
    _values: [crate::owned_entity::OwnedEntity<'w>; N],
) -> Result<(), crate::entity_values::StrongArrayInputError<'w, N>> {
    todo!("尚未实现整组校验准备及强责任转移")
}
pub(crate) fn weak_entities_inner<'w, const N: usize, T: 'static>(
    _input: &mut EntityValues<'w>,
    _values: [&crate::owned_entity::OwnedEntity<'w>; N],
) -> Result<(), EcsError> {
    todo!("尚未实现弱请求准备和失败释放临时 weak")
}
pub(crate) fn drop_weak_request_inner(_request: &mut crate::entity_values::WeakArrayRequest<'_>) {
    todo!("尚未实现未安装弱责任释放")
}

pub(crate) fn builtin_empty_inner<C: 'static>(
    _values: &mut crate::entity_values::EntityValues<'_>,
) -> Result<(), crate::error::EcsError> {
    todo!("尚未实现内置空头请求登记")
}
use crate::{entity_values::EntityValues, error::EcsError};
pub(crate) fn new_inner<'w>() -> EntityValues<'w> {
    todo!("尚未实现")
}
pub(crate) fn insert_inner<
    T: crate::component::Component<Access = crate::component::PlainAccess>,
>(
    _values: &mut EntityValues<'_>,
    _value: T,
) -> Result<(), EcsError> {
    todo!("尚未实现")
}
