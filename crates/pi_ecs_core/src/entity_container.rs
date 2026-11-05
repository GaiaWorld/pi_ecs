//! 内置容器头、统一节点池及 Query 临时专用访问；存储值无 World 指针、无普通 Drop。
use crate::{entity::Entity, error::EcsError, owned_entity::OwnedEntity, world::World};
use std::marker::PhantomData;

/// 内置List的共享访问，借用头并绑定节点池能力，无构造/取走/可变头入口。
pub struct EntityListRef<'a, 'w, T: 'static> {
    /// 本次只读头，节点保护期覆盖遍历结果。
    pub(crate) value: &'a EntityList<T>,
    /// 节点访问受头借用期约束；增持结果只携带 World/运行授权寿命，不携带读权限。
    pub(crate) world: &'w World,
}

/// 内置Queue的共享访问，借用头并绑定节点池能力，无构造/取走/可变头入口。
pub struct EntityQueueRef<'a, 'w, T: 'static> {
    /// 本次只读头，节点保护期覆盖遍历结果。
    pub(crate) value: &'a EntityQueue<T>,
    /// 节点访问受头借用期约束；增持结果只携带 World/运行授权寿命，不携带读权限。
    pub(crate) world: &'w World,
}

/// 内置Deque的共享访问，借用头并绑定节点池能力，无构造/取走/可变头入口。
pub struct EntityDequeRef<'a, 'w, T: 'static> {
    /// 本次只读头，节点保护期覆盖遍历结果。
    pub(crate) value: &'a EntityDeque<T>,
    /// 节点访问受头借用期约束；增持结果只携带 World/运行授权寿命，不携带读权限。
    pub(crate) world: &'w World,
}
impl<T: 'static> EntityListRef<'_, '_, T> {
    /// 当前长度，只读、不增持且不标记 Changed。
    pub fn len(&self) -> usize {
        crate::entity_container_inner::list_read_len(self)
    }
    /// 当前是否为空，不消费节点或引用责任。
    pub fn is_empty(&self) -> bool {
        crate::entity_container_inner::list_read_empty(self)
    }
    /// 窥视头部裸身份，不增持；能力校验拒绝不改存储。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        crate::entity_container_inner::list_read_front(self)
    }
    /// 从本头按链顺序遍历裸 Entity；借用 self，阻止同期修改/回收，不授予目标组件权限。
    /// 返回错误不消费容器；节点内部不变量损坏停止，不以跳过节点掩盖。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        crate::entity_container_inner::list_read_iter(self)
    }
}
impl<T: 'static> EntityListMut<'_, '_, T> {
    /// 空检查，不标记变化。
    pub fn is_empty(&self) -> bool {
        crate::entity_container_inner::list_write_empty(self)
    }
    /// 借用本写对象只读遍历，迭代器结束前不能 push/pop/clear；不增持或标记。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        crate::entity_container_inner::list_write_iter(self)
    }
}
impl<T: 'static> EntityQueueRef<'_, '_, T> {
    /// 当前长度，只读、不增持且不标记 Changed。
    pub fn len(&self) -> usize {
        crate::entity_container_inner::queue_read_len(self)
    }
    /// 当前是否为空，不消费节点或引用责任。
    pub fn is_empty(&self) -> bool {
        crate::entity_container_inner::queue_read_empty(self)
    }
    /// 窥视头部裸身份，不增持；能力校验拒绝不改存储。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        crate::entity_container_inner::queue_read_front(self)
    }
    /// 从本头按链顺序遍历裸 Entity；借用 self，阻止同期修改/回收，不授予目标组件权限。
    /// 返回错误不消费容器；节点内部不变量损坏停止，不以跳过节点掩盖。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        crate::entity_container_inner::queue_read_iter(self)
    }
}
impl<T: 'static> EntityQueueMut<'_, '_, T> {
    /// 空检查，不标记变化。
    pub fn is_empty(&self) -> bool {
        crate::entity_container_inner::queue_write_empty(self)
    }
    /// 借用本写对象只读遍历，迭代器结束前不能 push/pop/clear；不增持或标记。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        crate::entity_container_inner::queue_write_iter(self)
    }
}
impl<T: 'static> EntityDequeRef<'_, '_, T> {
    /// 当前长度，只读、不增持且不标记 Changed。
    pub fn len(&self) -> usize {
        crate::entity_container_inner::deque_read_len(self)
    }
    /// 当前是否为空，不消费节点或引用责任。
    pub fn is_empty(&self) -> bool {
        crate::entity_container_inner::deque_read_empty(self)
    }
    /// 窥视头部裸身份，不增持；能力校验拒绝不改存储。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        crate::entity_container_inner::deque_read_front(self)
    }
    /// 从本头按链顺序遍历裸 Entity；借用 self，阻止同期修改/回收，不授予目标组件权限。
    /// 返回错误不消费容器；节点内部不变量损坏停止，不以跳过节点掩盖。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        crate::entity_container_inner::deque_read_iter(self)
    }
}
impl<T: 'static> EntityDequeMut<'_, '_, T> {
    /// 空检查，不标记变化。
    pub fn is_empty(&self) -> bool {
        crate::entity_container_inner::deque_write_empty(self)
    }
    /// 借用本写对象只读遍历，迭代器结束前不能 push/pop/clear；不增持或标记。
    pub fn iter(&self) -> Result<EntityChainIter<'_>, EcsError> {
        crate::entity_container_inner::deque_write_iter(self)
    }
}
/// 按头至尾遍历裸身份；复制结果不延长目标寿命，目标组件另经 Lookup 授权点查。
/// 活头与 Removed 旧头使用相同遍历协议；迭代器借用访问对象，不能提前回收节点。
pub struct EntityChainIter<'a> {
    /// 节点池读取能力，与访问对象及头的共享借用同保护期。
    pub(crate) world: &'a World,
    /// 下一节点凭据，不是目标实体拥有引用。
    pub(crate) next: Option<DequeKey>,
    /// 固定头的剩余节点数，用于约束遍历，不允许并发修改链。
    pub(crate) remaining: usize,
    /// 本容器归属，不使用目标 Entity 推导链归属。
    pub(crate) owner: &'a Option<ChainIdentity>,
}
impl Iterator for EntityChainIter<'_> {
    type Item = Entity;
    /// 返回 Copy 裸身份，不返回节点引用；内部损坏停止，正常尾部为 None。
    fn next(&mut self) -> Option<Self::Item> {
        crate::entity_container_inner::chain_next(self)
    }
}

