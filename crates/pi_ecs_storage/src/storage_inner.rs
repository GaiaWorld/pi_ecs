//! 物理所有权组装配对芯；不依赖逻辑注册表或其他配对芯。
#[cfg(all(test, feature = "rc"))]
mod shutdown_take_tests {
    use super::*;
    use pi_ecs_types::{
        component::ComponentId,
        entity::{Entity, KeyData},
    };
    use std::{alloc::Layout, mem::ManuallyDrop, ptr::NonNull};
    // Seam: 关闭取责遇到损坏必须报故障，不能把仍存在的责任伪装成取尽。
    #[test]
    fn shutdown_take_mapping_corruption_panics_before_revoking_owner() {
        let mut world = WorldStorage::new(
            Box::new([]),
            vec![Some(
                SparseSetStorage::sparse_array(ComponentId(0), Layout::new::<String>()).unwrap(),
            )]
            .into_boxed_slice(),
        )
        .unwrap();
        let entity = Entity::from(KeyData::new(1, 1));
        world.prepare_sparse(ComponentId(0), entity).unwrap();
        let mut value = ManuallyDrop::new(String::from("still-owned"));
        world.insert_sparse(ComponentId(0), entity, unsafe {
            crate::ptr::OwningPtr::from_raw(NonNull::from(&mut *value).cast())
        });
        let sparse = world.sparse_address(ComponentId(0)).unwrap();
        // 仅故障注入破坏反向身份；失败后修复以完整清理真实输入，不泄漏夹具。
        let SparseSetStorage::SparseArray(set) = unsafe { &mut *sparse.as_ptr() };
        set.owners.members.dense.get_mut(0).unwrap().key = KeyData::new(2, 1).as_ffi();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                world.take_one_sparse_for_shutdown_raw()
            }))
            .is_err()
        );
        let SparseSetStorage::SparseArray(set) = unsafe { &mut *sparse.as_ptr() };
        assert_eq!(
            set.owners.members.dense.get(0).unwrap().key,
            KeyData::new(2, 1).as_ffi()
        );
        set.owners.members.dense.get_mut(0).unwrap().key = KeyData::new(1, 1).as_ffi();
        let (id, pointer) = unsafe { world.take_one_sparse_for_shutdown_raw() }.unwrap();
        assert_eq!(id, ComponentId(0));
        assert_eq!(
            unsafe { pointer.cast::<String>().as_ptr().read() },
            "still-owned"
        );
        assert!(unsafe { world.take_one_sparse_for_shutdown_raw() }.is_none());
    }
    static ZST_DROPS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    struct Zst;
    impl Drop for Zst {
        fn drop(&mut self) {
            ZST_DROPS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    // Seam: WorldStorage::take_one_sparse_for_shutdown_raw，真实 plain/tracked 安装及 prepare 空洞。
    #[test]
    fn shutdown_take_interleaved_prepares_and_zst_are_each_owned_once() {
        let mut world = WorldStorage::new(
            Box::new([]),
            vec![
                Some(
                    SparseSetStorage::sparse_array(ComponentId(0), Layout::new::<String>())
                        .unwrap(),
                ),
                None,
                Some(SparseSetStorage::sparse_array(ComponentId(2), Layout::new::<Zst>()).unwrap()),
            ]
            .into_boxed_slice(),
        )
        .unwrap();
        let entity = |n| Entity::from(KeyData::new(n, 1));
        for n in [0, 2, 4] {
            world.prepare_sparse(ComponentId(0), entity(n)).unwrap();
        }
        world
            .apply_tracking(
                &[crate::storage::TrackingTarget {
                    component: ComponentId(0),
                    added: true,
                    changed: true,
                }],
                pi_ecs_types::change_detection::ChangeTick::new(
                    std::num::NonZeroU64::new(2).unwrap(),
                ),
            )
            .unwrap();
        for n in [1, 3] {
            world
                .prepare_sparse_tracked(ComponentId(0), entity(n))
                .unwrap();
            let mut value = ManuallyDrop::new(format!("value-{n}"));
            world.insert_sparse_tracked(
                ComponentId(0),
                entity(n),
                unsafe { crate::ptr::OwningPtr::from_raw(NonNull::from(&mut *value).cast()) },
                pi_ecs_types::change_detection::ComponentTicks {
                    added: Some(pi_ecs_types::change_detection::ChangeTick::new(
                        std::num::NonZeroU64::new(2).unwrap(),
                    )),
                    changed: Some(pi_ecs_types::change_detection::ChangeTick::new(
                        std::num::NonZeroU64::new(2).unwrap(),
                    )),
                },
            );
        }
        for n in 0..4 {
            world.prepare_sparse(ComponentId(2), entity(n)).unwrap();
        }
        for n in [0, 2] {
            let mut value = ManuallyDrop::new(Zst);
            world.insert_sparse(ComponentId(2), entity(n), unsafe {
                crate::ptr::OwningPtr::from_raw(NonNull::from(&mut *value).cast())
            });
        }
        let root = world.sparse_address(ComponentId(0)).unwrap();
        let mut strings = Vec::new();
        let mut zst = 0;
        while let Some((id, pointer)) = unsafe { world.take_one_sparse_for_shutdown_raw() } {
            if id == ComponentId(0) {
                strings.push(unsafe { pointer.cast::<String>().as_ptr().read() });
            } else {
                assert_eq!(id, ComponentId(2));
                assert_eq!(ZST_DROPS.load(std::sync::atomic::Ordering::SeqCst), zst);
                unsafe { pointer.cast::<Zst>().as_ptr().drop_in_place() };
                zst += 1;
            }
        }
        strings.sort();
        assert_eq!(strings, ["value-1", "value-3"]);
        assert_eq!(zst, 2);
        assert_eq!(ZST_DROPS.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert!(unsafe { world.take_one_sparse_for_shutdown_raw() }.is_none());
        assert_eq!(world.sparse_address(ComponentId(0)).unwrap(), root);
        for n in [1, 3] {
            assert!(world
                .sparse(ComponentId(0))
                .unwrap()
                .get(entity(n))
                .is_none());
        }
    }
}
#[cfg(feature = "rc")]
impl WorldStorage {
    pub(super) unsafe fn take_one_sparse_for_shutdown_raw_inner(
        &mut self,
    ) -> Option<(pi_ecs_types::component::ComponentId, std::ptr::NonNull<u8>)> {
        use crate::sparse_set::SparseDenseIndex;
        use pi_ecs_types::entity::{domain_bits, slot_index, Entity, KeyData};
        for (component, slot) in self.sparse.iter().enumerate() {
            let Some(slot) = slot else {
                continue;
            };
            // SAFETY: 关闭独占且全部局部责任已收束；不替换固定分配体。
            let SparseSetStorage::SparseArray(set) = unsafe { &mut *slot.get() };
            assert_eq!(set.component.0 as usize, component, "稀疏关闭组件目录损坏");
            for dense in 0..set.owners.members.len {
                let key = set
                    .owners
                    .members
                    .dense
                    .get(dense)
                    .expect("稀疏关闭 owner 缺失")
                    .key;
                if key == u64::MAX {
                    continue;
                }
                let entity = Entity::from(KeyData::from_ffi(key));
                let domain = domain_bits(&entity) as usize;
                let local = slot_index(&entity) as usize;
                let index = set
                    .owners
                    .members
                    .indices
                    .directory
                    .entries
                    .get(domain)
                    .and_then(|entry| entry.values.as_ref())
                    .and_then(|values| values.get(local))
                    .expect("稀疏关闭反向映射缺失")
                    .cast::<SparseDenseIndex>();
                assert_eq!(
                    unsafe { (*index).index } as usize,
                    dense,
                    "稀疏关闭双向映射损坏"
                );
                let pointer = set
                    .values
                    .directory
                    .entries
                    .get(domain)
                    .and_then(|entry| entry.values.as_ref())
                    .and_then(|values| values.get(local))
                    .and_then(std::ptr::NonNull::new)
                    .expect("稀疏关闭活值槽缺失");
                // 所有检查在撤销前完成。关闭后不再进行普通操作，不压缩 owner/准备空槽。
                unsafe {
                    (*index).index = u32::MAX;
                }
                set.owners
                    .members
                    .dense
                    .get_mut(dense)
                    .expect("已验证 owner")
                    .key = u64::MAX;
                return Some((set.component, pointer));
            }
        }
        None
    }
}

#[cfg(all(test, feature = "rc"))]
mod capacity_tests {
    use super::*;
    #[test]
    fn member_limit_rejects_before_allocating_or_reusing_holes() {
        let id = ArchetypeId {
            table: pi_ecs_types::entity::TableId(0),
            variant: pi_ecs_types::entity::VariantId(0),
        };
        let layout = crate::table::TableLayout::new(id, Box::new([])).unwrap();
        let mut world = WorldStorage::new(
            vec![Table::new(&layout, Box::new([])).unwrap()].into_boxed_slice(),
            Box::new([]),
        )
        .unwrap();
        world.archetypes[0].variants[0].len = 1 << 20;
        assert!(world
            .append_member(
                id,
                pi_ecs_types::entity::Entity::from(pi_ecs_types::entity::KeyData::new(0, 1)),
                TableRow::new(0)
            )
            .is_err());
        assert_eq!(world.archetypes[0].variants[0].len, 1 << 20);
        assert_eq!(world.archetypes[0].variants[0].members.contiguous_len(), 0);
    }
}

#[cfg(feature = "rc")]
impl super::WorldStorage {
    /// Returns the raw destination of a prepared but unpublished sparse slot.
    pub fn prepared_sparse_slot(
        &mut self,
        component: pi_ecs_types::component::ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) -> Option<std::ptr::NonNull<u8>> {
        let slot = self.sparse.get(component.0 as usize)?.as_ref()?;
        // SAFETY: maintenance is exclusive and the slot cannot be replaced.
        unsafe { &mut *slot.get() }.prepared_value_slot(entity)
    }

    /// Publishes a sparse value written in place.
    ///
    /// # Safety
    /// The slot must hold a fully initialized value of the registered type and
    /// have no conflicting views. The caller transfers its unique responsibility.
    pub unsafe fn publish_prepared_sparse(
        &mut self,
        component: pi_ecs_types::component::ComponentId,
        entity: pi_ecs_types::entity::Entity,
        ticks: pi_ecs_types::change_detection::ComponentTicks,
    ) {
        let slot = self.sparse[component.0 as usize]
            .as_ref()
            .expect("稀疏组件未登记");
        // SAFETY: same slot and responsibility as the caller's proof.
        unsafe { (&mut *slot.get()).publish_prepared_value(entity, ticks) }
    }

    pub(super) fn prepare_sparse_tracked_inner(
        &mut self,
        component: pi_ecs_types::component::ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) -> Result<(), pi_ecs_types::error::EcsError> {
        let slot = self
            .sparse
            .get(component.0 as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| error(pi_ecs_types::error::ErrorKind::NotFound, "稀疏组件未登记"))?;
        // SAFETY: 独占维护期间短借固定分配体，无运行视图或结果。
        unsafe { &mut *slot.get() }.prepare_tracked(entity)
    }
    pub(super) fn discard_sparse_reservation_inner(
        &mut self,
        component: pi_ecs_types::component::ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) {
        let slot = self.sparse[component.0 as usize]
            .as_ref()
            .expect("稀疏组件未登记");
        // SAFETY: the same exclusive maintenance operation owns the prepared slot.
        unsafe { (&mut *slot.get()).discard_prepared_tail(entity) };
    }

    pub(super) fn sparse_ticks_inner(
        &self,
        component: pi_ecs_types::component::ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) -> Option<pi_ecs_types::change_detection::ComponentTicks> {
        self.sparse(component)?.ticks(entity)
    }
    pub(super) fn insert_sparse_tracked_inner(
        &mut self,
        component: pi_ecs_types::component::ComponentId,
        entity: pi_ecs_types::entity::Entity,
        value: crate::ptr::OwningPtr<'_>,
        ticks: pi_ecs_types::change_detection::ComponentTicks,
    ) {
        let slot = self
            .sparse
            .get(component.0 as usize)
            .and_then(Option::as_ref)
            .expect("稀疏组件必须已登记");
        // SAFETY: 独占结构安装期，源责任和冻结视图约束沿用原入口，不替换容器。
        unsafe { &mut *slot.get() }.insert_tracked(entity, value, ticks)
    }
}
#[cfg(feature = "rc")]
impl super::TableMaintenance<'_> {
    /// Returns an uninitialized slot in a claimed, unpublished row.
    pub fn claimed_column_slot(
        &mut self,
        row: TableRow,
        column: crate::table::ColumnId,
    ) -> Option<std::ptr::NonNull<u8>> {
        self.table.claimed_column_slot(row, column)
    }
}

use crate::{
    archetype::ArchetypeStorage,
    sparse_set::SparseSetStorage,
    storage::{TableArchetypes, WorldStorage},
    table::{Table, TableRow},
};
use pi_ecs_types::{entity::ArchetypeId, error::EcsError};

#[cfg(feature = "rc")]
fn prepare_sparse_tracking(
    set: &crate::sparse_set::SparseArraySparseSet<{ crate::sparse_set::SPARSE_PAGE }>,
    tick: pi_ecs_types::change_detection::ChangeTick,
) -> Result<
    crate::sparse_set::SparseArray<
        Option<pi_ecs_types::change_detection::ChangeTick>,
        { crate::sparse_set::SPARSE_PAGE },
    >,
    EcsError,
> {
    use crate::sparse_set::{RawSparseArray, SparseArray, SparseArrayDirectory, SparseArrayDomain};
    use pi_ecs_types::entity::{domain_bits, slot_index, Entity, KeyData};
    unsafe fn init(pointer: *mut u8) {
        // SAFETY: 数组布局为 Option<ChangeTick>，每槽初始化一次。
        unsafe {
            pointer
                .cast::<Option<pi_ecs_types::change_detection::ChangeTick>>()
                .write(None)
        };
    }
    let mut array = SparseArray {
        storage: RawSparseArray {
            directory: SparseArrayDirectory {
                entries: Vec::new(),
            },
        },
        members: (),
        marker: std::marker::PhantomData,
    };
    for index in 0..set.owners.members.len {
        let key = set
            .owners
            .members
            .dense
            .get(index)
            .expect("owner 范围已准备")
            .key;
        if key == u64::MAX {
            continue;
        }
        let entity = Entity::from(KeyData::from_ffi(key));
        let domain = domain_bits(&entity) as usize;
        let slot = slot_index(&entity) as usize;
        let entries = &mut array.storage.directory.entries;
        if entries.len() <= domain {
            entries.resize_with(domain + 1, SparseArrayDomain::default);
        }
        let values = entries[domain].values.get_or_insert_with(|| {
            // SAFETY: tick 元数据无析构责任，回调布局匹配。
            unsafe {
                pi_arr::RawArr::new(
                    std::alloc::Layout::new::<Option<pi_ecs_types::change_detection::ChangeTick>>(),
                    init,
                    None,
                )
            }
        });
        let pointer = values.alloc(slot).map_err(|cause| {
            pi_ecs_types::error::Report::new(pi_ecs_types::error::ErrorKind::ResourceExhausted)
                .attach_opaque(cause)
                .attach("稀疏追踪准备失败")
        })?;
        // SAFETY: 临时数组独占，目标槽已分配且类型正确。
        unsafe {
            pointer
                .cast::<Option<pi_ecs_types::change_detection::ChangeTick>>()
                .write(Some(tick))
        };
    }
    Ok(array)
}

fn error(kind: pi_ecs_types::error::ErrorKind, message: &str) -> EcsError {
    pi_ecs_types::error::classified_error(kind, message)
}

impl WorldStorage {
    pub(super) fn published_rows_inner(
        &self,
        table: pi_ecs_types::entity::TableId,
    ) -> Result<u32, EcsError> {
        let table = self
            .tables
            .get(usize::from(table))
            .ok_or_else(|| error(pi_ecs_types::error::ErrorKind::NotFound, "Table 未登记"))?;
        #[cfg(feature = "rc")]
        {
            Ok(table.published_rows)
        }
        #[cfg(not(feature = "rc"))]
        {
            Ok(table
                .published_rows
                .load(std::sync::atomic::Ordering::Acquire))
        }
    }
    pub(super) fn row_entity_inner(
        &self,
        table: pi_ecs_types::entity::TableId,
        row: TableRow,
    ) -> Option<pi_ecs_types::entity::Entity> {
        if row.index() >= self.published_rows(table).ok()? {
            return None;
        }
        let cell = self
            .tables
            .get(usize::from(table))?
            .rows
            .get(row.index() as usize)?;
        #[cfg(feature = "rc")]
        {
            match cell.0 {
                crate::table::RowState::Live(entity) => Some(entity),
                crate::table::RowState::Retired => None,
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            let key = cell.0.load(std::sync::atomic::Ordering::Acquire);
            (key != u64::MAX).then(|| {
                pi_ecs_types::entity::Entity::from(pi_ecs_types::entity::KeyData::from_ffi(key))
            })
        }
    }
    pub(super) fn member_len_inner(&self, id: ArchetypeId) -> Result<u32, EcsError> {
        let variant = self
            .archetypes
            .get(usize::from(id.table))
            .and_then(|table| table.variants.get(usize::from(id.variant)))
            .ok_or_else(|| error(pi_ecs_types::error::ErrorKind::NotFound, "原型未登记"))?;
        Ok(variant.len as u32)
    }
    pub(super) fn member_inner(
        &self,
        id: ArchetypeId,
        row: u32,
    ) -> Option<crate::storage::MemberLocation> {
        let variant = self
            .archetypes
            .get(usize::from(id.table))?
            .variants
            .get(usize::from(id.variant))?;
        if row as usize >= variant.len {
            return None;
        }
        let member = variant.members.get(row as usize)?.as_ref()?;
        Some(crate::storage::MemberLocation {
            entity: member.entity,
            table_row: member.table_row,
        })
    }
    #[cfg(feature = "rc")]
    pub(super) fn apply_tracking_inner(
        &mut self,
        targets: &[crate::storage::TrackingTarget],
        tick: pi_ecs_types::change_detection::ChangeTick,
    ) -> Result<(), EcsError> {
        // 全部替代元数据由临时计划拥有，准备失败不会触及旧开关或旧 tick。
        let mut columns = Vec::new();
        let mut sparse = Vec::new();
        for (index, target) in targets.iter().enumerate() {
            if targets[..index]
                .iter()
                .any(|old| old.component == target.component)
            {
                return Err(error(
                    pi_ecs_types::error::ErrorKind::InvalidInput,
                    "追踪目标重复",
                ));
            }
            let mut found = false;
            for (table_index, table) in self.tables.iter().enumerate() {
                for (column_index, slot) in table.columns.iter().enumerate() {
                    // SAFETY: 独占维护；准备期仅短读容器元数据。
                    let column = unsafe { &*slot.get() };
                    if column.component != target.component {
                        continue;
                    }
                    found = true;
                    let prepare = |enabled: bool,
                                   old: &Option<
                        pi_arr::Arr<Option<pi_ecs_types::change_detection::ChangeTick>>,
                    >| {
                        if !enabled || old.is_some() {
                            return None;
                        }
                        let mut array = pi_arr::Arr::new();
                        if table.claimed_rows > 0 {
                            array.alloc(table.claimed_rows as usize - 1);
                        }
                        for row in 0..table.published_rows as usize {
                            if matches!(
                                table.rows.get(row).expect("已发布行缺失").0,
                                crate::table::RowState::Live(_)
                            ) {
                                *array.get_mut(row).expect("tick 已准备") = Some(tick);
                            }
                        }
                        Some(array)
                    };
                    let added = prepare(target.added, &column.added);
                    let changed = prepare(target.changed, &column.changed);
                    columns.push((
                        table_index,
                        column_index,
                        target.added,
                        target.changed,
                        added,
                        changed,
                    ));
                }
            }
            if let Some(slot) = self
                .sparse
                .get(target.component.0 as usize)
                .and_then(Option::as_ref)
            {
                found = true;
                // SAFETY: 准备期只读旧状态，替代数组私有。
                let SparseSetStorage::SparseArray(set) = unsafe { &*slot.get() };
                let added = if target.added && set.added.is_none() {
                    Some(prepare_sparse_tracking(set, tick)?)
                } else {
                    None
                };
                let changed = if target.changed && set.changed.is_none() {
                    Some(prepare_sparse_tracking(set, tick)?)
                } else {
                    None
                };
                sparse.push((
                    target.component.0 as usize,
                    target.added,
                    target.changed,
                    added,
                    changed,
                ));
            }
            if !found {
                return Err(error(
                    pi_ecs_types::error::ErrorKind::NotFound,
                    "追踪组件无物理存储",
                ));
            }
        }
        // 提交只有元数据所有权移动/释放，无分配、无业务值操作。
        for (table, column, added_enabled, changed_enabled, added, changed) in columns {
            // SAFETY: 独占维护结束所有视图；不替换稳定 Column。
            let column = unsafe { &mut *self.tables[table].columns[column].get() };
            if !added_enabled {
                column.added = None;
            } else if let Some(array) = added {
                column.added = Some(array);
            }
            if !changed_enabled {
                column.changed = None;
            } else if let Some(array) = changed {
                column.changed = Some(array);
            }
        }
        for (component, added_enabled, changed_enabled, added, changed) in sparse {
            // SAFETY: 同上，只切换固定容器内部的追踪数组。
            let SparseSetStorage::SparseArray(set) = unsafe {
                &mut *self.sparse[component]
                    .as_ref()
                    .expect("已验证稀疏后端")
                    .get()
            };
            if !added_enabled {
                set.added = None;
            } else if let Some(array) = added {
                set.added = Some(array);
            }
            if !changed_enabled {
                set.changed = None;
            } else if let Some(array) = changed {
                set.changed = Some(array);
            }
        }
        Ok(())
    }
    #[cfg(feature = "rc")]
    pub(super) fn settle_inner(
        &mut self,
    ) -> Result<Vec<crate::storage::SettledLocation>, EcsError> {
        use crate::storage::SettledLocation;
        use crate::{
            settle::{RowMove, SettlePlan},
            table::{RowCell, RowState},
        };
        let exhausted = || {
            error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "整理计划容量不足",
            )
        };
        let mut plans = Vec::new();
        plans
            .try_reserve(self.tables.len())
            .map_err(|_| exhausted())?;
        let mut member_plans = Vec::new();
        let mut result = Vec::new();
        // 首先完成所有动作、映射和最终交接记录。此阶段完全不修改旧物理状态。
        for (table_index, table) in self.tables.iter().enumerate() {
            if table.claimed_rows != table.published_rows {
                return Err(error(
                    pi_ecs_types::error::ErrorKind::InvalidInput,
                    "整理前须撤销未发布准备行",
                ));
            }
            let old_len = table.published_rows as usize;
            let mut mapping = Vec::new();
            mapping
                .try_reserve_exact(old_len)
                .map_err(|_| exhausted())?;
            mapping.extend((0..old_len).map(|row| row as u32));
            let mut moves = Vec::new();
            moves.try_reserve(old_len / 2).map_err(|_| exhausted())?;
            let mut end = old_len;
            let mut hole = 0;
            while hole < end {
                if matches!(
                    table.rows.get(hole).expect("已发布行缺失").0,
                    RowState::Live(_)
                ) {
                    hole += 1;
                    continue;
                }
                while end > hole
                    && matches!(
                        table.rows.get(end - 1).expect("已发布行缺失").0,
                        RowState::Retired
                    )
                {
                    end -= 1;
                }
                if hole >= end {
                    break;
                }
                end -= 1;
                mapping[end] = hole as u32;
                moves.push(RowMove {
                    source: TableRow::new(end as u32),
                    target: TableRow::new(hole as u32),
                });
                hole += 1;
            }
            for (variant_index, variant) in self.archetypes[table_index].variants.iter().enumerate()
            {
                let mut order = Vec::new();
                order
                    .try_reserve_exact(variant.len)
                    .map_err(|_| exhausted())?;
                order.extend((0..variant.len).map(|row| {
                    variant
                        .members
                        .get(row)
                        .expect("成员范围缺槽")
                        .as_ref()
                        .map(|_| row)
                }));
                let mut member_end = variant.len;
                let mut hole = 0;
                let mut member_moves = Vec::new();
                member_moves
                    .try_reserve(variant.len / 2)
                    .map_err(|_| exhausted())?;
                while hole < member_end {
                    if order[hole].is_some() {
                        hole += 1;
                        continue;
                    }
                    while member_end > hole && order[member_end - 1].is_none() {
                        member_end -= 1;
                    }
                    if hole >= member_end {
                        break;
                    }
                    member_end -= 1;
                    order[hole] = order[member_end].take();
                    member_moves.push((member_end, hole));
                    hole += 1;
                }
                result.try_reserve(member_end).map_err(|_| exhausted())?;
                for (new_row, old_row) in order[..member_end].iter().enumerate() {
                    let old_row = old_row.expect("紧缩成员前缀不能有洞");
                    let member = variant
                        .members
                        .get(old_row)
                        .and_then(Option::as_ref)
                        .expect("活成员缺失");
                    let table_row = member.table_row.index() as usize;
                    if table_row >= old_len
                        || !matches!(table.rows.get(table_row).expect("成员行缺失").0, RowState::Live(entity) if entity == member.entity)
                    {
                        return Err(error(
                            pi_ecs_types::error::ErrorKind::Internal,
                            "成员与 Table 完整身份不一致",
                        ));
                    }
                    let final_row = mapping[table_row];
                    if new_row != old_row || final_row != member.table_row.index() {
                        result.push(SettledLocation {
                            entity: member.entity,
                            archetype: variant.id,
                            table_row: TableRow::new(final_row),
                            archetype_row: new_row as u32,
                        });
                    }
                }
                member_plans.try_reserve(1).map_err(|_| exhausted())?;
                member_plans.push((table_index, variant_index, member_moves, member_end));
            }
            plans.push((SettlePlan { moves, len: end }, mapping));
        }
        // 从此只做无分配提交。RC 数组已连续，保留空容量而不调用重分配整理 API。
        for (table_index, (plan, mapping)) in plans.iter().enumerate() {
            let table = &mut self.tables[table_index];
            for slot in &table.columns {
                // SAFETY: 静止点独占维护；所有列消费同一个已验证计划。
                unsafe { (&mut *slot.get()).settle(plan) };
            }
            for action in &plan.moves {
                let state = std::mem::take(
                    &mut table
                        .rows
                        .get_mut(action.source.index() as usize)
                        .expect("源行缺失")
                        .0,
                );
                *table
                    .rows
                    .get_mut(action.target.index() as usize)
                    .expect("目标行缺失") = RowCell(state);
            }
            for row in plan.len..table.published_rows as usize {
                *table.rows.get_mut(row).expect("尾行缺失") = RowCell::default();
            }
            for variant in &mut self.archetypes[table_index].variants {
                for (_, member) in variant.members.slice_mut(0..variant.len) {
                    if let Some(member) = member {
                        member.table_row =
                            TableRow::new(mapping[member.table_row.index() as usize]);
                    }
                }
            }
            table.claimed_rows = plan.len as u32;
            table.published_rows = plan.len as u32;
            table.pending_removals.rows.clear();
        }
        for (table, variant, moves, len) in member_plans {
            let variant = &mut self.archetypes[table].variants[variant];
            for (source, target) in moves {
                let member = variant.members.get_mut(source).expect("源成员缺失").take();
                *variant.members.get_mut(target).expect("目标成员缺失") = member;
            }
            for row in len..variant.len {
                *variant.members.get_mut(row).expect("尾成员缺失") = None;
            }
            variant.len = len;
            variant.pending_removals.clear();
        }
        Ok(result)
    }
    pub(super) fn world_new_inner(
        tables: Box<[Table]>,
        sparse: Box<[Option<SparseSetStorage>]>,
    ) -> Result<WorldStorage, EcsError> {
        if tables.len() > usize::from(u16::MAX) {
            return Err(error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "Table 数量超出 16 位编号容量",
            ));
        }
        let mut archetypes = Vec::with_capacity(tables.len());
        for table in 0..tables.len() {
            let id = ArchetypeId {
                table: pi_ecs_types::entity::TableId(table as u16),
                variant: pi_ecs_types::entity::VariantId(0),
            };
            archetypes.push(TableArchetypes {
                variants: vec![ArchetypeStorage::new(id)],
            });
        }
        Ok(WorldStorage {
            tables,
            #[cfg(not(feature = "rc"))]
            sparse,
            #[cfg(feature = "rc")]
            sparse: sparse
                .into_vec()
                .into_iter()
                .map(|slot| slot.map(|value| std::rc::Rc::new(std::cell::UnsafeCell::new(value))))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            archetypes: archetypes.into_boxed_slice(),
        })
    }
}

