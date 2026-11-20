#[path = "entity_store_inner.rs"]
mod entity_store_inner;
/// # 用途
/// rc 身份表的局部内部可变单元；不共享组件存储，不新增实体控制块。
/// Ref/RefMut 不得逃逸或跨业务析构/用户回调，只返回位置或身份副本。
#[cfg(feature = "rc")]
pub(crate) type EntityIdentityStore = std::cell::RefCell<EntityStore>;
/// # 用途
/// 非 rc 保留原子身份后端，不引入 RefCell 或改变并发协议。
#[cfg(not(feature = "rc"))]
pub(crate) type EntityIdentityStore = EntityStore;
// 核心固定身份映射，不再接受插件替换实现。
use crate::{
    entity::{AllocationDomain, Entity, EntityLocation},
    error::EcsError,
};
/// rc 身份记录：位置与 strong/weak 计数放在同一槽值，避免第二张表。
#[cfg(feature = "rc")]
pub(crate) struct EntityRecord {
    /// strong>0 为活位置；strong=0 且非 Null 为待销毁；begin_destroy 写 NULL 后查询不可见。
    /// NULL 不独立证明清理完成：收束调用栈负责完成边界，所有回收延后至整轮级联完成。
    pub(crate) location: EntityLocation,
    /// 强引用数；初始为 1，归零进入 PendingDestroy。
    pub(crate) strong: u32,
    /// 弱引用数；初始为 0，不阻止强归零，但延迟槽回收。
    pub(crate) weak: u32,
}
/// 每个出生分区一张本地槽表；本地槽索引只在该分区内解释。
/// 外部 Entity 索引由出生分区高位和本地槽低位组成，槽表键仍保存本地键。
/// rc 用 pi_slotmap：独占 insert，计数放槽值 EntityRecord。
/// 非 rc 用 pi_slot：insert_with_version_bits(&self) 共享插入，计数放槽 link 高/低 32 位。
/// 两者键布局一致；外部只经本类型方法访问，不暴露底层槽表。
pub(crate) struct EntityStore {
    /// rc 单线程身份表：按出生分区索引，槽值同时保存位置与计数。
    #[cfg(feature = "rc")]
    pub(crate) slots: Box<[pi_slotmap::SlotMap<Entity, EntityRecord>]>,
    /// 非 rc 共享槽表：按出生分区索引，槽值为位置，计数在 link（高 32 strong / 低 32 weak）。
    #[cfg(not(feature = "rc"))]
    pub(crate) slots: Box<[pi_slot::SlotMap<Entity, EntityLocation>]>,
}
impl EntityStore {
    /// # 用途
    /// RC 收束开始隐藏 entity 的位置，返回唯一旧位置供当前批次移出组件。
    /// 仅 strong=0 且非 Null 时成功并写 NULL；重复隐藏或失效为内部协议错误，不改计数。
    #[cfg(feature = "rc")]
    pub(crate) fn begin_destroy(&mut self, entity: Entity) -> Result<EntityLocation, EcsError> {
        Self::begin_destroy_inner(self, entity)
    }
    /// # 用途
    /// RC 当前实体组件全部释放或移交历史后调用；须位置 Null 且 strong=0，无额外 life 状态。
    /// 返回 weak=0 的回收判定；候选允许重复，收束末尾按首次登记顺序去重后回收。
    /// 本方法必须每个实体清理恰好调用一次，由收束调用栈保证；无额外位检测重复完成。
    #[cfg(feature = "rc")]
    pub(crate) fn finish_destroy(&mut self, entity: Entity) -> Result<bool, EcsError> {
        Self::finish_destroy_inner(self, entity)
    }
    /// 按构造时固定的分区数创建每出生分区一张空本地槽表；非法数量返回错误。
    pub(crate) fn new(partition_count: usize) -> Result<Self, EcsError> {
        Self::new_inner(partition_count)
    }
    /// 为合法的未发布行分配最终键；同出生分区有序执行，组件失败允许消耗代际。
    /// domain 是出生分区，location 是核心预留行；分区无效或容量失败返回错误。
    /// 成功时 strong=1、weak=0，并把 domain.0 写入实体索引高 DOMAIN_BITS 位。
    pub(crate) fn allocate(
        &mut self,
        domain: AllocationDomain,
        location: EntityLocation,
    ) -> Result<Entity, EcsError> {
        Self::allocate_inner(self, domain, location)
    }
    /// # 用途
    /// entity 为完整键；失效/代际不符/位置 Null 返回 None，状态不变。
    /// RC strong=0 且非 Null 仍返回位置，表示尚未收束的已发布组件可见，不授予强增持或结构变更。
    pub(crate) fn locate(&self, entity: Entity) -> Option<EntityLocation> {
        Self::locate_inner(self, entity)
    }
    /// 将存活 entity 指向已准备的 location，不改变键；无效键错误且映射不变。
    pub(crate) fn relocate(
        &mut self,
        entity: Entity,
        location: EntityLocation,
    ) -> Result<(), EcsError> {
        Self::relocate_inner(self, entity, location)
    }
    /// rc：独占增持强引用；仅 strong>0 且位置非 Null 成功，归零即拒绝复活，溢出拒绝。
    #[cfg(feature = "rc")]
    pub(crate) fn acquire_strong(&mut self, entity: Entity) -> Result<(), EcsError> {
        Self::acquire_strong_inner(self, entity)
    }
    /// 非 rc：原子增持强引用；不清除 PendingDestroy 标记，归零后拒绝。
    #[cfg(not(feature = "rc"))]
    pub(crate) fn acquire_strong(&self, entity: Entity) -> Result<(), EcsError> {
        Self::acquire_strong_inner(self, entity)
    }
    /// 交还一份强引用；Ok(true) 表示本次归零，需登记 PendingDestroy。
    /// 键失效/代际不符返回错误；调用者每份责任只调用一次。
    #[cfg(feature = "rc")]
    pub(crate) fn release_strong(&mut self, entity: Entity) -> Result<bool, EcsError> {
        Self::release_strong_inner(self, entity)
    }
    /// 非 rc 原子交还强引用；Ok(true) 表示本次归零。
    #[cfg(not(feature = "rc"))]
    pub(crate) fn release_strong(&self, entity: Entity) -> Result<bool, EcsError> {
        Self::release_strong_inner(self, entity)
    }
    /// rc：增持弱引用；strong 归零后仍可增持，Dead 后拒绝，溢出拒绝。
    #[cfg(feature = "rc")]
    pub(crate) fn acquire_weak(&mut self, entity: Entity) -> Result<(), EcsError> {
        Self::acquire_weak_inner(self, entity)
    }
    /// 非 rc：原子增持弱引用；Dead 后拒绝，溢出拒绝。
    #[cfg(not(feature = "rc"))]
    pub(crate) fn acquire_weak(&self, entity: Entity) -> Result<(), EcsError> {
        Self::acquire_weak_inner(self, entity)
    }
    /// 交还一份弱引用；RC 本次 weak 归零且 strong=0、位置 Null 时返回 true，登记回收候选。
    /// Null 期间可能仍在清理，候选不得立即回收；收束末尾统一去重，失败标停则不回收。
    #[cfg(feature = "rc")]
    pub(crate) fn release_weak(&mut self, entity: Entity) -> Result<bool, EcsError> {
        Self::release_weak_inner(self, entity)
    }
    /// 非 rc 原子交还弱引用；Ok(true) 表示 weak=0 且 strong=0。
    #[cfg(not(feature = "rc"))]
    pub(crate) fn release_weak(&self, entity: Entity) -> Result<bool, EcsError> {
        Self::release_weak_inner(self, entity)
    }
    /// 读取当前 strong/weak；失效键返回 None，不改变状态。
    pub(crate) fn counts(&self, entity: Entity) -> Option<(u32, u32)> {
        Self::counts_inner(self, entity)
    }
    /// 仅 strong=weak=0 且组件销毁收束后最终回收身份槽；不用于交还单份强引用。
    /// 仅安全收束末尾调用：同次候选按出生分区/完整 Entity 键升序逐个回收，分区内逐个压入 LIFO。
    /// 释放线程只登记候选，不直接调用；顺序主体见 World::flush_destruction。
    /// 普通代际拒绝旧键；截尾及回绕不保证永久失效。
    #[cfg(not(feature = "rc"))]
    pub(crate) fn remove(&mut self, entity: Entity) -> Option<EntityLocation> {
        Self::remove_inner(self, entity)
    }
    /// # 用途
    /// RC 全部级联清理完成后回收 strong=weak=0、位置 Null 的完整键，成功 true；不返回过期物理位置。
    /// 顺序单源见 destruction::flush_destruction：按首次登记顺序去重，不排序；本入口只回收给定槽。
    /// 失效或不满足前提返回 false，不删除活槽；调用者将意外 false 视为内部故障。
    #[cfg(feature = "rc")]
    pub(crate) fn remove(&mut self, entity: Entity) -> bool {
        Self::remove_rc_inner(self, entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{domain_bits, slot_index, AllocationDomain, MAX_PARTITIONS};

    fn location() -> EntityLocation {
        // SAFETY: EntityLocation is repr(C) over three u32 fields; all bit patterns are valid.
        unsafe { std::mem::zeroed() }
    }

    #[test]
    fn partitions_are_isolated_and_versions_advance() {
        let mut store = EntityStore::new(2).unwrap();
        let zero = AllocationDomain(0);
        let one = AllocationDomain(1);

        let first = store.allocate(zero, location()).unwrap();
        let other_partition = store.allocate(one, location()).unwrap();
        let second = store.allocate(zero, location()).unwrap();

        let first_slot = if cfg!(feature = "rc") { 1 } else { 0 };
        assert_eq!(domain_bits(&first), 0);
        assert_eq!(slot_index(&first), first_slot);
        assert_eq!(domain_bits(&other_partition), 1);
        assert_eq!(slot_index(&other_partition), first_slot);
        assert_eq!(domain_bits(&second), 0);
        assert_eq!(slot_index(&second), first_slot + 1);
        assert_eq!(store.locate(first), Some(location()));

        assert_eq!(store.counts(first), Some((1, 0)));
        store.acquire_strong(first).unwrap();
        assert_eq!(store.counts(first), Some((2, 0)));
        assert!(!store.release_strong(first).unwrap());
        assert!(store.release_strong(first).unwrap());
        assert_eq!(store.counts(first), Some((0, 0)));
        assert!(store.acquire_strong(first).is_err());

        store.acquire_weak(first).unwrap();
        assert_eq!(store.counts(first), Some((0, 1)));
        let reclaim = store.release_weak(first).unwrap();
        #[cfg(feature = "rc")]
        assert!(!reclaim);
        #[cfg(not(feature = "rc"))]
        assert!(reclaim);
        assert_eq!(store.counts(first), Some((0, 0)));

        #[cfg(feature = "rc")]
        {
            assert_eq!(store.begin_destroy(first).unwrap(), location());
            assert_eq!(store.locate(first), None);
            assert!(store.finish_destroy(first).unwrap());
            assert!(store.remove(first));
        }
        #[cfg(not(feature = "rc"))]
        assert_eq!(store.remove(first), Some(location()));
        assert_eq!(store.locate(first), None);

        let reused = store.allocate(zero, location()).unwrap();
        assert_eq!(slot_index(&reused), first_slot);
        assert_ne!(reused, first);
        assert_eq!(store.locate(reused), Some(location()));
    }

    #[test]
    fn rejects_invalid_partition_and_domain() {
        assert!(EntityStore::new(0).is_err());
        assert!(EntityStore::new(MAX_PARTITIONS + 1).is_err());

        let mut store = EntityStore::new(1).unwrap();
        assert!(store.allocate(AllocationDomain(1), location()).is_err());
    }
}
