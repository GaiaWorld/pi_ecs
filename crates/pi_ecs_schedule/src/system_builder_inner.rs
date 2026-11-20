//! 显式构建器与参数配置配对芯。
#[cfg(test)]
mod builder_tests {
    use crate::resource_param::MultiResConfig;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[cfg(feature = "rc")]
    #[test]
    fn builder_build_is_uninitialized_and_unregister_releases_config_once() {
        use crate::{
            local::{Local, LocalConfig},
            system::{System, SystemId},
            system_builder::SystemBuilder,
            system_param::ParamRegistration,
        };
        let drops = Arc::new(AtomicUsize::new(0));
        let called = std::rc::Rc::new(std::cell::Cell::new(false));
        let capture = called.clone();
        let function = move |_: Local<'_, Payload>| capture.set(true);
        let mut system =
            SystemBuilder::with_config(function, (LocalConfig::new(Payload(drops.clone())),))
                .build(SystemId(37))
                .unwrap();
        assert_eq!(system.id(), SystemId(37));
        assert!(!called.get());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        let mut world = pi_ecs_core::world::World::new().unwrap();
        world.finish_registration().unwrap();
        world
            .parameter_unregistration_scope(|access| {
                system.unregister(&mut ParamRegistration::new(access))
            })
            .unwrap();
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        drop(system);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(!called.get());
        world.close().unwrap();
    }
    #[test]
    fn builder_default_constructs_once_without_calling_function() {
        static DEFAULTS: AtomicUsize = AtomicUsize::new(0);
        struct Initial;
        impl Default for Initial {
            fn default() -> Self {
                DEFAULTS.fetch_add(1, Ordering::SeqCst);
                Self
            }
        }
        let called = std::rc::Rc::new(std::cell::Cell::new(false));
        let capture = called.clone();
        let builder = crate::system_builder::SystemBuilder::new(
            move |_: crate::local::Local<'_, Initial>| capture.set(true),
        );
        assert_eq!(DEFAULTS.load(Ordering::SeqCst), 1);
        assert!(builder.config.value.0.recovery.is_none());
        drop(builder);
        assert_eq!(DEFAULTS.load(Ordering::SeqCst), 1);
        assert!(!called.get());
    }
    #[test]
    fn builder_explicit_config_keeps_positions_and_defers_call() {
        use crate::{
            local::{Local, LocalConfig},
            system_builder::SystemBuilder,
        };
        let called = std::rc::Rc::new(std::cell::Cell::new(false));
        let capture = called.clone();
        let function = move |_: Local<'_, u32>, _: Local<'_, u32>| capture.set(true);
        let builder =
            SystemBuilder::with_config(function, (LocalConfig::new(7), LocalConfig::new(19)));
        assert_eq!(builder.config.value.0.initial, 7);
        assert_eq!(builder.config.value.1.initial, 19);
        assert!(!called.get());
        drop(builder);
        assert!(!called.get());
    }
    #[test]
    fn builder_local_default_once() {
        static DEFAULTS: AtomicUsize = AtomicUsize::new(0);
        struct Initial(std::cell::Cell<u32>);
        impl Default for Initial {
            fn default() -> Self {
                DEFAULTS.fetch_add(1, Ordering::SeqCst);
                Self(std::cell::Cell::new(13))
            }
        }
        let config = crate::local::LocalConfig::<Initial>::default();
        assert_eq!(config.initial.0.get(), 13);
        assert!(config.recovery.is_none());
        assert_eq!(DEFAULTS.load(Ordering::SeqCst), 1);
    }
    // Seam: Local 配置接管支持 !Sync 的初值，不创建共享状态。
    #[test]
    fn builder_local_initial_owned_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let config =
            crate::local::LocalConfig::new((Payload(drops.clone()), std::cell::Cell::new(7)));
        assert!(config.recovery.is_none());
        assert_eq!(config.initial.1.get(), 7);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(config);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
    // Seam: Default 构造仅调用一次 T::default，且不选择恢复策略。
    #[test]
    fn builder_multires_default_once() {
        static DEFAULTS: AtomicUsize = AtomicUsize::new(0);
        struct Initial;
        impl Default for Initial {
            fn default() -> Self {
                DEFAULTS.fetch_add(1, Ordering::SeqCst);
                Self
            }
        }
        let config = MultiResConfig::<Initial>::default();
        assert!(config.recovery.is_none());
        assert_eq!(DEFAULTS.load(Ordering::SeqCst), 1);
    }
    // Seam: 公共配置构造接管非 Clone 初值；释放配置恰好释放一次。
    #[test]
    fn builder_multires_initial_owned_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let config = MultiResConfig::new(Payload(drops.clone()));
        assert!(config.recovery.is_none());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(config);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::MultiResConfig<T> {
    pub(crate) fn multiresconfig_new(_initial: T) -> crate::resource_param::MultiResConfig<T> {
        Self {
            initial: _initial,
            recovery: None,
            _pending_capabilities: (),
        }
    }
}
impl<T: Send + Sync + Default + 'static> crate::resource_param::MultiResConfig<T> {
    pub(crate) fn multiresconfig_default() -> crate::resource_param::MultiResConfig<T> {
        Self::new(T::default())
    }
}
impl<T: Send + Sync + Clone + 'static> crate::resource_param::MultiResConfig<T> {
    pub(crate) fn multiresconfig_clone(
        _config: crate::resource_param::MultiResConfig<T>,
    ) -> Result<crate::resource_param::MultiResConfig<T>, EcsError> {
        todo!("编号：T99，尚未实现， 初值与恢复策略配置")
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::MultiResConfig<T> {
    pub(crate) fn multiresconfig_with<S: 'static>(
        _config: crate::resource_param::MultiResConfig<T>,
        _capture: pi_ecs_core::snapshot_rules::SnapshotCapture<T, S>,
        _restore: pi_ecs_core::snapshot_rules::SnapshotRestore<T, S>,
    ) -> Result<crate::resource_param::MultiResConfig<T>, EcsError> {
        todo!("编号：T100，尚未实现， 初值与恢复策略配置")
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::MultiResConfig<T> {
    pub(crate) fn multiresconfig_rebuild(
        _config: crate::resource_param::MultiResConfig<T>,
        _reset: pi_ecs_core::snapshot_rules::DerivedInitial<T>,
    ) -> Result<crate::resource_param::MultiResConfig<T>, EcsError> {
        todo!("编号：T101，尚未实现， 初值与恢复策略配置")
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::MultiResConfig<T> {
    pub(crate) fn multiresconfig_preserve(
        _config: crate::resource_param::MultiResConfig<T>,
    ) -> Result<crate::resource_param::MultiResConfig<T>, EcsError> {
        todo!("编号：T102，尚未实现， 初值与恢复策略配置")
    }
}

impl<T: Send + 'static> crate::local::LocalConfig<T> {
    pub(crate) fn localconfig_new(_initial: T) -> crate::local::LocalConfig<T> {
        Self {
            initial: _initial,
            recovery: None,
            _pending_capabilities: (),
        }
    }
}
impl<T: Send + Default + 'static> crate::local::LocalConfig<T> {
    pub(crate) fn localconfig_default() -> crate::local::LocalConfig<T> {
        Self::new(T::default())
    }
}
impl<T: Send + Clone + 'static> crate::local::LocalConfig<T> {
    pub(crate) fn localconfig_clone(
        _config: crate::local::LocalConfig<T>,
    ) -> Result<crate::local::LocalConfig<T>, EcsError> {
        todo!("编号：T103，尚未实现， 初值与恢复策略配置")
    }
}
impl<T: Send + 'static> crate::local::LocalConfig<T> {
    pub(crate) fn localconfig_with<S: 'static>(
        _config: crate::local::LocalConfig<T>,
        _capture: pi_ecs_core::snapshot_rules::SnapshotCapture<T, S>,
        _restore: pi_ecs_core::snapshot_rules::SnapshotRestore<T, S>,
    ) -> Result<crate::local::LocalConfig<T>, EcsError> {
        todo!("编号：T104，尚未实现， 初值与恢复策略配置")
    }
}
impl<T: Send + 'static> crate::local::LocalConfig<T> {
    pub(crate) fn localconfig_rebuild(
        _config: crate::local::LocalConfig<T>,
        _reset: pi_ecs_core::snapshot_rules::DerivedInitial<T>,
    ) -> Result<crate::local::LocalConfig<T>, EcsError> {
        todo!("编号：T105，尚未实现， 初值与恢复策略配置")
    }
}
impl<T: Send + 'static> crate::local::LocalConfig<T> {
    pub(crate) fn localconfig_preserve(
        _config: crate::local::LocalConfig<T>,
    ) -> Result<crate::local::LocalConfig<T>, EcsError> {
        todo!("编号：T106，尚未实现， 初值与恢复策略配置")
    }
}

