//! 稀疏数组映射维护配对芯；rc 路径实现两级目录、紧凑 owner 与点查，非 rc 仍为失败桩。
#[cfg(all(test, feature = "rc"))]
#[test]
fn prepared_sparse_slot_is_invisible_until_in_place_publication() {
    use pi_ecs_types::entity::KeyData;
    use std::{alloc::Layout, mem::MaybeUninit};
    let mut storage = SparseSetStorage::sparse_array(ComponentId(0), Layout::new::<u64>()).unwrap();
    let entity = Entity::from(KeyData::new(1, 1));
    storage.prepare_tracked(entity).unwrap();
    let mut target = storage
        .prepared_value_slot(entity)
        .unwrap()
        .cast::<MaybeUninit<u64>>();
    assert!(storage.get(entity).is_none());
    // SAFETY: this is the unique writer of the prepared u64 slot.
    unsafe {
        target.as_mut().write(317);
        storage.publish_prepared_value(
            entity,
            pi_ecs_types::change_detection::ComponentTicks {
                added: None,
                changed: None,
            },
        );
    }
    assert!(storage.prepared_value_slot(entity).is_none());
    let value = storage.remove(entity).unwrap();
    // SAFETY: the removed slot contains the u64 installed above, with one owner.
    assert_eq!(unsafe { value.read::<u64>() }, 317);
}
#[cfg(feature = "rc")]
impl super::SparseSetStorage {
    /// Returns the raw destination of an already prepared, unpublished slot.
    /// Maintenance may invalidate it; only the original initializer may write it.
    pub fn prepared_value_slot(&mut self, entity: Entity) -> Option<NonNull<u8>> {
        let Self::SparseArray(set) = self;
        let index = prepared_index(set, entity)?;
        if set.owners.members.dense.get(index as usize)?.key != u64::MAX {
            return None;
        }
        value_slot_ptr(set, entity)
    }

    /// Publishes a slot initialized directly in its final allocation.
    ///
    /// # Safety
    /// The matching slot must contain a fully initialized value with exactly one
    /// owner, and there must be no conflicting views of this storage.
    pub unsafe fn publish_prepared_value(
        &mut self,
        entity: Entity,
        ticks: pi_ecs_types::change_detection::ComponentTicks,
    ) {
        self.commit_prepared(entity, None, ticks);
    }

    pub(crate) fn discard_prepared_tail(&mut self, entity: Entity) {
        let Self::SparseArray(set) = self;
        let index = prepared_index(set, entity).expect("缺少待撤销的稀疏预留");
        assert_eq!(set.owners.members.len - 1, index as usize);
        assert_eq!(
            set.owners.members.dense.get(index as usize).unwrap().key,
            u64::MAX
        );
        update_index(set, entity, u32::MAX);
        set.owners.members.len -= 1;
    }

    pub(super) fn prepare_tracked_inner(&mut self, entity: Entity) -> Result<(), EcsError> {
        let Self::SparseArray(set) = self;
        if prepared_index(set, entity).is_some() {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::AlreadyExists,
                "实体已有稀疏准备槽",
            ));
        }
        let dense = set.owners.members.len;
        if dense >= u32::MAX as usize {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "稀疏 owner 下标超出 u32",
            ));
        }
        let domain = domain_bits(&entity) as usize;
        let slot = slot_index(&entity) as usize;
        ensure_domain(
            &mut set.owners.members.indices,
            domain,
            std::alloc::Layout::new::<SparseDenseIndex>(),
            init_dense_index,
        )
        .alloc(slot)
        .map_err(raw_error)?;
        ensure_domain(&mut set.values, domain, set.value_layout, noop_init)
            .alloc(slot)
            .map_err(raw_error)?;
        for ticks in [&mut set.added, &mut set.changed].into_iter().flatten() {
            ensure_domain(
                &mut ticks.storage,
                domain,
                std::alloc::Layout::new::<Option<pi_ecs_types::change_detection::ChangeTick>>(),
                init_tick,
            )
            .alloc(slot)
            .map_err(raw_error)?;
        }
        // 所有可恢复准备错误均在认领前返回；空 owner 明确初始化，最后登记准备下标。
        set.owners.members.dense.alloc(dense).key = u64::MAX;
        update_index(set, entity, dense as u32);
        set.owners.members.len = dense + 1;
        Ok(())
    }
    pub(super) fn ticks_inner(
        &self,
        entity: Entity,
    ) -> Option<pi_ecs_types::change_detection::ComponentTicks> {
        let Self::SparseArray(set) = self;
        live_index(set, entity)?;
        Some(pi_ecs_types::change_detection::ComponentTicks {
            added: set
                .added
                .as_ref()
                .map(|array| read_tick(&array.storage, entity)),
            changed: set
                .changed
                .as_ref()
                .map(|array| read_tick(&array.storage, entity)),
        })
    }
    pub(super) fn insert_tracked_inner(
        &mut self,
        entity: Entity,
        value: OwningPtr<'_>,
        ticks: pi_ecs_types::change_detection::ComponentTicks,
    ) {
        self.commit_prepared(entity, Some(value), ticks);
    }

    fn commit_prepared(
        &mut self,
        entity: Entity,
        value: Option<OwningPtr<'_>>,
        ticks: pi_ecs_types::change_detection::ComponentTicks,
    ) {
        let Self::SparseArray(set) = self;
        assert_eq!(
            set.added.is_some(),
            ticks.added.is_some(),
            "Added tick 形状不匹配"
        );
        assert_eq!(
            set.changed.is_some(),
            ticks.changed.is_some(),
            "Changed tick 形状不匹配"
        );
        let index = prepared_index(set, entity).expect("tracked 安装要求 prepare_tracked");
        assert_eq!(
            set.owners
                .members
                .dense
                .get(index as usize)
                .expect("owner 槽缺失")
                .key,
            u64::MAX,
            "拒绝覆盖活稀疏值"
        );
        let target = value_slot_ptr(set, entity).expect("值槽缺失");
        let added = set
            .added
            .as_ref()
            .map(|array| raw_slot(&array.storage, entity).expect("Added 槽缺失"));
        let changed = set
            .changed
            .as_ref()
            .map(|array| raw_slot(&array.storage, entity).expect("Changed 槽缺失"));
        // 类型/布局由 core 登记校验；OwningPtr 不携带类型。以下不分配、不调用业务代码，owner 最后发布。
        unsafe {
            if let Some(value) = value {
                OwningPtr::move_into_layout(value, target, set.value_layout);
            }
            if let Some(pointer) = added {
                pointer
                    .cast::<Option<pi_ecs_types::change_detection::ChangeTick>>()
                    .write(ticks.added);
            }
            if let Some(pointer) = changed {
                pointer
                    .cast::<Option<pi_ecs_types::change_detection::ChangeTick>>()
                    .write(ticks.changed);
            }
        }
        set_owner(set, index, entity.data().as_ffi());
    }
}
use crate::{
    ptr::{OwningPtr, Ptr, PtrMut},
    sparse_set::{
        RawSparseArray, SparseArray, SparseArrayDomain, SparseArraySparseSet, SparseDenseIndex,
        SparseOwners, SparseSet, SparseSetStorage,
    },
};
use pi_ecs_types::component::ComponentId;
use pi_ecs_types::entity::{domain_bits, slot_index, Entity, Key, KeyData};
use pi_ecs_types::error::EcsError;
use std::ptr::NonNull;
/// 构造空两级稀疏后端；组件编号和布局用于登记及后续值域创建，不创建业务值。
impl SparseSetStorage {
    pub(super) fn sparse_array_inner(
        component: ComponentId,
        layout: std::alloc::Layout,
    ) -> Result<SparseSetStorage, EcsError> {
        Ok(SparseSetStorage::SparseArray(SparseArraySparseSet {
            component,
            value_layout: layout,
            values: RawSparseArray::new(),
            owners: SparseArray {
                storage: RawSparseArray::new(),
                members: SparseOwners::new(),
                marker: std::marker::PhantomData,
            },
            added: None,
            changed: None,
        }))
    }
}

