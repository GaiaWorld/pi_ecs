//! 登记配对芯；负责类型/Table/基础原型的元数据冻结。

#[cfg(test)]
mod registry_tests {
    use super::*;
    use crate::component::{Component, PlainAccess};

    #[repr(align(64))]
    struct Aligned([u8; 65]);
    impl Component for Aligned {
        type Access = PlainAccess;
    }

    struct External;
    struct ExternalAccess;
    impl Component for External {
        type Access = ExternalAccess;
    }
    // SAFETY: 本测试策略只读单槽，不开放写权限；release 唯一清偿普通 ZST。
    unsafe impl crate::component::ComponentPolicy<External> for ExternalAccess {
        type Read<'a, 'w> = &'a External;
        type Write<'a, 'w> = ();

        unsafe fn read<'a, 'w>(
            value: &'a External,
            _world: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
        ) -> Self::Read<'a, 'w> {
            value
        }
        unsafe fn write<'a, 'w>(
            _value: &'a mut External,
            _world: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
            _marker: crate::change_detection::ChangeMarker<'a>,
        ) -> Self::Write<'a, 'w> {
        }
        unsafe fn read_removed<'a>(
            value: &'a External,
            _world: crate::unsafe_world_cell::UnsafeWorldCell<'a>,
        ) -> Self::Read<'a, 'a> {
            value
        }
        unsafe fn release(
            value: &mut std::mem::ManuallyDrop<External>,
            _world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
            _mode: crate::component_policy::ReleaseMode,
        ) {
            unsafe { std::mem::ManuallyDrop::drop(value) };
        }
    }

    #[test]
    fn registry_accepts_external_policy_and_interleaves_stable_ids_with_zst() {
        let mut registry = TypeRegistry::new();
        let external = registry
            .register_component::<External>(StoragePolicy::SparseOnly)
            .unwrap();
        let plain = registry
            .register_component::<Aligned>(StoragePolicy::TableEligible)
            .unwrap();
        assert_eq!(external, ComponentId(0));
        assert_eq!(plain, ComponentId(1));
        let info = registry.component(external).unwrap();
        assert_eq!(info.type_id, TypeId::of::<External>());
        assert_eq!(info.layout.size(), 0);
        assert_eq!(info.layout.align(), 1);
        assert_eq!(info.storage, StoragePolicy::SparseOnly);
        // 仅验证登记的擦除入口身份，不能代替真实 World 下的释放调用验收。
        let expected: crate::component::ComponentRelease = release_component::<External>;
        assert!(std::ptr::fn_addr_eq(info.release, expected));
    }

    #[test]
    fn registry_repeat_and_conflict_preserve_original_metadata() {
        let mut registry = TypeRegistry::new();
        let id = registry
            .register_component::<Aligned>(StoragePolicy::TableEligible)
            .unwrap();
        registry.components[id.0 as usize].tracking.changed_notice = true;
        let release = registry.component(id).unwrap().release;
        assert_eq!(
            registry
                .register_component::<Aligned>(StoragePolicy::TableEligible)
                .unwrap(),
            id
        );
        assert!(registry
            .register_component::<Aligned>(StoragePolicy::SparseOnly)
            .is_err());
        let info = registry.component(id).unwrap();
        assert_eq!(info.storage, StoragePolicy::TableEligible);
        assert!(info.tracking.changed_notice);
        assert!(std::ptr::fn_addr_eq(info.release, release));
        assert_eq!(registry.components.len(), 1);
        assert_eq!(registry.component_ids.len(), 1);
        assert_eq!(
            registry.component_ids.get(&TypeId::of::<Aligned>()),
            Some(&id)
        );
        assert_eq!(
            registry
                .register_component::<External>(StoragePolicy::SparseOnly)
                .unwrap(),
            ComponentId(1)
        );
    }

    #[test]
    fn registry_freeze_allows_repeat_but_rejects_new_type_and_conflict() {
        let mut registry = TypeRegistry::new();
        let id = registry
            .register_component::<External>(StoragePolicy::SparseOnly)
            .unwrap();
        registry.freeze(1).unwrap();
        assert_eq!(
            registry
                .register_component::<External>(StoragePolicy::SparseOnly)
                .unwrap(),
            id
        );
        assert!(registry
            .register_component::<External>(StoragePolicy::TableEligible)
            .is_err());
        assert!(registry
            .register_component::<Aligned>(StoragePolicy::TableEligible)
            .is_err());
        assert_eq!(registry.components.len(), 1);
        assert_eq!(registry.component_ids.len(), 1);
        assert!(!registry
            .component_ids
            .contains_key(&TypeId::of::<Aligned>()));
        let info = registry.component(id).unwrap();
        assert_eq!(info.type_id, TypeId::of::<External>());
        assert_eq!(info.storage, StoragePolicy::SparseOnly);
        assert_eq!(info.layout.size(), 0);
    }

    // Seam: TypeRegistry::register_component / component；不构造 World 或组件值。
    #[test]
    fn registry_publishes_real_type_layout_and_initial_tracking() {
        let mut registry = TypeRegistry::new();
        let id = registry
            .register_component::<Aligned>(StoragePolicy::TableEligible)
            .unwrap();
        assert_eq!(id, ComponentId(0));
        let info = registry.component(id).unwrap();
        assert_eq!(info.id, id);
        assert_eq!(info.type_id, TypeId::of::<Aligned>());
        assert_eq!(info.name, std::any::type_name::<Aligned>());
        assert_eq!(info.layout.size(), 128);
        assert_eq!(info.layout.align(), 64);
        assert_eq!(info.storage, StoragePolicy::TableEligible);
        let demand = info.tracking;
        assert!(!demand.added_tick && !demand.changed_tick);
        assert!(!demand.added_notice && !demand.changed_notice && !demand.removed_notice);
    }
}
use crate::{
    archetype::{Archetype, ArchetypeComponent, ComponentSource},
    component::{ComponentId, ComponentInfo, StoragePolicy},
    entity::ArchetypeId,
    error::EcsError,
    registry::TypeRegistry,
    table::{ColumnId, TableLayout},
};
use std::{alloc::Layout, any::TypeId, collections::HashMap};

