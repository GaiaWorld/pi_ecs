//! 临时拥有型实体引用配对芯。
fn array_get<const N: usize>(
    values: &[Entity; N],
    index: usize,
) -> Result<Entity, crate::error::EcsError> {
    values.get(index).copied().ok_or_else(|| {
        crate::error::classified_error(crate::error::ErrorKind::InvalidInput, "数组位置越界")
    })
}
impl<'w, const N: usize, T> crate::owned_entity::WeakEntitiesMut<'_, 'w, N, T> {
    pub(crate) fn weak_replace_from_inner(
        _value: &mut crate::owned_entity::WeakEntitiesMut<'_, 'w, N, T>,
        _index: usize,
        _target: &OwnedEntity<'w>,
    ) -> Result<(), crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            if array_stopped(_value.access) {
                return Err(crate::error::classified_error(
                    crate::error::ErrorKind::InvalidState,
                    "World 已停止",
                ));
            }
            array_get(&_value.value.entities, _index)?;
            if _value.world_identity != _target.world_identity {
                return Err(crate::error::classified_error(
                    crate::error::ErrorKind::InvalidInput,
                    "来源 World 不符",
                ));
            }
            let acquired = {
                let mut store = _value
                    .entities
                    .try_borrow_mut()
                    .unwrap_or_else(|_| invariant_failure(_value.access));
                if !matches!(store.counts(_target.entity), Some((strong, _)) if strong > 0) {
                    invariant_failure(_value.access);
                }
                store.acquire_weak(_target.entity)
            };
            if let Err(error) = acquired {
                if *error.current_context() == crate::error::ErrorKind::ResourceExhausted {
                    return Err(error);
                }
                invariant_failure(_value.access);
            }
            let old = std::mem::replace(&mut _value.value.entities[_index], _target.entity);
            if old != Entity::default() {
                let released = _value
                    .entities
                    .try_borrow_mut()
                    .unwrap_or_else(|_| invariant_failure(_value.access))
                    .release_weak(old);
                match released {
                    Ok(true) => {
                        _value.pending_destruction.reclamation.push(old);
                    }
                    Ok(false) => {}
                    Err(_) => invariant_failure(_value.access),
                }
            }
            array_mark(&mut _value.marker, &mut _value.marked, _value.access);
            Ok(())
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (_value, _index, _target);
            todo!("编号：T61，尚未实现， 非 RC 弱数组替换延期")
        }
    }
}