impl<const P: usize> super::RawSparseArray<P> {
    pub(super) fn new_inner() -> Self {
        Self {
            directory: super::SparseArrayDirectory::new(),
        }
    }
}

impl super::SparseOwner {
    pub(super) fn empty_inner() -> Self {
        #[cfg(feature = "rc")]
        {
            Self { key: u64::MAX }
        }
        #[cfg(not(feature = "rc"))]
        {
            Self {
                key: std::sync::atomic::AtomicU64::new(u64::MAX),
            }
        }
    }
}

impl super::SparseDenseIndex {
    pub(super) fn empty_inner() -> Self {
        #[cfg(feature = "rc")]
        {
            Self { index: u32::MAX }
        }
        #[cfg(not(feature = "rc"))]
        {
            Self {
                index: std::sync::atomic::AtomicU32::new(u32::MAX),
            }
        }
    }
}

impl<const P: usize> super::SparseOwners<P> {
    pub(super) fn new_inner() -> Self {
        #[cfg(feature = "rc")]
        {
            Self {
                dense: pi_arr::Arr::new(),
                indices: RawSparseArray::new(),
                len: 0,
            }
        }
        #[cfg(not(feature = "rc"))]
        {
            Self {
                dense: pi_arr::Arr::new(),
                indices: RawSparseArray::new(),
                len: std::sync::atomic::AtomicUsize::new(0),
                pending_removals: super::PendingSparseRemovals::default(),
            }
        }
    }
}

#[cfg(not(feature = "rc"))]
impl super::PendingSparseRemovals {
    pub(super) fn default_inner() -> Self {
        Self {
            indices: Default::default(),
        }
    }
}

#[cfg(not(feature = "rc"))]
impl<const P: usize> super::SparseArrayDirectory<P> {
    pub(super) fn new_inner() -> Self {
        Self {
            pages: pi_buckets::Pages::new(),
        }
    }
}

#[cfg(feature = "rc")]
impl<const P: usize> super::SparseArrayDirectory<P> {
    pub(super) fn new_inner() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}
