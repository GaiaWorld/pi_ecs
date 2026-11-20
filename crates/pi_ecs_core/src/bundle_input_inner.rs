//! 内置输入同源校验、安装和未移交责任清偿。

#[cfg(all(test, feature = "rc"))]
mod tests {
    use super::*;
    use crate::{
        entity::{AllocationDomain, EntityLocation},
        world::World,
    };

    #[test]
    fn array_input_cross_world_and_repeated_weak_responsibility() {
        let mut world = World::new().unwrap();
        let mut other = World::new().unwrap();
        fn publish(world: &mut World) -> Entity {
            world.finish_registration().unwrap();
            let storage = world.storage.as_mut().unwrap();
            let row = storage.table_mut(0).unwrap().claim_row().unwrap();
            let id = crate::entity::ArchetypeId {
                table: 0,
                variant: 0,
            };
            let location = EntityLocation::from_parts(0, 0, row.index(), 0, 0);
            let entity = world
                .entities
                .get_mut()
                .allocate(AllocationDomain(0), location)
                .unwrap();
            storage.append_member(id, entity, row).unwrap();
            storage
                .table_mut(0)
                .unwrap()
                .publish_row(row, entity)
                .unwrap();
            entity
        }
        let entity = publish(&mut world);
        let foreign = publish(&mut other);
        let original = OwnedEntity {
            world_identity: world.identity,
            entity,
            entities: &world.entities,
            pending_destruction: &world.pending_destruction,
            access: &world.access,
        };
        let first = original.try_clone().unwrap();
        // Keep the original externally owned responsibility until the assertions finish.
        let second = OwnedEntity {
            world_identity: other.identity,
            entity: foreign,
            entities: &other.entities,
            pending_destruction: &other.pending_destruction,
            access: &other.access,
        };
        let rejected = StrongArrayInput::<2, ()>::new([first, second])
            .err()
            .unwrap();
        let [first, second] = rejected.values;
        assert!(WeakArrayInput::<2, ()>::new([&first, &second]).is_err());
        assert_eq!(world.entities.borrow().counts(entity), Some((2, 0)));
        let weak = WeakArrayInput::<2, ()>::new([&first, &first]).unwrap();
        assert_eq!(world.entities.borrow().counts(entity), Some((2, 2)));
        let source = UnsafeWorldCell::from_ref(&world);
        let wrong = UnsafeWorldCell::from_ref(&other);
        assert!(unsafe { ComponentInput::validate(&weak, wrong) }.is_err());
        unsafe { ComponentInput::validate(&weak, source) }.unwrap();
        drop(weak);
        assert_eq!(world.entities.borrow().counts(entity), Some((2, 0)));
        drop(first);
        drop(second);
        drop(original);
    }
}
use super::*;
use crate::{
    entity::Entity,
    entity_container::{EntityDeque, EntityList, EntityQueue},
    error::{classified_error, ErrorKind},
    owned_entity::{StrongEntities, WeakEntities},
    unsafe_world_cell::UnsafeWorldCell,
};

impl<'s> WeakInputBinding<'s> {
    fn from_owned(value: &OwnedEntity<'s>) -> Self {
        Self {
            identity: value.world_identity,
            entities: value.entities,
            pending: value.pending_destruction,
            access: value.access,
        }
    }

    fn matches(&self, context: &BundleContext<'_>) -> bool {
        self.identity == context.world_identity
            && std::ptr::eq(self.entities, context.entities)
            && std::ptr::eq(self.pending, context.pending)
            && std::ptr::eq(self.access, context.access)
    }
}

fn check_active(access: &crate::world::AccessState) -> Result<(), EcsError> {
    #[cfg(feature = "rc")]
    let stopped = {
        use crate::world::AccessMode;
        let mode = access.mode.replace(AccessMode::Stopped);
        let stopped = matches!(mode, AccessMode::Stopped);
        access.mode.set(mode);
        stopped
    };
    #[cfg(not(feature = "rc"))]
    let stopped = access.0.load(std::sync::atomic::Ordering::Acquire) & 0xff == 4;
    if stopped {
        return Err(classified_error(ErrorKind::InvalidState, "World 已停止"));
    }
    Ok(())
}

fn incompatible() -> EcsError {
    classified_error(ErrorKind::InvalidInput, "实体输入来源不属于目标 World")
}

fn invariant_failure(access: &crate::world::AccessState) -> ! {
    #[cfg(feature = "rc")]
    access.mode.set(crate::world::AccessMode::Stopped);
    #[cfg(not(feature = "rc"))]
    access.0.store(4, std::sync::atomic::Ordering::Release);
    std::process::abort()
}

fn check_strong(value: &OwnedEntity<'_>) {
    #[cfg(feature = "rc")]
    let counts = value
        .entities
        .try_borrow()
        .unwrap_or_else(|_| invariant_failure(value.access))
        .counts(value.entity);
    #[cfg(not(feature = "rc"))]
    let counts = value.entities.counts(value.entity);
    if !matches!(counts, Some((strong, _)) if strong > 0) {
        invariant_failure(value.access);
    }
}