impl<'w, const N: usize, T> crate::owned_entity::StrongEntitiesRef<'_, 'w, N, T> {
    pub(crate) fn strong_ref_get(
        _v: &crate::owned_entity::StrongEntitiesRef<'_, '_, N, T>,
        _index: usize,
    ) -> Result<Entity, crate::error::EcsError> {
        array_get(&_v.value.entities, _index)
    }
}
impl<'w, const N: usize, T> crate::owned_entity::StrongEntitiesRef<'_, 'w, N, T> {
    pub(crate) fn strong_ref_get_owned(
        _v: &crate::owned_entity::StrongEntitiesRef<'_, 'w, N, T>,
        _index: usize,
    ) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
        let entity = array_get(&_v.value.entities, _index)?;
        // 临时借用并不拥有槽责任；ManuallyDrop 防止错误地交还原位置责任。
        let source = std::mem::ManuallyDrop::new(OwnedEntity {
            world_identity: _v.world_identity,
            entity,
            entities: _v.entities,
            pending_destruction: _v.pending_destruction,
            access: _v.access,
        });
        source.try_clone()
    }
}
impl<'w, const N: usize, T> crate::owned_entity::StrongEntitiesMut<'_, 'w, N, T> {
    pub(crate) fn strong_mut_get(
        _v: &crate::owned_entity::StrongEntitiesMut<'_, '_, N, T>,
        _index: usize,
    ) -> Result<Entity, crate::error::EcsError> {
        array_get(&_v.value.entities, _index)
    }
}
impl<'w, const N: usize, T> crate::owned_entity::StrongEntitiesMut<'_, 'w, N, T> {
    pub(crate) fn strong_mut_get_owned(
        _v: &crate::owned_entity::StrongEntitiesMut<'_, 'w, N, T>,
        _index: usize,
    ) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
        crate::owned_entity::StrongEntitiesRef {
            value: _v.value,
            world_identity: _v.world_identity,
            entities: _v.entities,
            pending_destruction: _v.pending_destruction,
            access: _v.access,
        }
        .get_owned(_index)
    }
}
impl<'w, const N: usize, T> crate::owned_entity::WeakEntitiesRef<'_, 'w, N, T> {
    pub(crate) fn weak_ref_get(
        _v: &crate::owned_entity::WeakEntitiesRef<'_, '_, N, T>,
        _index: usize,
    ) -> Result<Entity, crate::error::EcsError> {
        array_get(&_v.value.entities, _index)
    }
}
impl<'w, const N: usize, T> crate::owned_entity::WeakEntitiesRef<'_, 'w, N, T> {
    pub(crate) fn weak_ref_upgrade(
        _v: &crate::owned_entity::WeakEntitiesRef<'_, 'w, N, T>,
        _index: usize,
    ) -> Result<Option<OwnedEntity<'w>>, crate::error::EcsError> {
        let entity = array_get(&_v.value.entities, _index)?;
        #[cfg(feature = "rc")]
        {
            if array_stopped(_v.access) {
                return Err(crate::error::classified_error(
                    crate::error::ErrorKind::InvalidState,
                    "World 已停止",
                ));
            }
            if entity == Entity::default() {
                return Ok(None);
            }
            let acquired = {
                let mut store = _v
                    .entities
                    .try_borrow_mut()
                    .unwrap_or_else(|_| invariant_failure(_v.access));
                match store.counts(entity) {
                    Some((_, 0)) | None => invariant_failure(_v.access),
                    Some((0, _)) => return Ok(None),
                    Some(_) => store.acquire_strong(entity),
                }
            };
            if let Err(error) = acquired {
                if *error.current_context() == crate::error::ErrorKind::ResourceExhausted {
                    return Err(error);
                }
                invariant_failure(_v.access);
            }
            Ok(Some(OwnedEntity {
                world_identity: _v.world_identity,
                entity,
                entities: _v.entities,
                pending_destruction: _v.pending_destruction,
                access: _v.access,
            }))
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = entity;
            todo!("编号：T62，尚未实现， 非 RC 弱数组升级延期")
        }
    }
}
impl<'w, const N: usize, T> crate::owned_entity::WeakEntitiesMut<'_, 'w, N, T> {
    pub(crate) fn weak_mut_get(
        _v: &crate::owned_entity::WeakEntitiesMut<'_, '_, N, T>,
        _index: usize,
    ) -> Result<Entity, crate::error::EcsError> {
        array_get(&_v.value.entities, _index)
    }
}
impl<'w, const N: usize, T> crate::owned_entity::WeakEntitiesMut<'_, 'w, N, T> {
    pub(crate) fn weak_mut_upgrade(
        _v: &crate::owned_entity::WeakEntitiesMut<'_, 'w, N, T>,
        _index: usize,
    ) -> Result<Option<OwnedEntity<'w>>, crate::error::EcsError> {
        crate::owned_entity::WeakEntitiesRef {
            value: _v.value,
            world_identity: _v.world_identity,
            entities: _v.entities,
            pending_destruction: _v.pending_destruction,
            access: _v.access,
        }
        .upgrade(_index)
    }
}
use crate::{
    entity::Entity,
    entity_container::EntityInsertError,
    owned_entity::{OwnedEntity, StrongEntitiesMut},
};
impl<'w> OwnedEntity<'w> {
    pub(crate) fn entity_inner(value: &OwnedEntity<'w>) -> Entity {
        value.entity
    }
}
impl<'w> OwnedEntity<'w> {
    pub(crate) fn forget_inner(value: OwnedEntity<'w>) -> Entity {
        let entity = value.entity;
        std::mem::forget(value);
        entity
    }
}
impl<'w> OwnedEntity<'w> {
    pub(crate) fn drop_inner(value: &mut OwnedEntity<'w>) {
        #[cfg(feature = "rc")]
        {
            let released = match value.entities.try_borrow_mut() {
                Ok(mut entities) => entities.release_strong(value.entity),
                Err(_) => invariant_failure(value.access),
            };
            // RefMut 已退出；仅归零转换负责登记，不在此物理销毁。
            match released {
                Ok(true) => value.pending_destruction.enqueue(value.entity),
                Ok(false) => {}
                Err(_) => invariant_failure(value.access),
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = value;
            todo!("编号：T63，尚未实现， 非 RC OwnedEntity 释放后端延期")
        }
    }
}
impl<'w> OwnedEntity<'w> {
    pub(crate) fn try_clone_inner(
        value: &OwnedEntity<'w>,
    ) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            use crate::error::{classified_error, ErrorKind};
            // Cell::replace 只瞬时替换，不调用业务代码或借用身份表。
            let mode = value.access.mode.replace(crate::world::AccessMode::Idle);
            let stopped = matches!(mode, crate::world::AccessMode::Stopped);
            value.access.mode.set(mode);
            if stopped {
                return Err(classified_error(ErrorKind::InvalidState, "World 已停止"));
            }
            let acquired = match value.entities.try_borrow_mut() {
                Ok(mut entities) => entities.acquire_strong(value.entity),
                Err(_) => invariant_failure(value.access),
            };
            if let Err(error) = acquired {
                if *error.current_context() == ErrorKind::ResourceExhausted {
                    return Err(error);
                }
                invariant_failure(value.access);
            }
            Ok(OwnedEntity {
                world_identity: value.world_identity,
                entity: value.entity,
                entities: value.entities,
                pending_destruction: value.pending_destruction,
                access: value.access,
            })
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = value;
            todo!("编号：T64，尚未实现， 非 RC OwnedEntity 增持后端延期")
        }
    }
}
#[cfg(feature = "rc")]
fn array_stopped(access: &crate::world::AccessState) -> bool {
    let mode = access.mode.replace(crate::world::AccessMode::Idle);
    let stopped = matches!(mode, crate::world::AccessMode::Stopped);
    access.mode.set(mode);
    stopped
}
#[cfg(feature = "rc")]
fn array_mark(
    marker: &mut crate::change_detection::ChangeMarker<'_>,
    marked: &mut bool,
    access: &crate::world::AccessState,
) {
    struct Stop<'a>(&'a crate::world::AccessState);
    impl Drop for Stop<'_> {
        fn drop(&mut self) {
            if std::thread::panicking() {
                self.0.mode.set(crate::world::AccessMode::Stopped);
            }
        }
    }
    let _guard = Stop(access);
    marker.set_changed(marked);
}
#[cfg(feature = "rc")]
fn invariant_failure(access: &crate::world::AccessState) -> ! {
    access.mode.set(crate::world::AccessMode::Stopped);
    #[cfg(test)]
    if std::env::var_os("PI_ECS_ARRAY_ABORT_CASE").is_some() && array_stopped(access) {
        eprintln!("owned-array-stopped-before-abort");
    }
    std::process::abort()
}

