//! 固定身份映射配对芯；出生分区键与本地槽表在此转换。
#[cfg(feature = "rc")]
use crate::entity_store::EntityRecord;
use crate::{
    entity::{
        domain_bits, slot_index, AllocationDomain, Entity, EntityLocation, LOCAL_SLOT_MASK,
        MAX_PARTITIONS,
    },
    entity_store::EntityStore,
    error::EcsError,
};
use pi_key::Key;

const STRONG_SHIFT: u32 = 32;

#[cfg(all(test, feature = "rc"))]
mod rc_identity_tests {
    use super::*;

    fn location(partition: u32) -> EntityLocation {
        EntityLocation::from_parts(
            partition,
            7,
            11,
            crate::entity::TableId(2),
            crate::entity::VariantId(3),
        )
    }

    #[test]
    fn checked_count_exhaustion_preserves_identity_and_both_counts() {
        use pi_ecs_types::error::ErrorKind;
        let mut store = EntityStore::new(1).unwrap();
        let entity = store.allocate(AllocationDomain(0), location(0)).unwrap();
        // Boundary fixture only: exercising u32::MAX acquisitions would obscure this contract.
        let record = store.slots[0].get_mut(local_key(entity)).unwrap();
        record.strong = u32::MAX;
        record.weak = u32::MAX;
        assert_eq!(
            *store.acquire_strong(entity).unwrap_err().current_context(),
            ErrorKind::ResourceExhausted
        );
        assert_eq!(
            *store.acquire_weak(entity).unwrap_err().current_context(),
            ErrorKind::ResourceExhausted
        );
        assert_eq!(store.counts(entity), Some((u32::MAX, u32::MAX)));
        assert_eq!(store.locate(entity), Some(location(0)));
        assert!(!store.release_strong(entity).unwrap());
        assert!(!store.release_weak(entity).unwrap());
        assert_eq!(store.counts(entity), Some((u32::MAX - 1, u32::MAX - 1)));
    }

    #[test]
    fn invalid_keys_never_mutate_live_records() {
        use pi_ecs_types::error::ErrorKind;
        let mut store = EntityStore::new(1).unwrap();
        let entity = store.allocate(AllocationDomain(0), location(0)).unwrap();
        let wrong_version = Entity::from(pi_key::KeyData::new(
            entity.data().index(),
            entity.data().version() + 2,
        ));
        let wrong_birth = external(1, local_key(entity));
        for (invalid, kind) in [
            (wrong_version, ErrorKind::NotFound),
            (wrong_birth, ErrorKind::InvalidInput),
        ] {
            assert_eq!(store.locate(invalid), None);
            assert_eq!(store.counts(invalid), None);
            assert_eq!(
                *store.acquire_strong(invalid).unwrap_err().current_context(),
                kind
            );
            assert_eq!(
                *store.acquire_weak(invalid).unwrap_err().current_context(),
                kind
            );
            assert_eq!(
                *store.release_strong(invalid).unwrap_err().current_context(),
                kind
            );
            assert_eq!(
                *store.release_weak(invalid).unwrap_err().current_context(),
                kind
            );
            assert_eq!(
                *store
                    .relocate(invalid, location(0))
                    .unwrap_err()
                    .current_context(),
                kind
            );
            assert_eq!(
                *store.begin_destroy(invalid).unwrap_err().current_context(),
                kind
            );
            assert_eq!(
                *store.finish_destroy(invalid).unwrap_err().current_context(),
                kind
            );
            assert!(!store.remove(invalid));
            assert_eq!(store.counts(entity), Some((1, 0)));
            assert_eq!(store.locate(entity), Some(location(0)));
        }
        assert_eq!(
            *store.release_weak(entity).unwrap_err().current_context(),
            ErrorKind::InvalidState
        );
        store.release_strong(entity).unwrap();
        assert_eq!(
            *store.release_strong(entity).unwrap_err().current_context(),
            ErrorKind::InvalidState
        );
        assert_eq!(
            *store.acquire_strong(entity).unwrap_err().current_context(),
            ErrorKind::InvalidState
        );
        assert_eq!(store.counts(entity), Some((0, 0)));
        assert_eq!(store.locate(entity), Some(location(0)));
    }