impl WorldStorage {
    #[cfg(not(feature = "rc"))]
    pub(super) fn sparse_inner(
        storage: &WorldStorage,
        component: pi_ecs_types::component::ComponentId,
    ) -> Option<&SparseSetStorage> {
        storage
            .sparse
            .get(component.0 as usize)
            .and_then(Option::as_ref)
    }
}
impl WorldStorage {
    #[cfg(not(feature = "rc"))]
    pub(super) fn sparse_mut_inner(
        storage: &mut WorldStorage,
        component: pi_ecs_types::component::ComponentId,
    ) -> Option<&mut SparseSetStorage> {
        storage
            .sparse
            .get_mut(component.0 as usize)
            .and_then(Option::as_mut)
    }
}

impl WorldStorage {
    pub(super) fn prepare_archetype_inner(
        storage: &mut WorldStorage,
        id: ArchetypeId,
    ) -> Result<(), EcsError> {
        if pi_null::Null::is_null(&id.table) || pi_null::Null::is_null(&id.variant) {
            return Err(error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "原型不可使用空 Table 或变体编号",
            ));
        }
        let table = storage
            .archetypes
            .get_mut(usize::from(id.table))
            .ok_or_else(|| {
                error(
                    pi_ecs_types::error::ErrorKind::Internal,
                    "原型 Table 编号超出已准备存储",
                )
            })?;
        let variant = usize::from(id.variant);
        if variant < table.variants.len() {
            return Ok(());
        }
        if variant != table.variants.len() {
            return Err(error(
                pi_ecs_types::error::ErrorKind::Internal,
                "原型 variant 必须按序追加",
            ));
        }
        table.variants.push(ArchetypeStorage::new(id));
        Ok(())
    }
}