#[cfg(all(test, feature = "rc"))]
mod owned_entity_batch {
    use super::*;
    use crate::{
        destruction::PendingDestruction,
        entity::{AllocationDomain, EntityLocation},
        entity_store::{EntityIdentityStore, EntityStore},
        world::{AccessMode, AccessState},
    };
    use std::cell::{Cell, RefCell};

    // 字段级合法夹具：分配产生初始 strong=1，该责任直接交给 OwnedEntity。
    // 无组件/节点/业务值；所有字段正常析构，不进入尚未实现的 World::Drop。
    struct Fields {
        entities: EntityIdentityStore,
        pending: PendingDestruction,
        access: AccessState,
    }
    impl Fields {
        fn new() -> Self {
            Self {
                entities: RefCell::new(EntityStore::new(1).unwrap()),
                pending: PendingDestruction {
                    pending: Default::default(),
                    reclamation: Default::default(),
                },
                access: AccessState {
                    mode: Cell::new(AccessMode::Idle),
                },
            }
        }
        fn allocate(&self) -> OwnedEntity<'_> {
            let entity = self
                .entities
                .borrow_mut()
                .allocate(
                    AllocationDomain(0),
                    EntityLocation::from_parts(0, 0, 0, 0, 0),
                )
                .unwrap();
            OwnedEntity {
                world_identity: 17,
                entity,
                entities: &self.entities,
                pending_destruction: &self.pending,
                access: &self.access,
            }
        }
    }

    #[test]
    fn owned_array_batch_component_marker_owns_notice_and_deduplicates() {
        use crate::{change_detection::*, notification::*, owned_entity::*};
        use std::marker::PhantomData;
        let fields = Fields::new();
        let target = fields.allocate();
        let owner = fields.allocate();
        let component = crate::component::ComponentId(0);
        let mut subscriptions = pi_slotmap::SlotMap::with_key();
        subscriptions.insert(SubscriptionState {
            stream: (component, NoticeKind::Changed),
            next: Cell::new(0),
        });
        let notices = Notifications {
            subscriptions,
            components: vec![ComponentNoticeStreams {
                added: None,
                removed: None,
                changed: Some(RefCell::new(NoticeStream {
                    headers: Default::default(),
                    range: NoticeStreamRange {
                        first: NoticeSequence(0),
                        end: NoticeSequence(0),
                    },
                })),
            }],
            layouts: vec![],
            type_ids: vec![],
        };
        let demand = TrackingDemand {
            added_tick: true,
            changed_tick: true,
            changed_notice: true,
            ..Default::default()
        };
        let original = ChangeTick::new(std::num::NonZeroU64::new(2).unwrap());
        let tick = ChangeTick::new(std::num::NonZeroU64::new(8).unwrap());
        let mut added = Some(original);
        let mut changed = Some(original);
        let mut strong = StrongEntities::<1> {
            entities: [target.try_clone().unwrap().forget()],
            marker: PhantomData,
        };
        let marker = ChangeMarker {
            tick,
            target: ChangeTarget::Component {
                ticks: pi_ecs_types::change_detection::ComponentTicksMut {
                    added: Some(&mut added),
                    changed: Some(&mut changed),
                },
                demand: &demand,
                notice: Some(NoticeWrite {
                    notifications: &notices,
                    access: &fields.access,
                    component,
                    entity: owner.entity(),
                    borrow: PhantomData,
                }),
            },
        };
        let mut view = StrongEntitiesMut {
            value: &mut strong,
            world_identity: 17,
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
            marker,
            marked: false,
        };
        let input = target.try_clone().unwrap();
        let rejected = view.replace(3, input).err().unwrap();
        assert!(notices.components[0]
            .changed
            .as_ref()
            .unwrap()
            .borrow()
            .headers
            .is_empty());
        let old = view.replace(0, rejected.value).ok().unwrap();
        let old = view.replace(0, old).ok().unwrap();
        drop(old);
        drop(view);
        assert_eq!(added, Some(original));
        assert_eq!(changed, Some(tick));
        let mut weak = WeakEntities::<1> {
            entities: [Entity::default()],
            marker: PhantomData,
        };
        let demand = TrackingDemand {
            changed_notice: true,
            ..Default::default()
        };
        let marker = ChangeMarker {
            tick,
            target: ChangeTarget::Component {
                ticks: pi_ecs_types::change_detection::ComponentTicksMut {
                    added: None,
                    changed: None,
                },
                demand: &demand,
                notice: Some(NoticeWrite {
                    notifications: &notices,
                    access: &fields.access,
                    component,
                    entity: owner.entity(),
                    borrow: PhantomData,
                }),
            },
        };
        let mut view = WeakEntitiesMut {
            value: &mut weak,
            world_identity: 17,
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
            marker,
            marked: false,
        };
        view.replace_from(0, &target).unwrap();
        view.replace_from(0, &target).unwrap();
        drop(view);
        let stream = notices.components[0].changed.as_ref().unwrap().borrow();
        assert_eq!(stream.headers.len(), 2);
        assert_eq!(stream.range.end.0, 2);
        for header in stream.headers.iter() {
            assert_eq!(header.entity, owner.entity());
            assert_eq!(header.tick, tick);
        }
        drop(stream);
        assert!(!fields
            .entities
            .borrow_mut()
            .release_weak(weak.entities[0])
            .unwrap());
        drop(OwnedEntity {
            world_identity: 17,
            entity: strong.entities[0],
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
        });
        drop((target, owner));
    }

    #[test]
    fn owned_array_batch_invariant_abort_child() {
        let Ok(case) = std::env::var("PI_ECS_ARRAY_ABORT_CASE") else {
            return;
        };
        std::panic::set_hook(Box::new(|_| std::process::exit(91)));
        let fields = Fields::new();
        let owner = fields.allocate();
        let entity = owner.entity();
        let mut head = crate::owned_entity::StrongEntities::<1> {
            entities: [owner.forget()],
            marker: std::marker::PhantomData,
        };
        if case.ends_with("zero") {
            fields.entities.borrow_mut().release_strong(entity).unwrap();
        }
        if case.ends_with("invalid") {
            use crate::entity::{Key, KeyData};
            head.entities[0] = Entity::from(KeyData::new(
                entity.data().index(),
                entity.data().version() + 2,
            ));
        }
        let tick = crate::change_detection::ChangeTick::new(std::num::NonZeroU64::new(2).unwrap());
        let mut ticks = crate::resource::ResourceTicks {
            added: tick,
            changed: tick,
        };
        let mut view = StrongEntitiesMut {
            value: &mut head,
            world_identity: 17,
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
            marker: crate::change_detection::ChangeMarker {
                tick,
                target: crate::change_detection::ChangeTarget::Resource(&mut ticks),
            },
            marked: false,
        };
        let input = fields.allocate();
        let borrow = if case.ends_with("borrow") {
            Some(fields.entities.borrow_mut())
        } else {
            None
        };
        eprintln!("owned-array-ready:{case}");
        if case.starts_with("replace") {
            let _result = std::mem::ManuallyDrop::new(view.replace(0, input));
            std::process::exit(92);
        } else if case.starts_with("weak") {
            let weak = crate::owned_entity::WeakEntities::<1> {
                entities: [entity],
                marker: std::marker::PhantomData,
            };
            let read = crate::owned_entity::WeakEntitiesRef {
                value: &weak,
                world_identity: 17,
                entities: &fields.entities,
                pending_destruction: &fields.pending,
                access: &fields.access,
            };
            let _result = std::mem::ManuallyDrop::new(read.upgrade(0));
            std::process::exit(92);
        } else {
            let _result = std::mem::ManuallyDrop::new(view.get_owned(0));
            let _ = borrow;
            std::process::exit(92);
        }
    }

    #[test]
    fn owned_array_batch_corrupt_responsibility_stops_then_aborts() {
        let control = abort_child("control-abort").status;
        for case in [
            "read-zero",
            "read-invalid",
            "read-borrow",
            "replace-zero",
            "replace-invalid",
            "replace-borrow",
            "weak-missing",
            "weak-borrow",
        ] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact","owned_entity::owned_entity_inner::owned_entity_batch::owned_array_batch_invariant_abort_child","--nocapture"])
                .env("PI_ECS_ARRAY_ABORT_CASE",case).output().unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(&format!("owned-array-ready:{case}")),
                "{case}: {output:?}"
            );
            assert!(
                is_target_abort(output.status, control),
                "{case}: {output:?}"
            );
            assert!(
                stderr.contains("owned-array-stopped-before-abort"),
                "{case}: {output:?}"
            );
        }
        // 平台目标状态的反例由现有同模块用例完整验证，不能接受普通失败或 panic。
    }

    #[test]
    fn owned_array_batch_rejections_preserve_slots_counts_and_marker() {
        use crate::{change_detection::*, owned_entity::*};
        let fields = Fields::new();
        let owner = fields.allocate();
        let entity = owner.entity();
        let foreign_fields = Fields::new();
        let mut foreign = foreign_fields.allocate();
        foreign.world_identity = 99;
        let mut head = WeakEntities::<1> {
            entities: [Entity::default()],
            marker: std::marker::PhantomData,
        };
        let tick = ChangeTick::new(std::num::NonZeroU64::new(2).unwrap());
        let later = ChangeTick::new(std::num::NonZeroU64::new(8).unwrap());
        let mut ticks = crate::resource::ResourceTicks {
            added: tick,
            changed: tick,
        };
        let mut view = WeakEntitiesMut {
            value: &mut head,
            world_identity: 17,
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
            marker: ChangeMarker {
                tick: later,
                target: ChangeTarget::Resource(&mut ticks),
            },
            marked: false,
        };
        assert!(view.replace_from(0, &foreign).is_err());
        fields.entities.borrow_mut().slots[0]
            .get_mut(entity)
            .unwrap()
            .weak = u32::MAX;
        let error = view.replace_from(0, &owner).unwrap_err();
        assert_eq!(
            *error.current_context(),
            crate::error::ErrorKind::ResourceExhausted
        );
        assert_eq!(view.get(0).unwrap(), Entity::default());
        assert_eq!(fields.entities.borrow().counts(entity), Some((1, u32::MAX)));
        assert!(!view.marked);
        fields.entities.borrow_mut().slots[0]
            .get_mut(entity)
            .unwrap()
            .weak = 0;
        fields.access.mode.set(AccessMode::Stopped);
        let borrow = fields.entities.borrow_mut();
        assert!(view.replace_from(0, &owner).is_err());
        assert!(view.upgrade(0).is_err());
        assert!(!view.marked);
        drop(borrow);
        drop(view);
        assert_eq!(ticks.changed, tick);
        assert_eq!(ticks.added, tick);
        fields.access.mode.set(AccessMode::Idle);
        let mut strong = StrongEntities::<1> {
            entities: [owner.forget()],
            marker: std::marker::PhantomData,
        };
        let mut view = StrongEntitiesMut {
            value: &mut strong,
            world_identity: 17,
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
            marker: ChangeMarker {
                tick: later,
                target: ChangeTarget::Resource(&mut ticks),
            },
            marked: false,
        };
        let rejected = view.replace(0, foreign).err().unwrap();
        assert_eq!(rejected.value.world_identity, 99);
        assert!(!view.marked);
        drop(rejected);
        fields.entities.borrow_mut().slots[0]
            .get_mut(entity)
            .unwrap()
            .strong = u32::MAX;
        let error = view.get_owned(0).err().unwrap();
        assert_eq!(
            *error.current_context(),
            crate::error::ErrorKind::ResourceExhausted
        );
        assert_eq!(fields.entities.borrow().counts(entity), Some((u32::MAX, 0)));
        fields.entities.borrow_mut().slots[0]
            .get_mut(entity)
            .unwrap()
            .strong = 1;
        fields.access.mode.set(AccessMode::Stopped);
        let input = foreign_fields.allocate();
        let borrow = fields.entities.borrow_mut();
        assert!(view.get_owned(0).is_err());
        let rejected = view.replace(0, input).err().unwrap();
        drop(borrow);
        drop(rejected);
        assert!(!view.marked);
        drop(view);
        assert_eq!(ticks.changed, tick);
        drop(OwnedEntity {
            world_identity: 17,
            entity,
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
        });
        assert_eq!(fields.entities.borrow().counts(entity), Some((0, 0)));
    }

    #[test]
    fn owned_array_batch_weak_replace_prepares_first_and_dead_last_weak_is_deferred() {
        use crate::entity::Key;
        use crate::{change_detection::*, owned_entity::*};
        let fields = Fields::new();
        let first = fields.allocate();
        let entity = first.entity();
        let mut head = WeakEntities::<2> {
            entities: [Entity::default(); 2],
            marker: std::marker::PhantomData,
        };
        let tick = ChangeTick::new(std::num::NonZeroU64::new(2).unwrap());
        let mut ticks = crate::resource::ResourceTicks {
            added: tick,
            changed: tick,
        };
        let mut view = WeakEntitiesMut {
            value: &mut head,
            world_identity: 17,
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
            marker: ChangeMarker {
                tick,
                target: ChangeTarget::Resource(&mut ticks),
            },
            marked: false,
        };
        assert!(view.upgrade(0).unwrap().is_none());
        assert!(view.replace_from(2, &first).is_err());
        assert!(!view.marked);
        view.replace_from(0, &first).unwrap();
        view.replace_from(1, &first).unwrap();
        view.replace_from(0, &first).unwrap();
        assert_eq!(view.get(0).unwrap(), entity);
        let extra = view.upgrade(0).unwrap().unwrap();
        assert_eq!(fields.entities.borrow().counts(entity), Some((2, 2)));
        drop((first, extra));
        assert!(view.upgrade(0).unwrap().is_none());
        fields.entities.borrow_mut().begin_destroy(entity).unwrap();
        assert!(!fields.entities.borrow_mut().finish_destroy(entity).unwrap());
        let next = fields.allocate();
        view.replace_from(0, &next).unwrap();
        assert!(fields.pending.reclamation.is_empty());
        view.replace_from(1, &next).unwrap();
        assert_eq!(fields.entities.borrow().counts(entity), Some((0, 0)));
        assert_eq!(fields.pending.reclamation.get(0), Some(&entity));
        assert!(fields.entities.borrow_mut().remove(entity));
        drop(view);
        let shared = WeakEntitiesRef {
            value: &head,
            world_identity: 17,
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
        };
        assert_eq!(shared.get(1).unwrap(), next.entity());
        drop(shared.upgrade(1).unwrap());
        for entity in head.entities {
            assert!(!fields.entities.borrow_mut().release_weak(entity).unwrap());
        }
        drop(next);
    }

    #[test]
    fn owned_array_batch_strong_replace_transfers_without_zero_and_rejects_unchanged() {
        use crate::{change_detection::*, owned_entity::*};
        let fields = Fields::new();
        let owner = fields.allocate();
        let entity = owner.entity();
        let incoming = owner.try_clone().unwrap();
        let mut head = StrongEntities::<1> {
            entities: [owner.forget()],
            marker: std::marker::PhantomData,
        };
        let original = ChangeTick::new(std::num::NonZeroU64::new(2).unwrap());
        let changed = ChangeTick::new(std::num::NonZeroU64::new(7).unwrap());
        let mut ticks = crate::resource::ResourceTicks {
            added: original,
            changed: original,
        };
        let mut view = StrongEntitiesMut {
            value: &mut head,
            world_identity: 17,
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
            marker: ChangeMarker {
                tick: changed,
                target: ChangeTarget::Resource(&mut ticks),
            },
            marked: false,
        };
        let rejected = view.replace(1, incoming).err().unwrap();
        assert!(!view.marked);
        assert_eq!(rejected.value.entity(), entity);
        let returned = view.replace(0, rejected.value).ok().unwrap();
        assert_eq!(returned.entity(), entity);
        assert_eq!(view.get(0).unwrap(), entity);
        let extra = view.get_owned(0).unwrap();
        assert_eq!(fields.entities.borrow().counts(entity), Some((3, 0)));
        assert!(view.marked);
        assert!(fields.pending.pending.is_empty());
        drop(view);
        assert_eq!(ticks.added, original);
        assert_eq!(ticks.changed, changed);
        drop((returned, extra));
        assert_eq!(fields.entities.borrow().counts(entity), Some((1, 0)));
        drop(OwnedEntity {
            world_identity: 17,
            entity: head.entities[0],
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
        });
        assert_eq!(fields.pending.pending.len(), 1);
    }

    // Seam: 数组公开 get/get_owned；拥有结果不依赖短期数组视图。
    #[test]
    fn owned_array_batch_strong_read_keeps_source_alive() {
        use crate::owned_entity::{StrongEntities, StrongEntitiesRef};
        let fields = Fields::new();
        let owner = fields.allocate();
        let entity = owner.entity();
        let head = StrongEntities::<2, String> {
            entities: [owner.forget(), fields.allocate().forget()],
            marker: std::marker::PhantomData,
        };
        let acquired = {
            let view = StrongEntitiesRef {
                value: &head,
                world_identity: 17,
                entities: &fields.entities,
                pending_destruction: &fields.pending,
                access: &fields.access,
            };
            assert_eq!(view.get(0).unwrap(), entity);
            assert!(view.get(2).is_err());
            assert!(view.get_owned(2).is_err());
            view.get_owned(0).unwrap()
        };
        assert_eq!(fields.entities.borrow().counts(entity), Some((2, 0)));
        for entity in head.entities {
            drop(OwnedEntity {
                world_identity: 17,
                entity,
                entities: &fields.entities,
                pending_destruction: &fields.pending,
                access: &fields.access,
            });
        }
        assert_eq!(fields.entities.borrow().counts(entity), Some((1, 0)));
        drop(acquired);
        assert_eq!(fields.entities.borrow().counts(entity), Some((0, 0)));
        assert_eq!(fields.pending.pending.len(), 2);
    }

    #[test]
    fn owned_entity_batch_stopped_clone_rejects_before_identity_borrow() {
        let fields = Fields::new();
        let owned = fields.allocate();
        let entity = owned.entity();
        fields.access.mode.set(AccessMode::Stopped);
        let borrow = fields.entities.borrow_mut();
        let error = owned.try_clone().err().expect("停止态须拒绝");
        assert_eq!(
            *error.current_context(),
            crate::error::ErrorKind::InvalidState
        );
        assert_eq!(borrow.counts(entity), Some((1, 0)));
        drop(borrow);
        drop(owned);
        assert_eq!(fields.pending.pending.len(), 1);
    }

    #[test]
    fn owned_entity_batch_exhausted_clone_preserves_counts_and_responsibility() {
        let fields = Fields::new();
        let owned = fields.allocate();
        let entity = owned.entity();
        // 边界注入，避免执行 u32::MAX 次增持；测试结束还原注入的计数。
        fields.entities.borrow_mut().slots[0]
            .get_mut(entity)
            .unwrap()
            .strong = u32::MAX;
        let error = owned.try_clone().err().expect("满额须拒绝");
        assert_eq!(
            *error.current_context(),
            crate::error::ErrorKind::ResourceExhausted
        );
        assert_eq!(fields.entities.borrow().counts(entity), Some((u32::MAX, 0)));
        assert!(fields.pending.pending.is_empty());
        assert!(matches!(
            fields.access.mode.replace(AccessMode::Idle),
            AccessMode::Idle
        ));
        fields.entities.borrow_mut().slots[0]
            .get_mut(entity)
            .unwrap()
            .strong = 1;
        drop(owned);
        assert_eq!(fields.pending.pending.len(), 1);
    }

    #[test]
    fn owned_entity_batch_forget_transfers_without_release() {
        let fields = Fields::new();
        let owned = fields.allocate();
        let entity = owned.entity();
        assert_eq!(owned.forget(), entity);
        assert_eq!(fields.entities.borrow().counts(entity), Some((1, 0)));
        assert!(fields.pending.pending.is_empty());
        // 接收 forget 交出的唯一责任，构造同域拥有值清偿；不是新增责任。
        let transferred = OwnedEntity {
            world_identity: 17,
            entity,
            entities: &fields.entities,
            pending_destruction: &fields.pending,
            access: &fields.access,
        };
        drop(transferred);
        assert_eq!(fields.entities.borrow().counts(entity), Some((0, 0)));
        assert_eq!(fields.pending.pending.len(), 1);
    }

    #[test]
    fn owned_entity_batch_invariant_abort_child() {
        let Ok(case) = std::env::var("PI_ECS_OWNED_ENTITY_ABORT_CASE") else {
            return;
        };
        // panic 不是 abort 契约：单独退出码令父进程能识别错误实现。
        std::panic::set_hook(Box::new(|_| std::process::exit(91)));
        match case.as_str() {
            "control-abort" => {
                eprintln!("owned-entity-control-ready");
                std::process::abort();
            }
            "wrong-exit" | "test-failure" | "access-violation" => {
                eprintln!("owned-entity-abort-ready:{case}");
                std::process::exit(match case.as_str() {
                    "wrong-exit" => 93,
                    "test-failure" => 101,
                    _ => 0xc0000005_u32 as i32,
                });
            }
            "panic" => {
                eprintln!("owned-entity-abort-ready:{case}");
                panic!("普通 panic 不可代替 abort");
            }
            _ => {}
        }
        let fields = Fields::new();
        let mut owned = fields.allocate();
        if case.ends_with("zero") {
            assert!(fields
                .entities
                .borrow_mut()
                .release_strong(owned.entity())
                .unwrap());
        } else if case.ends_with("invalid") {
            use crate::entity::{Key, KeyData};
            let data = owned.entity.data();
            owned.entity = Entity::from(KeyData::new(data.index(), data.version() + 2));
        }
        eprintln!("owned-entity-abort-ready:{case}");
        if case == "clone-return" {
            clone_or_exit(&owned);
        }
        if case.ends_with("borrow") {
            let _borrow = fields.entities.borrow_mut();
            if case.starts_with("clone") {
                clone_or_exit(&owned);
            } else {
                drop(owned);
            }
        } else if case.starts_with("clone") {
            clone_or_exit(&owned);
        } else {
            drop(owned);
        }
        std::process::exit(94);
    }

    fn clone_or_exit(owned: &OwnedEntity<'_>) -> ! {
        // 即便错误实现返回 Ok，也不允许结果在身份表仍被借用时 Drop，
        // 否则 Drop 的 abort 会掩盖 clone 未 abort。Err 同样立即失败退出。
        let _result = std::mem::ManuallyDrop::new(owned.try_clone());
        std::process::exit(92);
    }

    fn abort_child(case: &str) -> std::process::Output {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "owned_entity::owned_entity_inner::owned_entity_batch::owned_entity_batch_invariant_abort_child",
                "--nocapture",
            ])
            .env("PI_ECS_OWNED_ENTITY_ABORT_CASE", case)
            .output()
            .unwrap();
        let marker = if case == "control-abort" {
            "owned-entity-control-ready".to_owned()
        } else {
            format!("owned-entity-abort-ready:{case}")
        };
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(&marker),
            "{case}: 子进程未到达目标路径: {output:?}"
        );
        output
    }

    fn is_target_abort(
        status: std::process::ExitStatus,
        control: std::process::ExitStatus,
    ) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            let _ = control;
            status.signal() == Some(6) // SIGABRT，不接受其他终止信号。
        }
        #[cfg(windows)]
        {
            // Rust/CRT 的 abort 状态以当前工具链独立控制进程为准。
            !status.success() && status == control
        }
    }

    #[test]
    fn owned_entity_batch_invalid_responsibilities_abort_without_unwinding() {
        let control = abort_child("control-abort").status;
        assert!(!control.success(), "控制进程必须 abort");
        assert!(is_target_abort(control, control));
        for (case, code) in [
            ("wrong-exit", 93),
            ("panic", 91),
            ("test-failure", 101),
            ("clone-return", 92),
            ("access-violation", 0xc0000005_u32 as i32),
        ] {
            let output = abort_child(case);
            #[cfg(unix)]
            let code = code & 0xff;
            assert_eq!(output.status.code(), Some(code), "{case}");
            assert!(
                !is_target_abort(output.status, control),
                "{case}: 非目标退出不可代替 abort"
            );
        }
        for case in [
            "clone-zero",
            "clone-invalid",
            "clone-borrow",
            "drop-zero",
            "drop-invalid",
            "drop-borrow",
        ] {
            let output = abort_child(case);
            assert!(
                is_target_abort(output.status, control),
                "{case}: 目标必须 abort，实际 {:?}，控制 {control:?}",
                output.status
            );
        }
    }

    #[test]
    fn owned_entity_batch_clone_preserves_source_and_each_drop_releases_one() {
        let fields = Fields::new();
        let owned = fields.allocate();
        let entity = owned.entity();
        let cloned = owned.try_clone().unwrap();
        assert_eq!(cloned.entity(), entity);
        assert_eq!(cloned.world_identity, owned.world_identity);
        assert!(std::ptr::eq(cloned.entities, owned.entities));
        assert!(std::ptr::eq(
            cloned.pending_destruction,
            owned.pending_destruction
        ));
        assert!(std::ptr::eq(cloned.access, owned.access));
        assert_eq!(fields.entities.borrow().counts(entity), Some((2, 0)));
        drop(owned);
        assert_eq!(fields.entities.borrow().counts(entity), Some((1, 0)));
        assert!(fields.pending.pending.is_empty());
        drop(cloned);
        assert_eq!(fields.entities.borrow().counts(entity), Some((0, 0)));
        assert_eq!(fields.pending.pending.len(), 1);
    }

    #[test]
    fn owned_entity_batch_drop_registers_only_final_release_even_when_stopped() {
        let fields = Fields::new();
        let owned = fields.allocate();
        let entity = owned.entity();
        fields.access.mode.set(AccessMode::Stopped);
        drop(owned);
        assert_eq!(fields.entities.borrow().counts(entity), Some((0, 0)));
        assert_eq!(fields.pending.pending.len(), 1);
        assert_eq!(fields.pending.pending.get(0), Some(&entity));
        assert!(fields.pending.reclamation.is_empty());
        assert!(fields.entities.borrow().locate(entity).is_some());
        assert!(fields.entities.try_borrow_mut().is_ok());
    }
}

