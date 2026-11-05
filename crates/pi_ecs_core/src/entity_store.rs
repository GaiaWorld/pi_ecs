//! 核心固定身份映射，不再接受插件替换实现。
use crate::{
    entity::{AllocationDomain, Entity, EntityLocation},
    error::EcsError,
};
/// 每个来源域独立维护槽和 LIFO 复用状态，由 World 独占持有。
/// @to_design 分配来源与类别扩展位编码、DOMAIN_BITS 选取、维护及快照兼容协议待定。
pub(crate) struct EntityStore {
    // @to_design 数量固定的域路由和 SlotMap 表示，不能用 PARTITION_COUNT 充当扩展位数。
    pub(crate) _pending_domains: (),
}
impl EntityStore {
    /// 为合法的未发布行分配最终键；同域有序执行，组件失败允许消耗代际。
    /// domain 是来源域，location 是核心预留行；域无效或容量失败返回错误。
    /// @to_design 分配本身失败的状态保证须按错误分类确定。
    pub(crate) fn allocate(
        &mut self,
        domain: AllocationDomain,
        location: EntityLocation,
    ) -> Result<Entity, EcsError> {
        crate::entity_store_inner::allocate_inner(self, domain, location)
    }
    /// entity 为完整键；失效或代际不符返回 None，状态不变。
    pub(crate) fn locate(&self, entity: Entity) -> Option<EntityLocation> {
        crate::entity_store_inner::locate_inner(self, entity)
    }
    /// 将存活 entity 指向已准备的 location，不改变键；无效键错误且映射不变。
    /// @to_design 多步搬移的失败协调由 World 迁移协议确定。
    pub(crate) fn relocate(
        &mut self,
        entity: Entity,
        location: EntityLocation,
    ) -> Result<(), EcsError> {
        crate::entity_store_inner::relocate_inner(self, entity, location)
    }
    /// 仅 strong=weak=0 且组件销毁收束后最终回收身份槽；不用于交还单份强引用。
    /// 仅安全收束末尾调用：同次候选按 AllocationDomain/完整 Entity 键升序逐个回收，域内逐个压入 LIFO。
    /// 释放线程只登记候选，不直接调用；顺序主体见 World::flush_destruction。
    /// 普通代际拒绝旧键；截尾及回绕不保证永久失效。
    pub(crate) fn remove(&mut self, entity: Entity) -> Option<EntityLocation> {
        crate::entity_store_inner::remove_inner(self, entity)
    }
}
/// 旧底层迁移占位别名，不再代表最终选型。
/// @to_design 改用 pi_slot LIVE link 强弱计数；需统一 pi_key 版本、位宽及原子协议。
/// DOMAIN_BITS 是版本扩展位数，不是分区数量。
pub type DomainSlotMap<const DOMAIN_BITS: u32> =
    pi_slotmap::SlotMap<Entity, EntityLocation, DOMAIN_BITS>;
