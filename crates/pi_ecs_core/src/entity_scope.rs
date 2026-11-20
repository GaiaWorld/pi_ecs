//! 宿主实体操作会话；与 access::EntityScope 的查询范围声明不同。
use crate::{
    component::{Component, ComponentId, ComponentRead, ComponentWrite},
    entity::{AllocationDomain, Entity},
    error::EcsError,
    owned_entity::OwnedEntity,
};
#[path = "entity_scope_inner.rs"]
mod entity_scope_inner;

/// # 用途
/// 由 World::entity_scope 的外层调用栈持有，独立于传给用户的 EntityScope。
/// 只借门禁字段；不保存业务字段，不公开构造或提前解除接口，失败不能把 Stopped 恢复为 Idle。
pub(crate) struct EntityScopeSession<'a> {
    /// 与业务字段分开的门禁引用；rc 用 Cell，非 rc 用原子字；会话入口唯一认领维护态。
    pub(crate) access: &'a crate::world::AccessState,
    /// 仅 run 正常返回（含普通 Err）且字段借用已结束后置位；未完成退出保守停止。
    pub(crate) completed: bool,
}
impl EntityScopeSession<'_> {
    /// # 用途
    /// 结束已正常返回的会话；无参数，调用前全部组件/字段借用已结束。
    /// 已 Stopped 保持停止，否则 Maintaining 转 Idle；不清空登记，不分配 tick，不回滚。
    /// 只可调用一次，完成后 Drop 不重复释放；协议错误不得伪装为成功。
    pub(crate) fn complete(&mut self) {
        self.complete_inner()
    }
}
impl Drop for EntityScopeSession<'_> {
    /// # 用途
    /// 未完成退出（含 panic 展开）将 World 停止；已完成则无额外动作。
    /// 不借身份表、不运行用户析构、不发布通知、不再次 panic，不能依赖业务引用 Drop 成功。
    fn drop(&mut self) {
        self.drop_inner()
    }
}

/// # 用途
/// 由 World::entity_scope 建立的受限操作会话，不可独立构造或取得整份 World。
/// 's 绑定临时拥有责任的作用域；组件结果仅绑定操作短借用，不阻止拥有引用跨操作保留。
/// 从宿主独占根派生同源受限能力；仅在操作时投影组件/节点等字段，不还原整 World 引用。
/// 普通值析构、内置责任释放及回调前必须结束身份表动态借用。
/// 所有公开操作先校验会话仍在 Maintaining；Stopped 或不属于本会话时返回 EcsError，
/// 不定位实体、不借身份表、不消费 tick、不访问组件；消费型输入仍须正常交还未安装责任。
/// 普通输入错误不标停；首次创建修改后的不可恢复失败先标停再 panic，后续操作不得继续。
pub struct EntityScope<'s> {
    /// 本 World 不复用身份，用于拒绝跨 World 引用/请求转交。
    pub(crate) world_identity: u64,
    /// rc 为 RefCell<EntityStore>，非 rc 为原子身份表；不保存长期 Ref/RefMut。
    pub(crate) entities: &'s crate::entity_store::EntityIdentityStore,
    /// strong 归零仅登记，不在临时引用 Drop 中物理销毁。
    pub(crate) pending_destruction: &'s crate::destruction::PendingDestruction,
    /// 宿主独占根派生的同源能力；不与长期独占字段引用共存，组件结果仅占用操作短借用。
    #[cfg(feature = "rc")]
    pub(crate) world: crate::unsafe_world_cell::UnsafeWorldCell<'s>,
    /// 与外层会话共用的状态字段；操作只检查或将其置 Stopped，不能解锁或恢复 Idle。
    /// rc 使用 Cell<AccessMode>，非 rc 原子状态；不依赖 catch_unwind 后才记录失败。
    pub(crate) access: &'s crate::world::AccessState,
}

