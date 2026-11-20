//! 空参数配对芯，不以默认成功替代未实现生命周期。
#[cfg(all(test, feature = "rc"))]
mod tests {
    use crate::{
        param_composition::ParamAccess,
        system::SystemRunContext,
        system_param::{ParamRegistration, SystemParam},
    };
    use pi_ecs_core::{
        access::{AccessTree, OrderingArchetypes},
        change_detection::ChangeWindow,
        unsafe_world_cell::UnsafeWorldCell,
        world::World,
    };

    // Seam: SystemParam 的完整空参数生命周期，使用真实登记与系统授权作用域。
    #[test]
    fn rc_empty_param_lifecycle_has_no_access_or_registration() {
        let mut world = World::new().unwrap();
        world.finish_registration().unwrap();
        world.schedule_scope(|world| {
            world.parameter_registration_scope(|access| {
                <() as SystemParam>::initialize((), &mut ParamRegistration::new(access))
            })?;
            assert!(matches!(<() as SystemParam>::prepare(&mut (), world)?, ParamAccess::Together(items) if items.is_empty()));
            let tree = <() as SystemParam>::declaration(&(), world)?;
            assert!(matches!(&tree, AccessTree::Together(items) if items.is_empty()));
            let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
            let tick = unsafe { world.next_system_tick()? };
            // SAFETY: 空参数不借业务数据，State/Pending 及结果均在本次会话内收束。
            unsafe {
                world.system_scope(&tree, version, |access| {
                    let context = SystemRunContext::new(access, ChangeWindow { last_run: None, this_run: tick }).unwrap();
                    <() as SystemParam>::begin(&mut ());
                    <() as SystemParam>::fetch(&mut (), &mut (), context);
                })?;
                world.system_scope(&tree, version, |access| {
                    let context = SystemRunContext::new(access, ChangeWindow { last_run: None, this_run: tick }).unwrap();
                    <() as SystemParam>::finish(&mut (), (), &context);
                })?;
            }
            Ok(())
        }).unwrap();
        world
            .parameter_unregistration_scope(|access| {
                <() as SystemParam>::unregister((), &mut ParamRegistration::new(access))
            })
            .unwrap();
        world.close().unwrap();
    }

    #[test]
    fn rc_empty_param_ordering_has_no_entities_or_resources() {
        let logical = OrderingArchetypes {
            components: vec![Box::new([]) as Box<[_]>].into_boxed_slice(),
        };
        assert!(<() as SystemParam>::ordering_access(&(), &logical)
            .unwrap()
            .is_empty());
    }
}
use crate::{
    param_composition::ParamAccess, system::SystemRunContext, system_param::ParamRegistration,
};
use pi_ecs_core::{error::EcsError, world::WorldRef};
impl ParamRegistration<'_> {
    pub(super) fn empty_initialize(
        _config: (),
        _registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        Ok(())
    }
}
impl SystemRunContext<'_> {
    pub(super) fn empty_prepare(
        _state: &mut (),
        _world: WorldRef<'_>,
    ) -> Result<ParamAccess, EcsError> {
        Ok(ParamAccess::Together(Vec::new()))
    }
}
impl SystemRunContext<'_> {
    pub(super) fn empty_begin(_state: &mut ()) {
        // 空参数没有本次工作区。
    }
}
impl SystemRunContext<'_> {
    pub(super) fn empty_fetch(_state: &mut (), _pending: &mut (), _context: SystemRunContext<'_>) {
        // 空参数不兑现任何业务引用或登记能力。
    }
}
impl SystemRunContext<'_> {
    pub(super) fn empty_finish(_state: &mut (), _pending: (), _context: &SystemRunContext<'_>) {
        // 空参数没有待提交的修改或观察点。
    }
}
impl ParamRegistration<'_> {
    pub(super) fn empty_unregister(
        _state: (),
        _registration: &mut ParamRegistration<'_>,
    ) -> Result<(), EcsError> {
        Ok(())
    }
}

impl SystemRunContext<'_> {
    pub(super) fn empty_declaration(
        _state: &(),
        _world: WorldRef<'_>,
    ) -> Result<pi_ecs_core::access::AccessTree, EcsError> {
        Ok(pi_ecs_core::access::AccessTree::Together(Vec::new()))
    }
}

impl SystemRunContext<'_> {
    pub(super) fn empty_ordering_access(
        _state: &(),
        _logical: &pi_ecs_core::access::OrderingArchetypes,
    ) -> Result<Vec<pi_ecs_core::access::OrderingAccess>, pi_ecs_core::error::EcsError> {
        Ok(Vec::new())
    }
}
