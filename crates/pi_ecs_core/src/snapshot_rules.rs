//! 快照能力登记壳；复制、初始值构造与系统重建职责分离。
#[path = "snapshot_rules_inner.rs"]
mod snapshot_rules_inner;
use crate::error::EcsError;

/// 历史捕获上下文，不授予任意 World 修改；内部引用只记录身份和拥有关系，不增减运行计数。
pub struct SnapshotCopyContext {
    // 原样保存身份计数，不扫描拥有关系重建；调用方负责选定数据/外部引用与计数一致。
    // @to_design 受控身份/节点历史导出接口待闭合。
    /// # 用途
    /// 设计占位，不携带访问能力；具体受限表示尚待上方协议闭合。
    pub(crate) _pending: (),
}
/// 恢复准备上下文；重绑引用与目标 World，但安装前不得产生运行分支计数副作用。
/// 失败清理使用准备态协议，不能让临时 T 的普通 Drop 误减当前运行计数。
/// 允许绑定兼容的空目标 World；只重绑指针，不额外增持，计数从快照原样安装。
pub struct SnapshotRestoreContext {
    // @to_design 非活动绑定、准备失败释放和安装激活协议待闭合。
    /// # 用途
    /// 恢复准备能力占位；未授予活动 World 访问或计数修改权限。
    pub(crate) _pending: (),
}
/// 捕获独立历史表示；S 的普通析构只释放历史内存，不得操作运行 World。
pub type SnapshotCapture<T, S> = fn(&T, &mut SnapshotCopyContext) -> Result<S, EcsError>;
/// 从借用历史构造运行值，不消耗 S，允许重复恢复；所有可失败准备先于安装。
pub type SnapshotRestore<T, S> = fn(&S, &mut SnapshotRestoreContext) -> Result<T, EcsError>;

