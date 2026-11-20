//! RC 整体关闭；不依赖引用归零，不执行普通销毁级联。
#[path = "shutdown_inner.rs"]
mod shutdown_inner;
use crate::{error::EcsError, world::World};

/// # 用途
/// RC 整体关闭状态，独立于运行门禁；Stopped 仍可首次关闭，Closed 不允许恢复运行。
#[cfg(feature = "rc")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShutdownState {
    /// 尚未开始整体清理；允许正常运行或停止后的首次关闭。
    Open,
    /// 关闭截点已建立；禁止重入/重试，部分清理失败时保持此态。
    Closing,
    /// 所有业务责任已释放，保留的空字段可由 Rust 正常析构；重复 close 无动作。
    Closed,
}
#[cfg(feature = "rc")]
impl World {
    /// # 用途
    /// RC 显式整体关闭；self 为独占 World，所有参数/组件/节点/通知借用已结束。
    /// 所属 Schedule/App 必须先注销系统参数、释放 Local/生产者/订阅责任，之后不能再运行该 World。
    /// Idle 或 Stopped 可首次关闭，Closed 幂等 Ok；Running/Pinning/Maintaining 或 Closing 前置拒绝。
    /// 不等待引用归零，强环也整体处理；不分配 tick、不发通知、不运行命令处理器、不调用 flush/settle。
    /// 建立关闭截点后，业务析构 panic 或清理不变量损坏尽可能标停后 abort，不向调用者传播或重试半状态。
    /// 清理边界必须在展开离开前终止；panic=abort 时直接沿用进程终止，不依赖捕获机制运行。
    /// Removed 活/未发布旧值必须按 values 的实际逻辑长度清偿，不仅遍历 first..end 的已发布头。
    /// 追加在值移入后头分配 panic 可留下未发布尾值；它仍有唯一清偿责任，不能被关闭遗漏。
    pub fn close(&mut self) -> Result<(), EcsError> {
        /* 设计已确认：
        检查状态及所有运行借用结束；Closed 直接完成，未开始的前置拒绝保持数据。
        关闭截点：Open 转 Closing，门禁永久 Stopped，调用 CommandHub::close，
        先禁止所有命令提交/消费，再在短借外清理全部 payload；panic 纳入本方法 abort 边界。
        按真实活槽逐值转交 WholeState 释放，Removed 历史同样逐值特殊释放；不再按订阅游标保留。
        每个 C 仅调用 ComponentInfo.release 的类型化策略，普通值与托管值同路，不再补 Rust 析构。
        调用前撤销源活槽/旧值的释放责任并结束流/身份表借用，分配仍须保活至回调结束。
        清理资源、多例、默认值；命令 payload 已由 Hub 唯一清理，不重复析构或执行处理器。
        所有内置头处理完后节点池必须无责任残留；不能对相同节点再交还一次计数。
        释放仅维护现有计数，不登记 pending/reclamation，不递归销毁；所有值包括强环由整体遍历处理。
        身份表及登记元数据保活至最后内置释放结束，再清空待处理队列、身份槽及存储目录。
        标 Closed，保持 Stopped；保留空壳供正常字段析构，不能同时由自动析构再次释放业务值。
        */
        self.close_inner()
    }
}

#[cfg(feature = "rc")]
impl Drop for World {
    /// # 用途
    /// 遗忘显式 close 时复用同一关闭主体；Closed 无动作，不重复释放已清理值。
    /// Open 从 Idle/Stopped 进入关闭；安全来源不应留下活借用，遗留运行态视为内部故障并 abort。
    /// Closing 表示异常半状态，不重试；尽可能标停后 abort。Drop 自身不得向外展开，包括外层已有 panic 时。
    /// 业务值责任在关闭主体内清空一次，再由 Rust 释放空字段分配；不能将裸存储析构视为特殊释放替代。
    fn drop(&mut self) {
        self.drop_shutdown_inner()
    }
}