// SAFETY: 登记绑定真实 T；调用者提供同类型、已撤销且唯一的值责任及同源清理上下文。
// 只将该责任转给 T::Access，不追加普通析构；底层分配仍归来源所有。
unsafe fn release_component<T: crate::component::Component>(
    context: &crate::destruction::ReleaseContext<'_>,
    value: crate::ptr::OwningPtr<'_>,
) {
    let value = unsafe { &mut *value.into_raw().cast::<std::mem::ManuallyDrop<T>>() };
    unsafe {
        <T::Access as crate::component::ComponentPolicy<T>>::release(
            value,
            context.world,
            context.mode,
        );
    }
}

fn error(kind: pi_ecs_types::error::ErrorKind, message: &str) -> EcsError {
    pi_ecs_types::error::classified_error(kind, message)
}

impl TypeRegistry {
    pub(crate) fn new_inner() -> TypeRegistry {
        let empty_id = ArchetypeId {
            table: 0,
            variant: 0,
        };
        let empty = TableLayout::new(empty_id, Box::new([])).expect("empty table layout is valid");
        let mut table_ids = HashMap::new();
        table_ids.insert(Box::<[ComponentId]>::from([]), empty_id);
        TypeRegistry {
            components: Vec::new(),
            component_ids: HashMap::new(),
            tables: vec![empty],
            table_ids,
            archetypes: Vec::new(),
            archetype_ids: HashMap::new(),
            frozen: false,
        }
    }
}

impl TypeRegistry {
    pub(crate) fn register_component_inner<T: crate::component::Component>(
        registry: &mut TypeRegistry,
        storage: StoragePolicy,
    ) -> Result<ComponentId, EcsError> {
        let type_id = TypeId::of::<T>();
        if let Some(&id) = registry.component_ids.get(&type_id) {
            if registry.components[id.0 as usize].storage != storage {
                return Err(error(
                    pi_ecs_types::error::ErrorKind::InvalidInput,
                    "组件的存储策略与既有登记不一致",
                ));
            }
            return Ok(id);
        }
        if registry.frozen {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "注册表已冻结，不能新增组件",
            ));
        }
        let index = u32::try_from(registry.components.len()).map_err(|_| {
            error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "组件数量超出 32 位编号容量",
            )
        })?;
        let id = ComponentId(index);
        registry.components.push(ComponentInfo {
            id,
            type_id,
            name: std::any::type_name::<T>(),
            layout: Layout::new::<T>(),
            release: release_component::<T>,
            storage,
            tracking: crate::change_detection::TrackingDemand::default(),
        });
        registry.component_ids.insert(type_id, id);
        Ok(id)
    }
}