/// 返回组件编号；不访问目录、不初始化域。
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn component_inner(set: &SparseArraySparseSet<P>) -> ComponentId {
        set.component
    }
}
#[cfg(feature = "rc")]
unsafe fn noop_init(_value: *mut u8) {}
#[cfg(feature = "rc")]
unsafe fn init_tick(value: *mut u8) {
    // SAFETY: 仅用于 Option<ChangeTick> 布局的未初始化槽。
    unsafe {
        value
            .cast::<Option<pi_ecs_types::change_detection::ChangeTick>>()
            .write(None)
    };
}
#[cfg(feature = "rc")]
unsafe fn init_dense_index(value: *mut u8) {
    // SAFETY: RawArr 只在未初始化槽调用本回调，槽布局为 SparseDenseIndex。
    unsafe {
        value
            .cast::<SparseDenseIndex>()
            .write(SparseDenseIndex::empty())
    }
}
#[cfg(feature = "rc")]
fn ensure_domain<'a, const P: usize>(
    array: &'a mut RawSparseArray<P>,
    domain: usize,
    layout: std::alloc::Layout,
    init: unsafe fn(*mut u8),
) -> &'a mut pi_arr::RawArr {
    if array.directory.entries.len() <= domain {
        array
            .directory
            .entries
            .resize_with(domain + 1, SparseArrayDomain::default);
    }
    let entry = &mut array.directory.entries[domain];
    entry
        .values
        .get_or_insert_with(|| unsafe { pi_arr::RawArr::new(layout, init, None) })
}
#[cfg(feature = "rc")]
fn raw_error(error: pi_arr::RawArrError) -> EcsError {
    {
        use pi_ecs_types::error::{ErrorKind, Report};
        let kind = match error {
            pi_arr::RawArrError::InvalidRange => ErrorKind::InvalidInput,
            pi_arr::RawArrError::LayoutOverflow
            | pi_arr::RawArrError::AllocationFailed
            | pi_arr::RawArrError::IndexOverflow
            | pi_arr::RawArrError::IndexOutOfBounds => ErrorKind::ResourceExhausted,
        };
        Report::new(kind)
            .attach_opaque(error)
            .attach("pi_arr 稀疏存储准备失败")
    }
}
fn raw_slot<const P: usize>(array: &RawSparseArray<P>, entity: Entity) -> Option<*mut u8> {
    #[cfg(feature = "rc")]
    let domain = array.directory.entries.get(domain_bits(&entity) as usize)?;
    #[cfg(not(feature = "rc"))]
    let domain = array.directory.pages.get(domain_bits(&entity) as usize)?;
    domain.values.as_ref()?.get(slot_index(&entity) as usize)
}
fn tick_slot<const P: usize>(
    array: &RawSparseArray<P>,
    entity: Entity,
) -> *mut Option<pi_ecs_types::change_detection::ChangeTick> {
    let pointer = raw_slot(array, entity)
        .expect("活稀疏 tick 槽缺失")
        .cast::<Option<pi_ecs_types::change_detection::ChangeTick>>();
    // SAFETY: 本数组由 init_tick 初始化，授权排除同槽冲突访问，仅检查该槽。
    assert!(unsafe { (*pointer).is_some() }, "活稀疏 tick 未初始化");
    pointer
}
fn read_tick<const P: usize>(
    array: &RawSparseArray<P>,
    entity: Entity,
) -> pi_ecs_types::change_detection::ChangeTick {
    let pointer = raw_slot(array, entity).expect("活稀疏 tick 槽缺失");
    // SAFETY: 本数组由 init_tick 初始化，调用方排除本槽写入，仅短读单槽。
    unsafe {
        (*pointer.cast::<Option<pi_ecs_types::change_detection::ChangeTick>>())
            .expect("活稀疏 tick 未初始化")
    }
}
#[cfg(feature = "rc")]
fn value_slot_ptr<const P: usize>(
    set: &SparseArraySparseSet<P>,
    entity: Entity,
) -> Option<NonNull<u8>> {
    let domain = domain_bits(&entity) as usize;
    let slot = slot_index(&entity) as usize;
    let entry = set.values.directory.entries.get(domain)?;
    let values = entry.values.as_ref()?;
    let pointer = values.get(slot)?;
    // SAFETY: RawArr::get 只返回已初始化槽的非空地址。
    Some(unsafe { NonNull::new_unchecked(pointer) })
}
#[cfg(feature = "rc")]
fn prepared_index<const P: usize>(set: &SparseArraySparseSet<P>, entity: Entity) -> Option<u32> {
    let domain = domain_bits(&entity) as usize;
    let slot = slot_index(&entity) as usize;
    let entry = set.owners.members.indices.directory.entries.get(domain)?;
    let indices = entry.values.as_ref()?;
    let pointer = indices.get(slot)? as *const SparseDenseIndex;
    // SAFETY: 索引槽只在 prepare 后写入，读取期内独占或共享借用由调用方约束。
    let index = unsafe { (*pointer).index };
    (index != u32::MAX).then_some(index)
}
#[cfg(feature = "rc")]
fn live_index<const P: usize>(set: &SparseArraySparseSet<P>, entity: Entity) -> Option<u32> {
    let index = prepared_index(set, entity)?;
    let owner = set.owners.members.dense.get(index as usize)?;
    (owner.key == entity.data().as_ffi()).then_some(index)
}
#[cfg(feature = "rc")]
fn update_index<const P: usize>(set: &mut SparseArraySparseSet<P>, entity: Entity, index: u32) {
    let domain = domain_bits(&entity) as usize;
    let slot = slot_index(&entity) as usize;
    let Some(entry) = set.owners.members.indices.directory.entries.get_mut(domain) else {
        panic!("稀疏索引域缺失");
    };
    let Some(indices) = entry.values.as_mut() else {
        panic!("稀疏索引数组缺失");
    };
    let pointer = indices.get(slot).expect("移动实体必须有已准备稀疏索引") as *mut SparseDenseIndex;
    // SAFETY: 该槽属于本实体，移动 owner 时独占访问。
    unsafe { (*pointer).index = index }
}
#[cfg(feature = "rc")]
fn set_owner<const P: usize>(set: &mut SparseArraySparseSet<P>, index: u32, key: u64) {
    let owner = set
        .owners
        .members
        .dense
        .get_mut(index as usize)
        .expect("稀疏 owner 下标必须已分配");
    owner.key = key;
}
#[cfg(feature = "rc")]
fn take_value<const P: usize>(
    set: &mut SparseArraySparseSet<P>,
    entity: Entity,
) -> Option<*mut u8> {
    let index = live_index(set, entity)?;
    let pointer = value_slot_ptr(set, entity)?.as_ptr();
    let domain = domain_bits(&entity) as usize;
    let slot = slot_index(&entity) as usize;
    {
        let entry = set
            .owners
            .members
            .indices
            .directory
            .entries
            .get_mut(domain)?;
        let indices = entry.values.as_mut()?;
        let index_pointer = indices.get(slot)? as *mut SparseDenseIndex;
        // SAFETY: 删除先失效稀疏下标，再移动 owner；该槽由本实体独占。
        unsafe { (*index_pointer).index = u32::MAX }
    }
    let last = set.owners.members.len.checked_sub(1)?;
    if index as usize != last {
        let moved_key = set
            .owners
            .members
            .dense
            .get(last)
            .expect("稀疏 owner 尾部必须已分配")
            .key;
        set_owner(set, index, moved_key);
        set_owner(set, last as u32, u64::MAX);
        if moved_key != u64::MAX {
            let moved = Entity::from(KeyData::from_ffi(moved_key));
            update_index(set, moved, index);
        } else {
            // null 尾项仍由某个 prepare 独占，不能当退休洞丢弃。
            // 现有表示不保存其 Entity；扫描已初始化的映射槽，按唯一尾下标修复。
            // 每次 prepare 认领唯一 dense 下标，删除已撤销自身映射，因此恰好命中一次。
            // 不分配、不读业务值/tick；只有未发布尾项需要此线性慢路径。
            let mut repaired = false;
            for entry in &set.owners.members.indices.directory.entries {
                if let Some(indices) = &entry.values {
                    for (_, pointer) in indices.slice(0..indices.contiguous_len()) {
                        let pointer = pointer.cast::<SparseDenseIndex>();
                        // SAFETY: 映射槽经 init_dense_index 初始化，当前独占整个 set，
                        // 扫描期间不增长数组，且不构造映射槽引用。
                        unsafe {
                            if (*pointer).index == last as u32 {
                                assert!(!repaired, "准备尾项具有重复稀疏映射");
                                (*pointer).index = index;
                                repaired = true;
                            }
                        }
                    }
                }
            }
            assert!(repaired, "准备尾项缺少稀疏映射");
        }
    } else {
        set_owner(set, index, u64::MAX);
    }
    set.owners.members.len = last;
    Some(pointer)
}
/// 按完整键点查；域由键代际低位解码，槽为键索引。
#[cfg(feature = "rc")]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn get_inner(set: &SparseArraySparseSet<P>, entity: Entity) -> Option<Ptr<'_>> {
        live_index(set, entity)?;
        let pointer = value_slot_ptr(set, entity)?;
        // SAFETY: owner 已匹配完整 Entity，值槽已初始化且借用由 set 约束。
        Some(unsafe { Ptr::new(pointer) })
    }
}
#[cfg(not(feature = "rc"))]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn get_inner(_set: &SparseArraySparseSet<P>, _entity: Entity) -> Option<Ptr<'_>> {
        todo!("编号：T118，尚未实现， 非 rc 稀疏点查")
    }
}
/// 按完整键取得独占引用；调用方负责同位置无冲突写授权。
#[cfg(feature = "rc")]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn get_mut_inner(
        set: &mut SparseArraySparseSet<P>,
        entity: Entity,
    ) -> Option<PtrMut<'_>> {
        live_index(set, entity)?;
        let pointer = value_slot_ptr(set, entity)?;
        // SAFETY: &mut set 保证独占，owner 已核对完整 Entity。
        Some(unsafe { PtrMut::new(pointer) })
    }
}
#[cfg(not(feature = "rc"))]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn get_mut_inner(
        _set: &mut SparseArraySparseSet<P>,
        _entity: Entity,
    ) -> Option<PtrMut<'_>> {
        todo!("编号：T119，尚未实现， 非 rc 稀疏独占点查")
    }
}
/// 同时取得独占值与已启用 tick 槽；未启用对应 tick 为 None。
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn get_mut_with_ticks_inner(
        set: &mut SparseArraySparseSet<P>,
        entity: Entity,
    ) -> Option<(
        PtrMut<'_>,
        pi_ecs_types::change_detection::ComponentTicksMut<'_>,
    )> {
        let pointer = SparseSet::get(set, entity)?.as_non_null();
        let added = set
            .added
            .as_ref()
            .map(|array| tick_slot(&array.storage, entity));
        let changed = set
            .changed
            .as_ref()
            .map(|array| tick_slot(&array.storage, entity));
        // SAFETY: &mut set 排除所有冲突借用，各字段互不重叠；先完成全部槽验证。
        unsafe {
            Some((
                PtrMut::new(pointer),
                pi_ecs_types::change_detection::ComponentTicksMut {
                    added: added.map(|pointer| &mut *pointer),
                    changed: changed.map(|pointer| &mut *pointer),
                },
            ))
        }
    }
}
/// 预留目标槽；普通容量失败在认领前返回。
#[cfg(feature = "rc")]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn prepare_inner(
        set: &mut SparseArraySparseSet<P>,
        entity: Entity,
    ) -> Result<(), EcsError> {
        if set.added.is_some() || set.changed.is_some() {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "启用追踪的稀疏安装必须使用 prepare_tracked/insert_tracked",
            ));
        }
        if prepared_index(set, entity).is_some() {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::AlreadyExists,
                "实体已有稀疏准备槽",
            ));
        }
        let domain = domain_bits(&entity) as usize;
        let slot = slot_index(&entity) as usize;
        let index_pointer = {
            let indices = ensure_domain(
                &mut set.owners.members.indices,
                domain,
                std::alloc::Layout::new::<SparseDenseIndex>(),
                init_dense_index,
            );
            indices.alloc(slot).map_err(raw_error)?
        };
        let value_layout = set.value_layout;
        let values = ensure_domain(&mut set.values, domain, value_layout, noop_init);
        values.alloc(slot).map_err(raw_error)?;
        let dense = set.owners.members.len;
        if dense >= u32::MAX as usize {
            return Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "稀疏 owner 下标超出 u32",
            ));
        }
        set.owners.members.dense.alloc(dense);
        set.owners.members.len = dense + 1;
        // SAFETY: 索引槽刚分配且当前为 u32::MAX；owner 尚未发布，读取仍不可见。
        unsafe {
            index_pointer
                .cast::<SparseDenseIndex>()
                .write(SparseDenseIndex {
                    index: dense as u32,
                })
        }
        Ok(())
    }
}
#[cfg(not(feature = "rc"))]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn prepare_inner(
        _set: &mut SparseArraySparseSet<P>,
        _entity: Entity,
    ) -> Result<(), EcsError> {
        todo!("编号：T120，尚未实现， 非 rc 稀疏预留")
    }
}
/// 安装已校验类型的值；不隐式替换现有值。
#[cfg(feature = "rc")]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn insert_inner(
        set: &mut SparseArraySparseSet<P>,
        entity: Entity,
        value: OwningPtr<'_>,
    ) {
        let index = prepared_index(set, entity).expect("稀疏 insert 要求 prepare");
        let owner = set
            .owners
            .members
            .dense
            .get(index as usize)
            .expect("稀疏 owner 下标必须已分配");
        assert!(owner.key == u64::MAX, "稀疏插入拒绝覆盖现有值");
        let pointer = value_slot_ptr(set, entity).expect("稀疏值槽必须已准备");
        let source = value.as_non_null().as_ptr();
        if set.value_layout.size() != 0 {
            // SAFETY: 目标槽已准备且为空，source 是唯一拥有值；按登记布局移动字节。
            unsafe {
                std::ptr::copy_nonoverlapping(source, pointer.as_ptr(), set.value_layout.size())
            };
        }
        set_owner(set, index, entity.data().as_ffi());
    }
}
#[cfg(not(feature = "rc"))]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn insert_inner(
        _set: &mut SparseArraySparseSet<P>,
        _entity: Entity,
        _value: OwningPtr<'_>,
    ) {
        todo!("编号：T121，尚未实现， 非 rc 稀疏安装")
    }
}
/// 把原始字节值移入已准备槽；不析构 source。
#[cfg(feature = "rc")]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) unsafe fn insert_raw_inner(
        set: &mut SparseArraySparseSet<P>,
        entity: Entity,
        source: *mut u8,
        layout: std::alloc::Layout,
    ) {
        assert!(layout == set.value_layout, "稀疏原始值布局不一致");
        let index = prepared_index(set, entity).expect("稀疏 insert_raw 要求 prepare");
        let owner = set
            .owners
            .members
            .dense
            .get(index as usize)
            .expect("稀疏 owner 下标必须已分配");
        assert!(owner.key == u64::MAX, "稀疏插入拒绝覆盖现有值");
        let pointer = value_slot_ptr(set, entity).expect("稀疏值槽必须已准备");
        if layout.size() != 0 {
            // SAFETY: 目标槽已准备且为空，source 由调用方保证唯一且可移动。
            unsafe { std::ptr::copy_nonoverlapping(source, pointer.as_ptr(), layout.size()) };
        }
        set_owner(set, index, entity.data().as_ffi());
    }
}
#[cfg(not(feature = "rc"))]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) unsafe fn insert_raw_inner(
        _set: &mut SparseArraySparseSet<P>,
        _entity: Entity,
        _source: *mut u8,
        _layout: std::alloc::Layout,
    ) {
        todo!("编号：T122，尚未实现， 非 rc 稀疏原始安装")
    }
}
/// 移出拥有值；不析构、不移动其他实体。
#[cfg(feature = "rc")]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn remove_inner(
        set: &mut SparseArraySparseSet<P>,
        entity: Entity,
    ) -> Option<OwningPtr<'_>> {
        let pointer = take_value(set, entity)?;
        // SAFETY: 值仍初始化，且槽已从可见映射移除；调用方取得唯一移出责任。
        Some(unsafe { OwningPtr::new(NonNull::new_unchecked(pointer)) })
    }
}
#[cfg(not(feature = "rc"))]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) fn remove_inner(
        _set: &mut SparseArraySparseSet<P>,
        _entity: Entity,
    ) -> Option<OwningPtr<'_>> {
        todo!("编号：T123，尚未实现， 非 rc 稀疏移出")
    }
}
/// 移出原始地址；不析构，供 core 按登记析构能力处理。
#[cfg(feature = "rc")]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) unsafe fn remove_raw_inner(
        set: &mut SparseArraySparseSet<P>,
        entity: Entity,
    ) -> Option<*mut u8> {
        take_value(set, entity)
    }
}
#[cfg(not(feature = "rc"))]
impl<const P: usize> SparseArraySparseSet<P> {
    pub(super) unsafe fn remove_raw_inner(
        _set: &mut SparseArraySparseSet<P>,
        _entity: Entity,
    ) -> Option<*mut u8> {
        todo!("编号：T124，尚未实现， 非 rc 稀疏原始移出")
    }
}
/// 枚举分派：当前只有两级 SparseArray 一个变体。
impl SparseSetStorage {
    pub(super) fn storage_component_inner(storage: &SparseSetStorage) -> ComponentId {
        match storage {
            SparseSetStorage::SparseArray(set) => SparseSet::component(set),
        }
    }
}
/// 枚举分派：点查；调用前提同 SparseSet::get。
impl SparseSetStorage {
    pub(super) fn storage_get_inner(storage: &SparseSetStorage, entity: Entity) -> Option<Ptr<'_>> {
        match storage {
            SparseSetStorage::SparseArray(set) => SparseSet::get(set, entity),
        }
    }
}
/// 枚举分派：独占点查；调用前提同 SparseSet::get_mut。
impl SparseSetStorage {
    pub(super) fn storage_get_mut_inner(
        storage: &mut SparseSetStorage,
        entity: Entity,
    ) -> Option<PtrMut<'_>> {
        match storage {
            SparseSetStorage::SparseArray(set) => SparseSet::get_mut(set, entity),
        }
    }
}
/// 枚举分派：预留；调用前提同 SparseSet::prepare。
impl SparseSetStorage {
    pub(super) fn storage_prepare_inner(
        storage: &mut SparseSetStorage,
        entity: Entity,
    ) -> Result<(), EcsError> {
        match storage {
            SparseSetStorage::SparseArray(set) => SparseSet::prepare(set, entity),
        }
    }
}
/// 枚举分派：安装；调用前提同 SparseSet::insert。
impl SparseSetStorage {
    pub(super) fn storage_insert_inner(
        storage: &mut SparseSetStorage,
        entity: Entity,
        value: OwningPtr<'_>,
    ) {
        match storage {
            SparseSetStorage::SparseArray(set) => SparseSet::insert(set, entity, value),
        }
    }
}
/// 枚举分派：移出；调用前提同 SparseSet::remove。
impl SparseSetStorage {
    pub(super) fn storage_remove_inner(
        storage: &mut SparseSetStorage,
        entity: Entity,
    ) -> Option<OwningPtr<'_>> {
        match storage {
            SparseSetStorage::SparseArray(set) => SparseSet::remove(set, entity),
        }
    }
}
/// 枚举分派：原始安装；不析构 source，布局由调用方按登记保证。
impl SparseSetStorage {
    pub(super) unsafe fn storage_insert_raw_inner(
        storage: &mut SparseSetStorage,
        entity: Entity,
        source: *mut u8,
        layout: std::alloc::Layout,
    ) {
        match storage {
            SparseSetStorage::SparseArray(set) => unsafe {
                SparseArraySparseSet::insert_raw_inner(set, entity, source, layout)
            },
        }
    }
}
/// 枚举分派：原始移出；不析构返回值。
impl SparseSetStorage {
    pub(super) unsafe fn storage_remove_raw_inner(
        storage: &mut SparseSetStorage,
        entity: Entity,
    ) -> Option<*mut u8> {
        match storage {
            SparseSetStorage::SparseArray(set) => unsafe {
                SparseArraySparseSet::remove_raw_inner(set, entity)
            },
        }
    }
}
/// 非 rc 在独占收束点整理退休洞；rc 删除已即时填洞，本函数为空操作。
impl<const P: usize> SparseOwners<P> {
    pub(super) fn compact_inner(_owners: &mut SparseOwners<P>) {
        #[cfg(not(feature = "rc"))]
        todo!("编号：T125，尚未实现， 非 rc 延迟映射整理")
    }
}