    #[test]
    fn explicit_reclaim_order_is_partition_local_lifo_without_sorting() {
        let mut store = EntityStore::new(2).unwrap();
        let first = store.allocate(AllocationDomain(0), location(0)).unwrap();
        let second = store.allocate(AllocationDomain(0), location(0)).unwrap();
        let third = store.allocate(AllocationDomain(0), location(0)).unwrap();
        let other = store.allocate(AllocationDomain(1), location(1)).unwrap();
        // Deliberately non-key order; the identity seam recycles precisely the supplied slot.
        for entity in [second, other, third, first] {
            store.release_strong(entity).unwrap();
            store.begin_destroy(entity).unwrap();
            assert!(store.finish_destroy(entity).unwrap());
            assert!(store.remove(entity));
        }
        let reused_other = store.allocate(AllocationDomain(1), location(1)).unwrap();
        assert_eq!(slot_index(&reused_other), slot_index(&other));
        for old in [first, third, second] {
            let new = store.allocate(AllocationDomain(0), location(0)).unwrap();
            assert_eq!(slot_index(&new), slot_index(&old));
            assert_ne!(new, old);
            assert_eq!(store.locate(old), None);
            assert_eq!(store.counts(new), Some((1, 0)));
        }
        // Ordinary reuse advances the generation; truncation/wraparound are dependency limits.
        assert_eq!(store.locate(other), None);
    }

    #[test]
    fn destruction_hides_once_retains_weak_and_reclaims_only_at_boundary() {
        let mut store = EntityStore::new(1).unwrap();
        let entity = store.allocate(AllocationDomain(0), location(0)).unwrap();
        assert!(store.begin_destroy(entity).is_err());
        assert!(store.finish_destroy(entity).is_err());
        assert!(!store.remove(entity));
        store.acquire_weak(entity).unwrap();
        store.release_strong(entity).unwrap();
        assert!(store.finish_destroy(entity).is_err());
        assert!(!store.remove(entity));
        assert_eq!(store.begin_destroy(entity).unwrap(), location(0));
        assert_eq!(store.locate(entity), None);
        assert!(store.begin_destroy(entity).is_err());
        assert!(!store.finish_destroy(entity).unwrap());
        assert!(!store.remove(entity));
        assert_eq!(store.counts(entity), Some((0, 1)));
        assert!(store.release_weak(entity).unwrap());
        assert_eq!(store.counts(entity), Some((0, 0)));
        assert!(store.remove(entity));
        assert!(!store.remove(entity));
        assert_eq!(store.counts(entity), None);
        assert!(store.begin_destroy(entity).is_err());
        assert!(store.finish_destroy(entity).is_err());
        let reused = store.allocate(AllocationDomain(0), location(0)).unwrap();
        assert_eq!(slot_index(&reused), slot_index(&entity));
        assert_ne!(reused, entity);
        assert_eq!(store.locate(entity), None);
        assert_eq!(store.locate(reused), Some(location(0)));
        store.release_strong(reused).unwrap();
        store.begin_destroy(reused).unwrap();
        assert!(store.finish_destroy(reused).unwrap());
        assert_eq!(store.counts(reused), Some((0, 0)));
        assert!(store.remove(reused));
    }

    #[test]
    fn weak_zero_only_proposes_reclaim_after_hiding() {
        let mut store = EntityStore::new(1).unwrap();
        let entity = store.allocate(AllocationDomain(0), location(0)).unwrap();
        assert!(store.release_weak(entity).is_err());
        store.acquire_weak(entity).unwrap();
        assert!(!store.release_weak(entity).unwrap());
        store.release_strong(entity).unwrap();
        store.acquire_weak(entity).unwrap();
        assert!(!store.release_weak(entity).unwrap());
        store.acquire_weak(entity).unwrap();
        assert_eq!(store.begin_destroy(entity).unwrap(), location(0));
        assert!(store.acquire_weak(entity).is_err());
        assert!(store.release_weak(entity).unwrap());
        assert!(store.release_weak(entity).is_err());
        assert_eq!(store.counts(entity), Some((0, 0)));
    }

