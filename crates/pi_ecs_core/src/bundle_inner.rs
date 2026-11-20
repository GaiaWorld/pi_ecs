//! 类型化组合配对芯；RC 直接写入已准备的槽，不收集擦除值。
use super::*;
#[cfg(all(test, feature = "rc"))]
mod bundle_install_tests {
    use super::*;
    use crate::{component::Component, unsafe_world_cell::UnsafeWorldCell, world::World};
    struct Number(u32);
    impl Component for Number {
        type Access = PlainAccess;
    }
    struct Flag(bool);
    impl Component for Flag {
        type Access = PlainAccess;
    }

    // Seam: Bundle 的类型展平、验证和公开 writer 安装；使用真实 World 保活来源。
    #[test]
    fn bundle_install_nested_tuple_preserves_order_and_initializes_prefix() {
        type Values = ((ComponentValue<Number>, ()), ComponentValue<Flag>);
        assert_eq!(
            Values::component_types(),
            vec![TypeId::of::<Number>(), TypeId::of::<Flag>()]
        );
        let mut world = World::new().unwrap();
        let number = world
            .register_component::<Number>(crate::component::StoragePolicy::SparseOnly)
            .unwrap();
        let flag = world
            .register_component::<Flag>(crate::component::StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let mut first = std::mem::MaybeUninit::<Number>::uninit();
        let mut second = std::mem::MaybeUninit::<Flag>::uninit();
        let targets = [
            std::ptr::NonNull::from(&mut first).cast(),
            std::ptr::NonNull::from(&mut second).cast(),
        ];
        let mut written = 0;
        let values = ((ComponentValue(Number(37)), ()), ComponentValue(Flag(true)));
        let context = BundleContext {
            world_identity: world.identity,
            entities: &world.entities,
            pending: &world.pending_destruction,
            access: &world.access,
        };
        values.validate(&context).unwrap();
        let mut writer = BundleWriter {
            world: UnsafeWorldCell::from_ref(&world),
            context,
            components: &[number, flag],
            types: &[TypeId::of::<Number>(), TypeId::of::<Flag>()],
            layouts: &[
                std::alloc::Layout::new::<Number>(),
                std::alloc::Layout::new::<Flag>(),
            ],
            targets: &targets,
            written: &mut written,
        };
        values.write(&mut writer);
        assert_eq!(written, 2);
        // SAFETY: writer 成功返回且已报告两个完整初始化槽；各槽只消费一次。
        unsafe {
            assert_eq!(first.assume_init().0, 37);
            assert!(second.assume_init().0);
        }
        world.close().unwrap();
    }
}
pub(super) trait BundleParts: Sized {
    fn types_parts() -> Vec<TypeId>;
    fn validate_parts(&self, context: &BundleContext<'_>) -> Result<(), EcsError>;
    fn write_parts(self, writer: &mut BundleWriter<'_>);
}

impl BundleWriter<'_> {
    pub(super) fn write_input_inner<I: crate::bundle_input::ComponentInput>(&mut self, _input: I) {
        let index = *self.written;
        let reject = || -> ! {
            #[cfg(feature = "rc")]
            self.context
                .access
                .mode
                .set(crate::world::AccessMode::Stopped);
            #[cfg(not(feature = "rc"))]
            self.context
                .access
                .0
                .store(4, std::sync::atomic::Ordering::Release);
            panic!("Bundle 实际输入与已准备槽不匹配");
        };
        let layout = std::alloc::Layout::new::<I::Component>();
        if self.world.id() != self.context.world_identity
            || self.components.get(index).is_none()
            || self.types.get(index) != Some(&TypeId::of::<I::Component>())
            || self.layouts.get(index) != Some(&layout)
        {
            reject();
        }
        let Some(target) = self.targets.get(index).copied() else {
            reject()
        };
        if target.as_ptr() as usize % layout.align() != 0 {
            reject();
        }
        // SAFETY: 原安装作用域保活来源；只调用输入的独立验证，不读取其他业务值。
        if unsafe { _input.validate(self.world) }.is_err() {
            reject();
        }
        struct AbortOnUnwind;
        impl Drop for AbortOnUnwind {
            fn drop(&mut self) {
                std::process::abort();
            }
        }
        let guard = AbortOnUnwind;
        // SAFETY: 类型、布局、顺序和输入来源均已校验；目标由安装作用域独占提供。
        unsafe {
            _input.install(
                &mut *target
                    .cast::<std::mem::MaybeUninit<I::Component>>()
                    .as_ptr(),
                self.world,
            );
        }
        std::mem::forget(guard);
        *self.written = index + 1;
    }
    pub(super) fn write_value_inner<C: Component<Access = PlainAccess>>(&mut self, _value: C) {
        self.write_input(ComponentValue(_value))
    }
}
impl BundleParts for () {
    fn types_parts() -> Vec<TypeId> {
        Vec::new()
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        Ok(())
    }
    fn write_parts(self, _writer: &mut BundleWriter<'_>) {}
}
impl<B0: Bundle> BundleParts for (B0,) {
    fn types_parts() -> Vec<TypeId> {
        let mut types = Vec::new();
        types.extend(B0::component_types());
        types
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.0.validate(_context)?;
        Ok(())
    }
    fn write_parts(self, _writer: &mut BundleWriter<'_>) {
        self.0.write(_writer);
    }
}
impl<B0: Bundle, B1: Bundle> BundleParts for (B0, B1) {
    fn types_parts() -> Vec<TypeId> {
        let mut types = Vec::new();
        types.extend(B0::component_types());
        types.extend(B1::component_types());
        types
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.0.validate(_context)?;
        self.1.validate(_context)?;
        Ok(())
    }
    fn write_parts(self, _writer: &mut BundleWriter<'_>) {
        self.0.write(_writer);
        self.1.write(_writer);
    }
}
impl<B0: Bundle, B1: Bundle, B2: Bundle> BundleParts for (B0, B1, B2) {
    fn types_parts() -> Vec<TypeId> {
        let mut types = Vec::new();
        types.extend(B0::component_types());
        types.extend(B1::component_types());
        types.extend(B2::component_types());
        types
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.0.validate(_context)?;
        self.1.validate(_context)?;
        self.2.validate(_context)?;
        Ok(())
    }
    fn write_parts(self, _writer: &mut BundleWriter<'_>) {
        self.0.write(_writer);
        self.1.write(_writer);
        self.2.write(_writer);
    }
}
impl<B0: Bundle, B1: Bundle, B2: Bundle, B3: Bundle> BundleParts for (B0, B1, B2, B3) {
    fn types_parts() -> Vec<TypeId> {
        let mut types = Vec::new();
        types.extend(B0::component_types());
        types.extend(B1::component_types());
        types.extend(B2::component_types());
        types.extend(B3::component_types());
        types
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.0.validate(_context)?;
        self.1.validate(_context)?;
        self.2.validate(_context)?;
        self.3.validate(_context)?;
        Ok(())
    }
    fn write_parts(self, _writer: &mut BundleWriter<'_>) {
        self.0.write(_writer);
        self.1.write(_writer);
        self.2.write(_writer);
        self.3.write(_writer);
    }
}
impl<B0: Bundle, B1: Bundle, B2: Bundle, B3: Bundle, B4: Bundle> BundleParts
    for (B0, B1, B2, B3, B4)
{
    fn types_parts() -> Vec<TypeId> {
        let mut types = Vec::new();
        types.extend(B0::component_types());
        types.extend(B1::component_types());
        types.extend(B2::component_types());
        types.extend(B3::component_types());
        types.extend(B4::component_types());
        types
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.0.validate(_context)?;
        self.1.validate(_context)?;
        self.2.validate(_context)?;
        self.3.validate(_context)?;
        self.4.validate(_context)?;
        Ok(())
    }
    fn write_parts(self, _writer: &mut BundleWriter<'_>) {
        self.0.write(_writer);
        self.1.write(_writer);
        self.2.write(_writer);
        self.3.write(_writer);
        self.4.write(_writer);
    }
}
impl<B0: Bundle, B1: Bundle, B2: Bundle, B3: Bundle, B4: Bundle, B5: Bundle> BundleParts
    for (B0, B1, B2, B3, B4, B5)
{
    fn types_parts() -> Vec<TypeId> {
        let mut types = Vec::new();
        types.extend(B0::component_types());
        types.extend(B1::component_types());
        types.extend(B2::component_types());
        types.extend(B3::component_types());
        types.extend(B4::component_types());
        types.extend(B5::component_types());
        types
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.0.validate(_context)?;
        self.1.validate(_context)?;
        self.2.validate(_context)?;
        self.3.validate(_context)?;
        self.4.validate(_context)?;
        self.5.validate(_context)?;
        Ok(())
    }
    fn write_parts(self, _writer: &mut BundleWriter<'_>) {
        self.0.write(_writer);
        self.1.write(_writer);
        self.2.write(_writer);
        self.3.write(_writer);
        self.4.write(_writer);
        self.5.write(_writer);
    }
}
impl<B0: Bundle, B1: Bundle, B2: Bundle, B3: Bundle, B4: Bundle, B5: Bundle, B6: Bundle> BundleParts
    for (B0, B1, B2, B3, B4, B5, B6)
{
    fn types_parts() -> Vec<TypeId> {
        let mut types = Vec::new();
        types.extend(B0::component_types());
        types.extend(B1::component_types());
        types.extend(B2::component_types());
        types.extend(B3::component_types());
        types.extend(B4::component_types());
        types.extend(B5::component_types());
        types.extend(B6::component_types());
        types
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.0.validate(_context)?;
        self.1.validate(_context)?;
        self.2.validate(_context)?;
        self.3.validate(_context)?;
        self.4.validate(_context)?;
        self.5.validate(_context)?;
        self.6.validate(_context)?;
        Ok(())
    }
    fn write_parts(self, _writer: &mut BundleWriter<'_>) {
        self.0.write(_writer);
        self.1.write(_writer);
        self.2.write(_writer);
        self.3.write(_writer);
        self.4.write(_writer);
        self.5.write(_writer);
        self.6.write(_writer);
    }
}
impl<
        B0: Bundle,
        B1: Bundle,
        B2: Bundle,
        B3: Bundle,
        B4: Bundle,
        B5: Bundle,
        B6: Bundle,
        B7: Bundle,
    > BundleParts for (B0, B1, B2, B3, B4, B5, B6, B7)
{
    fn types_parts() -> Vec<TypeId> {
        let mut types = Vec::new();
        types.extend(B0::component_types());
        types.extend(B1::component_types());
        types.extend(B2::component_types());
        types.extend(B3::component_types());
        types.extend(B4::component_types());
        types.extend(B5::component_types());
        types.extend(B6::component_types());
        types.extend(B7::component_types());
        types
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.0.validate(_context)?;
        self.1.validate(_context)?;
        self.2.validate(_context)?;
        self.3.validate(_context)?;
        self.4.validate(_context)?;
        self.5.validate(_context)?;
        self.6.validate(_context)?;
        self.7.validate(_context)?;
        Ok(())
    }
    fn write_parts(self, _writer: &mut BundleWriter<'_>) {
        self.0.write(_writer);
        self.1.write(_writer);
        self.2.write(_writer);
        self.3.write(_writer);
        self.4.write(_writer);
        self.5.write(_writer);
        self.6.write(_writer);
        self.7.write(_writer);
    }
}
impl<C: Component<Access = PlainAccess>> BundleParts for ComponentValue<C> {
    fn types_parts() -> Vec<TypeId> {
        vec![TypeId::of::<C>()]
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        let stopped = {
            let mode = _context.access.mode.replace(crate::world::AccessMode::Idle);
            let stopped = matches!(mode, crate::world::AccessMode::Stopped);
            _context.access.mode.set(mode);
            stopped
        };
        #[cfg(not(feature = "rc"))]
        let stopped = _context.access.0.load(std::sync::atomic::Ordering::Acquire) & 0xff == 4;
        if stopped {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "World 已停止",
            ));
        }
        Ok(())
    }
    fn write_parts(self, writer: &mut BundleWriter<'_>) {
        writer.write_input(self)
    }
}
impl BundleWriter<'_> {
    pub(super) fn write_strong_inner<'s, const N: usize, T: 'static>(
        &mut self,
        _value: crate::bundle_input::StrongArrayInput<'s, N, T>,
    ) {
        self.write_input(_value)
    }
}
impl BundleWriter<'_> {
    pub(super) fn write_weak_inner<'s, const N: usize, T: 'static>(
        &mut self,
        _value: crate::bundle_input::WeakArrayInput<'s, N, T>,
    ) {
        self.write_input(_value)
    }
}
impl BundleWriter<'_> {
    pub(super) fn write_list_inner<T: 'static>(
        &mut self,
        _value: crate::bundle_input::EmptyList<T>,
    ) {
        self.write_input(_value)
    }
}
impl BundleWriter<'_> {
    pub(super) fn write_queue_inner<T: 'static>(
        &mut self,
        _value: crate::bundle_input::EmptyQueue<T>,
    ) {
        self.write_input(_value)
    }
}
impl BundleWriter<'_> {
    pub(super) fn write_deque_inner<T: 'static>(
        &mut self,
        _value: crate::bundle_input::EmptyDeque<T>,
    ) {
        self.write_input(_value)
    }
}
