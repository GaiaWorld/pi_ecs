//! 查询配对芯，无独立 View 路径，均为明确失败桩。
pub(crate) fn query_with_window_inner<'w, Q: ReadOnlyQueryData, F: QueryFilter>(
    _state: &'w mut QueryState<Q, F>,
    _world: &'w World,
    _window: crate::change_detection::ChangeWindow,
) -> Result<Query<'w, Q, F>, EcsError> {
    todo!("尚未实现显式窗口只读绑定")
}
pub(crate) fn query_mut_since_inner<'w, Q: QueryData, F: QueryFilter>(
    _state: &'w mut QueryState<Q, F>,
    _world: &'w World,
    _last_run: Option<crate::change_detection::ChangeTick>,
) -> Result<QueryMut<'w, Q, F>, EcsError> {
    todo!("尚未实现写 tick 上界窗口绑定")
}
pub(crate) fn this_run_inner<Q: QueryData, F: QueryFilter>(
    _query: &QueryMut<'_, Q, F>,
) -> crate::change_detection::ChangeTick {
    todo!("尚未实现本次时钟读取")
}

use crate::{
    entity::Entity,
    error::EcsError,
    query::{Query, QueryData, QueryFilter, QueryMut, QueryState, ReadOnlyQueryData},
    world::World,
};
pub(crate) fn query_inner<'w, Q: ReadOnlyQueryData, F: QueryFilter>(
    _state: &'w mut QueryState<Q, F>,
    _world: &'w World,
) -> Result<Query<'w, Q, F>, EcsError> {
    todo!("@to_design 紧凑 fetch 重绑定及位宽编码")
}
pub(crate) fn query_mut_inner<'w, Q: QueryData, F: QueryFilter>(
    _state: &'w mut QueryState<Q, F>,
    _world: &'w World,
) -> Result<QueryMut<'w, Q, F>, EcsError> {
    todo!("@to_design 紧凑 fetch 重绑定与权限")
}
pub(crate) fn query_get_inner<'a, Q: ReadOnlyQueryData, F: QueryFilter>(
    _query: &'a Query<'_, Q, F>,
    _entity: Entity,
) -> Result<Q::Item<'a, 'a>, EcsError> {
    todo!("@to_design 压缩映射解码与 fetch_at")
}
pub(crate) fn query_get_mut_inner<'a, Q: QueryData, F: QueryFilter>(
    _query: &'a mut QueryMut<'_, Q, F>,
    _entity: Entity,
) -> Result<Q::Item<'a, 'a>, EcsError> {
    todo!("@to_design 内置 fetch 的短借用访问")
}
pub(crate) fn get_inner<'w, Q: ReadOnlyQueryData, F: QueryFilter>(
    _state: &'w mut QueryState<Q, F>,
    _world: &'w World,
    _entity: Entity,
) -> Result<Q::Item<'w, 'w>, EcsError> {
    todo!("@to_design 目标紧凑条目准备与 OwningPtr 转换")
}
pub(crate) fn get_mut_inner<'w, Q: QueryData, F: QueryFilter>(
    _state: &'w mut QueryState<Q, F>,
    _world: &'w World,
    _entity: Entity,
) -> Result<Q::Item<'w, 'w>, EcsError> {
    todo!("@to_design 目标紧凑条目准备与权限")
}
