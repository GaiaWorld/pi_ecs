//! 容器专用访问配对芯，仅失败桩。
pub(crate) fn list_ref_front_owned<'w, T: 'static>(
    _v: &crate::entity_container::EntityListRef<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现首成员增持")
}
pub(crate) fn list_ref_iter_owned<'a, 'w, T: 'static>(
    _v: &'a crate::entity_container::EntityListRef<'_, 'w, T>,
) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
    todo!("尚未实现逐项拥有遍历")
}
pub(crate) fn list_mut_front_owned<'w, T: 'static>(
    _v: &crate::entity_container::EntityListMut<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现首成员增持")
}
pub(crate) fn list_mut_iter_owned<'a, 'w, T: 'static>(
    _v: &'a crate::entity_container::EntityListMut<'_, 'w, T>,
) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
    todo!("尚未实现逐项拥有遍历")
}
pub(crate) fn queue_ref_front_owned<'w, T: 'static>(
    _v: &crate::entity_container::EntityQueueRef<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现首成员增持")
}
pub(crate) fn queue_ref_iter_owned<'a, 'w, T: 'static>(
    _v: &'a crate::entity_container::EntityQueueRef<'_, 'w, T>,
) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
    todo!("尚未实现逐项拥有遍历")
}
pub(crate) fn queue_mut_front_owned<'w, T: 'static>(
    _v: &crate::entity_container::EntityQueueMut<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现首成员增持")
}
pub(crate) fn queue_mut_iter_owned<'a, 'w, T: 'static>(
    _v: &'a crate::entity_container::EntityQueueMut<'_, 'w, T>,
) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
    todo!("尚未实现逐项拥有遍历")
}
pub(crate) fn deque_ref_front_owned<'w, T: 'static>(
    _v: &crate::entity_container::EntityDequeRef<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现首成员增持")
}
pub(crate) fn deque_ref_iter_owned<'a, 'w, T: 'static>(
    _v: &'a crate::entity_container::EntityDequeRef<'_, 'w, T>,
) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
    todo!("尚未实现逐项拥有遍历")
}
pub(crate) fn deque_ref_back_owned<'w, T: 'static>(
    _v: &crate::entity_container::EntityDequeRef<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现尾成员增持")
}
pub(crate) fn deque_ref_get_owned<'w, T: 'static>(
    _v: &crate::entity_container::EntityDequeRef<'_, 'w, T>,
    _key: crate::entity_container::DequeKey,
) -> Result<crate::owned_entity::OwnedEntity<'w>, crate::error::EcsError> {
    todo!("尚未实现按成员键增持")
}
pub(crate) fn deque_mut_front_owned<'w, T: 'static>(
    _v: &crate::entity_container::EntityDequeMut<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现首成员增持")
}
pub(crate) fn deque_mut_iter_owned<'a, 'w, T: 'static>(
    _v: &'a crate::entity_container::EntityDequeMut<'_, 'w, T>,
) -> Result<crate::entity_container::EntityOwnedIter<'a, 'w>, crate::error::EcsError> {
    todo!("尚未实现逐项拥有遍历")
}
pub(crate) fn deque_mut_back_owned<'w, T: 'static>(
    _v: &crate::entity_container::EntityDequeMut<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现尾成员增持")
}
pub(crate) fn deque_mut_get_owned<'w, T: 'static>(
    _v: &crate::entity_container::EntityDequeMut<'_, 'w, T>,
    _key: crate::entity_container::DequeKey,
) -> Result<crate::owned_entity::OwnedEntity<'w>, crate::error::EcsError> {
    todo!("尚未实现按成员键增持")
}
pub(crate) fn owned_next<'w>(
    _iter: &mut crate::entity_container::EntityOwnedIter<'_, 'w>,
) -> Option<Result<crate::owned_entity::OwnedEntity<'w>, crate::error::EcsError>> {
    todo!("尚未实现逐项原子增持及成功后推进")
}

pub(crate) fn list_read_len<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityListRef<'_, '_, T>,
) -> usize {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn list_read_empty<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityListRef<'_, '_, T>,
) -> bool {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn list_read_front<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityListRef<'_, '_, T>,
) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn list_read_iter<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityListRef<'_, '_, T>,
) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn list_write_empty<T: 'static>(
    _v: &crate::entity_container::EntityListMut<'_, '_, T>,
) -> bool {
    todo!("尚未实现空检查")
}
pub(crate) fn list_write_iter<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityListMut<'_, '_, T>,
) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
    todo!("尚未实现短借用遍历")
}
pub(crate) fn queue_read_len<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityQueueRef<'_, '_, T>,
) -> usize {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn queue_read_empty<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityQueueRef<'_, '_, T>,
) -> bool {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn queue_read_front<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityQueueRef<'_, '_, T>,
) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn queue_read_iter<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityQueueRef<'_, '_, T>,
) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn queue_write_empty<T: 'static>(
    _v: &crate::entity_container::EntityQueueMut<'_, '_, T>,
) -> bool {
    todo!("尚未实现空检查")
}
pub(crate) fn queue_write_iter<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityQueueMut<'_, '_, T>,
) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
    todo!("尚未实现短借用遍历")
}
pub(crate) fn deque_read_len<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityDequeRef<'_, '_, T>,
) -> usize {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn deque_read_empty<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityDequeRef<'_, '_, T>,
) -> bool {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn deque_read_front<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityDequeRef<'_, '_, T>,
) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn deque_read_iter<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityDequeRef<'_, '_, T>,
) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
    todo!("尚未实现只读容器访问")
}
pub(crate) fn deque_write_empty<T: 'static>(
    _v: &crate::entity_container::EntityDequeMut<'_, '_, T>,
) -> bool {
    todo!("尚未实现空检查")
}
pub(crate) fn deque_write_iter<'a, T: 'static>(
    _v: &'a crate::entity_container::EntityDequeMut<'_, '_, T>,
) -> Result<crate::entity_container::EntityChainIter<'a>, crate::error::EcsError> {
    todo!("尚未实现短借用遍历")
}
pub(crate) fn chain_next(
    _iter: &mut crate::entity_container::EntityChainIter<'_>,
) -> Option<crate::entity::Entity> {
    todo!("尚未实现链迭代")
}

