//! 引用归零与组件物理销毁分离，首版不按冲突分析切换立即路径。
use crate::{entity::Entity, error::EcsError, world::World};
/// 特殊释放的运行模式，不允许业务自由切换。
#[derive(Clone, Copy)]
pub(crate) enum ReleaseMode {
    /// 交还引用；强归零登记工作项，组件销毁延后至安全收束。
    Normal,
    /// 关闭或旧分支整体清理；不递归登记业务销毁、不发布通知，各值由整体遍历负责。
    WholeState,
}
/// 释放时临时传入的受限 World 能力，不在内置组件中永久保存。
pub(crate) struct ReleaseContext<'w> {
    /// 所属 World 的节点池/身份能力；共享引用不自动授予任意组件修改。
    pub(crate) world: &'w World,
    /// 本次已授权的清理模式。
    pub(crate) mode: ReleaseMode,
    // @to_design 节点/计数与回收的实际授权凭据及内存序待接线。
}
/// 身份控制槽生命周期，strong=0 不能复活。
pub(crate) enum EntityLife {
    /// strong>0，正常访问。
    Alive,
    /// strong=0，禁增持/弱升级；本阶段 Query 仍可见，组件内存保持有效。
    PendingDestroy,
    /// 组件已处理，无位置且查询不可见；weak>0 保留控制槽，weak=0 登记延迟回收。
    Dead,
}
/// 并发归零登记与静止点销毁状态；同实体只登记一次。
pub(crate) struct PendingDestruction {
    // @to_design 并发列表及唯一状态转换的原子表示/内存序待闭合。
    pub(crate) _pending: (),
    /// 已 Dead 且 strong=weak=0 的唯一回收登记；不由释放线程直接修改域内 free 链。
    /// @to_design 并发登记的具体容器及与身份槽保活协议待闭合。
    pub(crate) _pending_reclamation: (),
}
impl PendingDestruction {
    /// entity 已完成唯一 strong 归零转换；并发追加，不回收组件、不改变当前成员可见性。
    /// 列表必须保活身份槽至收束，不能使旧代际键指向已复用槽；归零不分配变化 tick、不发 Removed。
    pub(crate) fn enqueue(&self, entity: Entity) {
        crate::destruction_inner::enqueue_inner(self, entity)
    }
}
impl World {
    /// 阶段任务/参数/所有组件引用及 guard 已结束后收束归零销毁；宿主通过共享 World 取得内部维护授权。
    /// 活借用/guard 或运行/维护冲突在修改前拒绝不等待；正常返回后才释放维护授权。
    /// 宿主 OwnedEntity 可保留，它不借用组件；收束截点仍需排除并发计数操作越界。
    /// 系统阶段末收束复用根运行授权，不能重入宿主维护门禁；实际两入口编排待闭合。
    /// 失败不回滚，不能捕获后继续模拟；普通 Drop 只登记，不隐式调用此方法。
    pub fn flush_destruction(&self) -> Result<(), EcsError> {
        /* 设计已确认：
        无待处理工作则返回，不分配 tick；非空销毁收束在物理修改前取得一次非零 tick。
        tick 耗尽在移出组件前报错并释放本次授权，不清空待处理责任、不回绕。
        阶段末或宿主显式收束均只取一次，全部级联批次 Removed 使用该 tick，不逐实体分配。
        在安全收束点截取首批归零身份，按 AllocationDomain 升序分组，域内完整 Entity 键升序。
        对当前批次逐实体隐藏位置，按 ComponentId 升序从真实存储移出组件；
        数组按索引升序、链容器按头至尾释放，不以哈希遍历或线程入队顺序决定处理顺序。
        有 Removed 订阅则连同节点/引用责任转入通知存储，不复制节点、不提前释放。
        无订阅则按 ComponentId 查元数据：普通 drop_value 或内置 release_builtin 二选一。
        当前批次释放产生的归零项只追加下一批，不插入当前批；下一批重新按同一规则排序。
        重复至无待销毁项；组件处理完成标 Dead，weak=0 只登记可回收身份，源存储洞由 settle 整理。
        Dead 后最后 weak 归零也只唯一登记，不立即进 LIFO 空闲链；非 Dead 的 weak 归零不能回收。
        本次级联全部收束后合并可回收身份，按域升序、域内完整键升序，依序调用 EntityStore::remove。
        每域按上述升序逐个压入 LIFO 链，因此该批最后入链的键优先复用，不由释放线程抢先决定。
        收束期间不允许外部计数释放跨越批次截点；实际授权/门禁证明仍待闭合。
        Removed 旧值保留节点和引用，最终回收按同一元数据特殊释放；慢读者会延后目标归零。
        容器逐节点保存后继、取走目标责任、回收节点、交还强计数；不按 Entity 去重释放。
        固定强/弱数组逐位置交还；重复目标各算一份，只有归零工作项去重。
        级联使用迭代工作队列，不在释放函数/临时引用 Drop 中递归销毁，强环不自动回收。
        */
        crate::destruction_inner::flush_inner(self)
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
// @to_design 释放入口与共享 World 维护授权、并发登记的截点及内存序待闭合。
