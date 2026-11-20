//! 每个有效节点拥有一份目标 strong，包括 Removed 旧头保留的节点；故目标不得 strong=0。
//! 容器头所属实体可待销毁，不影响节点责任；允许按既有值写权限 pop/clear，不等于对所属实体做 Alter。
//! 转移节点责任不减后再增，复制才新增 strong；内部损坏沿 OwnedEntity 不变量规则，不伪装空容器/过期成员。
//! 内置容器头、统一节点池及 Query 临时专用访问；存储值无 World 指针、无普通 Drop。
//! 责任交接：输入保持 OwnedEntity 直到节点安装；分配/检查在原责任仍有效时完成，不先 forget。
//! 节点/头发布至解除输入责任、移出节点至建立 OwnedEntity 之间必须无分配/回调/可展开操作。
//! 移出后先建立栈上拥有对象、结束池借用，再标记；标记 panic 时依普通 Drop 清偿，不回滚头。
//! clear 逐节点转移再释放，剩余节点始终由有效头链接持有；不得先摘空整条链却无清理主体。
#[path = "entity_container_inner.rs"]
mod entity_container_inner;
use crate::{
    destruction::ReleaseContext, entity::Entity, error::EcsError, owned_entity::OwnedEntity,
    ptr::OwningPtr,
};
use std::marker::PhantomData;
pi_key::new_key_type! {
    /// 单链节点键；List/Queue 各自的节点池内部解释，不提供跨池转换。
    pub struct LinkKey;
    /// Deque 非拥有节点键；完整键只证明池内代际，安全按键 API 另验证当前头链成员归属。
    pub struct DequeKey;
}

/// 活值包装由 component::read_component/write_component 接收单槽类型化引用；不在策略层解码地址。
/// 头/节点访问绑定短寿命，身份/归零登记绑定来源授权；取得 OwnedEntity 不延长头或节点借用。
/// 不持整池 Ref/RefMut 跨业务调用，多个不相交头可以共用节点池。
/// 内置List的共享访问，借用头并绑定节点池能力，无构造/取走/可变头入口。
pub struct EntityListRef<'a, 'w, T: 'static> {
    /// 本次只读头，节点保护期覆盖遍历结果。
    pub(crate) value: &'a EntityList<T>,
    /// 节点池共享短借用，随头借用结束；不得从中构造整个 World。
    pub(crate) nodes: &'a EntityNodePools,
    /// 所属 World 唯一身份；跨域输入前置拒绝。
    pub(crate) world_identity: u64,
    /// 只用于身份及 strong/weak 短借用，不携带组件权限；结果可保留到 'w。
    pub(crate) entities: &'w crate::entity_store::EntityIdentityStore,
    /// 归零只登记，禁止访问对象/临时引用隐式物理销毁。
    pub(crate) pending_destruction: &'w crate::destruction::PendingDestruction,
    /// 共享停止状态；新增责任须先拒绝 Stopped，释放已有责任仍允许，不提供解锁能力。
    pub(crate) access: &'w crate::world::AccessState,
}

/// 内置Queue的共享访问，借用头并绑定节点池能力，无构造/取走/可变头入口。
pub struct EntityQueueRef<'a, 'w, T: 'static> {
    /// 本次只读头，节点保护期覆盖遍历结果。
    pub(crate) value: &'a EntityQueue<T>,
    /// 节点池共享短借用，随头借用结束；不得从中构造整个 World。
    pub(crate) nodes: &'a EntityNodePools,
    /// 所属 World 唯一身份；跨域输入前置拒绝。
    pub(crate) world_identity: u64,
    /// 只用于身份及 strong/weak 短借用，不携带组件权限；结果可保留到 'w。
    pub(crate) entities: &'w crate::entity_store::EntityIdentityStore,
    /// 归零只登记，禁止访问对象/临时引用隐式物理销毁。
    pub(crate) pending_destruction: &'w crate::destruction::PendingDestruction,
    /// 共享停止状态；新增责任须先拒绝 Stopped，释放已有责任仍允许，不提供解锁能力。
    pub(crate) access: &'w crate::world::AccessState,
}

