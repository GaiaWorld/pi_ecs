//! 设计：unified-partitioned-ecs · 芯；仅未实现桩，不包含业务逻辑。
pub(crate) fn read_change_tick_inner(_world: &World) -> crate::change_detection::ChangeTick {
    todo!("尚未实现非零变化时钟读取")
}
pub(crate) fn try_acquire_inner(
    _world: &World,
    _entity: Entity,
) -> Result<crate::owned_entity::OwnedEntity<'_>, EcsError> {
    todo!("尚未实现授权下原子增持")
}
pub(crate) fn pin_inner(_world: &World) -> Result<crate::world::WorldPin<'_>, EcsError> {
    todo!("尚未实现多读授权申请")
}
pub(crate) fn pin_get_inner<'a, 'w, C: crate::component::Component>(
    _pin: &'a crate::world::WorldPin<'w>,
    _entity: Entity,
) -> Option<crate::component::ComponentRead<'a, 'w, C>> {
    todo!("尚未实现 pin 授权读取")
}
pub(crate) fn drop_pin_inner(_pin: &mut crate::world::WorldPin<'_>) {
    todo!("尚未实现读授权释放")
}
pub(crate) fn read_scope_inner<'w, R>(
    _world: &'w World,
    _run: impl for<'a> FnOnce(&'a crate::world::WorldPin<'w>) -> R,
) -> Result<R, EcsError> {
    todo!("尚未实现复用 pin 的闭包访问")
}
pub(crate) fn write_inner(_world: &World) -> Result<crate::world::WorldWrite<'_>, EcsError> {
    todo!("尚未实现宿主独占组件授权")
}
pub(crate) fn drop_write_inner(_access: &mut crate::world::WorldWrite<'_>) {
    todo!("尚未实现写授权释放")
}
pub(crate) fn write_scope_inner<'w, R>(
    _world: &'w World,
    _run: impl for<'a> FnOnce(&'a mut crate::world::WorldWrite<'w>) -> R,
) -> Result<R, EcsError> {
    todo!("尚未实现复用 write 的作用域")
}
pub(crate) fn write_get_inner<'a, 'w, C: crate::component::Component>(
    _access: &'a crate::world::WorldWrite<'w>,
    _entity: Entity,
) -> Option<crate::component::ComponentRead<'a, 'w, C>> {
    todo!("尚未实现授权共享读取")
}
pub(crate) fn write_get_mut_inner<'a, 'w, C: crate::component::Component>(
    _access: &'a mut crate::world::WorldWrite<'w>,
    _entity: Entity,
) -> Option<crate::component::ComponentWrite<'a, 'w, C>> {
    todo!("尚未实现授权写读取")
}
pub(crate) fn write_query_one_inner<'a, 'w, Q: crate::query::QueryData>(
    _access: &'a mut crate::world::WorldWrite<'w>,
    _entity: Entity,
) -> Result<Q::Item<'a, 'w>, EcsError> {
    todo!("尚未实现目标原型组合及别名验证")
}
pub(crate) fn query_inner<Q: crate::query::QueryData>(
    _world: &mut World,
) -> Result<crate::query::QueryState<Q>, EcsError> {
    todo!("@to_design 长期匹配缓存及 World 身份")
}
pub(crate) fn query_filtered_inner<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
    _world: &mut World,
) -> Result<crate::query::QueryState<Q, F>, EcsError> {
    todo!("@to_design 长期匹配缓存和过滤状态")
}

use crate::{
    archetype::Archetype,
    component::{ComponentId, ComponentInfo, StoragePolicy},
    entity::ArchetypeId,
    entity::{Entity, EntityLocation},
    error::EcsError,
    partition::PartitionMove,
    table::TableLayout,
    world::World,
};
pub(crate) fn register_component_inner<T: crate::component::Component>(
    world: &mut World,
    storage: StoragePolicy,
) -> Result<ComponentId, EcsError> {
    world.registry.register_component::<T>(storage)
}
pub(crate) fn register_table_inner(
    world: &mut World,
    components: &[ComponentId],
) -> Result<ArchetypeId, EcsError> {
    world.registry.register_table(components)
}
pub(crate) fn finish_registration_inner(world: &mut World) -> Result<(), EcsError> {
    world.registry.freeze(world.partition_count)
}
pub(crate) fn register_archetype_inner(
    world: &mut World,
    components: &[ComponentId],
) -> Result<ArchetypeId, EcsError> {
    world
        .registry
        .register_archetype(components, world.partition_count)
}
pub(crate) fn component_info_inner(world: &World, id: ComponentId) -> Option<&ComponentInfo> {
    world.registry.component(id)
}
pub(crate) fn table_layout_inner(world: &World, id: ArchetypeId) -> Option<&TableLayout> {
    world.registry.table(id)
}
pub(crate) fn archetype_inner(world: &World, id: ArchetypeId) -> Option<&Archetype> {
    world.registry.archetype(id)
}
pub(crate) fn with_partitions_inner<const PARTITION_COUNT: usize>() -> Result<World, EcsError> {
    todo!("@to_design 固定分区初始化、数量上限及分配来源编码尚未确定")
}
pub(crate) fn spawn_inner<'w>(
    _world: &'w World,
    _partition: crate::entity::PartitionId,
    _domain: crate::entity::AllocationDomain,
    _archetype: ArchetypeId,
    _values: crate::entity_values::EntityValues<'_>,
) -> Result<crate::owned_entity::OwnedEntity<'w>, EcsError> {
    todo!("@to_design 域/区合法组合与容量边界；执行失败直接 panic")
}
pub(crate) fn despawn_inner(_world: &mut World, _entity: Entity) -> Result<bool, EcsError> {
    todo!("@to_design 删除记录表示；执行失败直接 panic")
}
pub(crate) fn change_archetype_inner(
    _world: &World,
    _entity: Entity,
    _target: ArchetypeId,
    _added: crate::entity_values::EntityValues<'_>,
) -> Result<(), EcsError> {
    todo!("@to_design 变化记录与访问协议；搬移失败直接 panic")
}
pub(crate) fn register_archetype_on_inner(
    _world: &mut World,
    _base: ArchetypeId,
    _components: &[ComponentId],
) -> Result<ArchetypeId, EcsError> {
    todo!("@to_design 指定 Table 原型登记")
}
pub(crate) fn locate_inner(_world: &World, _entity: Entity) -> Option<EntityLocation> {
    todo!("尚未实现")
}
pub(crate) fn settle_inner(_world: &mut World) -> Result<(), EcsError> {
    todo!("尚未实现")
}
pub(crate) fn move_partition_inner(
    _world: &mut World,
    _request: PartitionMove,
) -> Result<(), EcsError> {
    todo!("尚未实现")
}