pub(crate) fn list_len<T: 'static>(
    _v: &crate::entity_container::EntityListMut<'_, '_, T>,
) -> usize {
    todo!("尚未实现长度读取")
}
pub(crate) fn list_front<T: 'static>(
    _v: &crate::entity_container::EntityListMut<'_, '_, T>,
) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
    todo!("尚未实现首项读取")
}
pub(crate) fn list_clear<T: 'static>(
    _v: &mut crate::entity_container::EntityListMut<'_, '_, T>,
) -> Result<(), crate::error::EcsError> {
    todo!("@to_design 特殊释放与级联收束")
}
pub(crate) fn list_push_front<'w, T: 'static>(
    _v: &mut crate::entity_container::EntityListMut<'_, 'w, T>,
    _value: crate::owned_entity::OwnedEntity<'w>,
) -> Result<(), crate::entity_container::EntityInsertError<'w>> {
    todo!("尚未实现节点与临时责任转移")
}
pub(crate) fn list_pop_front<'w, T: 'static>(
    _v: &mut crate::entity_container::EntityListMut<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现节点与临时责任转移")
}
pub(crate) fn queue_len<T: 'static>(
    _v: &crate::entity_container::EntityQueueMut<'_, '_, T>,
) -> usize {
    todo!("尚未实现长度读取")
}
pub(crate) fn queue_front<T: 'static>(
    _v: &crate::entity_container::EntityQueueMut<'_, '_, T>,
) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
    todo!("尚未实现首项读取")
}
pub(crate) fn queue_clear<T: 'static>(
    _v: &mut crate::entity_container::EntityQueueMut<'_, '_, T>,
) -> Result<(), crate::error::EcsError> {
    todo!("@to_design 特殊释放与级联收束")
}
pub(crate) fn queue_push_back<'w, T: 'static>(
    _v: &mut crate::entity_container::EntityQueueMut<'_, 'w, T>,
    _value: crate::owned_entity::OwnedEntity<'w>,
) -> Result<(), crate::entity_container::EntityInsertError<'w>> {
    todo!("尚未实现节点与临时责任转移")
}
pub(crate) fn queue_pop_front<'w, T: 'static>(
    _v: &mut crate::entity_container::EntityQueueMut<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现节点与临时责任转移")
}
pub(crate) fn deque_len<T: 'static>(
    _v: &crate::entity_container::EntityDequeMut<'_, '_, T>,
) -> usize {
    todo!("尚未实现长度读取")
}
pub(crate) fn deque_front<T: 'static>(
    _v: &crate::entity_container::EntityDequeMut<'_, '_, T>,
) -> Result<Option<crate::entity::Entity>, crate::error::EcsError> {
    todo!("尚未实现首项读取")
}
pub(crate) fn deque_clear<T: 'static>(
    _v: &mut crate::entity_container::EntityDequeMut<'_, '_, T>,
) -> Result<(), crate::error::EcsError> {
    todo!("@to_design 特殊释放与级联收束")
}
pub(crate) fn deque_push_front<'w, T: 'static>(
    _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
    _value: crate::owned_entity::OwnedEntity<'w>,
) -> Result<crate::entity_container::DequeKey, crate::entity_container::EntityInsertError<'w>> {
    todo!("尚未实现节点与临时责任转移")
}
pub(crate) fn deque_push_back<'w, T: 'static>(
    _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
    _value: crate::owned_entity::OwnedEntity<'w>,
) -> Result<crate::entity_container::DequeKey, crate::entity_container::EntityInsertError<'w>> {
    todo!("尚未实现节点与临时责任转移")
}
pub(crate) fn deque_pop_front<'w, T: 'static>(
    _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现节点与临时责任转移")
}
pub(crate) fn deque_pop_back<'w, T: 'static>(
    _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, crate::error::EcsError> {
    todo!("尚未实现节点与临时责任转移")
}
pub(crate) fn deque_insert_before<'w, T: 'static>(
    _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
    _key: crate::entity_container::DequeKey,
    _value: crate::owned_entity::OwnedEntity<'w>,
) -> Result<crate::entity_container::DequeKey, crate::entity_container::EntityInsertError<'w>> {
    todo!("尚未实现按节点键转移责任")
}
pub(crate) fn deque_insert_after<'w, T: 'static>(
    _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
    _key: crate::entity_container::DequeKey,
    _value: crate::owned_entity::OwnedEntity<'w>,
) -> Result<crate::entity_container::DequeKey, crate::entity_container::EntityInsertError<'w>> {
    todo!("尚未实现按节点键转移责任")
}
pub(crate) fn deque_remove<'w, T: 'static>(
    _v: &mut crate::entity_container::EntityDequeMut<'_, 'w, T>,
    _key: crate::entity_container::DequeKey,
) -> Result<crate::owned_entity::OwnedEntity<'w>, crate::error::EcsError> {
    todo!("尚未实现按节点键转移责任")
}