impl TypeRegistry {
    pub(crate) fn register_table_inner(
        registry: &mut TypeRegistry,
        components: &[ComponentId],
    ) -> Result<ArchetypeId, EcsError> {
        if registry.frozen {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "注册表已冻结，不能新增 Table",
            ));
        }
        let mut canonical = Vec::with_capacity(components.len());
        for &component in components {
            let info = registry
                .components
                .get(component.0 as usize)
                .ok_or_else(|| {
                    error(
                        pi_ecs_types::error::ErrorKind::NotFound,
                        "Table 引用了未登记组件",
                    )
                })?;
            if info.storage == StoragePolicy::SparseOnly {
                return Err(error(
                    pi_ecs_types::error::ErrorKind::InvalidInput,
                    "SparseOnly 组件不能进入 Table",
                ));
            }
            canonical.push(component);
        }
        canonical.sort_unstable();
        if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "Table 组件集合不能重复",
            ));
        }
        let canonical = canonical.into_boxed_slice();
        if let Some(id) = registry.table_ids.get(&canonical) {
            return Ok(*id);
        }
        if registry.tables.len() > u16::MAX as usize {
            return Err(error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "Table 数量超出 16 位编号容量",
            ));
        }
        let id = ArchetypeId {
            table: registry.tables.len() as u16,
            variant: 0,
        };
        let layout = TableLayout::new(id, canonical.clone())?;
        registry.tables.push(layout);
        registry.table_ids.insert(canonical, id);
        Ok(id)
    }
}

impl TypeRegistry {
    pub(crate) fn freeze_inner(
        registry: &mut TypeRegistry,
        partition_count: usize,
    ) -> Result<(), EcsError> {
        if registry.frozen {
            return Ok(());
        }
        if !(1..=crate::entity::MAX_PARTITIONS).contains(&partition_count) {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "分区数量超出允许范围",
            ));
        }
        if registry.tables.len() > u16::MAX as usize + 1 {
            return Err(error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "Table 数量超出 16 位编号容量",
            ));
        }
        if registry.archetypes.is_empty() {
            for layout in &registry.tables {
                let columns = layout.columns();
                let mut components = Vec::with_capacity(columns.len());
                for (index, &component) in columns.iter().enumerate() {
                    components.push(ArchetypeComponent {
                        component,
                        source: ComponentSource::Table(ColumnId(index as u32)),
                    });
                }
                let id = layout.id();
                let archetype = Archetype {
                    id,
                    table: id,
                    components: components.into_boxed_slice(),
                    sparse_components: Box::new([]),
                };
                registry
                    .archetype_ids
                    .insert(columns.to_vec().into_boxed_slice(), id);
                registry.archetypes.push(archetype);
            }
        }
        registry.frozen = true;
        Ok(())
    }
}

impl TypeRegistry {
    pub(crate) fn plan_archetype_inner(
        registry: &TypeRegistry,
        components: &[ComponentId],
    ) -> Result<crate::registry::ArchetypePlan, EcsError> {
        if !registry.frozen {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "注册表尚未冻结",
            ));
        }
        let canonical = normalize(registry, components)?;
        if let Some(id) = registry.archetype_ids.get(&canonical) {
            return Ok(crate::registry::ArchetypePlan {
                id: *id,
                canonical,
                archetype: None,
                is_new: false,
            });
        }
        let mut base: Option<&TableLayout> = None;
        for table in &registry.tables {
            if table
                .columns()
                .iter()
                .all(|component| canonical.contains(component))
                && base
                    .map(|current| table.columns().len() > current.columns().len())
                    .unwrap_or(true)
            {
                base = Some(table);
            }
        }
        let base = base.ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::Internal,
                "没有可绑定的基础 Table",
            )
        })?;
        build_plan(registry, base, canonical)
    }
}