/// 内置Deque的共享访问，借用头并绑定节点池能力，无构造/取走/可变头入口。
pub struct EntityDequeRef<'a, 'w, T: 'static> {
    /// 本次只读头，节点保护期覆盖遍历结果。
    pub(crate) value: &'a EntityDeque<T>,
    /// 节点池共享短借用，随头借用结束；不得从中构造整个 World。
    pub(crate) nodes: &'a EntityNodePools,
    /// 所属 World 唯一身份；跨域输入前置拒绝。
    pub(crate) world_identity: u64,
    /// 只用于身份及 strong/weak 短借用，不携带组件权限；结果可保留到 'w。
    pub(crate) entities: &'w crate::entity_store::EntityIdentityStore,
    /// 归零只登记，禁止访问对象/临时引用隐式物理销毁。
    pub(crate) pending_destruction: &'w crate::destruction::PendingDestruction,
    /// 共享停止状态；新增责任须先拒绝 Stopped，释放已有责任仍允许，不提供解锁能力。
    pub(crate) access: &'w crate::world::AccessState,
}
impl<T: 'static> EntityListRef<'_, '_, T> {
    /// 当前长度，只读、不增持且不标记 Changed。
    pub fn len(&self) -> usize {
        Self::list_read_len(self)
    }
    /// 当前是否为空，不消费节点或引用责任。
    pub fn is_empty(&self) -> bool {
        Self::list_read_empty(self)
    }
    /// 窥视头部裸身份，不增持；能力校验拒绝不改存储。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        Self::list_read_front(self)
    }
    /// 从本头按链顺序遍历裸 Entity；借用 self，阻止同期修改/回收，不授予目标组件权限。
    /// 返回错误不消费容器；节点内部不变量损坏停止，不以跳过节点掩盖。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        Self::list_read_iter(self)
    }
}
impl<T: 'static> EntityListMut<'_, '_, T> {
    /// 空检查，不标记变化。
    pub fn is_empty(&self) -> bool {
        Self::list_write_empty(self)
    }
    /// 借用本写对象只读遍历，迭代器结束前不能 push/pop/clear；不增持或标记。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        Self::list_write_iter(self)
    }
}
impl<T: 'static> EntityQueueRef<'_, '_, T> {
    /// 当前长度，只读、不增持且不标记 Changed。
    pub fn len(&self) -> usize {
        Self::queue_read_len(self)
    }
    /// 当前是否为空，不消费节点或引用责任。
    pub fn is_empty(&self) -> bool {
        Self::queue_read_empty(self)
    }
    /// 窥视头部裸身份，不增持；能力校验拒绝不改存储。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        Self::queue_read_front(self)
    }
    /// 从本头按链顺序遍历裸 Entity；借用 self，阻止同期修改/回收，不授予目标组件权限。
    /// 返回错误不消费容器；节点内部不变量损坏停止，不以跳过节点掩盖。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        Self::queue_read_iter(self)
    }
}
impl<T: 'static> EntityQueueMut<'_, '_, T> {
    /// 空检查，不标记变化。
    pub fn is_empty(&self) -> bool {
        Self::queue_write_empty(self)
    }
    /// 借用本写对象只读遍历，迭代器结束前不能 push/pop/clear；不增持或标记。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        Self::queue_write_iter(self)
    }
}
impl<T: 'static> EntityDequeRef<'_, '_, T> {
    /// 当前长度，只读、不增持且不标记 Changed。
    pub fn len(&self) -> usize {
        Self::deque_read_len(self)
    }
    /// 当前是否为空，不消费节点或引用责任。
    pub fn is_empty(&self) -> bool {
        Self::deque_read_empty(self)
    }
    /// 窥视头部裸身份，不增持；能力校验拒绝不改存储。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        Self::deque_read_front(self)
    }
    /// 从本头按链顺序遍历裸 Entity；借用 self，阻止同期修改/回收，不授予目标组件权限。
    /// 返回错误不消费容器；节点内部不变量损坏停止，不以跳过节点掩盖。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        Self::deque_read_iter(self)
    }
}
impl<T: 'static> EntityDequeMut<'_, '_, T> {
    /// 空检查，不标记变化。
    pub fn is_empty(&self) -> bool {
        Self::deque_write_empty(self)
    }
    /// 借用本写对象只读遍历，迭代器结束前不能 push/pop/clear；不增持或标记。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        Self::deque_write_iter(self)
    }
}
/// 按头至尾遍历裸身份；复制结果不延长目标寿命，目标组件另经 Lookup 授权点查。
/// 活头与 Removed 旧头使用相同遍历协议；迭代器借用访问对象，不能提前回收节点。
pub struct EntityChainIter<'a> {
    /// 节点池读取能力，与访问对象及头的共享借用同保护期。
    pub(crate) nodes: &'a EntityNodePools,
    /// 下一节点游标：单链 LinkKey 或 DequeKey；类型化键阻止跨容器种类混用。
    pub(crate) next: Option<ChainCursor>,
    /// 固定头的剩余节点数，用于约束遍历，不允许并发修改链。
    pub(crate) remaining: usize,
}
/// 链遍历游标；由容器种类决定，不携带容器身份。
pub(crate) enum ChainCursor {
    /// List/Queue 单链节点。
    Link(LinkKey),
    /// Deque 双链节点。
    Deque(DequeKey),
}
impl Iterator for EntityChainIter<'_> {
    type Item = Entity;
    /// 返回 Copy 裸身份，不返回节点引用；RC 每次 next 短借后端并复制目标/后继后结束借用。
    /// 所属头的共享借用阻止改本链，但不阻止其他头操作；不缓存后端槽地址。
    /// 内部损坏/借用重入停止，正常尾部为 None。
    fn next(&mut self) -> Option<Self::Item> {
        Self::chain_next(self)
    }
}