#[cfg(all(test, feature = "rc"))]
mod tests {
    use super::*;
    use pi_ecs_types::entity::{Entity, KeyData};

    fn entity(domain: u32, slot: u32) -> Entity {
        Entity::from(KeyData::new((domain << 20) | slot, 1))
    }

    fn insert(set: &mut SparseSetStorage, entity: Entity, value: u32) {
        let mut value = value;
        // SAFETY: value 是唯一拥有者，测试随后不再读取原变量。
        unsafe {
            set.insert_raw(
                entity,
                &mut value as *mut u32 as *mut u8,
                std::alloc::Layout::new::<u32>(),
            )
        };
    }

    fn tick(value: u64) -> pi_ecs_types::change_detection::ChangeTick {
        pi_ecs_types::change_detection::ChangeTick::new(std::num::NonZeroU64::new(value).unwrap())
    }
    fn tracked() -> SparseSetStorage {
        let mut storage =
            SparseSetStorage::sparse_array(ComponentId(0), std::alloc::Layout::new::<u32>())
                .unwrap();
        let SparseSetStorage::SparseArray(set) = &mut storage;
        set.added = Some(SparseArray {
            storage: RawSparseArray::new(),
            members: (),
            marker: std::marker::PhantomData,
        });
        set.changed = Some(SparseArray {
            storage: RawSparseArray::new(),
            members: (),
            marker: std::marker::PhantomData,
        });
        storage
    }
    #[test]
    fn frozen_sparse_view_checks_generation_and_keeps_distinct_writes() {
        use pi_ecs_types::change_detection::ComponentTicks;
        let mut storage = tracked();
        for slot in [2, 5] {
            let key = entity(1, slot);
            storage.prepare_tracked(key).unwrap();
            let mut value = slot;
            unsafe {
                storage.insert_tracked(
                    key,
                    OwningPtr::from_raw(NonNull::from(&mut value).cast()),
                    ComponentTicks {
                        added: Some(tick(4)),
                        changed: Some(tick(8)),
                    },
                );
            }
        }
        unsafe {
            let first = storage.bind_view();
            let second = storage.bind_view();
            assert_eq!(*first.read(entity(1, 2)).unwrap().as_ref::<u32>(), 2);
            assert!(first
                .read(Entity::from(KeyData::new((1 << 20) | 2, 2)))
                .is_none());
            assert!(first.read(entity(1, 3)).is_none());
            assert_eq!(first.ticks(entity(1, 2)).unwrap().added, Some(tick(4)));
            let (mut a, at) = first.write(entity(1, 2)).unwrap();
            let a = a.as_mut::<u32>();
            let (mut b, bt) = second.write(entity(1, 5)).unwrap();
            let b = b.as_mut::<u32>();
            *a = 23;
            *b = 57;
            *at.changed.unwrap() = Some(tick(12));
            *bt.changed.unwrap() = Some(tick(15));
            assert_eq!((*a, *b), (23, 57));
        }
        let SparseSetStorage::SparseArray(set) = &mut storage;
        let (mut value, ticks) = set.get_mut_with_ticks(entity(1, 2)).unwrap();
        unsafe {
            assert_eq!(*value.as_mut::<u32>(), 23);
        }
        assert_eq!(*ticks.changed.unwrap(), Some(tick(12)));
    }

