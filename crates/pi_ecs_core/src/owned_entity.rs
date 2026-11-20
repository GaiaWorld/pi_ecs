//! 强数组每位置（包括 Removed 旧值）保有一份 strong，合法成员目标必 strong>0；头所属实体归零不等于成员目标归零。
//! 责任转移不经过 release/acquire；复制才增持。强责任与零计数并存是内部损坏，沿 OwnedEntity 不变量故障规则。
//! 内置引用记录与临时 OwnedEntity 分离；存储记录不持 World 指针或控制块。
#[path = "owned_entity_inner.rs"]
mod owned_entity_inner;
use crate::entity::Entity;
use std::marker::PhantomData;
/// 内置强引用组件，每个位置拥有一份责任；仅框架 spawn/Alter 创建和特殊释放。
/// 无公开构造/Clone/Drop，不作为普通组件字段使用，不允许裸可变引用逃逸。
pub struct StrongEntities<const N: usize, T = ()> {
    /// 完整身份，重复目标按位置分别计数，不使用节点池。
    pub(crate) entities: [Entity; N],
    /// 关系用途，不拥有 T。
    pub(crate) marker: PhantomData<fn() -> T>,
}
/// 内置弱引用组件；不保活组件，只保留目标身份槽，释放由框架处理。
pub struct WeakEntities<const N: usize, T = ()> {
    /// 每位置一份弱引用责任。
    pub(crate) entities: [Entity; N],
    /// 关系用途，不拥有 T。
    pub(crate) marker: PhantomData<fn() -> T>,
}
/// 临时拥有型强引用；寿命绑定 World/授权，不能放入要求 static 的组件存储。
/// 不持有 World 指针或整个 World 借用；字段级身份表与登记引用保活，Drop 只交还一份责任。
#[must_use = "丢弃会交还强引用"]
pub struct OwnedEntity<'w> {
    /// 来源 World 的唯一身份，转交时拒绝跨 World/恢复分支不兼容输入。
    pub(crate) world_identity: u64,
    /// 完整实体身份，每个对象拥有一份 strong 责任。
    pub(crate) entity: Entity,
    /// 寿命绑定身份表；rc 为 RefCell，非 rc 为原子后端，不提供组件权限。
    pub(crate) entities: &'w crate::entity_store::EntityIdentityStore,
    /// strong 归零只登记；此引用必须活到拥有对象最后释放。
    pub(crate) pending_destruction: &'w crate::destruction::PendingDestruction,
    /// 共享停止状态；新增责任须先拒绝 Stopped，释放已有责任仍允许，不提供解锁能力。
    pub(crate) access: &'w crate::world::AccessState,
}
impl<'w> OwnedEntity<'w> {
    /// # 用途
    /// 新增独立一份 strong，返回寿命绑定原 World/授权而非本次 self；不实现不可失败的 Clone。
    /// 先读共享停止状态，Stopped 返回错误且不借身份表；溢出/增持拒绝不改原责任，不标 Changed。
    /// rc 短借身份表，非 rc 使用原子增持；成功结果原样携带同一身份表、登记和停止状态。
    /// 有效拥有责任却身份失效/计数为零或身份表重入属于不变量破坏：尽可能标停后 abort，不伪装普通失败。
    pub fn try_clone(&self) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
        /* 设计已确认：
        先拒绝 Stopped；有效对象的完整身份/计数必须成立，rc 仅短借身份表。
        满额返回 Err 不变；增持成功后结束表借用，再构造同寿命同字段引用的拥有对象。
        内部责任不变量破坏标停并 abort，不能以错误结果掩盖损坏。
        */
        Self::try_clone_inner(self)
    }

    /// 非拥有身份，不增加计数或授予手动释放资格。
    pub fn entity(&self) -> Entity {
        Self::entity_inner(self)
    }
    /// 消费引用但不减计数，移交一次 unsafe despawn 责任；不得跨 World/分支误释放。
    pub fn forget(self) -> Entity {
        Self::forget_inner(self)
    }
}
impl Drop for OwnedEntity<'_> {
    /// # 用途
    /// 交还本对象唯一 strong 责任；Stopped 也必须执行，不检查组件存在性、不分配 tick/发通知。
    /// 先短借身份表校验并释放，结束借用后仅由归零转换登记 PendingDestroy，不递归物理销毁。
    /// 身份失效/下溢/动态借用重入是不变量破坏，尽可能标停后 abort，禁止忽略/重试或二次 panic。
    /// 归零登记不可恢复失败不得丢弃工作项或继续 World；采用终止策略，具体分配失败沿分配器行为。
    fn drop(&mut self) {
        Self::drop_inner(self)
    }
}
/// 内置强引用组件的本次专用写访问，不实现 DerefMut，不公开存储记录替换能力。
pub struct StrongEntitiesMut<'a, 'w, const N: usize, T = ()> {
    /// 当前实体的组件头写借用，不是整列可变引用。
    pub(crate) value: &'a mut StrongEntities<N, T>,
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
impl<'w, const N: usize, T> StrongEntitiesMut<'_, 'w, N, T> {
    /// index 为位置，value 来自同 World；替换转移新旧责任不重复计数，返回旧拥有引用。
    /// 越界/异 World 错误交回 value，不改原位置且不标 Changed。
    /// 新旧强责任直接交换，不先释放再增持；即使同 Entity 也不改变总 strong，不出现中间归零。
    /// 成功替换后返回前按需求标记，即使新旧 Entity 相同；同一对象只追加一次通知。
    /// 写入或标记发生不可恢复失败时停止，不回滚。
    /// 新旧责任交换与构造返回旧 OwnedEntity 是无分配/回调/展开窗口；先建立旧拥有对象再进行变化标记。
    /// 标记 panic 时旧对象在栈上清偿，新责任已归组件；不能重复释放新输入或遗漏旧责任。
    pub fn replace(
        &mut self,
        index: usize,
        value: OwnedEntity<'w>,
    ) -> Result<OwnedEntity<'w>, crate::entity_container::EntityInsertError<'w>> {
        Self::replace_inner(self, index, value)
    }
}
/// 内置弱数组的专用写访问，无 DerefMut/移出/替换整个数组能力。
pub struct WeakEntitiesMut<'a, 'w, const N: usize, T = ()> {
    /// 本实体弱数组的独占借用。
    pub(crate) value: &'a mut WeakEntities<N, T>,
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
/// 固定Strong数组的共享访问；裸身份读取不增持，拥有读取仅作用于本数组成员。
pub struct StrongEntitiesRef<'a, 'w, const N: usize, T = ()> {
    /// 当前数组共享借用，不允许移出存储记录。
    pub(crate) value: &'a StrongEntities<N, T>,
    /// 所属 World 唯一身份；跨域输入前置拒绝。
    pub(crate) world_identity: u64,
    /// 只用于身份及 strong/weak 短借用，不携带组件权限；结果可保留到 'w。
    pub(crate) entities: &'w crate::entity_store::EntityIdentityStore,
    /// 归零只登记，禁止访问对象/临时引用隐式物理销毁。
    pub(crate) pending_destruction: &'w crate::destruction::PendingDestruction,
    /// 共享停止状态；新增责任须先拒绝 Stopped，释放已有责任仍允许，不提供解锁能力。
    pub(crate) access: &'w crate::world::AccessState,
}
impl<'w, const N: usize, T> StrongEntitiesRef<'_, 'w, N, T> {
    /// index 越界报错；返回裸身份，不增持、不标 Changed，复制出的身份不单独保活目标。
    pub fn get(&self, index: usize) -> Result<Entity, crate::error::EcsError> {
        Self::strong_ref_get(self, index)
    }
    /// index 越界/计数溢出报错且不改计数；只新增 strong，不修改数组、不标 Changed。
    /// 有效强位置保持目标 strong>0（Removed 旧数组同样如此）；返回独立责任，不借住本次 self。
    /// 先拒绝 Stopped，满额普通 Err；合法强位置却零计数/身份失效按 OwnedEntity 不变量故障规则，不返回 None。
    pub fn get_owned(&self, index: usize) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
        Self::strong_ref_get_owned(self, index)
    }
}
impl<'w, const N: usize, T> StrongEntitiesMut<'_, 'w, N, T> {
    /// index 越界报错；返回裸身份，不增持、不标 Changed，复制出的身份不单独保活目标。
    pub fn get(&self, index: usize) -> Result<Entity, crate::error::EcsError> {
        Self::strong_mut_get(self, index)
    }
    /// index 越界/计数溢出报错且不改计数；只新增 strong，不修改数组、不标 Changed。
    /// 有效强位置保持目标 strong>0（Removed 旧数组同样如此）；返回独立责任，不借住本次 self。
    /// 先拒绝 Stopped，满额普通 Err；合法强位置却零计数/身份失效按 OwnedEntity 不变量故障规则，不返回 None。
    pub fn get_owned(&self, index: usize) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
        Self::strong_mut_get_owned(self, index)
    }
}
/// 固定Weak数组的共享访问；裸身份读取不增持，拥有读取仅作用于本数组成员。
pub struct WeakEntitiesRef<'a, 'w, const N: usize, T = ()> {
    /// 当前数组共享借用，不允许移出存储记录。
    pub(crate) value: &'a WeakEntities<N, T>,
    /// 所属 World 唯一身份；跨域输入前置拒绝。
    pub(crate) world_identity: u64,
    /// 只用于身份及 strong/weak 短借用，不携带组件权限；结果可保留到 'w。
    pub(crate) entities: &'w crate::entity_store::EntityIdentityStore,
    /// 归零只登记，禁止访问对象/临时引用隐式物理销毁。
    pub(crate) pending_destruction: &'w crate::destruction::PendingDestruction,
    /// 共享停止状态；新增责任须先拒绝 Stopped，释放已有责任仍允许，不提供解锁能力。
    pub(crate) access: &'w crate::world::AccessState,
}
impl<'w, const N: usize, T> WeakEntitiesRef<'_, 'w, N, T> {
    /// index 越界报错；返回裸身份，不增持、不标 Changed，复制出的身份不单独保活目标。
    pub fn get(&self, index: usize) -> Result<Entity, crate::error::EcsError> {
        Self::weak_ref_get(self, index)
    }
    /// index 越界/计数溢出报错且不改计数；只新增 strong，不修改数组、不标 Changed。
    /// 目标 strong=0 或位置 Null 返回 None，不因组件查询仍可见而升级；不改变原 weak 责任。
    /// RC 短借身份表，非 RC 原子仲裁；weak 有效而身份槽失效属于责任损坏，不能当正常到期。
    pub fn upgrade(&self, index: usize) -> Result<Option<OwnedEntity<'w>>, crate::error::EcsError> {
        Self::weak_ref_upgrade(self, index)
    }
}
impl<'w, const N: usize, T> WeakEntitiesMut<'_, 'w, N, T> {
    /// index 越界报错；返回裸身份，不增持、不标 Changed，复制出的身份不单独保活目标。
    pub fn get(&self, index: usize) -> Result<Entity, crate::error::EcsError> {
        Self::weak_mut_get(self, index)
    }
    /// index 越界/计数溢出报错且不改计数；只新增 strong，不修改数组、不标 Changed。
    /// 目标 strong=0 或位置 Null 返回 None，不因组件查询仍可见而升级；不改变原 weak 责任。
    /// RC 短借身份表，非 RC 原子仲裁；weak 有效而身份槽失效属于责任损坏，不能当正常到期。
    pub fn upgrade(&self, index: usize) -> Result<Option<OwnedEntity<'w>>, crate::error::EcsError> {
        Self::weak_mut_upgrade(self, index)
    }
}