impl<'w, T: 'static> EntityListRef<'_, 'w, T> {
    /// 对首成员复制一份 strong，源节点责任保留；空为 None，Stopped/满额普通 Err，不改链或 Changed。
    /// 有效节点目标必 strong>0；身份失效/零计数是内部责任损坏，不当作普通过期或空值。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::list_ref_front_owned(self)
    }
    /// 按链顺序逐项复制 strong，每个重复目标按节点分别增持；创建不增持，早停不处理余项。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        Self::list_ref_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityListMut<'_, 'w, T> {
    /// 对首成员复制一份 strong，源节点责任保留；空为 None，Stopped/满额普通 Err，不改链或 Changed。
    /// 有效节点目标必 strong>0；身份失效/零计数是内部责任损坏，不当作普通过期或空值。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::list_mut_front_owned(self)
    }
    /// 按链顺序逐项复制 strong，每个重复目标按节点分别增持；创建不增持，早停不处理余项。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        Self::list_mut_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityQueueRef<'_, 'w, T> {
    /// 对首成员复制一份 strong，源节点责任保留；空为 None，Stopped/满额普通 Err，不改链或 Changed。
    /// 有效节点目标必 strong>0；身份失效/零计数是内部责任损坏，不当作普通过期或空值。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::queue_ref_front_owned(self)
    }
    /// 按链顺序逐项复制 strong，每个重复目标按节点分别增持；创建不增持，早停不处理余项。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        Self::queue_ref_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityQueueMut<'_, 'w, T> {
    /// 对首成员复制一份 strong，源节点责任保留；空为 None，Stopped/满额普通 Err，不改链或 Changed。
    /// 有效节点目标必 strong>0；身份失效/零计数是内部责任损坏，不当作普通过期或空值。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::queue_mut_front_owned(self)
    }
    /// 按链顺序逐项复制 strong，每个重复目标按节点分别增持；创建不增持，早停不处理余项。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        Self::queue_mut_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityDequeRef<'_, 'w, T> {
    /// 对首成员复制一份 strong，源节点责任保留；空为 None，Stopped/满额普通 Err，不改链或 Changed。
    /// 有效节点目标必 strong>0；身份失效/零计数是内部责任损坏，不当作普通过期或空值。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::deque_ref_front_owned(self)
    }
    /// 按链顺序逐项复制 strong，每个重复目标按节点分别增持；创建不增持，早停不处理余项。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        Self::deque_ref_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityDequeRef<'_, 'w, T> {
    /// 对尾成员增持；空为 None，普通拒绝不改链/计数、不标 Changed。
    pub fn back_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::deque_ref_back_owned(self)
    }
    /// 本入口验证 key 属于当前头链且代际有效；拒绝报错，无任意 Entity 增持权限，不标 Changed。
    pub fn get_owned(&self, key: DequeKey) -> Result<OwnedEntity<'w>, EcsError> {
        Self::deque_ref_get_owned(self, key)
    }
}
impl<'w, T: 'static> EntityDequeMut<'_, 'w, T> {
    /// 对首成员复制一份 strong，源节点责任保留；空为 None，Stopped/满额普通 Err，不改链或 Changed。
    /// 有效节点目标必 strong>0；身份失效/零计数是内部责任损坏，不当作普通过期或空值。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::deque_mut_front_owned(self)
    }
    /// 按链顺序逐项复制 strong，每个重复目标按节点分别增持；创建不增持，早停不处理余项。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        Self::deque_mut_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityDequeMut<'_, 'w, T> {
    /// 对尾成员增持；空为 None，普通拒绝不改链/计数、不标 Changed。
    pub fn back_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::deque_mut_back_owned(self)
    }
    /// 本入口验证 key 属于当前头链且代际有效；拒绝报错，无任意 Entity 增持权限，不标 Changed。
    pub fn get_owned(&self, key: DequeKey) -> Result<OwnedEntity<'w>, EcsError> {
        Self::deque_mut_get_owned(self, key)
    }
}
/// 逐项增持的拥有遍历；每项成功只新增一份 strong，保留源节点及其原责任。
pub struct EntityOwnedIter<'a, 'w> {
    /// 借用本次访问对象和链，防止遍历期间修改或回收；错误时游标留在当前项。
    pub(crate) chain: EntityChainIter<'a>,
    /// 所属 World 唯一身份；跨域输入前置拒绝。
    pub(crate) world_identity: u64,
    /// 只用于身份及 strong/weak 短借用，不携带组件权限；结果可保留到 'w。
    pub(crate) entities: &'w crate::entity_store::EntityIdentityStore,
    /// 归零只登记，禁止访问对象/临时引用隐式物理销毁。
    pub(crate) pending_destruction: &'w crate::destruction::PendingDestruction,
    /// 共享停止状态；新增责任须先拒绝 Stopped，释放已有责任仍允许，不提供解锁能力。
    pub(crate) access: &'w crate::world::AccessState,
}
impl<'w> Iterator for EntityOwnedIter<'_, 'w> {
    type Item = Result<OwnedEntity<'w>, EcsError>;
    /// 尾部 None；增持拒绝返回 Err 且不推进，内部链损坏停止，不静默跳过。
    fn next(&mut self) -> Option<Self::Item> {
        Self::owned_next(self)
    }
}

