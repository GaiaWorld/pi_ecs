//! 临时拥有型实体引用配对芯，仅失败桩。
pub(crate) fn weak_replace_from_inner<'w, const N: usize, T>(
    _value: &mut crate::owned_entity::WeakEntitiesMut<'_, 'w, N, T>,
    _index: usize,
    _target: &OwnedEntity<'w>,
) -> Result<(), crate::error::EcsError> {
    todo!("尚未实现弱位置替换及责任交还")
}

pub(crate) fn strong_ref_get<const N: usize, T>(
    _v: &crate::owned_entity::StrongEntitiesRef<'_, '_, N, T>,
    _index: usize,
) -> Result<Entity, crate::error::EcsError> {
    todo!("尚未实现数组身份读取")
}
pub(crate) fn strong_ref_get_owned<'w, const N: usize, T>(
    _v: &crate::owned_entity::StrongEntitiesRef<'_, 'w, N, T>,
    _index: usize,
) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
    todo!("尚未实现成员增持或弱升级")
}
pub(crate) fn strong_mut_get<const N: usize, T>(
    _v: &crate::owned_entity::StrongEntitiesMut<'_, '_, N, T>,
    _index: usize,
) -> Result<Entity, crate::error::EcsError> {
    todo!("尚未实现数组身份读取")
}
pub(crate) fn strong_mut_get_owned<'w, const N: usize, T>(
    _v: &crate::owned_entity::StrongEntitiesMut<'_, 'w, N, T>,
    _index: usize,
) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
    todo!("尚未实现成员增持或弱升级")
}
pub(crate) fn weak_ref_get<const N: usize, T>(
    _v: &crate::owned_entity::WeakEntitiesRef<'_, '_, N, T>,
    _index: usize,
) -> Result<Entity, crate::error::EcsError> {
    todo!("尚未实现数组身份读取")
}
pub(crate) fn weak_ref_upgrade<'w, const N: usize, T>(
    _v: &crate::owned_entity::WeakEntitiesRef<'_, 'w, N, T>,
    _index: usize,
) -> Result<Option<OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现成员增持或弱升级")
}
pub(crate) fn weak_mut_get<const N: usize, T>(
    _v: &crate::owned_entity::WeakEntitiesMut<'_, '_, N, T>,
    _index: usize,
) -> Result<Entity, crate::error::EcsError> {
    todo!("尚未实现数组身份读取")
}
pub(crate) fn weak_mut_upgrade<'w, const N: usize, T>(
    _v: &crate::owned_entity::WeakEntitiesMut<'_, 'w, N, T>,
    _index: usize,
) -> Result<Option<OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现成员增持或弱升级")
}
pub(crate) fn try_clone_inner<'w>(
    _value: &OwnedEntity<'w>,
) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
    todo!("尚未实现显式可失败增持")
}
use crate::{
    entity::Entity,
    entity_container::EntityInsertError,
    owned_entity::{OwnedEntity, StrongEntitiesMut},
};
pub(crate) fn entity_inner(_value: &OwnedEntity<'_>) -> Entity {
    todo!("尚未实现身份读取")
}
pub(crate) fn forget_inner(_value: OwnedEntity<'_>) -> Entity {
    todo!("尚未实现责任移交")
}
pub(crate) fn drop_inner(_value: &mut OwnedEntity<'_>) {
    todo!("尚未实现完整身份校验及强计数释放")
}
pub(crate) fn replace_inner<'w, const N: usize, T>(
    _target: &mut StrongEntitiesMut<'_, 'w, N, T>,
    _index: usize,
    _value: OwnedEntity<'w>,
) -> Result<OwnedEntity<'w>, EntityInsertError<'w>> {
    todo!("尚未实现受控引用替换")
}
