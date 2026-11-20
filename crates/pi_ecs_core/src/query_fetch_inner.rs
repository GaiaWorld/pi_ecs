//! 自定义 fetch 受限入口的配对芯；RC 校验原授权后访问单槽。
use super::{FetchAccess, FetchEntity, FetchScope};
use crate::{
    archetype::Archetype,
    component::{Component, ComponentId, ComponentRead, ComponentWrite},
    entity::{ArchetypeId, Entity, EntityLocation},
    world::World,
};
impl FetchEntity {
    pub(super) fn entity_inner(_entity: &FetchEntity) -> Entity {
        _entity.entity
    }
}
impl FetchEntity {
    pub(super) fn location_inner(_entity: &FetchEntity) -> EntityLocation {
        _entity.location
    }
}
impl<'w> FetchAccess<'w> {
    pub(super) fn retain_inner(_access: &FetchAccess<'w>) -> FetchAccess<'w> {
        Self {
            world: _access.world,
            declaration: _access.declaration,
            window: _access.window,
            write_tick: _access.write_tick,
        }
    }
}
impl<'w> FetchAccess<'w> {
    pub(super) fn read_inner<T: Component>(
        _access: &FetchAccess<'w>,
        _entity: &FetchEntity,
    ) -> Option<ComponentRead<'w, 'w, T>> {
        #[cfg(feature = "rc")]
        unsafe {
            let root = _access.world.pointer.as_ptr();
            let registry = &*std::ptr::addr_of!((*root).registry);
            let component = registry
                .component_ids
                .get(&std::any::TypeId::of::<T>())
                .expect("Fetch 读取未登记组件");
            assert!(
                _access
                    .declaration
                    .components
                    .iter()
                    .any(|(id, _)| id == component),
                "Fetch 缺少读权限"
            );
            let entities = &*std::ptr::addr_of!((*root).entities);
            assert_eq!(
                entities.borrow().locate(_entity.entity),
                Some(_entity.location),
                "Fetch 实体位置过期"
            );
            let archetype = registry
                .archetype(_entity.location.archetype())
                .expect("Fetch 原型缺失");
            let has = |id: &ComponentId| {
                archetype
                    .components
                    .binary_search_by_key(id, |entry| entry.component)
                    .is_ok()
            };
            let scope = &_access.declaration.scope;
            assert!(
                scope.with.iter().all(has) && scope.without.iter().all(|id| !has(id)),
                "Fetch 实体超出授权范围"
            );
            World::read_component_at::<T>(_access.world, _entity.entity, _entity.location)
        }
        #[cfg(not(feature = "rc"))]
        panic!("非 RC Fetch 读取延期")
    }
}
impl<'w> FetchAccess<'w> {
    pub(super) fn write_inner<T: Component>(
        access: &FetchAccess<'w>,
        entity: &FetchEntity,
    ) -> Option<ComponentWrite<'w, 'w, T>> {
        #[cfg(feature = "rc")]
        unsafe {
            use crate::access::AccessMode;
            assert!(access.world.mutable, "Fetch 来源未授权可变访问");
            let tick = access.write_tick.expect("Fetch 缺少写 tick");
            let root = access.world.pointer.as_ptr();
            let registry = &*std::ptr::addr_of!((*root).registry);
            let component = registry
                .component_ids
                .get(&std::any::TypeId::of::<T>())
                .expect("Fetch 写入未登记组件");
            assert!(
                access
                    .declaration
                    .components
                    .contains(&(*component, AccessMode::Write)),
                "Fetch 缺少组件写权限"
            );
            let entities = &*std::ptr::addr_of!((*root).entities);
            assert_eq!(
                entities.borrow().locate(entity.entity),
                Some(entity.location),
                "Fetch 实体位置过期"
            );
            let archetype = registry
                .archetype(entity.location.archetype())
                .expect("Fetch 原型缺失");
            let has = |id: &ComponentId| {
                archetype
                    .components
                    .binary_search_by_key(id, |entry| entry.component)
                    .is_ok()
            };
            let scope = &access.declaration.scope;
            assert!(
                scope.with.iter().all(has) && scope.without.iter().all(|id| !has(id)),
                "Fetch 实体超出授权范围"
            );
            // SAFETY: the caller excludes overlapping results; the checks above validate the original leaf.
            World::write_component_at::<T>(access.world, entity.entity, entity.location, tick)
        }
        #[cfg(not(feature = "rc"))]
        panic!("非 RC Fetch 写入延期")
    }
}
impl World {
    pub(super) fn component_id_inner<T: Component>(world: &World) -> Option<ComponentId> {
        world
            .registry
            .component_ids
            .get(&std::any::TypeId::of::<T>())
            .copied()
    }
}

impl super::FetchBinding<'_> {
    pub(super) fn column_address_inner(
        &self,
        _table: u16,
        _column: pi_ecs_storage::table::ColumnId,
    ) -> Result<std::ptr::NonNull<pi_ecs_storage::column::Column>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        return self
            .storage
            .table(_table)
            .and_then(|table| table.column_address(_column))
            .ok_or_else(|| {
                crate::error::classified_error(crate::error::ErrorKind::NotFound, "列绑定不存在")
            });
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC 列绑定延期",
        ))
    }
    pub(super) fn sparse_address_inner(
        &self,
        _component: ComponentId,
    ) -> Result<
        std::ptr::NonNull<pi_ecs_storage::sparse_set::SparseSetStorage>,
        crate::error::EcsError,
    > {
        #[cfg(feature = "rc")]
        return self.storage.sparse_address(_component).ok_or_else(|| {
            crate::error::classified_error(crate::error::ErrorKind::NotFound, "稀疏绑定不存在")
        });
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC 稀疏绑定延期",
        ))
    }
}

impl FetchScope {
    pub(super) fn identity_inner(_archetype: &Archetype) -> Self {
        Self::Archetype {
            archetype: _archetype.id(),
        }
    }
}
