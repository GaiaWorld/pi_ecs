//! 引用归零与组件物理销毁分离，首版不按冲突分析切换立即路径。
#[path = "destruction_inner.rs"]
mod destruction_inner;
pub use crate::component_policy::ReleaseMode;
use crate::{entity::Entity, error::EcsError, world::World};
/// 释放时临时传入的受限 World 能力，不在内置组件中永久保存。
pub(crate) struct ReleaseContext<'w> {
    /// 同源统一句柄，释放仅使用身份/节点/状态字段，不还原整个 World 的独占引用。
    /// 容器释放可改节点池，调用者已结束相关引用；不允许在释放中创建实体或访问其他业务值。
    pub(crate) world: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
    /// 本次已授权的清理模式。
    pub(crate) mode: ReleaseMode,
    // 授权凭据只在本调用内有效：句柄提供已授权字段能力，mode 选择普通/整体释放；
    // 不保存强/弱计数所有权，不延长任何组件或旧值寿命。
}
/// 归零登记与静止点销毁状态；pending 唯一登记，RC 回收候选可重复、末尾去重。
pub(crate) struct PendingDestruction {
    /// strong 归零的唯一登记；AppendVec 支持多释放线程并发追加，len 只是已发布前缀。
    /// 收束点等待全部追加者退出后才截取/清空，不以观察到长度替代收束证明。
    /// 同一实体只由达到零的那次原子转换登记一次，不由列表去重。
    pub(crate) pending: pi_append_vec::AppendVec<Entity>,
    /// RC 为 Null 且 strong=weak=0 的回收候选，允许清理中登记；整轮完成后按首次顺序去重。
    /// 非 rc 回收协议保留原约束；不由释放线程直接修改域内 free 链。
    /// 与 pending 同一并发追加协议；rc 直接按操作顺序交给 EntityStore::remove，非 rc 如需确定性由本层排序。
    pub(crate) reclamation: pi_append_vec::AppendVec<Entity>,
}
impl PendingDestruction {
    /// entity 已完成唯一 strong 归零转换；并发追加，不回收组件、不改变当前成员可见性。
    /// 列表必须保活身份槽至收束，不能使旧代际键指向已复用槽；归零不分配变化 tick、不发 Removed。
    pub(crate) fn enqueue(&self, entity: Entity) {
        Self::enqueue_inner(self, entity)
    }
}
impl World {
    /// # 用途
    /// RC 阶段末收束入口，调用者为持有根 Running 授权的调度器；不重入 Maintaining。
    /// 所有系统参数、组件/节点引用、通知批次及临时拥有引用已退出，不存在用户回调重入。
    /// 与 flush_destruction 复用同一主体：先回收通知合格前缀，释放旧值所触发归零并入随后销毁级联。
    /// 完成后保留 Running；失败停止，不补阶段执行或重试已释放旧值。
    /// # Safety
    /// 调用者证明当前根运行会话唯一，已完成全部系统借用收束，不得在用户系统函数内部调用。
    #[cfg(feature = "rc")]
    pub unsafe fn flush_destruction_in_run(&mut self) -> Result<(), EcsError> {
        Self::flush_in_run_inner(self)
    }
    /// 阶段任务/参数/所有组件引用及 guard 已结束后收束归零销毁；宿主通过共享 World 取得内部维护授权。
    /// 活借用/guard 或运行/维护冲突在修改前拒绝不等待；正常返回后才释放维护授权。
    /// 宿主 OwnedEntity 可保留，它不借用组件；收束截点仍需排除并发计数操作越界。
    /// 系统阶段末收束复用根运行授权，不能重入宿主维护门禁；两入口共享下述主体，芯尚未实现。
    /// 失败不回滚，不能捕获后继续模拟；普通 Drop 只登记，不隐式调用此方法。
    #[cfg(feature = "rc")]
    pub fn flush_destruction(&mut self) -> Result<(), EcsError> {
        /* 设计已确认：
        宿主入口校验 Idle 并认领 Maintaining；阶段末入口校验并复用 Running。
        两入口使用相同收束主体，状态守卫展开标停；已停止不能恢复。
        先确认所有 Reader 批次/组件/节点引用已结束，按 ComponentId 升序回收通知合格前缀。
        Added/Changed 仅移出已通过头；Removed 按流内序号成对移出头/值，经 release_value 逐值释放。
        回收前结束通知存储及身份表借用再析构，旧值释放只登记 pending/reclamation，不递归销毁。
        无 pending 则不分配 tick；仅通知/身份回收不分配 tick，不因空 pending 跳过通知处理。
        通知回收后有待销毁组件，在取走首批前分配一次 tick，后续级联全部复用。
        tick 耗尽保留剩余责任并标停；此前已完成的通知释放不回滚，不重试旧值析构。
        截取当前 pending，按出生域/完整键排序；隐藏位置，按组件编号移出。
        有 Removed 订阅则转交完整旧值责任，否则统一 release_value；释放只追加下一批归零。
        每批全部处理后继续截取下一批，直到为空；不在释放栈递归销毁，不自动 settle。
        有订阅的新 Removed 从本轮未读尾追加，保留到后续 Reader；无订阅值直接释放并产生下一批归零。
        本轮无新游标推进或订阅变更，不再次扫描通知；慢读者保留旧值与强弱引用责任，不强行清空。
        组件处理完整调用 finish_destroy，weak=0 登记回收候选；最后 weak 归零同样只登记。
        所有级联完成后 RC 将回收候选按首次登记顺序去重并校验 Null/零计数后回收，不排序。
        普通析构/特殊释放前结束身份表动态借用；首次修改后故障标停，不回滚、不重试释放。
        宿主成功归还 Maintaining 至 Idle，阶段末成功保持 Running。
        */
        Self::flush_inner(self)
    }
    /// 非 rc 通过共享 World 取得维护授权后收束。
    #[cfg(not(feature = "rc"))]
    pub fn flush_destruction(&self) -> Result<(), EcsError> {
        /* 设计已确认：
        无待处理工作则返回，不分配 tick；非空销毁收束在物理修改前取得一次非零 tick。
        tick 耗尽在移出组件前报错并释放本次授权，不清空待处理责任、不回绕。
        阶段末或宿主显式收束均只取一次，全部级联批次 Removed 使用该 tick，不逐实体分配。
        在安全收束点截取首批归零身份，按出生分区升序分组，分区内完整 Entity 键升序。
        对当前批次逐实体隐藏位置，按 ComponentId 升序从真实存储移出组件；
        数组按索引升序、链容器按头至尾释放，不以哈希遍历或线程入队顺序决定处理顺序。
        有 Removed 订阅则连同节点/引用责任转入通知存储，不复制节点、不提前释放。
        无订阅则调用 release_value：按 ComponentId 选择 唯一策略释放入口。
        当前批次释放产生的归零项只追加下一批，不插入当前批；下一批重新按同一规则排序。
        重复至无待销毁项；组件处理完成标 Dead，weak=0 只登记可回收身份，源存储洞由 settle 整理。
        Dead 后最后 weak 归零也只唯一登记，不立即进 LIFO 空闲链；非 Dead 的 weak 归零不能回收。
        本次级联全部收束后合并可回收身份；rc 按操作顺序直接调用 EntityStore::remove，不排序。
        每域按上述升序逐个压入 LIFO 链，因此该批最后入链的键优先复用，不由释放线程抢先决定。
        收束期间不允许外部计数释放跨越批次截点；实际授权/门禁证明仍待闭合。
        Removed 旧值保留节点和引用，最终回收按同一元数据特殊释放；慢读者会延后目标归零。
        容器逐节点保存后继、取走目标责任、回收节点、交还强计数；不按 Entity 去重释放。
        固定强/弱数组逐位置交还；重复目标各算一份，只有归零工作项去重。
        级联使用迭代工作队列，不在释放函数/临时引用 Drop 中递归销毁，强环不自动回收。
        */
        Self::flush_inner(self)
    }
}
// 整体关闭不等待强计数归零，包含强环；先拒绝调度/维护/提交并收束全部任务/借用。
// 先释放系统参数状态，再遍历全部组件/资源/通知旧值/payload，清理剩余节点，最后释放身份控制和存储。
// 整体清理中句柄 Drop 仅交还责任，不递归归零销毁，不产生普通业务通知，各值只析构一次。
// 不保证跨业务值析构顺序；Drop 不查询其他业务值、不创建实体/任务或提交命令。
// 独立命令端点可晚于 World 存活，关闭截点前接受的值由队列清理，之后拒绝并交还 payload。
// 内置引用/容器不持控制块、不普通 Drop；框架按类型执行特殊释放。
// 临时 OwnedEntity 借用 World，关闭/恢复不得越过其存活期；检查完整身份不能替代寿命约束。
// pop/remove 转移责任不减计数，clear 释放全部责任保留空头，删除移除头；搬移/迁区/settle 不释放。
// ComponentInfo/释放上下文必须活到最后一份 Removed 旧值处理完。
// 上述顺序只保证相同收束输入/登记布局下的释放与回收顺序；不使业务 Drop 副作用或其他并发结构操作自动确定。
/// 单个组件值的统一释放入口：按 ComponentInfo 调用具体类型的唯一策略释放入口。
/// info 与 value 为同一登记类型；context 属于原 World，不能把已移交 Removed 的值再次交给本入口。
/// 未发布目标清理、无订阅移除及 Removed 最终回收都复用此入口；搬移不调用，失败标停不重试。
/// Removed 回收源队列分配须保活至全部移出值释放结束，不能在移出或推进游标后提前收缩缓冲。
/// 必须保证 value 对齐、初始化、类型匹配且拥有唯一释放责任；失败停止不重试。
/// # Safety
/// value 来自本 World 的真实存储/内置头；调用方已取得维护授权并结束相关借用。
pub(crate) unsafe fn release_value<'a, 'w>(
    context: &ReleaseContext<'w>,
    info: &crate::component::ComponentInfo,
    value: crate::ptr::OwningPtr<'a>,
) {
    destruction_inner::release_component_inner(context, info, value)
}
// 释放入口只登记不物理销毁；宿主 flush 与阶段末收束复用同一收束点，
// 在取得维护授权并确认全部借用/追加者结束后截取批次。AppendVec 的 push 提交只串行化
// 记录追加，不保护组件值；身份槽保活由收束前不调用 EntityStore::remove 保证。