#[cfg(test)]
mod compile_checks {
    use super::*;
    use crate::world::World;
    // 仅编译验证最高壳入口的寿命组合；运行时行为由独立测试覆盖。
    fn transfer_queue_to_spawn(
        world: &mut World,
        owner: Entity,
        domain: AllocationDomain,
    ) -> Result<Option<Entity>, EcsError> {
        world.entity_scope(|scope| {
            let owned = {
                let Some(mut queue) =
                    scope.get_mut::<crate::entity_container::EntityQueue<()>>(owner)?
                else {
                    return Ok(None);
                };
                queue.pop_front()?
            };
            let Some(owned) = owned else {
                return Ok(None);
            };
            let values = crate::bundle_input::StrongArrayInput::<1, ()>::new([owned])
                .map_err(|rejected| rejected.error)?;
            let created = scope.spawn(domain, values)?;
            let id = created.entity();
            drop(created);
            Ok(Some(id))
        })
    }
    fn acquire_then_change(
        world: &mut World,
        entity: Entity,
        target: Entity,
        removed: &[ComponentId],
    ) -> Result<(), EcsError> {
        world.entity_scope(|scope| {
            let owned = scope.try_acquire(target)?;
            let added = crate::bundle_input::StrongArrayInput::<1, ()>::new([owned])
                .map_err(|rejected| rejected.error)?;
            scope.change_archetype(entity, added, removed)
        })
    }
    // 编译验证混合输入保留作用域寿命：取得 weak 后不保留 strong 对象的借用。
    fn mixed_inputs(
        world: &mut World,
        target: Entity,
        domain: AllocationDomain,
    ) -> Result<Entity, EcsError> {
        world.entity_scope(|scope| {
            let owned = scope.try_acquire(target)?;
            let weak = crate::bundle_input::WeakArrayInput::<1, ()>::new([&owned])?;
            let strong = crate::bundle_input::StrongArrayInput::<1, ()>::new([owned])
                .map_err(|rejected| rejected.error)?;
            let input = (strong, weak, crate::bundle_input::EmptyQueue::<()>::new());
            let created = scope.spawn(domain, input)?;
            let entity = created.entity();
            drop(created);
            Ok(entity)
        })
    }
    // 拥有引用 Drop 不借住组件存储；验证其与活组件短借用的签名组合。
    fn release_while_component_borrowed<C: Component>(
        world: &mut World,
        entity: Entity,
    ) -> Result<(), EcsError> {
        world.entity_scope(|scope| {
            let owned = scope.try_acquire(entity)?;
            let component = scope.get_mut::<C>(entity)?;
            drop(owned);
            drop(component);
            Ok(())
        })
    }
}

impl<'s> EntityScope<'s> {
    /// # 用途
    /// 为本 World 的存活 entity 增加一份 strong，不授予组件权限。
    /// 失效/归零/溢出拒绝且计数不变；结果绑定 's，不借住本次 self，不标 Changed。
    pub fn try_acquire(&self, entity: Entity) -> Result<OwnedEntity<'s>, EcsError> {
        self.try_acquire_inner(entity)
    }
    /// # 用途
    /// 读取 entity 的 C；未知类型/失效实体/缺组件返回 Ok(None)，存在返回 Ok(Some)，不隐式登记。
    /// 停止或会话失效返回 Err，在任何数据访问前拒绝，不以 None 掩盖停止。
    /// 组件借用绑定 'a；内置访问转出的拥有引用绑定 's，无裸内置头替换能力。
    pub fn get<'a, C: Component>(
        &'a self,
        entity: Entity,
    ) -> Result<Option<ComponentRead<'a, 's, C>>, EcsError> {
        self.get_inner::<C>(entity)
    }
    /// # 用途
    /// 独占访问 entity 的 C；未知类型/失效实体/缺组件返回 Ok(None)，存在返回 Ok(Some)，不隐式登记。
    /// 停止或会话失效返回 Err，在身份定位/时钟分配/取值前拒绝，不改变组件和计数。
    /// 结果短借用阻止冲突操作，容器 pop 可转出 's 的拥有责任；写入按需求分配/复用操作 tick。
    /// 内置类型返回专用访问对象，不允许整体替换头。时间耗尽或 Changed 发布失败标停并 panic。
    pub fn get_mut<'a, C: Component>(
        &'a mut self,
        entity: Entity,
    ) -> Result<Option<ComponentWrite<'a, 's, C>>, EcsError> {
        self.get_mut_inner::<C>(entity)
    }
    /// # 用途
    /// 以 domain 同时作为出生和初始当前分区，从 values 推导原型并自动登记缺少的组合。
    /// domain 必须有效，values 所有类型和 Table 已登记且 World 已冻结；返回初始 strong=1。
    /// 校验错误释放未安装输入。首次创建修改后不可恢复失败标停；行已发布后的通知失败不回滚。
    /// 结果绑定 's，不借住 self；输入中的强/弱责任只安装一次。
    /// 内置安装及未发布清理遵守 bundle::Bundle 的责任协议；通过 BundleWriter 的封闭内置写通道直接安装。
    pub fn spawn<B: crate::bundle::Bundle>(
        &mut self,
        domain: AllocationDomain,
        values: B,
    ) -> Result<OwnedEntity<'s>, EcsError> {
        self.spawn_inner(domain, values)
    }
    /// # 用途
    /// entity 为存活完整键；added 是源中不存在的已登记新增值，removed 为源中存在的无重复编号。
    /// 增删不相交，空增删不修改；自动推导/登记目标，不改变实体当前分区。
    /// 输入校验错误不改实体，首次修改后失败 panic；具体搬移/释放/发布流程见 World::change_archetype。
    /// 调用前所有冲突组件借用必须结束，但可保留本作用域 OwnedEntity。
    /// 新增内置值安装遵守 bundle::Bundle，已安装部分与剩余输入不能重复释放。
    pub fn change_archetype<B: crate::bundle::Bundle>(
        &mut self,
        entity: Entity,
        added: B,
        removed: &[ComponentId],
    ) -> Result<(), EcsError> {
        self.change_archetype_inner(entity, added, removed)
    }
}