impl<'w, T: 'static> EntityListRef<'_, 'w, T> {
    /// 对首成员新增一份 strong；空为 None，溢出/授权拒绝报错，不移除节点、不标 Changed。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::list_ref_front_owned(self)
    }
    /// 按链顺序逐项增持；创建不增持，提前结束不处理余项，结果不借住迭代器/头。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        crate::entity_container_inner::list_ref_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityListMut<'_, 'w, T> {
    /// 对首成员新增一份 strong；空为 None，溢出/授权拒绝报错，不移除节点、不标 Changed。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::list_mut_front_owned(self)
    }
    /// 按链顺序逐项增持；创建不增持，提前结束不处理余项，结果不借住迭代器/头。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        crate::entity_container_inner::list_mut_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityQueueRef<'_, 'w, T> {
    /// 对首成员新增一份 strong；空为 None，溢出/授权拒绝报错，不移除节点、不标 Changed。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::queue_ref_front_owned(self)
    }
    /// 按链顺序逐项增持；创建不增持，提前结束不处理余项，结果不借住迭代器/头。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        crate::entity_container_inner::queue_ref_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityQueueMut<'_, 'w, T> {
    /// 对首成员新增一份 strong；空为 None，溢出/授权拒绝报错，不移除节点、不标 Changed。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::queue_mut_front_owned(self)
    }
    /// 按链顺序逐项增持；创建不增持，提前结束不处理余项，结果不借住迭代器/头。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        crate::entity_container_inner::queue_mut_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityDequeRef<'_, 'w, T> {
    /// 对首成员新增一份 strong；空为 None，溢出/授权拒绝报错，不移除节点、不标 Changed。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::deque_ref_front_owned(self)
    }
    /// 按链顺序逐项增持；创建不增持，提前结束不处理余项，结果不借住迭代器/头。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        crate::entity_container_inner::deque_ref_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityDequeRef<'_, 'w, T> {
    /// 对尾成员增持；空为 None，普通拒绝不改链/计数、不标 Changed。
    pub fn back_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::deque_ref_back_owned(self)
    }
    /// key 必须属于本容器且代际有效；拒绝报错，无任意 Entity 增持权限，不标 Changed。
    pub fn get_owned(&self, key: DequeKey) -> Result<OwnedEntity<'w>, EcsError> {
        crate::entity_container_inner::deque_ref_get_owned(self, key)
    }
}
impl<'w, T: 'static> EntityDequeMut<'_, 'w, T> {
    /// 对首成员新增一份 strong；空为 None，溢出/授权拒绝报错，不移除节点、不标 Changed。
    /// 返回寿命绑定 World/授权，不借住本次 self；底层原子增持规则同 World::try_acquire。
    pub fn front_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::deque_mut_front_owned(self)
    }
    /// 按链顺序逐项增持；创建不增持，提前结束不处理余项，结果不借住迭代器/头。
    /// 迭代器借用 self 阻止修改；next 失败不跳过当前项、不撤销此前成功项；不标 Changed。
    pub fn iter_owned(&self) -> Result<EntityOwnedIter<'_, 'w>, EcsError> {
        crate::entity_container_inner::deque_mut_iter_owned(self)
    }
}
impl<'w, T: 'static> EntityDequeMut<'_, 'w, T> {
    /// 对尾成员增持；空为 None，普通拒绝不改链/计数、不标 Changed。
    pub fn back_owned(&self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::deque_mut_back_owned(self)
    }
    /// key 必须属于本容器且代际有效；拒绝报错，无任意 Entity 增持权限，不标 Changed。
    pub fn get_owned(&self, key: DequeKey) -> Result<OwnedEntity<'w>, EcsError> {
        crate::entity_container_inner::deque_mut_get_owned(self, key)
    }
}
/// 逐项增持的拥有遍历；每项成功只新增一份 strong，保留源节点及其原责任。
pub struct EntityOwnedIter<'a, 'w> {
    /// 借用本次访问对象和链，防止遍历期间修改或回收；错误时游标留在当前项。
    pub(crate) chain: EntityChainIter<'a>,
    /// 返回拥有引用的 World/运行授权寿命，不绑定迭代器的短借用。
    pub(crate) world: &'w World,
}
impl<'w> Iterator for EntityOwnedIter<'_, 'w> {
    type Item = Result<OwnedEntity<'w>, EcsError>;
    /// 尾部 None；增持拒绝返回 Err 且不推进，内部链损坏停止，不静默跳过。
    fn next(&mut self) -> Option<Self::Item> {
        crate::entity_container_inner::owned_next(self)
    }
}