fn validate_sources(values: &[&OwnedEntity<'_>]) -> Result<(), EcsError> {
    if let Some(first) = values.first() {
        let source = WeakInputBinding::from_owned(first);
        let context = BundleContext {
            world_identity: source.identity,
            entities: source.entities,
            pending: source.pending,
            access: source.access,
        };
        check_active(source.access)?;
        for value in values {
            if !WeakInputBinding::from_owned(value).matches(&context) {
                return Err(incompatible());
            }
            check_strong(value);
        }
    }
    Ok(())
}

// SAFETY: caller provides the active installation handle; only disjoint identity fields are read.
unsafe fn input_context(world: UnsafeWorldCell<'_>) -> BundleContext<'_> {
    let root = world.pointer.as_ptr();
    unsafe {
        BundleContext {
            world_identity: world.id(),
            entities: &*std::ptr::addr_of!((*root).entities),
            pending: &*std::ptr::addr_of!((*root).pending_destruction),
            access: &*std::ptr::addr_of!((*root).access),
        }
    }
}
impl<'s, const N: usize, T: 'static> StrongArrayInput<'s, N, T> {
    pub(super) fn strong_new_inner(
        _values: [OwnedEntity<'s>; N],
    ) -> Result<StrongArrayInput<'s, N, T>, StrongArrayInputError<'s, N>> {
        if let Err(error) = validate_sources(&_values.each_ref()) {
            return Err(StrongArrayInputError {
                error,
                values: _values,
            });
        }
        Ok(Self {
            values: Some(_values),
            marker: PhantomData,
        })
    }
}
impl<'s, const N: usize, T: 'static> WeakArrayInput<'s, N, T> {
    pub(super) fn weak_new_inner(
        _values: [&OwnedEntity<'s>; N],
    ) -> Result<WeakArrayInput<'s, N, T>, EcsError> {
        validate_sources(&_values)?;
        let mut input = Self {
            binding: _values
                .first()
                .map(|value| WeakInputBinding::from_owned(value)),
            values: [None; N],
            marker: PhantomData,
        };
        for (slot, value) in input.values.iter_mut().zip(_values) {
            #[cfg(feature = "rc")]
            let acquired = value
                .entities
                .try_borrow_mut()
                .unwrap_or_else(|_| invariant_failure(value.access))
                .acquire_weak(value.entity);
            #[cfg(not(feature = "rc"))]
            let acquired = value.entities.acquire_weak(value.entity);
            acquired?;
            *slot = Some(value.entity);
        }
        Ok(input)
    }
}
impl<const N: usize, T: 'static> WeakArrayInput<'_, N, T> {
    pub(super) fn drop_inner(&mut self) {
        let Some(binding) = &self.binding else { return };
        for slot in &mut self.values {
            let Some(entity) = slot.take() else { continue };
            #[cfg(feature = "rc")]
            let released = binding
                .entities
                .try_borrow_mut()
                .unwrap_or_else(|_| invariant_failure(binding.access))
                .release_weak(entity);
            #[cfg(not(feature = "rc"))]
            let released = binding.entities.release_weak(entity);
            match released {
                Ok(true) => {
                    binding.pending.reclamation.push(entity);
                }
                Ok(false) => {}
                Err(_) => invariant_failure(binding.access),
            }
        }
    }
}
pub(super) trait InputParts: Sized {
    fn types_parts() -> Vec<TypeId>;
    fn validate_parts(&self, context: &BundleContext<'_>) -> Result<(), EcsError>;
    fn write_parts(self, writer: &mut BundleWriter<'_>);
}

impl<T: 'static> EmptyList<T> {
    pub(super) fn new_list_inner() -> EmptyList<T> {
        Self(PhantomData)
    }
}
impl<T: 'static> EmptyQueue<T> {
    pub(super) fn new_queue_inner() -> EmptyQueue<T> {
        Self(PhantomData)
    }
}
impl<T: 'static> EmptyDeque<T> {
    pub(super) fn new_deque_inner() -> EmptyDeque<T> {
        Self(PhantomData)
    }
}
impl<'s, const N: usize, T: 'static> InputParts for StrongArrayInput<'s, N, T> {
    fn types_parts() -> Vec<TypeId> {
        vec![TypeId::of::<StrongEntities<N, T>>()]
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        check_active(_context.access)?;
        let values = self.values.as_ref().ok_or_else(incompatible)?;
        for value in values {
            if !WeakInputBinding::from_owned(value).matches(_context) {
                return Err(incompatible());
            }
            check_strong(value);
        }
        Ok(())
    }
    fn write_parts(self, writer: &mut BundleWriter<'_>) {
        writer.write_input(self)
    }
}
impl<'s, const N: usize, T: 'static> InputParts for WeakArrayInput<'s, N, T> {
    fn types_parts() -> Vec<TypeId> {
        vec![TypeId::of::<WeakEntities<N, T>>()]
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        check_active(_context.access)?;
        match &self.binding {
            Some(binding) if binding.matches(_context) => {}
            None if N == 0 => {}
            _ => return Err(incompatible()),
        }
        if self.values.iter().any(Option::is_none) {
            return Err(incompatible());
        }
        Ok(())
    }
    fn write_parts(self, writer: &mut BundleWriter<'_>) {
        writer.write_input(self)
    }
}
impl<T: 'static> InputParts for EmptyList<T> {
    fn types_parts() -> Vec<TypeId> {
        vec![TypeId::of::<EntityList<T>>()]
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        check_active(_context.access)
    }
    fn write_parts(self, writer: &mut BundleWriter<'_>) {
        writer.write_input(self)
    }
}
impl<T: 'static> InputParts for EmptyQueue<T> {
    fn types_parts() -> Vec<TypeId> {
        vec![TypeId::of::<EntityQueue<T>>()]
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        check_active(_context.access)
    }
    fn write_parts(self, writer: &mut BundleWriter<'_>) {
        writer.write_input(self)
    }
}
impl<T: 'static> InputParts for EmptyDeque<T> {
    fn types_parts() -> Vec<TypeId> {
        vec![TypeId::of::<EntityDeque<T>>()]
    }
    fn validate_parts(&self, _context: &BundleContext<'_>) -> Result<(), EcsError> {
        check_active(_context.access)
    }
    fn write_parts(self, writer: &mut BundleWriter<'_>) {
        writer.write_input(self)
    }
}