/// 同类型独立复制；不得经内部可变性或全局状态修改源模拟数据。
/// 输入为被借用的运行值或快照值，输出为与源后续修改隔离的新值；错误不得改变源。
pub type SnapshotCopy<T> = fn(&T) -> Result<T, EcsError>;
/// 为派生组件构造合法初始值，不访问尚未安装的 World；错误交由恢复准备阶段处理。
pub type DerivedInitial<T> = fn() -> Result<T, EcsError>;
/// 显式登记保存与初始化能力；历史快照存续期间规则固定。
pub struct SnapshotRegistry {
    // @to_design 类型擦除、资源组件分组、配置兼容标识及规则完整性检查待定。
    /// # 用途
    /// 登记表示占位；不表示保存规则已完整或可开始快照。
    pub(crate) _pending_registry: (),
}
impl SnapshotRegistry {
    /// 登记组件的同类型 copy能力，不要求序列化。
    /// 调用者保证副本与源后续修改隔离；Copy 不证明裸指针所指数据独立。
    /// 规则在历史有效期内固定，冲突/冻结后登记错误不替换现行规则，未登记复制能力不得默认 memcpy。
    pub fn save_component_copy<T: Copy + 'static>(&mut self) -> Result<(), EcsError> {
        Self::save_component_copy_inner::<T>(self)
    }
    /// 登记组件的同类型 clone能力，不要求序列化。
    /// 调用者保证副本与源后续修改隔离；Clone 不证明 Arc/内部可变对象独立，不适用于带运行计数 Drop 的句柄。
    /// 规则在历史有效期内固定，冲突/冻结后登记错误不替换现行规则，未登记复制能力不得默认 memcpy。
    pub fn save_component_clone<T: Clone + 'static>(&mut self) -> Result<(), EcsError> {
        Self::save_component_clone_inner::<T>(self)
    }
    /// 登记组件的异表示捕获/恢复能力，不要求序列化。
    /// capture 必须产生独立 S；S 普通 Drop 不可修改运行 World；restore 不消耗历史且失败不改源。
    /// 规则在历史有效期内固定，冲突/冻结后登记错误不替换现行规则，未登记复制能力不得默认 memcpy。
    pub fn save_component_with<T: 'static, S: 'static>(
        &mut self,
        capture: SnapshotCapture<T, S>,
        restore: SnapshotRestore<T, S>,
    ) -> Result<(), EcsError> {
        Self::save_component_with_inner::<T, S>(self, capture, restore)
    }
    /// 登记唯一资源或多例实例值的同类型 copy能力，不要求序列化。
    /// 调用者保证副本与源后续修改隔离；Copy 不证明裸指针所指数据独立。
    /// 规则在历史有效期内固定，冲突/冻结后登记错误不替换现行规则，未登记复制能力不得默认 memcpy。
    pub fn save_resource_copy<T: Copy + 'static>(&mut self) -> Result<(), EcsError> {
        Self::save_resource_copy_inner::<T>(self)
    }
    /// 登记唯一资源或多例实例值的同类型 clone能力，不要求序列化。
    /// 调用者保证副本与源后续修改隔离；Clone 不证明 Arc/内部可变对象独立，不适用于带运行计数 Drop 的句柄。
    /// 规则在历史有效期内固定，冲突/冻结后登记错误不替换现行规则，未登记复制能力不得默认 memcpy。
    pub fn save_resource_clone<T: Clone + 'static>(&mut self) -> Result<(), EcsError> {
        Self::save_resource_clone_inner::<T>(self)
    }
    /// 登记唯一资源或多例实例值的异表示捕获/恢复能力，不要求序列化。
    /// capture 必须产生独立 S；S 普通 Drop 不可修改运行 World；restore 不消耗历史且失败不改源。
    /// 规则在历史有效期内固定，冲突/冻结后登记错误不替换现行规则，未登记复制能力不得默认 memcpy。
    pub fn save_resource_with<T: 'static, S: 'static>(
        &mut self,
        capture: SnapshotCapture<T, S>,
        restore: SnapshotRestore<T, S>,
    ) -> Result<(), EcsError> {
        Self::save_resource_with_inner::<T, S>(self, capture, restore)
    }

    /// 创建供插件初始化使用的登记容器，不隐式登记任意组件的保存能力。
    /// @to_design 内部表示尚未确定；此构造不表示规则已完整或可开始捕获历史。
    pub fn new() -> Self {
        Self::new_inner()
    }

    /// 登记组件的同类型独立复制能力；捕获与恢复均生成新值，不消耗源快照。
    ///
    /// # Arguments
    /// * `copy` - 从借用值生成独立值的可失败函数，不得修改源模拟状态。
    ///
    /// # Returns
    /// 成功表示组件复制能力已登记，不表示全部恢复规则已完整。
    ///
    /// # Errors
    /// 失败不得悄悄替换现行规则。
    /// @to_design 重复登记及冻结后登记的错误分类、检查顺序尚未确定。
    pub fn register_saved_component<T: 'static>(
        &mut self,
        copy: SnapshotCopy<T>,
    ) -> Result<(), EcsError> {
        /* @to_design 设计意图，未确认：这是本登记模块的规则写入流程主体。
        中文伪码：
          检查生命周期和同类型保存模式冲突；拒绝时保留原有登记。
          通过后按类型登记复制能力，供快照协调使用；此时不复制业务数据。
          具体冲突分类、是否允许等价重复登记以及类型擦除表示尚待确定。
        */
        Self::register_saved_component_inner(self, copy)
    }

    /// 登记省略保存的派生组件初始值能力；共用规则写入流程见 [Self::register_saved_component]。
    /// 差异是登记构造能力而非复制能力，且仍需单独关联后续重建系统。
    ///
    /// # Arguments
    /// * `initial` - 不访问 World 的构造函数，在恢复准备时为每个对应实体产生合法值。
    ///
    /// # Returns
    /// 成功仅表示初始化能力已登记，不表示重建系统及依赖完整。
    ///
    /// # Errors
    /// 登记失败保留现行规则；initial 运行失败由恢复准备阶段清理，不安装未完成数据。
    /// @to_design 重建生产系统关联及完整性校验尚未确定。
    pub fn register_derived_component<T: 'static>(
        &mut self,
        initial: DerivedInitial<T>,
    ) -> Result<(), EcsError> {
        Self::register_derived_component_inner(self, initial)
    }
}
// 强弱 Entity 及实体容器使用框架内建历史表示适配，禁止普通 Clone/Drop 操作运行计数。
// 节点池与容器头作为一致存储图捕获；存储无类型不意味着任意值可 memcpy。
// @to_design 内建身份/节点适配、历史清理、资源分类/多例生产者匹配及安装激活尚待闭合。