impl WorldStorage {
    pub(super) fn append_member_inner(
        storage: &mut WorldStorage,
        id: ArchetypeId,
        entity: pi_ecs_types::entity::Entity,
        table_row: TableRow,
    ) -> Result<u32, EcsError> {
        let variant = storage
            .archetypes
            .get_mut(usize::from(id.table))
            .and_then(|table| table.variants.get_mut(usize::from(id.variant)))
            .ok_or_else(|| error(pi_ecs_types::error::ErrorKind::NotFound, "原型未登记"))?;
        if variant.len >= 1 << 20 {
            return Err(error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "原型成员行超出 20 位容量",
            ));
        }
        let row = variant.len;
        *variant.members.alloc(row) = Some(crate::archetype::ArchetypeMember { entity, table_row });
        variant.len += 1;
        Ok(row as u32)
    }
}

impl WorldStorage {
    pub(super) fn remove_member_inner(storage: &mut WorldStorage, id: ArchetypeId, row: u32) {
        let Some(table) = storage.archetypes.get_mut(usize::from(id.table)) else {
            return;
        };
        let Some(variant) = table.variants.get_mut(usize::from(id.variant)) else {
            return;
        };
        if let Some(slot) = variant.members.get_mut(row as usize) {
            *slot = None;
        }
    }
}