    #[test]
    fn tracked_install_rejects_bad_shape_before_moving_value() {
        use pi_ecs_types::change_detection::ComponentTicks;
        let mut storage = tracked();
        let key = entity(0, 2);
        storage.prepare_tracked(key).unwrap();
        let mut value = 66u32;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            storage.insert_tracked(
                key,
                OwningPtr::from_raw(NonNull::from(&mut value).cast()),
                ComponentTicks {
                    added: Some(tick(3)),
                    changed: None,
                },
            );
        }));
        assert!(result.is_err());
        assert!(storage.get(key).is_none());
        assert_eq!(value, 66);
        unsafe {
            storage.insert_tracked(
                key,
                OwningPtr::from_raw(NonNull::from(&mut value).cast()),
                ComponentTicks {
                    added: Some(tick(3)),
                    changed: Some(tick(3)),
                },
            );
        }
        assert_eq!(storage.ticks(key).unwrap().changed, Some(tick(3)));
        assert!(storage.prepare_tracked(key).is_err());
        assert_eq!(storage.ticks(key).unwrap().added, Some(tick(3)));
    }

    #[test]
    fn sparse_ticks_support_each_tracking_shape() {
        use pi_ecs_types::change_detection::ComponentTicks;
        for (added, changed) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut storage = tracked();
            let SparseSetStorage::SparseArray(set) = &mut storage;
            if !added {
                set.added = None;
            }
            if !changed {
                set.changed = None;
            }
            let key = entity(0, 1);
            storage.prepare_tracked(key).unwrap();
            let mut value = 1u32;
            unsafe {
                storage.insert_tracked(
                    key,
                    OwningPtr::from_raw(NonNull::from(&mut value).cast()),
                    ComponentTicks {
                        added: added.then_some(tick(6)),
                        changed: changed.then_some(tick(6)),
                    },
                );
            }
            unsafe {
                let view = storage.bind_view();
                let snapshot = view.ticks(key).unwrap();
                assert_eq!(snapshot.added, added.then_some(tick(6)));
                assert_eq!(snapshot.changed, changed.then_some(tick(6)));
                let (_, slots) = view.write(key).unwrap();
                assert_eq!(slots.added.is_some(), added);
                assert_eq!(slots.changed.is_some(), changed);
            }
        }
    }

    #[test]
    fn tracked_install_preserves_ticks_and_transfers_ownership() {
        use pi_ecs_types::change_detection::ComponentTicks;
        let key = entity(1, 2);
        let mut source = tracked();
        source.prepare_tracked(key).unwrap();
        assert!(source.ticks(key).is_none());
        let mut value = 91u32;
        unsafe {
            source.insert_tracked(
                key,
                OwningPtr::from_raw(NonNull::from(&mut value).cast()),
                ComponentTicks {
                    added: Some(tick(4)),
                    changed: Some(tick(8)),
                },
            );
        }
        let snapshot = source.ticks(key).unwrap();
        assert_eq!(snapshot.added, Some(tick(4)));
        assert_eq!(snapshot.changed, Some(tick(8)));
        let mut target = tracked();
        target.prepare_tracked(key).unwrap();
        target.insert_tracked(key, source.remove(key).unwrap(), snapshot);
        assert!(source.get(key).is_none());
        assert_eq!(target.ticks(key).unwrap().changed, Some(tick(8)));
        unsafe {
            assert_eq!(*target.get(key).unwrap().as_ref::<u32>(), 91);
        }
    }

    #[test]
    fn tracked_prepare_keeps_entity_unpublished_and_failure_retryable() {
        let mut set =
            SparseSetStorage::sparse_array(ComponentId(0), std::alloc::Layout::new::<u32>())
                .unwrap();
        let key = entity(2, 3);
        set.prepare_tracked(key).unwrap();
        assert!(set.get(key).is_none());
        let SparseSetStorage::SparseArray(inner) = &set;
        assert_eq!(inner.owners.members.dense.get(0).unwrap().key, u64::MAX);
        let mut bad = SparseSetStorage::sparse_array(
            ComponentId(0),
            std::alloc::Layout::from_size_align(isize::MAX as usize, 1).unwrap(),
        )
        .unwrap();
        assert!(bad.prepare_tracked(entity(0, 1)).is_err());
        assert!(bad.get(entity(0, 1)).is_none());
        let SparseSetStorage::SparseArray(inner) = &bad;
        assert_eq!(inner.owners.members.len, 0);
        assert!(prepared_index(inner, entity(0, 1)).is_none());
    }

    // Seam: SparseSetStorage 的 prepare/insert/get/remove 与 tracked 对应公开入口。
    // tracked 开关仅在构造夹具设置；不读取内部映射作为验收依据。
    fn prepared_tail_removal_preserves_ownership(tracking: bool) {
        use pi_ecs_types::change_detection::ComponentTicks;
        use std::{cell::Cell, mem::ManuallyDrop, rc::Rc};

        struct Value {
            id: u32,
            drops: Rc<Cell<[usize; 3]>>,
        }
        impl Drop for Value {
            fn drop(&mut self) {
                let mut counts = self.drops.get();
                counts[self.id as usize] += 1;
                self.drops.set(counts);
            }
        }
        fn install(set: &mut SparseSetStorage, key: Entity, value: Value, tracking: bool) {
            let mut value = ManuallyDrop::new(value);
            // SAFETY: 唯一源值按已登记布局移入；源不再读取或析构。
            let pointer = unsafe { OwningPtr::from_raw(NonNull::from(&mut *value).cast()) };
            if tracking {
                set.insert_tracked(
                    key,
                    pointer,
                    ComponentTicks {
                        added: Some(tick(4)),
                        changed: Some(tick(8)),
                    },
                );
            } else {
                set.insert(key, pointer);
            }
        }
        let drops = Rc::new(Cell::new([0; 3]));
        let mut storage = if tracking {
            let mut storage = tracked();
            let SparseSetStorage::SparseArray(set) = &mut storage;
            // 空夹具尚未分配值槽，按本测试真实类型登记布局。
            set.value_layout = std::alloc::Layout::new::<Value>();
            storage
        } else {
            SparseSetStorage::sparse_array(ComponentId(0), std::alloc::Layout::new::<Value>())
                .unwrap()
        };
        let [a, b, c] = [entity(0, 2), entity(1, 5), entity(2, 7)];
        let prepare = |set: &mut SparseSetStorage, key| {
            if tracking {
                set.prepare_tracked(key)
            } else {
                set.prepare(key)
            }
            .unwrap();
        };
        prepare(&mut storage, a);
        install(
            &mut storage,
            a,
            Value {
                id: 0,
                drops: drops.clone(),
            },
            tracking,
        );
        prepare(&mut storage, b);
        assert!(storage.get(b).is_none());
        assert!(storage.remove(b).is_none());
        // SAFETY: 已移出唯一拥有值，按真实类型消费后才继续结构操作。
        unsafe { storage.remove(a).unwrap().drop_as::<Value>() };
        assert_eq!(drops.get(), [1, 0, 0]);
        install(
            &mut storage,
            b,
            Value {
                id: 1,
                drops: drops.clone(),
            },
            tracking,
        );
        prepare(&mut storage, c);
        assert!(storage.get(c).is_none());
        install(
            &mut storage,
            c,
            Value {
                id: 2,
                drops: drops.clone(),
            },
            tracking,
        );
        for (key, id) in [(b, 1), (c, 2)] {
            // SAFETY: 按登记类型读取短借用，下一次结构操作前结束。
            unsafe {
                assert_eq!(
                    storage
                        .get(key)
                        .expect("已安装身份必须仍可见")
                        .as_ref::<Value>()
                        .id,
                    id
                )
            };
            let stale = Entity::from(KeyData::new(key.data().as_ffi() as u32, 2));
            assert!(storage.get(stale).is_none());
            assert!(storage.remove(stale).is_none());
            if tracking {
                let ticks = storage.ticks(key).unwrap();
                assert_eq!(ticks.added, Some(tick(4)));
                assert_eq!(ticks.changed, Some(tick(8)));
            }
        }
        assert!(storage.get(a).is_none());
        unsafe { storage.remove(b).unwrap().drop_as::<Value>() };
        assert!(storage.remove(b).is_none());
        assert_eq!(drops.get(), [1, 1, 0]);
        unsafe { assert_eq!(storage.get(c).unwrap().as_ref::<Value>().id, 2) };
        unsafe { storage.remove(c).unwrap().drop_as::<Value>() };
        assert!(storage.remove(c).is_none());
        drop(storage);
        assert_eq!(drops.get(), [1, 1, 1]);
    }

    #[test]
    fn ordinary_prepared_tail_survives_removal_and_next_install() {
        prepared_tail_removal_preserves_ownership(false);
    }

    #[test]
    fn tracked_prepared_tail_survives_removal_and_next_install() {
        prepared_tail_removal_preserves_ownership(true);
    }

    #[test]
    fn sparse_rc_insert_get_mutate_and_remove() {
        let mut set =
            SparseSetStorage::sparse_array(ComponentId(0), std::alloc::Layout::new::<u32>())
                .unwrap();
        let first = entity(2, 3);
        let second = entity(2, 7);
        set.prepare(first).unwrap();
        set.prepare(second).unwrap();
        insert(&mut set, first, 11);
        insert(&mut set, second, 22);
        // SAFETY: 测试独占 set，指针来自已发布活值。
        unsafe {
            assert_eq!(*set.get(first).unwrap().as_ref::<u32>(), 11);
            *set.get_mut(second).unwrap().as_mut::<u32>() = 33;
        }
        // 移除 first 会把 second 的 owner 前移，必须同步修复稀疏下标。
        let removed = unsafe { set.remove_raw(first) }.unwrap();
        // SAFETY: 返回地址来自仍 initialization 的值，且已在测试中独占。
        unsafe { assert_eq!(*removed.cast::<u32>(), 11) };
        assert!(set.get(first).is_none());
        // SAFETY: second 仍是活值。
        unsafe { assert_eq!(*set.get(second).unwrap().as_ref::<u32>(), 33) };
        set.prepare(first).unwrap();
        insert(&mut set, first, 44);
        // SAFETY: first 重新插入后是活值。
        unsafe { assert_eq!(*set.get(first).unwrap().as_ref::<u32>(), 44) };
    }
}