use crate::{
    system::{FunctionSystem, SystemFunction, SystemId},
    system_builder::{ParamConfig, SystemBuilder},
    system_param::SystemParam,
};
use pi_ecs_core::error::EcsError;
impl<P: SystemParam> ParamConfig<P> {
    pub(crate) fn config_inner(value: P::Config) -> ParamConfig<P> {
        ParamConfig { value }
    }
}
impl<F: SystemFunction<M>, M> SystemBuilder<F, M> {
    pub(crate) fn new_inner(_function: F) -> SystemBuilder<F, M>
    where
        <F::Param as SystemParam>::Config: Default,
    {
        Self::with_config(_function, Default::default())
    }
}
impl<F: SystemFunction<M>, M> SystemBuilder<F, M> {
    pub(crate) fn with_config_inner(
        _function: F,
        _config: <F::Param as SystemParam>::Config,
    ) -> SystemBuilder<F, M> {
        Self {
            function: _function,
            config: ParamConfig::new(_config),
            signature: std::marker::PhantomData,
        }
    }
}
impl<F: SystemFunction<M>, M> SystemBuilder<F, M> {
    pub(super) fn build_inner(
        self,
        _id: SystemId,
    ) -> Result<FunctionSystem<F, F::Param, M>, EcsError> {
        Ok(FunctionSystem {
            matched_revision: None,
            prepared_access: None,
            signature: std::marker::PhantomData,
            id: _id,
            function: self.function,
            state: crate::system::FunctionSystemState::Uninitialized(self.config),
            last_run: None,
        })
    }
}