#[cfg(feature = "rc")]
impl WorldStorage {
    pub(super) fn sparse_inner(
        &self,
        component: pi_ecs_types::component::ComponentId,
    ) -> Option<&SparseSetStorage> {
        self.sparse.get(component.0 as usize)?.as_ref().map(|slot| {
            // SAFETY: 共享存储借用排除安全维护入口；运行内部调用须遵守原授权。
            unsafe { &*slot.get() }
        })
    }
    pub(super) fn table_maintenance_inner(
        &mut self,
        table: pi_ecs_types::entity::TableId,
    ) -> Option<super::TableMaintenance<'_>> {
        self.tables
            .get_mut(usize::from(table))
            .map(|table| super::TableMaintenance { table })
    }
}

#[cfg(feature = "rc")]
impl WorldStorage {
    pub(super) fn sparse_address_inner(
        &self,
        component: pi_ecs_types::component::ComponentId,
    ) -> Option<std::ptr::NonNull<SparseSetStorage>> {
        let slot = self.sparse.get(component.0 as usize)?.as_ref()?;
        // SAFETY: 地址来自固定 Rc 分配体的 UnsafeCell，不创建容器引用。
        Some(unsafe { std::ptr::NonNull::new_unchecked((*std::rc::Rc::as_ptr(slot)).get()) })
    }
    pub(super) fn prepare_sparse_inner(
        &mut self,
        component: pi_ecs_types::component::ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) -> Result<(), EcsError> {
        let slot = self
            .sparse
            .get(component.0 as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| error(pi_ecs_types::error::ErrorKind::NotFound, "稀疏组件未登记"))?;
        // SAFETY: 独占结构操作期无活结果；从固定分配体短借，不替换对象或经 Rc 取得独占所有权。
        unsafe { &mut *slot.get() }.prepare(entity)
    }
    pub(super) fn insert_sparse_inner(
        &mut self,
        component: pi_ecs_types::component::ComponentId,
        entity: pi_ecs_types::entity::Entity,
        value: crate::ptr::OwningPtr<'_>,
    ) {
        let slot = self
            .sparse
            .get(component.0 as usize)
            .and_then(Option::as_ref)
            .expect("稀疏组件必须已登记");
        // SAFETY: 独占结构操作且原 insert 前提不变。
        unsafe { &mut *slot.get() }.insert(entity, value)
    }
    pub(super) fn remove_sparse_inner(
        &mut self,
        component: pi_ecs_types::component::ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) -> Option<crate::ptr::OwningPtr<'_>> {
        let slot = self.sparse.get(component.0 as usize)?.as_ref()?;
        // SAFETY: 独占结构操作排除相关借用；移出责任绑定本次 self。
        unsafe { &mut *slot.get() }.remove(entity)
    }
    pub(super) unsafe fn insert_sparse_raw_inner(
        &mut self,
        component: pi_ecs_types::component::ComponentId,
        entity: pi_ecs_types::entity::Entity,
        source: *mut u8,
        layout: std::alloc::Layout,
    ) {
        let slot = self
            .sparse
            .get(component.0 as usize)
            .and_then(Option::as_ref)
            .expect("稀疏组件必须已登记");
        // SAFETY: 转发原 raw 安装前提；只短借分配体内部对象。
        unsafe { (&mut *slot.get()).insert_raw(entity, source, layout) }
    }
    pub(super) unsafe fn remove_sparse_raw_inner(
        &mut self,
        component: pi_ecs_types::component::ComponentId,
        entity: pi_ecs_types::entity::Entity,
    ) -> Option<*mut u8> {
        let slot = self.sparse.get(component.0 as usize)?.as_ref()?;
        // SAFETY: 转发原责任移出前提；长期根指针不由此临时借用派生。
        unsafe { (&mut *slot.get()).remove_raw(entity) }
    }
}