/// 单链节点，每份目标责任由 World 显式转移/释放，不持有临时引用。
pub(crate) struct EntityLink {
    /// 目标完整身份，一份强责任。
    pub(crate) value: Entity,
    /// 后继；链尾为 None。
    pub(crate) next: Option<LinkKey>,
}
/// Deque 节点负载；前驱/后继与代理由 pi_slot_deque 内部维护。
pub(crate) struct EntityNode {
    /// 一份目标强责任。
    pub(crate) value: Entity,
}
/// rc 单链节点池：pi_slotmap 稳定槽 + 代际，LinkKey 直接作为槽表键。
/// 插入/移出需要 &mut；节点链由 EntityList/Queue 头解释。
#[cfg(feature = "rc")]
pub(crate) struct NodePool<N> {
    /// 稳定槽表；槽值为单链节点。
    pub(crate) slots: pi_slotmap::SlotMap<LinkKey, N>,
}
#[cfg(feature = "rc")]
impl<N> NodePool<N> {
    /// 插入节点并返回稳定键；失败直接 panic，不返回半节点。
    /// 调用者在可分配/可展开期间仍持输入 OwnedEntity；节点已安装后的责任解除必须无展开。
    pub(crate) fn insert(&mut self, node: N) -> LinkKey {
        Self::node_pool_insert_inner(self, node)
    }
    /// key 归属本池且代际有效时返回共享借用；失效返回 None。
    pub(crate) fn get(&self, key: LinkKey) -> Option<&N> {
        Self::node_pool_get_inner(self, key)
    }
    /// key 归属本池且代际有效时返回独占借用；失效返回 None。
    pub(crate) fn get_mut(&mut self, key: LinkKey) -> Option<&mut N> {
        Self::node_pool_get_mut_inner(self, key)
    }
    /// 移出节点并归还槽；失效 key 返回 None，不移动其他节点。
    /// 操作前完成可失败检查，移出后到拥有责任建立间不分配/回调；不能把无 Drop 裸 Entity 遗留在可展开路径。
    pub(crate) fn remove(&mut self, key: LinkKey) -> Option<N> {
        Self::node_pool_remove_inner(self, key)
    }
}
/// World 统一节点池；RC 两后端分别 RefCell，所有活值/Removed 访问共享来源，只在操作内部短借。
/// 不允许 Ref/RefMut 或节点地址逃逸；扩容可搬槽，头/遍历器仅存完整键，next 只返回 Entity 副本。
/// 节点池借用结束后才强计数操作、标记/通知、释放或用户析构；不同头可交错操作，不嵌套借池。
/// 借用冲突/损坏标停，不伪装空容器；非 RC 后端仍待定，不由此增加线程能力。
pub(crate) struct EntityNodePools {
    /// List/Queue 单链节点池。
    #[cfg(feature = "rc")]
    pub(crate) links: std::cell::RefCell<NodePool<EntityLink>>,
    /// Deque 存储；各容器头只保存 head/tail/len。
    #[cfg(feature = "rc")]
    pub(crate) deque: std::cell::RefCell<pi_slot_deque::DequeStorage<DequeKey, EntityNode>>,
    /// 非 rc 节点池的并发表示随后端改造补齐；不在本轮声明假实现。
    // @to_design 非 rc 节点池并发分配/回收与级联工作项
    #[cfg(not(feature = "rc"))]
    pub(crate) _pending: (),
}
/// 插入拒绝交还原拥有引用，链和计数不变。
pub struct EntityInsertError<'w> {
    /// 拒绝原因。
    pub error: EcsError,
    /// 尚未消费的原责任。
    pub value: OwnedEntity<'w>,
}
/// 内置容器头，T 区分用途；只在 spawn/Alter 特定输入创建，无公开 new/Drop。
pub struct EntityList<T: 'static> {
    /// 首节点键；键类型化到单链池，不与其他容器种类混用。
    pub(crate) head: Option<LinkKey>,
    /// 尾节点键。
    pub(crate) tail: Option<LinkKey>,
    /// 节点数，重复目标分别计数。
    pub(crate) len: usize,
    /// 用途标记，不拥有 T。
    pub(crate) marker: PhantomData<fn() -> T>,
}
/// Query 对 &mut EntityList 返回的专用访问对象，不实现 DerefMut，不可替换容器头。
pub struct EntityListMut<'a, 'w, T: 'static> {
    /// 当前实体实例的头写借用，已确定 owner，无需方法再传 Entity。
    pub(crate) value: &'a mut EntityList<T>,
    /// 节点池共享来源；RC 每方法内短借 RefCell 后端，不跨结果/用户代码保留，不恢复整个 World。
    pub(crate) nodes: &'a EntityNodePools,
    /// 所属 World 唯一身份；跨域输入前置拒绝。
    pub(crate) world_identity: u64,
    /// 只用于身份及 strong/weak 短借用，不携带组件权限；结果可保留到 'w。
    pub(crate) entities: &'w crate::entity_store::EntityIdentityStore,
    /// 归零只登记，禁止访问对象/临时引用隐式物理销毁。
    pub(crate) pending_destruction: &'w crate::destruction::PendingDestruction,
    /// 共享停止状态；新增责任须先拒绝 Stopped，释放已有责任仍允许，不提供解锁能力。
    pub(crate) access: &'w crate::world::AccessState,
    /// 与普通 Mut 共用的受限变化标记能力，归属本组件而非引用目标。
    pub(crate) marker: crate::change_detection::ChangeMarker<'a>,
    /// 本访问对象首次成功修改后置位，后续修改不重复追加 Changed 通知。
    pub(crate) marked: bool,
}
impl<'w, T: 'static> EntityListMut<'_, 'w, T> {
    /// 当前节点数，不消费。
    pub fn len(&self) -> usize {
        Self::list_len(self)
    }
    /// 只读窥视首个目标身份，不增持或消费。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        Self::list_front(self)
    }
    /// 清空全部节点：逐个把目标 strong 交回 World（必要时登记销毁），再移除节点；不返回引用。
    /// 需要逐项引用时调用者应循环 pop_front。非空成功才标 Changed，空操作不标。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        Self::list_clear(self)
    }
    /// value 须同 World，消费并转入节点，不重复计数；成功修改后返回前标 Changed。
    /// 准备拒绝交回原引用且不标记；写入或标记开始后不可恢复失败停止，不回滚。
    pub fn push_front(&mut self, value: OwnedEntity<'w>) -> Result<(), EntityInsertError<'w>> {
        Self::list_push_front(self, value)
    }
    /// 移出原强责任，不减计数也不增持；直接从节点转入 OwnedEntity，不出现中间归零。
    /// 返回引用不独占借住容器头，成功取出后返回前标 Changed。
    /// 空为 None 且不标记；写入后不可恢复失败停止，不回滚。
    pub fn pop_front(&mut self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::list_pop_front(self)
    }
}
/// 内置容器头，T 区分用途；只在 spawn/Alter 特定输入创建，无公开 new/Drop。
pub struct EntityQueue<T: 'static> {
    /// 首节点键；键类型化到单链池。
    pub(crate) head: Option<LinkKey>,
    /// 尾节点键。
    pub(crate) tail: Option<LinkKey>,
    /// 节点数，重复目标分别计数。
    pub(crate) len: usize,
    /// 用途标记，不拥有 T。
    pub(crate) marker: PhantomData<fn() -> T>,
}
/// Query 对 &mut EntityQueue 返回的专用访问对象，不实现 DerefMut，不可替换容器头。
pub struct EntityQueueMut<'a, 'w, T: 'static> {
    /// 当前实体实例的头写借用，已确定 owner，无需方法再传 Entity。
    pub(crate) value: &'a mut EntityQueue<T>,
    /// 节点池共享来源；RC 每方法内短借 RefCell 后端，不跨结果/用户代码保留，不恢复整个 World。
    pub(crate) nodes: &'a EntityNodePools,
    /// 所属 World 唯一身份；跨域输入前置拒绝。
    pub(crate) world_identity: u64,
    /// 只用于身份及 strong/weak 短借用，不携带组件权限；结果可保留到 'w。
    pub(crate) entities: &'w crate::entity_store::EntityIdentityStore,
    /// 归零只登记，禁止访问对象/临时引用隐式物理销毁。
    pub(crate) pending_destruction: &'w crate::destruction::PendingDestruction,
    /// 共享停止状态；新增责任须先拒绝 Stopped，释放已有责任仍允许，不提供解锁能力。
    pub(crate) access: &'w crate::world::AccessState,
    /// 与普通 Mut 共用的受限变化标记能力，归属本组件而非引用目标。
    pub(crate) marker: crate::change_detection::ChangeMarker<'a>,
    /// 本访问对象首次成功修改后置位，后续修改不重复追加 Changed 通知。
    pub(crate) marked: bool,
}
impl<'w, T: 'static> EntityQueueMut<'_, 'w, T> {
    /// 当前节点数，不消费。
    pub fn len(&self) -> usize {
        Self::queue_len(self)
    }
    /// 只读窥视首个目标身份，不增持或消费。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        Self::queue_front(self)
    }
    /// 清空全部节点：逐个把目标 strong 交回 World（必要时登记销毁），再移除节点；不返回引用。
    /// 需要逐项引用时调用者应循环 pop_front。非空成功才标 Changed，空操作不标。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        Self::queue_clear(self)
    }
    /// value 须同 World，消费并转入节点，不重复计数；成功修改后返回前标 Changed。
    /// 准备拒绝交回原引用且不标记；写入或标记开始后不可恢复失败停止，不回滚。
    pub fn push_back(&mut self, value: OwnedEntity<'w>) -> Result<(), EntityInsertError<'w>> {
        Self::queue_push_back(self, value)
    }
    /// 移出原强责任，不减计数也不增持；直接从节点转入 OwnedEntity，不出现中间归零。
    /// 返回引用不独占借住容器头，成功取出后返回前标 Changed。
    /// 空为 None 且不标记；写入后不可恢复失败停止，不回滚。
    pub fn pop_front(&mut self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::queue_pop_front(self)
    }
}
/// 内置容器头，T 区分用途；只在 spawn/Alter 特定输入创建，无公开 new/Drop。
pub struct EntityDeque<T: 'static> {
    /// 容器头/尾/长度；节点存储由 World 的 DequeStorage 持有。
    pub(crate) deque: pi_slot_deque::Deque<DequeKey, EntityNode>,
    /// 用途标记，不拥有 T。
    pub(crate) marker: PhantomData<fn() -> T>,
}
/// Query 对 &mut EntityDeque 返回的专用访问对象，不实现 DerefMut，不可替换容器头。
pub struct EntityDequeMut<'a, 'w, T: 'static> {
    /// 当前实体实例的头写借用，已确定 owner，无需方法再传 Entity。
    pub(crate) value: &'a mut EntityDeque<T>,
    /// 节点池共享来源；RC 每方法内短借 RefCell 后端，不跨结果/用户代码保留，不恢复整个 World。
    pub(crate) nodes: &'a EntityNodePools,
    /// 所属 World 唯一身份；跨域输入前置拒绝。
    pub(crate) world_identity: u64,
    /// 只用于身份及 strong/weak 短借用，不携带组件权限；结果可保留到 'w。
    pub(crate) entities: &'w crate::entity_store::EntityIdentityStore,
    /// 归零只登记，禁止访问对象/临时引用隐式物理销毁。
    pub(crate) pending_destruction: &'w crate::destruction::PendingDestruction,
    /// 共享停止状态；新增责任须先拒绝 Stopped，释放已有责任仍允许，不提供解锁能力。
    pub(crate) access: &'w crate::world::AccessState,
    /// 与普通 Mut 共用的受限变化标记能力，归属本组件而非引用目标。
    pub(crate) marker: crate::change_detection::ChangeMarker<'a>,
    /// 本访问对象首次成功修改后置位，后续修改不重复追加 Changed 通知。
    pub(crate) marked: bool,
}
impl<'w, T: 'static> EntityDequeMut<'_, 'w, T> {
    /// 当前节点数，不消费。
    pub fn len(&self) -> usize {
        Self::deque_len(self)
    }
    /// 只读窥视首个目标身份，不增持或消费。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        Self::deque_front(self)
    }
    /// 用 DequeOps::clear 清空全部节点：逐个把目标 strong 交回 World（必要时登记销毁），再重置头尾。
    /// 不返回引用；需要逐项引用时调用者应循环 pop_front/pop_back。非空成功才标 Changed。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        Self::deque_clear(self)
    }
    /// value 须同 World，消费并转入节点，不重复计数；成功修改后返回前标 Changed。
    /// 准备拒绝交回原引用且不标记；写入或标记开始后不可恢复失败停止，不回滚。
    pub fn push_front(
        &mut self,
        value: OwnedEntity<'w>,
    ) -> Result<DequeKey, EntityInsertError<'w>> {
        Self::deque_push_front(self, value)
    }
    /// value 须同 World，消费并转入节点，不重复计数；成功修改后返回前标 Changed。
    /// 准备拒绝交回原引用且不标记；写入或标记开始后不可恢复失败停止，不回滚。
    pub fn push_back(&mut self, value: OwnedEntity<'w>) -> Result<DequeKey, EntityInsertError<'w>> {
        Self::deque_push_back(self, value)
    }
    /// 移出原强责任，不减计数也不增持；直接从节点转入 OwnedEntity，不出现中间归零。
    /// 返回引用不独占借住容器头，成功取出后返回前标 Changed。
    /// 空为 None 且不标记；写入后不可恢复失败停止，不回滚。
    pub fn pop_front(&mut self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::deque_pop_front(self)
    }
    /// 移出原强责任，不减计数也不增持；直接从节点转入 OwnedEntity，不出现中间归零。
    /// 返回引用不独占借住容器头，成功取出后返回前标 Changed。
    /// 空为 None 且不标记；写入后不可恢复失败停止，不回滚。
    pub fn pop_back(&mut self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        Self::deque_pop_back(self)
    }
    /// 本入口验证 key 属于当前头链且代际有效；value 同 World，拒绝交还原引用且不标记。
    /// 成功插入后返回前标 Changed 并返回非拥有节点键；写入后不可恢复失败停止。
    pub fn insert_before(
        &mut self,
        key: DequeKey,
        value: OwnedEntity<'w>,
    ) -> Result<DequeKey, EntityInsertError<'w>> {
        Self::deque_insert_before(self, key, value)
    }
    /// 本入口验证 key 属于当前头链且代际有效；value 同 World，拒绝交还原引用且不标记。
    /// 成功插入后返回前标 Changed 并返回非拥有节点键；写入后不可恢复失败停止。
    pub fn insert_after(
        &mut self,
        key: DequeKey,
        value: OwnedEntity<'w>,
    ) -> Result<DequeKey, EntityInsertError<'w>> {
        Self::deque_insert_after(self, key, value)
    }
    /// 本入口验证 key 属于当前头链且代际有效；移出已有强责任直接转入 OwnedEntity，不 release/acquire。
    /// 成功移出后返回前标 Changed；同一目标其他节点责任不受影响。
    /// 无效/跨容器键前置拒绝不改链且不标记，写入后不可恢复失败停止。
    pub fn remove(&mut self, key: DequeKey) -> Result<OwnedEntity<'w>, EcsError> {
        Self::deque_remove(self, key)
    }
}
// 专用写操作统一顺序：校验/准备→修改成员/链接并转移责任→按需标记 tick/通知→返回。
// Deque 按键验证单源：同一次池短借内查完整 key，再从当前头沿 next 检查成员资格。
// 只遍历本头声明 len 上界并校验首尾/相邻链接；未遇 key 为跨头拒绝，不改链/分配节点/标记。
// 断链/环/len 不一致为内部不变量损坏并标停，不当普通非法键；成功后才委托 DequeOps。
// 校验至依赖调用间不释放池借用或回调；insert 输入保持原责任。按键方法由框架验证，不转嫁调用者。
// 验收覆盖两个非空头交换有效 key、已删除旧 key、空头使用其他头 key；拒绝后两头和计数不变。
// pop/remove：校验完整链/键 → 短借节点池移出并修正头 → 无展开地建立 OwnedEntity → 释放池借用 → 标记 → 返回。
// push/insert：持有输入 OwnedEntity 完成来源/键/容量检查与节点分配 → 无展开地发布头/链接并解除输入责任 → 释放池借用 → 标记。
// 在原责任仍有效时创建的未发布节点只含裸身份，不因此新增 strong；不可让准备节点被其他遍历看到。
// clear：每步移出、建立 OwnedEntity、结束池借用后 Drop；若非空，首个成功修改即在可展开后续工作前标记。
// 已移交值不得在 panic 清理重复释放，剩余链仍有效；链/责任窗口损坏标停并 abort，不以普通 Err 返回半交接。
// WholeState 特殊释放保持独立模式，不构造会登记普通归零业务的 OwnedEntity，也不发 Changed。
// len/front/iter 不标；无效键、准备拒绝、空 pop/clear 不标；不依赖 Drop 或系统末尾标记。
// 同一写对象只登记一次 Changed，重新取得对象可再登记；Changed 表示成员/顺序而非目标组件变化。
/// 框架整头释放：清空 List 节点、逐份交还 strong，再丢弃头；不发布业务 Removed。
/// Normal 归零只登记，不递归销毁；WholeState 逐份交还计数但不登记 pending/reclamation，
/// 不构造 OwnedEntity。具体 ComponentPolicy<EntityList<T>> 释放路径以 ::<T> 调用。
/// # Safety
/// head 必须指向已初始化、正确对齐且拥有唯一释放责任的 EntityList<T>，不得按 EntityList<()> 擦除解释。
/// 调用方已取得维护授权、结束头与节点借用；context 必须属于同一 World，来源分配保活至返回。
pub(crate) unsafe fn release_list<'a, 'w, T: 'static>(
    context: &ReleaseContext<'w>,
    head: OwningPtr<'a>,
) {
    entity_container_inner::release_list_inner::<T>(context, head)
}
/// 框架整头释放：清空 Queue 节点并逐份交还 strong，再丢弃头；释放模式同 release_list。
/// 具体 ComponentPolicy<EntityQueue<T>> 释放路径以 ::<T> 调用。
/// # Safety
/// head 必须指向已初始化、正确对齐且拥有唯一释放责任的 EntityQueue<T>，不得按 EntityQueue<()> 擦除解释。
/// 维护授权、同源上下文、借用结束及来源分配保活要求同 release_list。
pub(crate) unsafe fn release_queue<'a, 'w, T: 'static>(
    context: &ReleaseContext<'w>,
    head: OwningPtr<'a>,
) {
    entity_container_inner::release_queue_inner::<T>(context, head)
}
/// 框架整头释放：用 DequeOps::clear 清空节点并逐份交还 strong，再丢弃头；释放模式同 release_list。
/// 具体 ComponentPolicy<EntityDeque<T>> 释放路径以 ::<T> 调用。
/// # Safety
/// head 必须指向已初始化、正确对齐且拥有唯一释放责任的 EntityDeque<T>，不得按 EntityDeque<()> 擦除解释。
/// 维护授权、同源上下文、借用结束及来源分配保活要求同 release_list。
pub(crate) unsafe fn release_deque<'a, 'w, T: 'static>(
    context: &ReleaseContext<'w>,
    head: OwningPtr<'a>,
) {
    entity_container_inner::release_deque_inner::<T>(context, head)
}
// QueryData 经 component::read_component/write_component 统一策略包装单槽，专用对象共享池能力并逐操作短借；
// 写入沿原 ChangeMarker，完整释放由 ReleaseContext 与登记的 ComponentInfo.release 承担，具体芯仍待实现。