impl SparseSetStorage {
    pub(super) fn bind_view_inner(&self) -> super::SparseView<'_> {
        let Self::SparseArray(set) = self;
        super::SparseView {
            component: set.component,
            values: &set.values,
            owners: &set.owners.members,
            added: set.added.as_ref().map(|array| &array.storage),
            changed: set.changed.as_ref().map(|array| &array.storage),
        }
    }
}
impl<'a> super::SparseView<'a> {
    fn contains(&self, entity: Entity) -> bool {
        let Some(pointer) = raw_slot(&self.owners.indices, entity) else {
            return false;
        };
        // SAFETY: 映射槽已初始化，冻结期间只读；非 RC 使用同一原子表示。
        #[cfg(feature = "rc")]
        let index = unsafe { (*pointer.cast::<SparseDenseIndex>()).index };
        #[cfg(not(feature = "rc"))]
        let index = unsafe {
            (*pointer.cast::<SparseDenseIndex>())
                .index
                .load(std::sync::atomic::Ordering::Acquire)
        };
        if index == u32::MAX {
            return false;
        }
        let owner = self
            .owners
            .dense
            .get(index as usize)
            .expect("稀疏 owner 下标损坏");
        #[cfg(feature = "rc")]
        let key = owner.key;
        #[cfg(not(feature = "rc"))]
        let key = owner.key.load(std::sync::atomic::Ordering::Acquire);
        key == entity.data().as_ffi()
    }
    pub(super) fn read_inner(&self, entity: Entity) -> Option<Ptr<'a>> {
        if !self.contains(entity) {
            return None;
        }
        let pointer = raw_slot(self.values, entity).expect("已发布稀疏值槽缺失");
        // SAFETY: 完整身份匹配且授权保活该单槽，不借整个值数组。
        Some(unsafe { Ptr::new(NonNull::new(pointer).expect("RawArr 非空槽地址")) })
    }
    pub(super) fn write_inner(
        &self,
        entity: Entity,
    ) -> Option<(
        PtrMut<'a>,
        pi_ecs_types::change_detection::ComponentTicksMut<'a>,
    )> {
        if !self.contains(entity) {
            return None;
        }
        let pointer = raw_slot(self.values, entity).expect("已发布稀疏值槽缺失");
        let added = self.added.map(|array| tick_slot(array, entity));
        let changed = self.changed.map(|array| tick_slot(array, entity));
        // SAFETY: 调用方保证仅此实体各单槽唯一；不重借整数组或存储对象。
        unsafe {
            Some((
                PtrMut::new(NonNull::new(pointer).expect("RawArr 非空槽地址")),
                pi_ecs_types::change_detection::ComponentTicksMut {
                    added: added.map(|pointer| &mut *pointer),
                    changed: changed.map(|pointer| &mut *pointer),
                },
            ))
        }
    }
}

impl super::SparseView<'_> {
    pub(super) fn ticks_inner(
        &self,
        entity: Entity,
    ) -> Option<pi_ecs_types::change_detection::ComponentTicks> {
        if !self.contains(entity) {
            return None;
        }
        Some(pi_ecs_types::change_detection::ComponentTicks {
            added: self.added.map(|array| read_tick(array, entity)),
            changed: self.changed.map(|array| read_tick(array, entity)),
        })
    }
}