impl<'w, const N: usize, T> WeakEntitiesMut<'_, 'w, N, T> {
    /// index 必须有效，target 须同 World/分支且持有效 strong；只新增 weak，不消费 target 或增加 strong。
    /// 先校验并取得新 weak，再替换位置、交还旧 weak，成功返回前标 Changed，同目标也标。
    /// 越界/异 World/weak 溢出前置拒绝不改位置/计数/标记；开始修改后不可恢复失败停止。
    /// 不返回弱拥有值；旧 weak 归零仅登记符合条件的延迟槽回收。
    pub fn replace_from(
        &mut self,
        index: usize,
        target: &OwnedEntity<'w>,
    ) -> Result<(), crate::error::EcsError> {
        Self::weak_replace_from_inner(self, index, target)
    }
}
// 强/弱数组统一：逐位置替换成功后返回前标 Changed，同目标也标；只读/越界/拒绝不标。
// 同一访问对象只追加一次通知，重新取得可再登记；Weak 替换规则适用于后续逐位置操作。
// 固定数组由 bundle_input 的 StrongArrayInput/WeakArrayInput 直接安装，不公开存储头构造。
// 输入安装/失败清理见 StrongArrayInput/WeakArrayInput 与 BundleWriter 的封闭类型化入口；
// 查询 GAT 通过 ComponentPolicy 强弱专用策略包装已定位单槽，编号与普通组件统一交错登记，不设前段。
// 普通 &mut 描述 fetch 返回专用访问对象，不能用通用 Mut<T> 暴露内置头。
// 不宣称 Rust 能自动反射拒绝任意嵌套；框架必须封住普通构造、移出和任意值替换入口。
