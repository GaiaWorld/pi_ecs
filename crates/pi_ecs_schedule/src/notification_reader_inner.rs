//! 通知参数配对芯，不调用其他芯。
pub(crate) fn added_empty_inner<T: 'static>(
    _reader: &crate::notification_reader::AddedReader<'_, T>,
) -> Result<bool, EcsError> {
    todo!("尚未实现未读检查")
}
pub(crate) fn added_clear_inner<T: 'static>(
    _reader: &mut crate::notification_reader::AddedReader<'_, T>,
) -> Result<(), EcsError> {
    todo!("尚未实现本读者游标放弃")
}
pub(crate) fn changed_empty_inner<T: 'static>(
    _reader: &crate::notification_reader::ChangedReader<'_, T>,
) -> Result<bool, EcsError> {
    todo!("尚未实现未读检查")
}
pub(crate) fn changed_clear_inner<T: 'static>(
    _reader: &mut crate::notification_reader::ChangedReader<'_, T>,
) -> Result<(), EcsError> {
    todo!("尚未实现本读者游标放弃")
}
pub(crate) fn removed_empty_inner<T: pi_ecs_core::component::Component>(
    _reader: &crate::notification_reader::RemovedReader<'_, T>,
) -> Result<bool, EcsError> {
    todo!("尚未实现未读检查")
}
pub(crate) fn removed_clear_inner<T: pi_ecs_core::component::Component>(
    _reader: &mut crate::notification_reader::RemovedReader<'_, T>,
) -> Result<(), EcsError> {
    todo!("尚未实现本读者游标放弃")
}

use pi_ecs_core::{error::EcsError, notification::NoticeBatch};
pub(crate) fn added_read_inner<'a, T: 'static>(
    _reader: &'a mut crate::notification_reader::AddedReader<'_, T>,
) -> Result<NoticeBatch<'a>, EcsError> {
    todo!("尚未实现固定批次上界读取，返回即消费")
}
pub(crate) fn changed_read_inner<'a, T: 'static>(
    _reader: &'a mut crate::notification_reader::ChangedReader<'_, T>,
) -> Result<NoticeBatch<'a>, EcsError> {
    todo!("尚未实现固定批次上界读取，返回即消费")
}
pub(crate) fn removed_read_inner<'a, T: pi_ecs_core::component::Component>(
    _reader: &'a mut crate::notification_reader::RemovedReader<'_, T>,
) -> Result<pi_ecs_core::notification::RemovedBatch<'a, T>, EcsError> {
    todo!("尚未实现固定批次上界读取，返回即消费")
}
