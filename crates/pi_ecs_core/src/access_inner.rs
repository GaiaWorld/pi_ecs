//! 权限配对芯；不实现授权或冲突算法。
pub(crate) fn conflicts_inner(
    _left: &crate::access::AccessDeclaration,
    _right: &crate::access::AccessDeclaration,
) -> bool {
    todo!("尚未实现")
}
use crate::{
    access::{AccessDeclaration, RunAccess},
    error::EcsError,
    query_fetch::FetchAccess,
    world::World,
};
pub(crate) fn from_executor_inner<'w>(
    _world: &'w World,
    _declaration: &'w AccessDeclaration,
    _archetype_version: u64,
) -> RunAccess<'w> {
    todo!("@to_design 跨执行器授权凭据绑定")
}
pub(crate) fn fetch_access_inner<'a>(
    _access: &'a RunAccess<'_>,
) -> Result<FetchAccess<'a>, EcsError> {
    todo!("@to_design guard 保活和受限取值接口")
}