impl TypeRegistry {
    pub(crate) fn plan_archetype_on_inner(
        registry: &TypeRegistry,
        base: ArchetypeId,
        components: &[ComponentId],
    ) -> Result<crate::registry::ArchetypePlan, EcsError> {
        if !registry.frozen {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "注册表尚未冻结",
            ));
        }
        if base.variant != 0 {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "register_archetype_on 的 base 必须是 variant=0",
            ));
        }
        let table = registry.tables.get(base.table as usize).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::NotFound,
                "register_archetype_on 的 base Table 未登记",
            )
        })?;
        let canonical = normalize(registry, components)?;
        if !table
            .columns()
            .iter()
            .all(|component| canonical.contains(component))
        {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "register_archetype_on 的 components 必须包含 base 全部列",
            ));
        }
        if let Some(id) = registry.archetype_ids.get(&canonical) {
            return Ok(crate::registry::ArchetypePlan {
                id: *id,
                canonical,
                archetype: None,
                is_new: false,
            });
        }
        build_plan(registry, table, canonical)
    }
}

fn normalize(
    registry: &TypeRegistry,
    components: &[ComponentId],
) -> Result<Box<[ComponentId]>, EcsError> {
    let mut canonical = Vec::with_capacity(components.len());
    for &component in components {
        if registry.components.get(component.0 as usize).is_none() {
            return Err(error(
                pi_ecs_types::error::ErrorKind::NotFound,
                "原型引用了未登记组件",
            ));
        }
        canonical.push(component);
    }
    canonical.sort_unstable();
    if canonical.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(error(
            pi_ecs_types::error::ErrorKind::InvalidInput,
            "原型组件集合不能重复",
        ));
    }
    Ok(canonical.into_boxed_slice())
}

fn build_plan(
    registry: &TypeRegistry,
    base: &TableLayout,
    canonical: Box<[ComponentId]>,
) -> Result<crate::registry::ArchetypePlan, EcsError> {
    let base_id = base.id();
    let variant = registry
        .archetypes
        .iter()
        .filter(|archetype| archetype.table == base_id)
        .count();
    if variant > u16::MAX as usize {
        return Err(error(
            pi_ecs_types::error::ErrorKind::ResourceExhausted,
            "原型变体数量超出 16 位编号容量",
        ));
    }
    let id = ArchetypeId {
        table: base_id.table,
        variant: variant as u16,
    };
    let mut mapped = Vec::with_capacity(canonical.len());
    let mut sparse = Vec::new();
    for &component in canonical.iter() {
        if let Some(column) = base.columns().iter().position(|value| *value == component) {
            mapped.push(ArchetypeComponent {
                component,
                source: ComponentSource::Table(ColumnId(column as u32)),
            });
        } else {
            mapped.push(ArchetypeComponent {
                component,
                source: ComponentSource::Sparse,
            });
            sparse.push(component);
        }
    }
    let archetype = Archetype {
        id,
        table: base_id,
        components: mapped.into_boxed_slice(),
        sparse_components: sparse.into_boxed_slice(),
    };
    Ok(crate::registry::ArchetypePlan {
        id,
        canonical,
        archetype: Some(archetype),
        is_new: true,
    })
}

impl TypeRegistry {
    pub(crate) fn commit_archetype_inner(
        registry: &mut TypeRegistry,
        plan: crate::registry::ArchetypePlan,
    ) -> ArchetypeId {
        if let Some(archetype) = plan.archetype {
            registry.archetype_ids.insert(plan.canonical, plan.id);
            registry.archetypes.push(archetype);
        }
        plan.id
    }
}

impl TypeRegistry {
    pub(crate) fn component_inner(
        registry: &TypeRegistry,
        id: ComponentId,
    ) -> Option<&ComponentInfo> {
        registry.components.get(id.0 as usize)
    }
}

impl TypeRegistry {
    pub(crate) fn table_inner(registry: &TypeRegistry, id: ArchetypeId) -> Option<&TableLayout> {
        if id.variant != 0 {
            return None;
        }
        registry.tables.get(id.table as usize)
    }
}

impl TypeRegistry {
    pub(crate) fn archetype_inner(registry: &TypeRegistry, id: ArchetypeId) -> Option<&Archetype> {
        registry
            .archetypes
            .iter()
            .find(|archetype| archetype.id == id)
    }
}
