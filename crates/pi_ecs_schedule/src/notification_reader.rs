//! 独立通知参数，不是 QueryFilter；每参数实例有独立订阅进度。
use pi_ecs_core::{
    error::EcsError,
    notification::{NoticeBatch, NoticeSubscription, Notifications},
};
use std::marker::PhantomData;
/// Added 通知读取参数；只启用对应通知，不强制开启活组件 tick。
/// 只提供发生通知的完整 Entity/时间，点查得到当前值而非历史快照。
pub struct AddedReader<'a, T: 'static> {
    /// 本次通知系统共享访问；调度须声明对应组件观察权限，排除冲突写入。
    pub(crate) notifications: &'a Notifications,
    /// 参数长期状态中的独立订阅游标，短借用阻止重复并发消费。
    pub(crate) subscription: &'a mut NoticeSubscription,
    /// 类型与订阅组件必须一致，由 SystemParam 初始化校验。
    pub(crate) marker: PhantomData<fn() -> T>,
}
impl<T: 'static> AddedReader<'_, T> {
    /// 查询当前有无未读项，不消费，不保证之后仍为空；失效订阅报错不改游标。
    pub fn is_empty(&self) -> Result<bool, EcsError> {
        crate::notification_reader_inner::added_empty_inner(self)
    }
    /// 放弃调用时流尾以前的本读者未读项，不清共享流、不影响其他读者或此后追加项。
    /// 活批次/旧值借用阻止本可变借用；失效订阅错误不推进。无 ack/事务或业务成功含义。
    /// 全部读者通过且无活借用后才可回收记录。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        crate::notification_reader_inner::added_clear_inner(self)
    }

    /// 固定本次读取上界，不自动确认全批；早停须保留未消费记录。
    /// 返回批次借用 self，引用结束前不得回收记录；错误不推进游标。
    pub fn read(&mut self) -> Result<NoticeBatch<'_>, EcsError> {
        crate::notification_reader_inner::added_read_inner(self)
    }
}
/// Changed 通知读取参数；只启用对应通知，不强制开启活组件 tick。
/// 只提供发生通知的完整 Entity/时间，点查得到当前值而非历史快照。
pub struct ChangedReader<'a, T: 'static> {
    /// 本次通知系统共享访问；调度须声明对应组件观察权限，排除冲突写入。
    pub(crate) notifications: &'a Notifications,
    /// 参数长期状态中的独立订阅游标，短借用阻止重复并发消费。
    pub(crate) subscription: &'a mut NoticeSubscription,
    /// 类型与订阅组件必须一致，由 SystemParam 初始化校验。
    pub(crate) marker: PhantomData<fn() -> T>,
}
impl<T: 'static> ChangedReader<'_, T> {
    /// 查询当前有无未读项，不消费，不保证之后仍为空；失效订阅报错不改游标。
    pub fn is_empty(&self) -> Result<bool, EcsError> {
        crate::notification_reader_inner::changed_empty_inner(self)
    }
    /// 放弃调用时流尾以前的本读者未读项，不清共享流、不影响其他读者或此后追加项。
    /// 活批次/旧值借用阻止本可变借用；失效订阅错误不推进。无 ack/事务或业务成功含义。
    /// 全部读者通过且无活借用后才可回收记录。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        crate::notification_reader_inner::changed_clear_inner(self)
    }

    /// 固定本次读取上界，不自动确认全批；早停须保留未消费记录。
    /// 返回批次借用 self，引用结束前不得回收记录；错误不推进游标。
    pub fn read(&mut self) -> Result<NoticeBatch<'_>, EcsError> {
        crate::notification_reader_inner::changed_read_inner(self)
    }
}
/// Removed 通知读取参数；只启用对应通知，不强制开启活组件 tick。
/// Removed 包含旧值共享借用，不允许取走其他读者的历史值。
pub struct RemovedReader<'a, T: pi_ecs_core::component::Component> {
    /// 本次通知系统共享访问；调度须声明对应组件观察权限，排除冲突写入。
    pub(crate) notifications: &'a Notifications,
    /// 参数长期状态中的独立订阅游标，短借用阻止重复并发消费。
    pub(crate) subscription: &'a mut NoticeSubscription,
    /// 类型与订阅组件必须一致，由 SystemParam 初始化校验。
    pub(crate) marker: PhantomData<fn() -> T>,
}
impl<T: pi_ecs_core::component::Component> RemovedReader<'_, T> {
    /// 查询当前有无未读项，不消费，不保证之后仍为空；失效订阅报错不改游标。
    pub fn is_empty(&self) -> Result<bool, EcsError> {
        crate::notification_reader_inner::removed_empty_inner(self)
    }
    /// 放弃调用时流尾以前的本读者未读项，不清共享流、不影响其他读者或此后追加项。
    /// 活批次/旧值借用阻止本可变借用；失效订阅错误不推进。无 ack/事务或业务成功含义。
    /// 全部读者通过且无活借用后才可回收记录。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        crate::notification_reader_inner::removed_clear_inner(self)
    }

    /// 全 World，从订阅尾起的未读记录；固定批次上界，next 返回即消费，早停保留剩余。
    /// 类型校验失败不推进；旧值只读引用存活期间不得回收。
    pub fn read(&mut self) -> Result<pi_ecs_core::notification::RemovedBatch<'_, T>, EcsError> {
        crate::notification_reader_inner::removed_read_inner(self)
    }
}
// 配置为 ()，初始化从流尾订阅，恢复历史游标；无 ack/peek/自动重试或去重。
// 全部 Reader 监听全 World；阶段前可注销，最后订阅注销后该流不再推送。
// @to_design SystemParam 长期状态注册/注销与注入尚未接线；当前无公开伪构造器。