    #[test]
    fn strong_zero_is_visible_but_cannot_revive_or_relocate() {
        let mut store = EntityStore::new(1).unwrap();
        let entity = store.allocate(AllocationDomain(0), location(0)).unwrap();
        store.acquire_strong(entity).unwrap();
        assert_eq!(store.counts(entity), Some((2, 0)));
        assert!(!store.release_strong(entity).unwrap());
        assert!(store.release_strong(entity).unwrap());
        assert_eq!(store.locate(entity), Some(location(0)));
        assert!(store.acquire_strong(entity).is_err());
        assert!(store.release_strong(entity).is_err());
        assert!(store.relocate(entity, location(1)).is_err());
        assert_eq!(store.counts(entity), Some((0, 0)));
        assert_eq!(store.locate(entity), Some(location(0)));
        let hidden = store
            .allocate(AllocationDomain(0), EntityLocation::NULL)
            .unwrap();
        assert!(store.acquire_strong(hidden).is_err());
        assert_eq!(store.counts(hidden), Some((1, 0)));
    }

    #[test]
    fn full_identity_is_partitioned_and_relocation_preserves_birth() {
        let mut store = EntityStore::new(MAX_PARTITIONS).unwrap();
        let first = store.allocate(AllocationDomain(0), location(0)).unwrap();
        for birth in 1..MAX_PARTITIONS as u32 {
            let entity = store
                .allocate(AllocationDomain(birth), location(birth))
                .unwrap();
            assert_eq!(slot_index(&entity), slot_index(&first));
            assert_eq!(domain_bits(&entity), birth);
            assert_eq!(store.locate(entity), Some(location(birth)));
            store.relocate(entity, location(0)).unwrap();
            assert_eq!(domain_bits(&entity), birth);
            assert_eq!(store.locate(entity), Some(location(0)));
            assert_eq!(store.locate(first), Some(location(0)));
        }
        let wrong_version = Entity::from(pi_key::KeyData::new(
            first.data().index(),
            first.data().version() + 2,
        ));
        assert_eq!(store.locate(wrong_version), None);
        assert!(store.relocate(wrong_version, location(1)).is_err());
        assert!(store.relocate(first, EntityLocation::NULL).is_err());
        assert_eq!(store.locate(first), Some(location(0)));
        let hidden = store
            .allocate(AllocationDomain(0), EntityLocation::NULL)
            .unwrap();
        assert_eq!(store.locate(hidden), None);
        assert!(store.relocate(hidden, location(0)).is_err());
    }
}

fn error(kind: pi_ecs_types::error::ErrorKind, message: &str) -> EcsError {
    pi_ecs_types::error::classified_error(kind, message)
}

fn external(birth: u32, local: Entity) -> Entity {
    let index = (birth << crate::entity::DOMAIN_SHIFT) | local.data().index();
    Entity::from(pi_key::KeyData::new(index, local.data().version()))
}

fn local_key(entity: Entity) -> Entity {
    Entity::from(pi_key::KeyData::new(
        slot_index(&entity),
        entity.data().version(),
    ))
}

#[cfg(not(feature = "rc"))]
fn link_value(strong: u32, weak: u32) -> u64 {
    ((strong as u64) << STRONG_SHIFT) | weak as u64
}

#[cfg(not(feature = "rc"))]
fn link_counts(link: u64) -> (u32, u32) {
    ((link >> STRONG_SHIFT) as u32, link as u32)
}

#[cfg(not(feature = "rc"))]
fn add_strong(link: &pi_slot::ShareU64) -> Result<(), EcsError> {
    use std::sync::atomic::Ordering;
    let mut current = link.load(Ordering::Acquire);
    loop {
        let (strong, weak) = link_counts(current);
        if strong == 0 {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "实体强引用已归零",
            ));
        }
        if strong == u32::MAX {
            return Err(error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "实体强引用计数溢出",
            ));
        }
        let next = link_value(strong + 1, weak);
        match link.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Ok(()),
            Err(actual) => current = actual,
        }
    }
}

#[cfg(not(feature = "rc"))]
fn release_strong(link: &pi_slot::ShareU64) -> Result<bool, EcsError> {
    use std::sync::atomic::Ordering;
    let mut current = link.load(Ordering::Acquire);
    loop {
        let (strong, weak) = link_counts(current);
        if strong == 0 {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "实体强引用已归零",
            ));
        }
        let next = link_value(strong - 1, weak);
        match link.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Ok(strong == 1),
            Err(actual) => current = actual,
        }
    }
}

