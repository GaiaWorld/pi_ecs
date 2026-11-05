//! 显式构建器与参数配置配对芯，仅失败桩。
pub(crate) fn multiresconfig_new<T: Send + Sync + 'static>(
    _initial: T,
) -> crate::resource_param::MultiResConfig<T> {
    todo!("尚未实现初值与恢复策略配置")
}
pub(crate) fn multiresconfig_default<T: Send + Sync + Default + 'static>(
) -> crate::resource_param::MultiResConfig<T> {
    todo!("尚未实现初值与恢复策略配置")
}
pub(crate) fn multiresconfig_clone<T: Send + Sync + Clone + 'static>(
    _config: crate::resource_param::MultiResConfig<T>,
) -> Result<crate::resource_param::MultiResConfig<T>, EcsError> {
    todo!("尚未实现初值与恢复策略配置")
}
pub(crate) fn multiresconfig_with<T: Send + Sync + 'static, S: 'static>(
    _config: crate::resource_param::MultiResConfig<T>,
    _capture: pi_ecs_core::snapshot_rules::SnapshotCapture<T, S>,
    _restore: pi_ecs_core::snapshot_rules::SnapshotRestore<T, S>,
) -> Result<crate::resource_param::MultiResConfig<T>, EcsError> {
    todo!("尚未实现初值与恢复策略配置")
}
pub(crate) fn multiresconfig_rebuild<T: Send + Sync + 'static>(
    _config: crate::resource_param::MultiResConfig<T>,
    _reset: pi_ecs_core::snapshot_rules::DerivedInitial<T>,
) -> Result<crate::resource_param::MultiResConfig<T>, EcsError> {
    todo!("尚未实现初值与恢复策略配置")
}
pub(crate) fn multiresconfig_preserve<T: Send + Sync + 'static>(
    _config: crate::resource_param::MultiResConfig<T>,
) -> Result<crate::resource_param::MultiResConfig<T>, EcsError> {
    todo!("尚未实现初值与恢复策略配置")
}

pub(crate) fn localconfig_new<T: Send + 'static>(_initial: T) -> crate::local::LocalConfig<T> {
    todo!("尚未实现初值与恢复策略配置")
}
pub(crate) fn localconfig_default<T: Send + Default + 'static>() -> crate::local::LocalConfig<T> {
    todo!("尚未实现初值与恢复策略配置")
}
pub(crate) fn localconfig_clone<T: Send + Clone + 'static>(
    _config: crate::local::LocalConfig<T>,
) -> Result<crate::local::LocalConfig<T>, EcsError> {
    todo!("尚未实现初值与恢复策略配置")
}
pub(crate) fn localconfig_with<T: Send + 'static, S: 'static>(
    _config: crate::local::LocalConfig<T>,
    _capture: pi_ecs_core::snapshot_rules::SnapshotCapture<T, S>,
    _restore: pi_ecs_core::snapshot_rules::SnapshotRestore<T, S>,
) -> Result<crate::local::LocalConfig<T>, EcsError> {
    todo!("尚未实现初值与恢复策略配置")
}
pub(crate) fn localconfig_rebuild<T: Send + 'static>(
    _config: crate::local::LocalConfig<T>,
    _reset: pi_ecs_core::snapshot_rules::DerivedInitial<T>,
) -> Result<crate::local::LocalConfig<T>, EcsError> {
    todo!("尚未实现初值与恢复策略配置")
}
pub(crate) fn localconfig_preserve<T: Send + 'static>(
    _config: crate::local::LocalConfig<T>,
) -> Result<crate::local::LocalConfig<T>, EcsError> {
    todo!("尚未实现初值与恢复策略配置")
}

use crate::{
    system::{FunctionSystem, SystemFunction, SystemId},
    system_builder::{ParamConfig, SystemBuilder},
    system_param::SystemParam,
};
use pi_ecs_core::error::EcsError;
pub(crate) fn config_inner<P: SystemParam>(_value: P::Config) -> ParamConfig<P> {
    todo!("尚未实现参数配置持有")
}
pub(crate) fn new_inner<F: SystemFunction<P>, P: SystemParam>(
    _function: F,
    _config: ParamConfig<P>,
) -> SystemBuilder<F, P> {
    todo!("尚未实现")
}
pub(crate) fn build_inner<F: SystemFunction<P>, P: SystemParam>(
    _builder: SystemBuilder<F, P>,
    _id: SystemId,
) -> Result<FunctionSystem<F, P>, EcsError> {
    todo!("@to_design 初始化配置与登记适配")
}
