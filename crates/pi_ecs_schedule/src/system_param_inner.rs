//! ???????????
use super::{EcsError, ParamRegistration, SystemParam};

impl ParamRegistration<'_> {
    pub(super) fn initialize_system_inner<P: SystemParam>(
        &mut self,
        system: crate::system::SystemId,
        config: P::Config,
    ) -> Result<P::State, EcsError> {
        #[cfg(feature = "rc")]
        {
            let previous = self.system.replace((system, 0));
            let result = self.initialize_parameter::<P>(config);
            self.system = previous;
            result
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = system;
            P::initialize(config, self)
        }
    }

    // Tuple and ParamSet traverse declaration order. A child that didn't advance the
    // cursor is one non-producing leaf (even with no access); nested children count
    // themselves. NamedParam delegates transparently to its underlying parameter.
    pub(super) fn initialize_parameter_inner<P: SystemParam>(
        &mut self,
        config: P::Config,
    ) -> Result<P::State, EcsError> {
        #[cfg(feature = "rc")]
        let before = self.system;
        let state = P::initialize(config, self)?;
        #[cfg(feature = "rc")]
        if let Some((_, position)) = self.system.as_mut() {
            if before.is_some_and(|(_, start)| start == *position) {
                *position += 1;
            }
        }
        Ok(state)
    }

    #[cfg(feature = "rc")]
    pub(super) fn producer_position_inner(
        &mut self,
    ) -> Result<(crate::system::SystemId, usize), EcsError> {
        let Some((system, position)) = self.system.as_mut() else {
            return Err(pi_ecs_core::error::classified_error(
                pi_ecs_core::error::ErrorKind::InvalidState,
                "multi-resource producer requires system initialization",
            ));
        };
        let current = *position;
        *position += 1;
        Ok((*system, current))
    }
}