#[cfg(not(feature = "rc"))]
fn add_weak(link: &pi_slot::ShareU64) -> Result<(), EcsError> {
    use std::sync::atomic::Ordering;
    let mut current = link.load(Ordering::Acquire);
    loop {
        let (strong, weak) = link_counts(current);
        if weak == u32::MAX {
            return Err(error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "实体弱引用计数溢出",
            ));
        }
        let next = link_value(strong, weak + 1);
        match link.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Ok(()),
            Err(actual) => current = actual,
        }
    }
}

#[cfg(not(feature = "rc"))]
fn release_weak(link: &pi_slot::ShareU64) -> Result<bool, EcsError> {
    use std::sync::atomic::Ordering;
    let mut current = link.load(Ordering::Acquire);
    loop {
        let (strong, weak) = link_counts(current);
        if weak == 0 {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "实体弱引用已归零",
            ));
        }
        let next = link_value(strong, weak - 1);
        match link.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Ok(strong == 0 && weak == 1),
            Err(actual) => current = actual,
        }
    }
}

impl EntityStore {
    pub(crate) fn new_inner(partition_count: usize) -> Result<EntityStore, EcsError> {
        if !(1..=MAX_PARTITIONS).contains(&partition_count) {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "分区数量超出允许范围",
            ));
        }
        #[cfg(feature = "rc")]
        let slots: Vec<pi_slotmap::SlotMap<Entity, EntityRecord>> =
            (0..partition_count).map(|_| Default::default()).collect();
        #[cfg(not(feature = "rc"))]
        let slots: Vec<pi_slot::SlotMap<Entity, EntityLocation>> =
            (0..partition_count).map(|_| Default::default()).collect();
        Ok(EntityStore {
            slots: slots.into_boxed_slice(),
        })
    }
}

impl EntityStore {
    pub(crate) fn allocate_inner(
        store: &mut EntityStore,
        domain: AllocationDomain,
        location: EntityLocation,
    ) -> Result<Entity, EcsError> {
        let birth = domain.0;
        let map = store.slots.get_mut(birth as usize).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "出生分区超出已配置分区数",
            )
        })?;
        #[cfg(feature = "rc")]
        let local = map.insert(EntityRecord {
            location,
            strong: 1,
            weak: 0,
        });
        #[cfg(not(feature = "rc"))]
        let local = map.insert_with_version_bits(location, link_value(1, 0), 0);
        if local.data().index() > LOCAL_SLOT_MASK {
            map.remove(local);
            return Err(error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "出生分区本地槽容量耗尽",
            ));
        }
        Ok(external(birth, local))
    }
}

impl EntityStore {
    pub(crate) fn locate_inner(store: &EntityStore, entity: Entity) -> Option<EntityLocation> {
        let birth = domain_bits(&entity);
        let map = store.slots.get(birth as usize)?;
        let local = local_key(entity);
        #[cfg(feature = "rc")]
        {
            map.get(local)
                .filter(|record| record.location != EntityLocation::NULL)
                .map(|record| record.location)
        }
        #[cfg(not(feature = "rc"))]
        {
            map.get(local).map(|(location, _)| *location)
        }
    }
}

impl EntityStore {
    pub(crate) fn relocate_inner(
        store: &mut EntityStore,
        entity: Entity,
        location: EntityLocation,
    ) -> Result<(), EcsError> {
        let birth = domain_bits(&entity);
        let map = store.slots.get_mut(birth as usize).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "出生分区超出已配置分区数",
            )
        })?;
        let local = local_key(entity);
        #[cfg(feature = "rc")]
        {
            let record = map.get_mut(local).ok_or_else(|| {
                error(
                    pi_ecs_types::error::ErrorKind::NotFound,
                    "实体身份无效或代际已失效",
                )
            })?;
            if record.strong == 0 || record.location == EntityLocation::NULL {
                return Err(error(
                    pi_ecs_types::error::ErrorKind::InvalidState,
                    "实体不可结构迁移",
                ));
            }
            if location == EntityLocation::NULL {
                return Err(error(
                    pi_ecs_types::error::ErrorKind::InvalidInput,
                    "结构迁移目标不可为空",
                ));
            }
            record.location = location;
        }
        #[cfg(not(feature = "rc"))]
        {
            let (current, _) = map.get_mut(local).ok_or_else(|| {
                error(
                    pi_ecs_types::error::ErrorKind::NotFound,
                    "实体身份无效或代际已失效",
                )
            })?;
            *current = location;
        }
        Ok(())
    }
}

