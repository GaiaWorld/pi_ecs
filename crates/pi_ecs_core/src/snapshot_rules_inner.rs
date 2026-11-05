//! 设计：unified-partitioned-ecs · 芯；仅有显式未实现入口。
use crate::{
    error::EcsError,
    snapshot_rules::{
        DerivedInitial, SnapshotCapture, SnapshotCopy, SnapshotRegistry, SnapshotRestore,
    },
};
pub(crate) fn save_component_copy_inner<T: Copy + 'static>(
    _registry: &mut SnapshotRegistry,
) -> Result<(), EcsError> {
    todo!("@to_design 类型擦除、配置冻结及副本准备清理")
}
pub(crate) fn save_component_clone_inner<T: Clone + 'static>(
    _registry: &mut SnapshotRegistry,
) -> Result<(), EcsError> {
    todo!("@to_design 类型擦除、配置冻结及副本准备清理")
}
pub(crate) fn save_component_with_inner<T: 'static, S: 'static>(
    _registry: &mut SnapshotRegistry,
    _capture: SnapshotCapture<T, S>,
    _restore: SnapshotRestore<T, S>,
) -> Result<(), EcsError> {
    todo!("@to_design 类型擦除、配置冻结及副本准备清理")
}
pub(crate) fn save_resource_copy_inner<T: Copy + 'static>(
    _registry: &mut SnapshotRegistry,
) -> Result<(), EcsError> {
    todo!("@to_design 类型擦除、配置冻结及副本准备清理")
}
pub(crate) fn save_resource_clone_inner<T: Clone + 'static>(
    _registry: &mut SnapshotRegistry,
) -> Result<(), EcsError> {
    todo!("@to_design 类型擦除、配置冻结及副本准备清理")
}
pub(crate) fn save_resource_with_inner<T: 'static, S: 'static>(
    _registry: &mut SnapshotRegistry,
    _capture: SnapshotCapture<T, S>,
    _restore: SnapshotRestore<T, S>,
) -> Result<(), EcsError> {
    todo!("@to_design 类型擦除、配置冻结及副本准备清理")
}

pub(crate) fn new_inner() -> SnapshotRegistry {
    todo!("@to_design 登记表示、冻结及冲突协议尚未确定")
}
pub(crate) fn register_saved_component_inner<T: 'static>(
    _registry: &mut SnapshotRegistry,
    _copy: SnapshotCopy<T>,
) -> Result<(), EcsError> {
    todo!("@to_design 登记表示、冻结及冲突协议尚未确定")
}
pub(crate) fn register_derived_component_inner<T: 'static>(
    _registry: &mut SnapshotRegistry,
    _initial: DerivedInitial<T>,
) -> Result<(), EcsError> {
    todo!("@to_design 登记表示、冻结及冲突协议尚未确定")
}
