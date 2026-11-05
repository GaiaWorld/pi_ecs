//! 通知配对芯，设计期仅失败桩。
use crate::{
    component::ComponentId,
    error::EcsError,
    notification::{NoticeBatch, NoticeKind, NoticeSubscription, Notifications},
};
pub(crate) fn subscribe_inner(
    _notifications: &mut Notifications,
    _component: ComponentId,
    _kind: NoticeKind,
) -> Result<NoticeSubscription, EcsError> {
    todo!("尚未实现")
}
pub(crate) fn read_inner<'a>(
    _notifications: &'a Notifications,
    _reader: &'a mut NoticeSubscription,
) -> Result<NoticeBatch<'a>, EcsError> {
    todo!("@to_design 稳定页、观察授权与确认借用")
}
pub(crate) fn unsubscribe_inner(
    _notifications: &mut Notifications,
    _reader: NoticeSubscription,
) -> Result<(), EcsError> {
    todo!("尚未实现")
}
pub(crate) fn next_inner(
    _batch: &mut NoticeBatch<'_>,
) -> Option<crate::notification::NoticeHeader> {
    todo!("尚未实现")
}
pub(crate) fn removed_next_inner<'a, T: crate::component::Component>(
    _batch: &mut crate::notification::RemovedBatch<'a, T>,
) -> Option<crate::notification::RemovedNotice<'a, T>> {
    todo!("@to_design 稳定页旧值保活及类型校验")
}
pub(crate) fn reclaim_inner(_notifications: &mut Notifications) {
    todo!("@to_design 最小确认游标与旧值析构")
}