impl<C: crate::component::Component<Access = crate::component::PlainAccess>>
    crate::bundle::ComponentValue<C>
{
    pub(super) fn plain_validate_input_inner(
        &self,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        // 字段级读取停止态，不恢复整个 World 引用。
        let access = unsafe { &*std::ptr::addr_of!((*_world.pointer.as_ptr()).access) };
        #[cfg(feature = "rc")]
        let stopped = {
            let mode = access.mode.replace(crate::world::AccessMode::Idle);
            let stopped = matches!(mode, crate::world::AccessMode::Stopped);
            access.mode.set(mode);
            stopped
        };
        #[cfg(not(feature = "rc"))]
        let stopped = access.0.load(std::sync::atomic::Ordering::Acquire) & 0xff == 4;
        if stopped {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "World 已停止",
            ));
        }
        Ok(())
    }
    pub(super) fn plain_install_input_inner(
        self,
        _target: &mut std::mem::MaybeUninit<C>,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        _target.write(self.0);
    }
}

impl<'s, const N: usize, T: 'static> StrongArrayInput<'s, N, T> {
    pub(super) fn strong_validate_input_inner(
        &self,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        self.validate_parts(&unsafe { input_context(_world) })
    }
    pub(super) fn strong_install_input_inner(
        self,
        _target: &mut std::mem::MaybeUninit<crate::owned_entity::StrongEntities<N, T>>,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        let Some(values) = self.values else {
            std::process::abort()
        };
        _target.write(StrongEntities {
            entities: values.map(OwnedEntity::forget),
            marker: PhantomData,
        });
    }
}

impl<'s, const N: usize, T: 'static> WeakArrayInput<'s, N, T> {
    pub(super) fn weak_validate_input_inner(
        &self,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        self.validate_parts(&unsafe { input_context(_world) })
    }
    pub(super) fn weak_install_input_inner(
        self,
        _target: &mut std::mem::MaybeUninit<crate::owned_entity::WeakEntities<N, T>>,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        let mut input = self;
        let mut entities = [Entity::default(); N];
        for (target, source) in entities.iter_mut().zip(&mut input.values) {
            let Some(entity) = source.take() else {
                std::process::abort()
            };
            *target = entity;
        }
        _target.write(WeakEntities {
            entities,
            marker: PhantomData,
        });
    }
}

impl<T: 'static> EmptyList<T> {
    pub(super) fn list_validate_input_inner(
        &self,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        check_active(unsafe { input_context(_world) }.access)
    }
    pub(super) fn list_install_input_inner(
        self,
        _target: &mut std::mem::MaybeUninit<crate::entity_container::EntityList<T>>,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        _target.write(EntityList {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        });
    }
}

impl<T: 'static> EmptyQueue<T> {
    pub(super) fn queue_validate_input_inner(
        &self,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        check_active(unsafe { input_context(_world) }.access)
    }
    pub(super) fn queue_install_input_inner(
        self,
        _target: &mut std::mem::MaybeUninit<crate::entity_container::EntityQueue<T>>,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        _target.write(EntityQueue {
            head: None,
            tail: None,
            len: 0,
            marker: PhantomData,
        });
    }
}

impl<T: 'static> EmptyDeque<T> {
    pub(super) fn deque_validate_input_inner(
        &self,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        check_active(unsafe { input_context(_world) }.access)
    }
    pub(super) fn deque_install_input_inner(
        self,
        _target: &mut std::mem::MaybeUninit<crate::entity_container::EntityDeque<T>>,
        _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        _target.write(EntityDeque {
            deque: Default::default(),
            marker: PhantomData,
        });
    }
}
