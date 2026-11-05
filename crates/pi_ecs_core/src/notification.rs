//! 可靠多读者组件通知；按流顺序消费，最后读者通过且借用结束才释放。
use crate::{
    change_detection::ChangeTick, component::ComponentId, entity::Entity, error::EcsError,
};
/// 通知种类；Added/Changed 不携带历史值，Removed 独立拥有旧值。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoticeKind {
    /// 首次加入组件，同时属于 Changed 的来源。
    Added,
    /// 发生可写访问或显式标记，不默认全局合并。
    Changed,
    /// 移除或 despawn 中移出的实际组件，含旧值。
    Removed,
}
/// 一个流内单调递增的序号，与 change tick 不同，不能用来跨流推断因果。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoticeSequence(
    /// 流中逻辑序号，回绕策略待明确。
    pub(crate) u64,
);
/// 每次订阅的身份，不能由组件编号代替多个独立读者。
pub struct SubscriptionId {
    /// 管理器分配的句柄，复用/代际策略待设计。
    pub(crate) slot: u64,
}
/// 查询初始化登记的观察需求，调度器必须把它作为访问而不只是队列读取。
pub struct NoticeSubscription {
    /// 此查询独立的订阅句柄。
    pub(crate) id: SubscriptionId,
    /// 组件和流种类，旧值共享读取需相应组件线程能力。
    pub(crate) stream: (ComponentId, NoticeKind),
    /// 下一条未确认记录，不因“创建迭代器”就跳到尾部。
    pub(crate) next: NoticeSequence,
}
/// 通知头不包含存储行或长期组件地址。
pub struct NoticeHeader {
    /// 事件所属完整 Entity；消费时实体可能已销毁或重用 index。
    pub entity: Entity,
    /// 事件发生的执行上下文时间。
    pub tick: ChangeTick,
    /// 本流序号。
    pub sequence: NoticeSequence,
}
/// Removed 历史值的独立存储，拥有分配和唯一析构责任，不保存临时 OwningPtr。
/// 内置容器连同节点/引用责任转入，全部读者通过且无借用才特殊释放；不是只复制容器头。
pub(crate) struct RemovedValue {
    /// 与已移入值一致的登记类型，用于普通/内置释放分派；注册元数据必须保活。
    pub(crate) component: ComponentId,
    // @to_design 对齐存储、显式移入/读取 Ptr/释放接口与运行期线程能力待闭合。
    pub(crate) _pending_storage: (),
}
/// 通知系统拥有所有流及订阅者；消费者慢则保留，不按固定帧数暗中丢弃。
pub struct Notifications {
    // @to_design 按组件/种类的并发追加、稳定记录页、读者表和回收前缀字段待闭合。
    pub(crate) _pending_streams: (),
}
impl Notifications {
    /// component 已登记；kind 选择流，登记后汇总开启对应 tick/通知需求。
    /// 在阶段前静止点登记，监听全 World，从当前流尾开始，不追溯已有记录；非法登记返回错误。
    pub fn subscribe(
        &mut self,
        component: ComponentId,
        kind: NoticeKind,
    ) -> Result<NoticeSubscription, EcsError> {
        crate::notification_inner::subscribe_inner(self, component, kind)
    }
    /// reader 属于本管理器，开始本次有限批次，固定读取上界但不自动确认所有记录。
    /// 消费者需持有效观察授权；借用期间不得回收返回引用对应记录。
    /// 仅接受 Added/Changed 流，Removed 返回错误且不推进游标；读取覆盖全 World。
    /// @to_design 实际 RunAccess 接入与 RemovedBatch 的可信类型化构造待闭合。
    pub fn read<'a>(
        &'a self,
        reader: &'a mut NoticeSubscription,
    ) -> Result<NoticeBatch<'a>, EcsError> {
        crate::notification_inner::read_inner(self, reader)
    }
    /// 在阶段前静止点注销 reader 并解除其未消费责任；不得存在读取批次或旧值借用。
    /// 不撤销其他订阅；重新汇总需求，该流无订阅时不再推送，Removed 新移出值直接析构。
    /// 不关闭仍被状态过滤器需要的 tick；旧积压按剩余读者及活借用决定回收。
    pub fn unsubscribe(&mut self, reader: NoticeSubscription) -> Result<(), EcsError> {
        crate::notification_inner::unsubscribe_inner(self, reader)
    }
    /// 同步维护点回收全部读者通过且无借用的前缀；按 ComponentInfo 普通/特殊释放旧值一次。
    /// Removed 流按 ComponentId 升序、流内按 NoticeSequence 升序回收合格前缀，不依赖哈希遍历顺序。
    /// 内置释放只登记归零/可回收槽，由 World::flush_destruction 分批销毁并排序回收；不在此递归销毁。
    /// 旧值最终回收不再分配其业务 tick 或重复 Removed；新归零实体由后续销毁收束使用该次收束 tick。
    /// World 受限上下文及与同一安全收束的编排接线待完成。
    /// 跳过未运行的读者不可推进其游标；无读者时无保留责任，不记录无订阅的通知。
    pub fn reclaim(&mut self) {
        crate::notification_inner::reclaim_inner(self)
    }
}
/// 本次批量读取借用，完成标记不得超出真正消费范围；早停不丢失剩余记录。
pub struct NoticeBatch<'a> {
    /// 所属管理器保活通知记录。
    pub(crate) notifications: &'a Notifications,
    /// 本读者的独立确认位置，批次结束前不能被另一次读取覆盖。
    pub(crate) reader: &'a mut NoticeSubscription,
    /// 批次开始时的流尾界限，新记录留给下次读。
    pub(crate) end: NoticeSequence,
    // Iterator::next 成功返回一条即推进该订阅游标；创建、早停或 drop 不消费未返回记录。
}
/// Added/Changed 批次以迭代消费，next 返回一条即确认，None 不推进。
/// 此入口拒绝 Removed 流，Removed 必须走类型校验后的 RemovedBatch。
impl Iterator for NoticeBatch<'_> {
    type Item = NoticeHeader;
    fn next(&mut self) -> Option<Self::Item> {
        crate::notification_inner::next_inner(self)
    }
}
/// Removed 的类型化读取结果；旧值不可取走，借用结束前不得回收。
pub struct RemovedNotice<'a, T: crate::component::Component> {
    /// 完整身份、时间及序号。
    pub header: NoticeHeader,
    /// ComponentRead 映射旧值：普通组件为共享引用，容器为绑定节点池的只读对象。
    /// 从独立旧头构造，不按 header.entity 查当前组件；借用/遍历期间禁止回收旧值及节点。
    pub value: crate::component::ComponentRead<'a, 'a, T>,
}
/// Removed 专用迭代批次，T 必须经登记类型与订阅匹配校验。
/// 即使本批次被 drop，已返回的引用仍保活通知系统，禁止独占 reclaim。
pub struct RemovedBatch<'a, T: crate::component::Component> {
    /// 共享借用管理器并独占借用该订阅游标。
    pub(crate) batch: NoticeBatch<'a>,
    /// 不拥有 T，旧值属于通知存储。
    pub(crate) marker: std::marker::PhantomData<fn() -> T>,
}
impl<'a, T: crate::component::Component> Iterator for RemovedBatch<'a, T> {
    type Item = RemovedNotice<'a, T>;
    /// 返回即消费；提前结束仅保留尚未返回的记录，不提供显式 ack。
    fn next(&mut self) -> Option<Self::Item> {
        crate::notification_inner::removed_next_inner(self)
    }
}
// 记录发布后不可变，Changed 不做跨发布记录全局合并；消费者可在自己的批次去重 Entity。
// 写者在标记时直接登记通知；观察者访问声明必须与同范围的组件写/结构修改排斥，
// 不能只声明通知队列读。仅消费 Entity 也必须遵守该语义，避免读到“已登记但仍在写”的状态。
// Added/Removed 随逻辑结构发布登记；迁区、转存、settle 不登记业务变化。
// 已返回的记录不因业务处理失败而撤销消费；系统 panic 停止模拟。快照保存未回收流、序号进度及各读者游标；Removed 旧值独立复制。
// 历史期订阅配置固定，按系统/参数身份匹配；安装不发通知，重演正常发游戏通知。