#[cfg(feature = "rc")]
impl EntityStore {
    pub(crate) fn publish_spawn(&mut self, entity: Entity, location: EntityLocation) {
        let map = &mut self.slots[domain_bits(&entity) as usize];
        let record = map.get_mut(local_key(entity)).expect("未发布身份缺失");
        assert_eq!((record.strong, record.weak), (1, 0));
        assert!(record.location.is_null() && !location.is_null());
        record.location = location;
    }

    /// Clears the provisional identity of a spawn that never published a row.
    pub(crate) fn discard_unpublished_spawn(&mut self, entity: Entity) {
        let map = &mut self.slots[domain_bits(&entity) as usize];
        let key = local_key(entity);
        let record = map.get(key).expect("未发布身份缺失");
        assert_eq!((record.strong, record.weak), (1, 0));
        map.remove(key).expect("未发布身份必须只撤销一次");
    }
}

#[cfg(feature = "rc")]
fn record_mut(store: &mut EntityStore, entity: Entity) -> Result<&mut EntityRecord, EcsError> {
    let map = store
        .slots
        .get_mut(domain_bits(&entity) as usize)
        .ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "出生分区超出已配置分区数",
            )
        })?;
    map.get_mut(local_key(entity)).ok_or_else(|| {
        error(
            pi_ecs_types::error::ErrorKind::NotFound,
            "实体身份无效或代际已失效",
        )
    })
}

#[cfg(feature = "rc")]
impl EntityStore {
    pub(crate) fn acquire_strong_inner(
        store: &mut EntityStore,
        entity: Entity,
    ) -> Result<(), EcsError> {
        let record = record_mut(store, entity)?;
        if record.strong == 0 || record.location == EntityLocation::NULL {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "实体不可增持强引用",
            ));
        }
        record.strong = record.strong.checked_add(1).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "实体强引用计数溢出",
            )
        })?;
        Ok(())
    }
}

#[cfg(not(feature = "rc"))]
impl EntityStore {
    pub(crate) fn acquire_strong_inner(
        store: &EntityStore,
        entity: Entity,
    ) -> Result<(), EcsError> {
        let birth = domain_bits(&entity);
        let map = store.slots.get(birth as usize).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "出生分区超出已配置分区数",
            )
        })?;
        let (_, link) = map.get(local_key(entity)).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::NotFound,
                "实体身份无效或代际已失效",
            )
        })?;
        add_strong(link)
    }
}

#[cfg(feature = "rc")]
impl EntityStore {
    pub(crate) fn release_strong_inner(
        store: &mut EntityStore,
        entity: Entity,
    ) -> Result<bool, EcsError> {
        let record = record_mut(store, entity)?;
        record.strong = record.strong.checked_sub(1).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "实体强引用已归零",
            )
        })?;
        Ok(record.strong == 0)
    }
}

#[cfg(not(feature = "rc"))]
impl EntityStore {
    pub(crate) fn release_strong_inner(
        store: &EntityStore,
        entity: Entity,
    ) -> Result<bool, EcsError> {
        let birth = domain_bits(&entity);
        let map = store.slots.get(birth as usize).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "出生分区超出已配置分区数",
            )
        })?;
        let (_, link) = map.get(local_key(entity)).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::NotFound,
                "实体身份无效或代际已失效",
            )
        })?;
        release_strong(link)
    }
}

#[cfg(feature = "rc")]
impl EntityStore {
    pub(crate) fn acquire_weak_inner(
        store: &mut EntityStore,
        entity: Entity,
    ) -> Result<(), EcsError> {
        let record = record_mut(store, entity)?;
        if record.location == EntityLocation::NULL {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "实体不可增持弱引用",
            ));
        }
        record.weak = record.weak.checked_add(1).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "实体弱引用计数溢出",
            )
        })?;
        Ok(())
    }
}

