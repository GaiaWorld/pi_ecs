//! 可靠多读者组件通知；按流顺序消费，最后读者通过且借用结束才释放。
#[path = "notification_inner.rs"]
mod notification_inner;
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
/// 一个流内单调递增的 u64 序号，与 change tick 不同，不能用来跨流推断因果。
/// 不回绕、不因回收/空流重置、不增加序号代际；无法分配合法新序号时停止当前 World。
/// 不静默丢通知、不覆盖旧记录、不伪发布半记录；快照保存并恢复分配进度与全部读者游标。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NoticeSequence(
    /// 流中逻辑序号；分配及尾后界必须 checked 运算，耗尽时不产生新记录。
    pub(crate) u64,
);
pi_key::new_key_type! {
    /// 本订阅表内的完整槽/代际身份，不能用组件编号代替多个独立读者。
    /// 注销使该次身份失效，槽复用须校验完整代际，不能仅按 index 命中新订阅。
    /// 不携带管理器标识，不承诺拒绝跨管理器误用；正确所属由调用者负责。
    /// 不是持久订阅身份：快照按系统/参数稳定身份匹配，不以本次 key 代替。
    pub struct SubscriptionId;
}
// 订阅表使用 pi_slotmap::SlotMap，登记/注销仅在静止点；槽复用及代际耗尽沿用依赖既有行为，不另造协议。
/// 查询初始化登记的观察需求，调度器必须把它作为访问而不只是队列读取。
pub struct NoticeSubscription {
    /// 此查询独立的订阅句柄。
    pub(crate) id: SubscriptionId,
}
/// 订阅表内的唯一状态；句柄不复制 stream/next，回收和 Reader 观察同一游标。
pub(crate) struct SubscriptionState {
    /// 组件和流种类，旧值共享读取需相应组件线程能力；登记后固定到注销。
    pub(crate) stream: (ComponentId, NoticeKind),
    /// 下一条未消费的逻辑序号；同一订阅始终只有一个独占句柄借用在推进。
    /// 初值为登记时已发布流尾；读取批次创建/丢弃不跳到尾部，next 成功才推进。
    #[cfg(feature = "rc")]
    pub(crate) next: std::cell::Cell<u64>,
    /// 非 rc 原子游标，允许维护点从共享通知系统安全推进。
    #[cfg(not(feature = "rc"))]
    pub(crate) next: std::sync::atomic::AtomicU64,
}
/// 通知头不包含存储行或长期组件地址。
#[derive(Clone, Copy)]
pub struct NoticeHeader {
    /// 事件所属完整 Entity；消费时实体可能已销毁或重用 index。
    pub entity: Entity,
    /// 事件发生的执行上下文时间。
    pub tick: ChangeTick,
    /// 本流序号。
    pub sequence: NoticeSequence,
}
/// 单组件的三个独立流；所属组件由 Notifications.components 的下标确定，不逐条保存类型编号。
/// 首次登记只在静止点创建流；None 仅表示当前 World 状态中该流从未创建。
/// 最后读者注销停止追加，独占回收清空积压后保留 Some 及序号；可释放空缓冲容量，不另建历史进度表。
pub(crate) struct ComponentNoticeStreams {
    /// Added 头流；无订阅不追加，与活组件 added tick 需求分开。
    #[cfg(feature = "rc")]
    pub(crate) added: Option<std::cell::RefCell<NoticeStream>>,
    /// # 用途
    /// 非 RC 的 Added 头流，不含活值访问权限。
    #[cfg(not(feature = "rc"))]
    pub(crate) added: Option<NoticeStream>,
    /// Changed 头流；序号及回收范围独立，不与 Added 合流。
    #[cfg(feature = "rc")]
    pub(crate) changed: Option<std::cell::RefCell<NoticeStream>>,
    /// # 用途
    /// 非 RC 的 Changed 头流，独立维护序号和回收范围。
    #[cfg(not(feature = "rc"))]
    pub(crate) changed: Option<NoticeStream>,
    /// Removed 头/旧值联合流；无订阅的新移出值直接按组件规则释放。
    #[cfg(feature = "rc")]
    pub(crate) removed: Option<std::cell::RefCell<RemovedNoticeStream>>,
    /// # 用途
    /// 非 RC 的 Removed 联合流，保留头与旧值的唯一释放责任。
    #[cfg(not(feature = "rc"))]
    pub(crate) removed: Option<RemovedNoticeStream>,
    // 空流 first == end，重新订阅从保留的 end 起读；整体恢复按快照替换状态，不混入被替换分支的进度。
}
/// 只存通知头的追加流；物理槽已分配不代表 NoticeHeader 已初始化或已发布。
pub(crate) struct NoticeStream {
    /// rc 通知队列，尾部追加、独占回收点从头弹出；sequence-first 为逻辑下标，不保证单段连续。
    /// RC 每个 (ComponentId, NoticeKind) 流独立 RefCell；追加短借 RefMut，批次持 Ref。
    /// 扩容/弹出会搬移元素，必须结束所有批次与旧值借用后才可执行。
    #[cfg(feature = "rc")]
    pub(crate) headers: std::collections::VecDeque<NoticeHeader>,
    /// 非 rc 通知存储与提交方式待定；不沿用 Arr 或短提交锁为已确认选型。
    #[cfg(not(feature = "rc"))]
    pub(crate) headers: PendingNoticeStorage,
    /// 已发布逻辑范围，物理整理不改逻辑序号/读者游标。
    pub(crate) range: NoticeStreamRange,
    // rc 单线程顺序追加：先装头值，再推进已发布 end，不暴露半条记录。
    // 非 rc 追加认领/提交与已发布尾界的原子表示随后端选型确定，不手工 Send/Sync 掩盖。
}
/// 非 rc 通知后端待定，不预定锁/无锁或具体容器。
#[cfg(not(feature = "rc"))]
pub(crate) struct PendingNoticeStorage {
    /// 存储与发布协议待定。
    _pending: (),
}
/// Removed 擦除值队列待定，不改成逐条堆分配。
#[cfg(not(feature = "rc"))]
pub(crate) struct PendingRemovedStorage {
    /// 运行时 Layout 和与头队列对应的回收表示待定。
    _pending: (),
}
/// 一个组件的 Removed 联合存储；拥有旧值分配及唯一释放责任，不保存临时 OwningPtr。
pub(crate) struct RemovedNoticeStream {
    /// 通知头和发布范围；头和值按同一逻辑队列下标对应，不要求物理下标相同。
    pub(crate) notices: NoticeStream,
    /// 使用所属组件 Layout 的擦除旧值队列；具体表示待定，不预定逐条分配。
    /// 值连同策略管理责任移入；回收经所属 ComponentInfo.release 完整释放一次。
    /// 旧值析构由 ECS 控制，底层不重复析构；类型元数据/节点能力保活到清理完成。
    #[cfg(not(feature = "rc"))]
    pub(crate) values: PendingRemovedStorage,
    /// rc 与头 VecDeque 同逻辑下标的擦除环形队列，回收时逐头移出并经 ECS 释放。
    #[cfg(feature = "rc")]
    pub(crate) values: pi_ecs_storage::raw_vec_deque::RawVecDeque,
    // 流不缓存独立普通析构函数。回收先把唯一旧值责任移出并结束流借用，
    // 再由 World::reclaim_notifications 经 release_value 调用同源类型策略。
    // 整体关闭覆盖全部剩余旧值，使用 WholeState，不依据游标跳过未读项；底层不得补析构。
}
#[cfg(feature = "rc")]
impl RemovedNoticeStream {
    /// rc 独占追加；entity 为实际移出实体，tick 为结构上下文时间，value 类型匹配所属组件 Layout。
    /// 无头/旧值/容器遍历借用时调用；同线程也不允许重入。序号由本流分配，不接受外部指定。
    /// 两个队列 push_back 各自按需自动扩容，不增加容量准备阶段或源组件移出前的预留协议。
    /// 头/值全部安装后才发布尾界；序号耗尽、容量溢出或追加失败停止，不返回交还值错误、不回滚或继续读取半条记录。
    /// 标准分配失败可能直接终止进程；不承诺可恢复分配错误或分配失败后的整体不变。
    /// value 来源在追加扩容期间有效且不与目标队列别名，不调用业务回调或业务析构。
    pub(crate) fn append<'a>(
        &mut self,
        entity: Entity,
        tick: ChangeTick,
        value: crate::ptr::OwningPtr<'a>,
    ) -> NoticeSequence {
        /* 设计已确认：
        校验类型/范围及头值长度一致，checked 计算新序号和尾后界。
        构造通知头，向 RawVecDeque 尾移入 value，再向头 VecDeque push_back 头；各自自动扩容。
        不设置准备阶段或容量认领对象；中途不可恢复失败沿既有停止协议处理，不继续运行、不假回滚。
        两边长度及逻辑下标一致后，最后推进已发布 end，返回本条序号。
        本入口不执行源结构发布、析构或级联；与源移出和失败清理的授权接线留上层闭合。
        */
        Self::append_removed_rc_inner(self, entity, tick, value)
    }
}
// Added/Changed 写完头才发布；Removed 的头与旧值全部安装后才联合发布，不暴露半条记录。
// 序号分配顺序就是记录顺序，预留尾界不是已发布尾界；前缀回收同步整理头和值，不重复释放。
/// 一个通知流的逻辑存储范围；物理整理不重编号，不修改读者下一条序号。
pub(crate) struct NoticeStreamRange {
    /// 当前保留范围的首序号；只有释放合格前缀并提交整理后才前移。
    pub(crate) first: NoticeSequence,
    /// 已发布记录的尾后序号，不是预留长度；first..end 为逻辑保留范围。
    /// 必须保持可表示的尾后界，不允许把最大序号发布后再回绕计算 end；达到容量即拒绝新记录并停止。
    pub(crate) end: NoticeSequence,
}
/// 通知系统拥有所有流及订阅者；消费者慢则保留，不按固定帧数暗中丢弃。
/// 快照按 GameDataSnapshot 的逻辑队列契约保存，不保存后端分配/环形布局；恢复不走普通追加或新订阅。
pub struct Notifications {
    /// 唯一订阅状态表，完整 key 校验由 SlotMap 承担；两种线程模式共用此结构。
    /// 只在无批次/引用的静止点增删，运行期表结构冻结，不做共享转整个 &mut SlotMap。
    pub(crate) subscriptions: pi_slotmap::SlotMap<SubscriptionId, SubscriptionState>,
    /// 以连续 ComponentId 直接索引，不使用 HashMap 查流；表增改仅发生在静止点。
    /// 运行期固定表结构、仅向流追加；独占回收可搬移保留记录，不引入在线页回收协议。
    pub(crate) components: Vec<ComponentNoticeStreams>,
    /// 按 ComponentId 保存已登记组件布局；Removed 旧值队列首次订阅时据此分配。
    pub(crate) layouts: Vec<Option<std::alloc::Layout>>,
    /// 按 ComponentId 保存 Rust 类型身份；Removed 类型化读取据此拒绝错配。
    pub(crate) type_ids: Vec<Option<std::any::TypeId>>,
}
impl Notifications {
    /// component 已登记；kind 选择流，真实订阅表决定该通知流需求，不强制启用活值 added/changed tick。
    /// 在阶段前静止点登记，监听全 World，从当前流尾开始，不追溯已有记录；非法登记返回错误。
    /// None 时首次创建，已有空流复用保留的 end，不重新初始化序号；无读者期间不补造通知。
    pub fn subscribe(
        &mut self,
        component: ComponentId,
        kind: NoticeKind,
    ) -> Result<NoticeSubscription, EcsError> {
        Self::subscribe_inner(self, component, kind)
    }
    /// 调用者保证 reader 属于本管理器，不要求跨管理器误用检测；本表校验完整槽/代际。
    /// 完整 key 点查 subscriptions，缺失/旧代际返回错误且不推进游标；状态由表单源持有。
    /// 开始本次有限批次，固定读取上界但不自动确认所有记录。
    /// 消费者需持有效观察授权；RC 批次持目标流 Ref，同流追加/回收拒绝，其他流不受影响。
    /// 仅接受 Added/Changed 流，Removed 返回错误且不推进游标；读取覆盖全 World。
    /// 调用者的访问声明必须覆盖通知流与同范围组件写；本方法不另持 RunAccess。
    /// RemovedBatch 只在类型与订阅匹配校验通过后由芯构造，不开放任意 T 转换。
    pub fn read<'a>(
        &'a self,
        reader: &'a mut NoticeSubscription,
    ) -> Result<NoticeBatch<'a>, EcsError> {
        Self::read_inner(self, reader)
    }
    /// 追加 Added/Changed 通知头；无订阅者或该流未创建时静默跳过。
    /// 共享目录只读，RC 仅短借目标 RefCell 的 RefMut，完成头发布即释放；禁止整管理器共享转可变。
    /// 同流借用冲突是授权内部故障，停止并传播，不丢通知；不同流借用不应冲突。
    pub(crate) fn append_change(
        &self,
        component: ComponentId,
        kind: NoticeKind,
        entity: Entity,
        tick: ChangeTick,
    ) {
        Self::append_change_inner(self, component, kind, entity, tick)
    }
    /// 判断该组件是否有 Removed 订阅；有订阅时移出值必须转交通知存储。
    pub(crate) fn has_removed(&self, component: ComponentId) -> bool {
        Self::has_removed_inner(self, component)
    }
    /// 追加 Removed 头和旧值；成功移交责任，无订阅时 Err 原样交还 value，由调用方统一策略释放。
    /// RC 只短借该组件 Removed 联合流，头和值安装完才发布；不覆盖其他流的守卫。
    pub(crate) fn append_removed<'a>(
        &self,
        component: ComponentId,
        entity: Entity,
        tick: ChangeTick,
        value: crate::ptr::OwningPtr<'a>,
    ) -> Result<NoticeSequence, crate::ptr::OwningPtr<'a>> {
        Self::append_removed_component_inner(self, component, entity, tick, value)
    }
    /// 查询订阅当前是否有未读记录；不消费、不推进游标。
    pub fn is_empty(&self, reader: &NoticeSubscription) -> Result<bool, EcsError> {
        Self::is_empty_inner(self, reader)
    }
    /// 取出所有读者已通过且无借用的 Removed 旧值裸地址；调用方必须按登记能力释放。
    /// 头/值成对移出并推进 first，不在这里执行任何业务析构。
    /// 返回结果按 ComponentId/NoticeSequence 顺序；释放前结束队列借用，Raw 指针责任只能移交一次。
    /// 只推进逻辑队首，不收缩/释放/搬动值队列分配；全部交还值消费前禁止任何队列结构操作。
    /// # Safety
    /// 调用者持独占收束授权，保活所有来源流分配和 ComponentInfo，先结束动态借用再逐值释放。
    /// 释放回调不可重入通知、分配/复用来源槽；全部完成后才允许释放空缓冲。
    /// 返回 Vec 的分配须在首次移出前准备完成；中途 panic 标停，不重试已撤销责任。
    pub(crate) unsafe fn reclaim_removed_raw(&mut self) -> Vec<(ComponentId, *mut u8)> {
        Self::reclaim_removed_raw_inner(self)
    }
    /// 开始 Removed 旧值批次；T 必须与订阅流登记类型一致。
    /// 返回项借用通知存储，批次/引用存活期间不得回收对应旧值。
    pub fn read_removed<'a, T: crate::component::Component>(
        &'a self,
        world: &'a crate::world::World,
        reader: &'a mut NoticeSubscription,
    ) -> Result<RemovedBatch<'a, T>, EcsError> {
        Self::read_removed_inner(self, world, reader)
    }
    /// 在阶段前静止点注销 reader 并解除其未消费责任；不得存在读取批次或旧值借用。
    /// 调用者保证所属正确；完整槽/代际无效则拒绝，成功注销使旧 key 不能命中本表后续复用槽。
    /// 不撤销其他订阅；重新汇总需求，该流无订阅时不再推送，Removed 新移出值直接析构。
    /// 不关闭仍被状态过滤器需要的 tick；旧积压按剩余读者及活借用决定回收。
    /// 最后读者注销不删除流描述符、不重置序号；旧积压由独占 reclaim 释放，不在注销中假定已清空。
    pub fn unsubscribe(&mut self, reader: NoticeSubscription) -> Result<(), EcsError> {
        Self::unsubscribe_inner(self, reader)
    }
    /// 同步维护点回收 Added/Changed 流中全部读者通过且无借用的头前缀。
    /// Removed 头与旧值由 World::reclaim_notifications 在持有 World 的收束点成对取出并释放；
    /// 本入口不析构 Removed 旧值，也不替其他读者推进游标。
    /// 所有写者、NoticeBatch/RemovedBatch、旧值引用及容器遍历结束后才可独占回收/搬移。
    /// 存在活借用则维护编排延后，不能忽略保护强行整理；不等待慢读者业务消费。
    /// rc 从头移出合格前缀，保留环形布局，不分配整理空间、不搬移剩余记录、不强制连续化。
    /// sequence-first 是队列逻辑下标，各队列自行映射环形物理槽；序号/end/订阅 next 不变。
    /// 非 rc 回收后物理整理方式随后端待定，不套用 rc 环形方案。
    /// 跳过未运行的读者不可推进其游标；无读者时无保留责任，不记录无订阅的通知。
    pub fn reclaim(&mut self) {
        /* 设计已确认：
        在安全收束点取得独占维护访问，确认全部写者及派生借用结束，否则延后调用。
        对每个流取所有读者 next 的最小值作为可回收尾界，无读者取已发布尾界。
        按 ComponentId/NoticeSequence 既定顺序逐一释放合格前缀；慢读者保留余项。
        rc 逐条 pop_front Added/Changed 头；Removed 的旧值释放不在本入口。
        rc 仅推进头前缀；Removed 头/值由 World 收束入口同步推进。
        非 rc 按待定后端完成前缀回收与必要整理，不据此选择容器；失败停止且不重复释放旧值。
        提交新的 first/物理偏移，逻辑 sequence/end 和各读者 next 保持不变。
        无读者时清至 first == end，保留 Some 描述符及 end；可释放空缓冲容量，不创建历史序号表。
        不分配业务 tick、不补发 Removed；Removed 释放导致归零由统一 flush 随后收束，不在此递归。
        */
        Self::reclaim_inner(self)
    }
}
/// 本次批量读取借用，完成标记不得超出真正消费范围；早停不丢失剩余记录。
pub struct NoticeBatch<'a> {
    /// 所属管理器保活通知记录。
    pub(crate) notifications: &'a Notifications,
    /// 本读者的独立确认位置，批次结束前不能被另一次读取覆盖。
    pub(crate) reader: &'a mut NoticeSubscription,
    /// 完整 key 校验后的共享订阅状态；RC 用 Cell 推进，不从共享表项强转可变引用。
    pub(crate) state: &'a SubscriptionState,
    /// RC 仅保活本 Added/Changed 流，允许多个独立订阅读同流，其他流可独立追加。
    #[cfg(feature = "rc")]
    pub(crate) stream: std::cell::Ref<'a, NoticeStream>,
    /// 批次开始时的流尾界限，新记录留给下次读。
    pub(crate) end: NoticeSequence,
    // Iterator::next 成功返回一条即推进该订阅游标；创建、早停或 drop 不消费未返回记录。
}
/// Added/Changed 批次以迭代消费，next 返回一条即确认，None 不推进。
/// 此入口拒绝 Removed 流，Removed 必须走类型校验后的 RemovedBatch。
impl Iterator for NoticeBatch<'_> {
    type Item = NoticeHeader;
    /// # 用途
    /// 返回一条头并推进本订阅游标；耗尽不推进，未消费记录留给后续批次。
    fn next(&mut self) -> Option<Self::Item> {
        Self::next_inner(self)
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
/// 借用型 next 的结果短借本批次；旧值活跃时不能 drop 批次或再次 next，禁止扩容/回收目标流。
pub struct RemovedBatch<'a, T: crate::component::Component> {
    /// 保活只读目录；不独占管理器，不保护其他流。
    pub(crate) notifications: &'a Notifications,
    /// 独占本订阅句柄，防止同一游标重复读取。
    pub(crate) reader: &'a mut NoticeSubscription,
    /// 共享表项，RC 只用 Cell 更新 next。
    pub(crate) state: &'a SubscriptionState,
    /// 固定批次开始时已发布尾界。
    pub(crate) end: NoticeSequence,
    /// RC 头和值联合流的共享守卫；所有返回旧值短借此守卫。
    #[cfg(feature = "rc")]
    pub(crate) stream: std::cell::Ref<'a, RemovedNoticeStream>,
    /// 内置容器旧值读取绑定所属 World；普通组件也不延长通知存储寿命。
    pub(crate) world: crate::unsafe_world_cell::UnsafeWorldCell<'a>,
    /// 不拥有 T，旧值属于通知存储。
    pub(crate) marker: std::marker::PhantomData<fn() -> T>,
}
impl<T: crate::component::Component> RemovedBatch<'_, T> {
    /// # 用途
    /// 返回即消费；提前结束仅保留尚未返回的记录，不提供显式 ack。
    /// 旧值短借 self；使用结束前不能再 next 或释放批次，不实现标准 Iterator/collect。
    ///
    /// 旧值仍使用时不能释放批次：
    /// ```no_run
    /// use pi_ecs_core::{notification::RemovedBatch, component::Component};
    /// fn sequential<T: Component>(mut batch: RemovedBatch<'_, T>) {
    ///     while let Some(value) = batch.next() {
    ///         std::hint::black_box(value);
    ///     }
    /// }
    /// ```
    /// 旧值仍使用时不能再取得下一项：
    /// ```compile_fail,E0499
    /// use pi_ecs_core::{notification::RemovedBatch, component::Component};
    /// fn overlapping<T: Component>(batch: &mut RemovedBatch<'_, T>) {
    ///     let first = batch.next().unwrap();
    ///     let second = batch.next().unwrap();
    ///     std::hint::black_box((first, second));
    /// }
    /// ```
    /// 旧值不能逃逸被释放的批次：
    /// ```compile_fail,E0505
    /// use pi_ecs_core::notification::RemovedBatch;
    /// use pi_ecs_core::component::Component;
    /// fn invalid<T: Component>(mut batch: RemovedBatch<'_ , T>) {
    ///     let value = batch.next().unwrap();
    ///     drop(batch);
    ///     std::hint::black_box(value);
    /// }
    /// ```
    ///
    pub fn next(&mut self) -> Option<RemovedNotice<'_, T>> {
        Self::removed_next_inner(self)
    }
}
// 记录发布后不可变，Changed 不做跨发布记录全局合并；消费者可在自己的批次去重 Entity。
// 写者在标记时直接登记通知；观察者访问声明必须与同范围的组件写/结构修改排斥，
// 不能只声明通知队列读。仅消费 Entity 也必须遵守该语义，避免读到“已登记但仍在写”的状态。
// Added/Removed 随逻辑结构发布登记；迁区、转存、settle 不登记业务变化。
// 已返回的记录不因业务处理失败而撤销消费；系统 panic 停止模拟。快照保存未回收流、序号进度及各读者游标；Removed 旧值独立复制。
// 历史期订阅配置固定，按系统/参数身份匹配；安装不发通知，重演正常发游戏通知。