impl<'w, const N: usize, T> crate::owned_entity::StrongEntitiesMut<'_, 'w, N, T> {
    pub(crate) fn replace_inner(
        _target: &mut StrongEntitiesMut<'_, 'w, N, T>,
        _index: usize,
        _value: OwnedEntity<'w>,
    ) -> Result<OwnedEntity<'w>, EntityInsertError<'w>> {
        #[cfg(feature = "rc")]
        {
            if array_stopped(_target.access)
                || _index >= N
                || _value.world_identity != _target.world_identity
            {
                return Err(EntityInsertError {
                    error: crate::error::classified_error(
                        crate::error::ErrorKind::InvalidInput,
                        "数组替换拒绝：停止、越界或来源不符",
                    ),
                    value: _value,
                });
            }
            {
                let store = _target
                    .entities
                    .try_borrow()
                    .unwrap_or_else(|_| invariant_failure(_target.access));
                for entity in [_target.value.entities[_index], _value.entity] {
                    if !matches!(store.counts(entity), Some((strong, _)) if strong > 0)
                        || store.locate(entity).is_none()
                    {
                        invariant_failure(_target.access);
                    }
                }
            }
            let mut old = std::mem::ManuallyDrop::new(_value);
            // 无分配/回调窗口：输入责任归槽，旧责任归返回值，不做计数减增。
            std::mem::swap(&mut _target.value.entities[_index], &mut old.entity);
            let old = std::mem::ManuallyDrop::into_inner(old);
            array_mark(&mut _target.marker, &mut _target.marked, _target.access);
            Ok(old)
        }
        #[cfg(not(feature = "rc"))]
        {
            let _ = (_target, _index, _value);
            todo!("编号：T65，尚未实现， 非 RC 数组替换延期")
        }
    }
}
