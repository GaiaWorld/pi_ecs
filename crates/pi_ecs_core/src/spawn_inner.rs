//! 系统创建的配对芯；真实安装复用宿主的直接写与发布协议。
use super::*;

impl<B: SystemBundle<Shape = B> + 'static> SpawnState<B> {
    pub(super) fn declaration_inner(&self) -> AccessDeclaration {
        let mut components = self.ordered_components.clone();
        components.sort_unstable();
        AccessDeclaration {
            resources: Box::new([]),
            multi_resources: Box::new([]),
            components: Box::new([]),
            tick_reads: Box::new([]),
            reads_entities: false,
            scope: crate::access::EntityScope {
                with: Box::new([]),
                without: Box::new([]),
            },
            structure: crate::access::StructuralAccess::Spawn {
                components,
                partition: self.partition,
            },
            observations: Box::new([]),
        }
    }
}

impl<'run, B: SystemBundle<Shape = B> + 'static> Spawn<'run, B> {
    pub(super) fn spawn_inner<I: SystemBundle<Shape = B>>(
        &mut self,
        values: I,
    ) -> Result<OwnedEntity<'run>, EcsError> {
        #[cfg(feature = "rc")]
        {
            // SAFETY: bind_spawn authorizes the entire fixed target and excludes conflicting live views.
            unsafe {
                crate::world::World::spawn_from_system(
                    &self.access,
                    self.state.partition,
                    &self.state.ordered_components,
                    self.tick,
                    values,
                )
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = values;
            Err(crate::error::classified_error(
                crate::error::ErrorKind::Unsupported,
                "非 RC Spawn 延期",
            ))
        }
    }
}
