//! 寿命绑定会话的配对芯；只投影同源字段，不还原整份 World 引用。
use super::EntityScope;
impl super::EntityScopeSession<'_> {
    pub(super) fn complete_inner(&mut self) {
        #[cfg(feature = "rc")]
        {
            use crate::world::AccessMode;
            match self.access.mode.replace(AccessMode::Stopped) {
                AccessMode::Maintaining => self.access.mode.set(AccessMode::Idle),
                AccessMode::Stopped => {}
                _ => std::process::abort(),
            }
            self.completed = true;
        }
        #[cfg(not(feature = "rc"))]
        unreachable!("非 RC 不建立实体会话");
    }
    pub(super) fn drop_inner(&mut self) {
        #[cfg(feature = "rc")]
        if !self.completed {
            self.access.mode.set(crate::world::AccessMode::Stopped);
        }
    }
}
use crate::{
    component::{Component, ComponentId, ComponentRead, ComponentWrite},
    entity::{AllocationDomain, Entity},
    error::EcsError,
    owned_entity::OwnedEntity,
};
impl<'s> EntityScope<'s> {
    pub(super) fn try_acquire_inner(&self, entity: Entity) -> Result<OwnedEntity<'s>, EcsError> {
        #[cfg(feature = "rc")]
        {
            self.require_maintaining()?;
            self.entities.borrow_mut().acquire_strong(entity)?;
            Ok(OwnedEntity {
                world_identity: self.world_identity,
                entity,
                entities: self.entities,
                pending_destruction: self.pending_destruction,
                access: self.access,
            })
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = entity;
            Err(scope_unsupported())
        }
    }
    pub(super) fn get_inner<'a, C: Component>(
        &'a self,
        entity: Entity,
    ) -> Result<Option<ComponentRead<'a, 's, C>>, EcsError> {
        #[cfg(feature = "rc")]
        {
            self.require_maintaining()?;
            let Some(location) = self.entities.borrow().locate(entity) else {
                return Ok(None);
            };
            let mut world = self.world;
            world.mutable = false;
            // SAFETY: 位置来自同源身份表且动态借用已结束；共享操作期无结构/写操作。
            Ok(unsafe { crate::world::World::read_component_at::<C>(world, entity, location) })
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = entity;
            Err(scope_unsupported())
        }
    }
    pub(super) fn get_mut_inner<'a, C: Component>(
        &'a mut self,
        entity: Entity,
    ) -> Result<Option<ComponentWrite<'a, 's, C>>, EcsError> {
        #[cfg(feature = "rc")]
        {
            self.require_maintaining()?;
            let Some(location) = self.entities.borrow().locate(entity) else {
                return Ok(None);
            };
            // SAFETY: 位置同源；独占短借用排斥冲突值和节点操作，长寿命仅用于身份责任。
            Ok(unsafe {
                crate::world::World::scope_write_component_at::<C>(self.world, entity, location)
            })
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = entity;
            Err(scope_unsupported())
        }
    }
    #[cfg(feature = "rc")]
    fn require_maintaining(&self) -> Result<(), EcsError> {
        if self.access.mode.get() != crate::world::AccessMode::Maintaining {
            return Err(crate::error::classified_error(
                crate::error::ErrorKind::InvalidState,
                "实体作用域不再持有维护授权",
            ));
        }
        Ok(())
    }
    pub(super) fn spawn_inner<B: crate::bundle::Bundle>(
        &mut self,
        domain: AllocationDomain,
        values: B,
    ) -> Result<OwnedEntity<'s>, EcsError> {
        #[cfg(feature = "rc")]
        {
            self.require_maintaining()?;
            // SAFETY: 独占操作期间没有冲突业务视图；只保留同源身份责任，复用宿主安装主体。
            unsafe { crate::world::World::install_spawn(self.world, domain, None, None, values) }
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (domain, values);
            Err(scope_unsupported())
        }
    }
    pub(super) fn change_archetype_inner<B: crate::bundle::Bundle>(
        &mut self,
        entity: Entity,
        added: B,
        removed: &[ComponentId],
    ) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        {
            self.require_maintaining()?;
            // SAFETY: 独占操作排斥活组件视图；调用后仍由外层会话持有 Maintaining。
            unsafe {
                crate::world::World::change_archetype_in_cell(self.world, entity, added, removed)
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (entity, added, removed);
            Err(scope_unsupported())
        }
    }
}

#[cfg(not(feature = "rc"))]
fn scope_unsupported() -> EcsError {
    crate::error::classified_error(crate::error::ErrorKind::Unsupported, "非 RC 实体作用域延期")
}
