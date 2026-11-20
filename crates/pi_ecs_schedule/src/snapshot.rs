//! 完整游戏历史的运行时归属。
/// 不可变游戏历史，可借用重复恢复；不要求 Clone 或序列化。
/// 保存 SimulationTick、ChangeTick 分配进度、启用的组件 tick、资源 tick 及系统 last_run，按原窗口继续。
/// Local/资源显式 Save/Rebuild/Preserve；游戏 Local 未配置拒绝捕获，资源 Save 包括存在性。
/// 多例保存系统+参数位置归属、值及 tick，物理偏移重建；空目标须提前提供必需 Preserve 值。
/// 通知保存未回收记录、序号进度和读者游标；Removed 旧值使用登记复制能力。
/// 未消费游戏命令及模拟定时项保存 payload、顺序、deadline、就绪状态、取消槽/代际及分配状态。
/// 系统/参数/订阅配置历史期固定，以稳定系统及参数身份匹配；配置变更清历史。
/// WorldEpoch 不回退，安装推进；旧 Sender 不刷新，恢复内容属于新分支，源 Sender 不获得目标资格。
/// 缓存/授权重建，函数/执行器/现实网络与任务不保存。宿主负责历史输入截点、重放去重和副作用隔离。
pub struct GameSnapshot {
    // @to_design 组合 core 数据快照与上述已确认运行状态；具体擦除表示待定；不能公开构造无效空快照。
    /// # 用途
    /// 完整历史表示占位；未形成可用快照，不能据此捕获或安装。
    pub(crate) _pending_snapshot: (),
}