#[cfg(not(feature = "rc"))]
impl EntityStore {
    pub(crate) fn acquire_weak_inner(store: &EntityStore, entity: Entity) -> Result<(), EcsError> {
        let birth = domain_bits(&entity);
        let map = store.slots.get(birth as usize).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "出生分区超出已配置分区数",
            )
        })?;
        let (_, link) = map.get(local_key(entity)).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::NotFound,
                "实体身份无效或代际已失效",
            )
        })?;
        add_weak(link)
    }
}

#[cfg(feature = "rc")]
impl EntityStore {
    pub(crate) fn release_weak_inner(
        store: &mut EntityStore,
        entity: Entity,
    ) -> Result<bool, EcsError> {
        let record = record_mut(store, entity)?;
        record.weak = record.weak.checked_sub(1).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "实体弱引用已归零",
            )
        })?;
        Ok(record.weak == 0 && record.strong == 0 && record.location == EntityLocation::NULL)
    }
}

#[cfg(not(feature = "rc"))]
impl EntityStore {
    pub(crate) fn release_weak_inner(
        store: &EntityStore,
        entity: Entity,
    ) -> Result<bool, EcsError> {
        let birth = domain_bits(&entity);
        let map = store.slots.get(birth as usize).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "出生分区超出已配置分区数",
            )
        })?;
        let (_, link) = map.get(local_key(entity)).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::NotFound,
                "实体身份无效或代际已失效",
            )
        })?;
        release_weak(link)
    }
}

impl EntityStore {
    pub(crate) fn counts_inner(store: &EntityStore, entity: Entity) -> Option<(u32, u32)> {
        let birth = domain_bits(&entity);
        let map = store.slots.get(birth as usize)?;
        let local = local_key(entity);
        #[cfg(feature = "rc")]
        {
            map.get(local).map(|record| (record.strong, record.weak))
        }
        #[cfg(not(feature = "rc"))]
        {
            map.get(local)
                .map(|(_, link)| link_counts(link.load(std::sync::atomic::Ordering::Acquire)))
        }
    }
}

#[cfg(not(feature = "rc"))]
impl EntityStore {
    pub(crate) fn remove_inner(store: &mut EntityStore, entity: Entity) -> Option<EntityLocation> {
        let birth = domain_bits(&entity);
        let map = store.slots.get_mut(birth as usize)?;
        let local = local_key(entity);
        #[cfg(feature = "rc")]
        {
            map.remove(local).map(|record| record.location)
        }
        #[cfg(not(feature = "rc"))]
        {
            map.remove(local)
        }
    }
}

#[cfg(feature = "rc")]
impl EntityStore {
    pub(crate) fn begin_destroy_inner(
        store: &mut EntityStore,
        entity: Entity,
    ) -> Result<EntityLocation, EcsError> {
        let record = record_mut(store, entity)?;
        if record.strong != 0 || record.location == EntityLocation::NULL {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "实体不在待销毁位置状态",
            ));
        }
        Ok(std::mem::replace(
            &mut record.location,
            EntityLocation::NULL,
        ))
    }
}
#[cfg(feature = "rc")]
impl EntityStore {
    pub(crate) fn finish_destroy_inner(
        store: &mut EntityStore,
        entity: Entity,
    ) -> Result<bool, EcsError> {
        let record = record_mut(store, entity)?;
        if record.strong != 0 || record.location != EntityLocation::NULL {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "实体尚未隐藏或仍有强引用",
            ));
        }
        // Cleanup completion and exactly-once invocation belong to the destruction call stack.
        Ok(record.weak == 0)
    }
}
#[cfg(feature = "rc")]
impl EntityStore {
    pub(crate) fn remove_rc_inner(store: &mut EntityStore, entity: Entity) -> bool {
        let Some(map) = store.slots.get_mut(domain_bits(&entity) as usize) else {
            return false;
        };
        let local = local_key(entity);
        let Some(record) = map.get(local) else {
            return false;
        };
        if record.strong != 0 || record.weak != 0 || record.location != EntityLocation::NULL {
            return false;
        }
        map.remove(local).is_some()
    }
}