// @to_design 节点保护期、计数操作与 ChangeMarker 的实际授权表示待闭合。
/// 非拥有节点凭据，校验 World/容器/代际，不授予头写权限。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DequeKey {
    /// World 和容器归属。
    pub(crate) owner: u128,
    /// 节点槽和代际。
    pub(crate) slot: u64,
}
/// World 内非拥有单链节点键。
#[derive(Clone, Copy)]
pub(crate) struct LinkKey {
    /// 槽和代际。
    pub(crate) slot: u64,
}
/// 稳定容器身份，不使用组件地址。
pub(crate) struct ChainIdentity {
    /// 归属及代际编码待细化。
    pub(crate) value: u128,
}
/// 单链节点，每份目标责任由 World 显式转移/释放，不持有临时引用。
pub(crate) struct EntityLink {
    /// 目标完整身份，一份强责任。
    pub(crate) value: Entity,
    /// 后继。
    pub(crate) next: Option<LinkKey>,
    /// 容器归属。
    pub(crate) owner: ChainIdentity,
}
/// 双链节点，前后链接不是实体强引用。
pub(crate) struct EntityDoubleLink {
    /// 一份目标强责任。
    pub(crate) value: Entity,
    /// 前驱。
    pub(crate) prev: Option<DequeKey>,
    /// 后继。
    pub(crate) next: Option<DequeKey>,
    /// 容器归属。
    pub(crate) owner: ChainIdentity,
}
/// World 统一节点池，后端 pi_slot 暂缓实现；分配/回收需并发安全。
pub(crate) struct EntityNodePools {
    // @to_design 节点存储及级联清理工作项待细化。
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
    /// 稳定归属。
    pub(crate) identity: Option<ChainIdentity>,
    /// 首节点键，具体单/双链解释由内置类型决定。
    pub(crate) head: Option<DequeKey>,
    /// 尾节点键。
    pub(crate) tail: Option<DequeKey>,
    /// 节点数，重复目标分别计数。
    pub(crate) len: usize,
    /// 用途标记，不拥有 T。
    pub(crate) marker: PhantomData<fn() -> T>,
}
/// Query 对 &mut EntityList 返回的专用访问对象，不实现 DerefMut，不可替换容器头。
pub struct EntityListMut<'a, 'w, T: 'static> {
    /// 当前实体实例的头写借用，已确定 owner，无需方法再传 Entity。
    pub(crate) value: &'a mut EntityList<T>,
    /// 临时 World 能力，节点/计数权限由参数准备授予，不是任意业务写权限。
    pub(crate) world: &'w World,
    /// 与普通 Mut 共用的受限变化标记能力，归属本组件而非引用目标。
    pub(crate) marker: crate::change_detection::ChangeMarker<'a>,
    /// 本访问对象首次成功修改后置位，后续修改不重复追加 Changed 通知。
    pub(crate) marked: bool,
}
impl<'w, T: 'static> EntityListMut<'_, 'w, T> {
    /// 当前节点数，不消费。
    pub fn len(&self) -> usize {
        crate::entity_container_inner::list_len(self)
    }
    /// 只读窥视首个目标身份，不增持或消费。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        crate::entity_container_inner::list_front(self)
    }
    /// 清空并交还节点及强引用责任，不递归立即销毁目标；非空成功才标 Changed，空操作不标。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        crate::entity_container_inner::list_clear(self)
    }
    /// value 须同 World，消费并转入节点，不重复计数；成功修改后返回前标 Changed。
    /// 准备拒绝交回原引用且不标记；写入或标记开始后不可恢复失败停止，不回滚。
    pub fn push_front(&mut self, value: OwnedEntity<'w>) -> Result<(), EntityInsertError<'w>> {
        crate::entity_container_inner::list_push_front(self, value)
    }
    /// 移出原强责任，不减计数；返回引用不独占借住容器头，成功取出后返回前标 Changed。
    /// 空为 None 且不标记；写入后不可恢复失败停止，不回滚。
    pub fn pop_front(&mut self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::list_pop_front(self)
    }
}
/// 内置容器头，T 区分用途；只在 spawn/Alter 特定输入创建，无公开 new/Drop。
pub struct EntityQueue<T: 'static> {
    /// 稳定归属。
    pub(crate) identity: Option<ChainIdentity>,
    /// 首节点键，具体单/双链解释由内置类型决定。
    pub(crate) head: Option<DequeKey>,
    /// 尾节点键。
    pub(crate) tail: Option<DequeKey>,
    /// 节点数，重复目标分别计数。
    pub(crate) len: usize,
    /// 用途标记，不拥有 T。
    pub(crate) marker: PhantomData<fn() -> T>,
}
/// Query 对 &mut EntityQueue 返回的专用访问对象，不实现 DerefMut，不可替换容器头。
pub struct EntityQueueMut<'a, 'w, T: 'static> {
    /// 当前实体实例的头写借用，已确定 owner，无需方法再传 Entity。
    pub(crate) value: &'a mut EntityQueue<T>,
    /// 临时 World 能力，节点/计数权限由参数准备授予，不是任意业务写权限。
    pub(crate) world: &'w World,
    /// 与普通 Mut 共用的受限变化标记能力，归属本组件而非引用目标。
    pub(crate) marker: crate::change_detection::ChangeMarker<'a>,
    /// 本访问对象首次成功修改后置位，后续修改不重复追加 Changed 通知。
    pub(crate) marked: bool,
}
impl<'w, T: 'static> EntityQueueMut<'_, 'w, T> {
    /// 当前节点数，不消费。
    pub fn len(&self) -> usize {
        crate::entity_container_inner::queue_len(self)
    }
    /// 只读窥视首个目标身份，不增持或消费。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        crate::entity_container_inner::queue_front(self)
    }
    /// 清空并交还节点及强引用责任，不递归立即销毁目标；非空成功才标 Changed，空操作不标。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        crate::entity_container_inner::queue_clear(self)
    }
    /// value 须同 World，消费并转入节点，不重复计数；成功修改后返回前标 Changed。
    /// 准备拒绝交回原引用且不标记；写入或标记开始后不可恢复失败停止，不回滚。
    pub fn push_back(&mut self, value: OwnedEntity<'w>) -> Result<(), EntityInsertError<'w>> {
        crate::entity_container_inner::queue_push_back(self, value)
    }
    /// 移出原强责任，不减计数；返回引用不独占借住容器头，成功取出后返回前标 Changed。
    /// 空为 None 且不标记；写入后不可恢复失败停止，不回滚。
    pub fn pop_front(&mut self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::queue_pop_front(self)
    }
}
/// 内置容器头，T 区分用途；只在 spawn/Alter 特定输入创建，无公开 new/Drop。
pub struct EntityDeque<T: 'static> {
    /// 稳定归属。
    pub(crate) identity: Option<ChainIdentity>,
    /// 首节点键，具体单/双链解释由内置类型决定。
    pub(crate) head: Option<DequeKey>,
    /// 尾节点键。
    pub(crate) tail: Option<DequeKey>,
    /// 节点数，重复目标分别计数。
    pub(crate) len: usize,
    /// 用途标记，不拥有 T。
    pub(crate) marker: PhantomData<fn() -> T>,
}
/// Query 对 &mut EntityDeque 返回的专用访问对象，不实现 DerefMut，不可替换容器头。
pub struct EntityDequeMut<'a, 'w, T: 'static> {
    /// 当前实体实例的头写借用，已确定 owner，无需方法再传 Entity。
    pub(crate) value: &'a mut EntityDeque<T>,
    /// 临时 World 能力，节点/计数权限由参数准备授予，不是任意业务写权限。
    pub(crate) world: &'w World,
    /// 与普通 Mut 共用的受限变化标记能力，归属本组件而非引用目标。
    pub(crate) marker: crate::change_detection::ChangeMarker<'a>,
    /// 本访问对象首次成功修改后置位，后续修改不重复追加 Changed 通知。
    pub(crate) marked: bool,
}
impl<'w, T: 'static> EntityDequeMut<'_, 'w, T> {
    /// 当前节点数，不消费。
    pub fn len(&self) -> usize {
        crate::entity_container_inner::deque_len(self)
    }
    /// 只读窥视首个目标身份，不增持或消费。
    pub fn front(&self) -> Result<Option<Entity>, EcsError> {
        crate::entity_container_inner::deque_front(self)
    }
    /// 清空并交还节点及强引用责任，不递归立即销毁目标；非空成功才标 Changed，空操作不标。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        crate::entity_container_inner::deque_clear(self)
    }
    /// value 须同 World，消费并转入节点，不重复计数；成功修改后返回前标 Changed。
    /// 准备拒绝交回原引用且不标记；写入或标记开始后不可恢复失败停止，不回滚。
    pub fn push_front(
        &mut self,
        value: OwnedEntity<'w>,
    ) -> Result<DequeKey, EntityInsertError<'w>> {
        crate::entity_container_inner::deque_push_front(self, value)
    }
    /// value 须同 World，消费并转入节点，不重复计数；成功修改后返回前标 Changed。
    /// 准备拒绝交回原引用且不标记；写入或标记开始后不可恢复失败停止，不回滚。
    pub fn push_back(&mut self, value: OwnedEntity<'w>) -> Result<DequeKey, EntityInsertError<'w>> {
        crate::entity_container_inner::deque_push_back(self, value)
    }
    /// 移出原强责任，不减计数；返回引用不独占借住容器头，成功取出后返回前标 Changed。
    /// 空为 None 且不标记；写入后不可恢复失败停止，不回滚。
    pub fn pop_front(&mut self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::deque_pop_front(self)
    }
    /// 移出原强责任，不减计数；返回引用不独占借住容器头，成功取出后返回前标 Changed。
    /// 空为 None 且不标记；写入后不可恢复失败停止，不回滚。
    pub fn pop_back(&mut self) -> Result<Option<OwnedEntity<'w>>, EcsError> {
        crate::entity_container_inner::deque_pop_back(self)
    }
    /// key 必须属于本容器且代际有效；value 同 World，拒绝交还原引用且不标记。
    /// 成功插入后返回前标 Changed 并返回非拥有节点键；写入后不可恢复失败停止。
    pub fn insert_before(
        &mut self,
        key: DequeKey,
        value: OwnedEntity<'w>,
    ) -> Result<DequeKey, EntityInsertError<'w>> {
        crate::entity_container_inner::deque_insert_before(self, key, value)
    }
    /// key 必须属于本容器且代际有效；value 同 World，拒绝交还原引用且不标记。
    /// 成功插入后返回前标 Changed 并返回非拥有节点键；写入后不可恢复失败停止。
    pub fn insert_after(
        &mut self,
        key: DequeKey,
        value: OwnedEntity<'w>,
    ) -> Result<DequeKey, EntityInsertError<'w>> {
        crate::entity_container_inner::deque_insert_after(self, key, value)
    }
    /// key 必须属于本容器且代际有效；成功移出强责任后返回前标 Changed 并返回临时引用。
    /// 无效/跨容器键前置拒绝不改链且不标记，写入后不可恢复失败停止。
    pub fn remove(&mut self, key: DequeKey) -> Result<OwnedEntity<'w>, EcsError> {
        crate::entity_container_inner::deque_remove(self, key)
    }
}
// 专用写操作统一顺序：校验/准备→修改成员/链接并转移责任→按需标记 tick/通知→返回。
// len/front/iter 不标；无效键、准备拒绝、空 pop/clear 不标；不依赖 Drop 或系统末尾标记。
// 同一写对象只登记一次 Changed，重新取得对象可再登记；Changed 表示成员/顺序而非目标组件变化。
// @to_design 通用 QueryData 关联策略 fetch、只读池遍历、标记通道及特殊释放实际授权待闭合。
