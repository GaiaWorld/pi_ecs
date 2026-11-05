//! 内置引用记录与临时 OwnedEntity 分离；存储记录不持 World 指针或控制块。
use crate::{entity::Entity, world::World};
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
/// 不持有整个可变 World，不授权组件读写；Drop 校验完整身份及控制记录并交还一份责任。
#[must_use = "丢弃会交还强引用"]
pub struct OwnedEntity<'w> {
    /// 保证释放时 World 仍有效；只通过受限计数通道操作，不代表组件借用。
    /// 宿主引用可跨普通维护，恢复/关闭需另行排除；系统引用不得超出运行授权。
    pub(crate) world: &'w World,
    /// 完整实体身份。
    pub(crate) entity: Entity,
}
impl<'w> OwnedEntity<'w> {
    /// 新增独立一份 strong，返回寿命绑定原 World/授权而非本次 self；不实现不可失败的 Clone。
    /// 计数溢出/授权拒绝不改原责任，不标组件 Changed，底层协议同 World::try_acquire。
    pub fn try_clone(&self) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
        crate::owned_entity_inner::try_clone_inner(self)
    }

    /// 非拥有身份，不增加计数或授予手动释放资格。
    pub fn entity(&self) -> Entity {
        crate::owned_entity_inner::entity_inner(self)
    }
    /// 消费引用但不减计数，移交一次 unsafe despawn 责任；不得跨 World/分支误释放。
    pub fn forget(self) -> Entity {
        crate::owned_entity_inner::forget_inner(self)
    }
}
impl Drop for OwnedEntity<'_> {
    /// 校验完整代际/控制状态，不仅检查组件存在性；归零只登记，不递归销毁。
    fn drop(&mut self) {
        crate::owned_entity_inner::drop_inner(self)
    }
}
/// 内置强引用组件的本次专用写访问，不实现 DerefMut，不公开存储记录替换能力。
pub struct StrongEntitiesMut<'a, 'w, const N: usize, T = ()> {
    /// 当前实体的组件头写借用，不是整列可变引用。
    pub(crate) value: &'a mut StrongEntities<N, T>,
    /// 本次受限计数能力所依赖的 World；授权必须覆盖所有转出引用。
    pub(crate) world: &'w World,
    /// 与普通 Mut 共用的受限变化标记能力，归属本组件而非引用目标。
    pub(crate) marker: crate::change_detection::ChangeMarker<'a>,
    /// 本访问对象首次成功修改后置位，后续修改不重复追加 Changed 通知。
    pub(crate) marked: bool,
}
impl<'w, const N: usize, T> StrongEntitiesMut<'_, 'w, N, T> {
    /// index 为位置，value 来自同 World；替换转移新旧责任不重复计数，返回旧拥有引用。
    /// 越界/异 World 错误交回 value，不改原位置且不标 Changed。
    /// 成功替换后返回前按需求标记，即使新旧 Entity 相同；同一对象只追加一次通知。
    /// 写入或标记发生不可恢复失败时停止，不回滚。
    pub fn replace(
        &mut self,
        index: usize,
        value: OwnedEntity<'w>,
    ) -> Result<OwnedEntity<'w>, crate::entity_container::EntityInsertError<'w>> {
        crate::owned_entity_inner::replace_inner(self, index, value)
    }
}
/// 内置弱数组的专用写访问，无 DerefMut/移出/替换整个数组能力。
pub struct WeakEntitiesMut<'a, 'w, const N: usize, T = ()> {
    /// 本实体弱数组的独占借用。
    pub(crate) value: &'a mut WeakEntities<N, T>,
    /// 本次计数能力；允许成员原子升级，逐位置弱责任替换接口仍待设计。
    pub(crate) world: &'w World,
    /// 与普通 Mut 共用的受限变化标记能力，归属本组件而非引用目标。
    pub(crate) marker: crate::change_detection::ChangeMarker<'a>,
    /// 本访问对象首次成功修改后置位，后续修改不重复追加 Changed 通知。
    pub(crate) marked: bool,
}
/// 固定Strong数组的共享访问；裸身份读取不增持，拥有读取仅作用于本数组成员。
pub struct StrongEntitiesRef<'a, 'w, const N: usize, T = ()> {
    /// 当前数组共享借用，不允许移出存储记录。
    pub(crate) value: &'a StrongEntities<N, T>,
    /// 本次 World/计数授权，覆盖返回拥有引用的存活期。
    pub(crate) world: &'w World,
}
impl<'w, const N: usize, T> StrongEntitiesRef<'_, 'w, N, T> {
    /// index 越界报错；返回裸身份，不增持、不标 Changed，身份可能已失效。
    pub fn get(&self, index: usize) -> Result<Entity, crate::error::EcsError> {
        crate::owned_entity_inner::strong_ref_get(self, index)
    }
    /// index 越界/计数溢出报错且不改计数；只新增 strong，不修改数组、不标 Changed。
    /// 有效强位置保持目标存活；返回独立强责任，不借住本次 self。
    pub fn get_owned(&self, index: usize) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
        crate::owned_entity_inner::strong_ref_get_owned(self, index)
    }
}
impl<'w, const N: usize, T> StrongEntitiesMut<'_, 'w, N, T> {
    /// index 越界报错；返回裸身份，不增持、不标 Changed，身份可能已失效。
    pub fn get(&self, index: usize) -> Result<Entity, crate::error::EcsError> {
        crate::owned_entity_inner::strong_mut_get(self, index)
    }
    /// index 越界/计数溢出报错且不改计数；只新增 strong，不修改数组、不标 Changed。
    /// 有效强位置保持目标存活；返回独立强责任，不借住本次 self。
    pub fn get_owned(&self, index: usize) -> Result<OwnedEntity<'w>, crate::error::EcsError> {
        crate::owned_entity_inner::strong_mut_get_owned(self, index)
    }
}
/// 固定Weak数组的共享访问；裸身份读取不增持，拥有读取仅作用于本数组成员。
pub struct WeakEntitiesRef<'a, 'w, const N: usize, T = ()> {
    /// 当前数组共享借用，不允许移出存储记录。
    pub(crate) value: &'a WeakEntities<N, T>,
    /// 本次 World/计数授权，覆盖返回拥有引用的存活期。
    pub(crate) world: &'w World,
}
impl<'w, const N: usize, T> WeakEntitiesRef<'_, 'w, N, T> {
    /// index 越界报错；返回裸身份，不增持、不标 Changed，身份可能已失效。
    pub fn get(&self, index: usize) -> Result<Entity, crate::error::EcsError> {
        crate::owned_entity_inner::weak_ref_get(self, index)
    }
    /// index 越界/计数溢出报错且不改计数；只新增 strong，不修改数组、不标 Changed。
    /// 目标非 Alive 时返回 None，原子升级与最后强释放仲裁，禁止复活。
    pub fn upgrade(&self, index: usize) -> Result<Option<OwnedEntity<'w>>, crate::error::EcsError> {
        crate::owned_entity_inner::weak_ref_upgrade(self, index)
    }
}
impl<'w, const N: usize, T> WeakEntitiesMut<'_, 'w, N, T> {
    /// index 越界报错；返回裸身份，不增持、不标 Changed，身份可能已失效。
    pub fn get(&self, index: usize) -> Result<Entity, crate::error::EcsError> {
        crate::owned_entity_inner::weak_mut_get(self, index)
    }
    /// index 越界/计数溢出报错且不改计数；只新增 strong，不修改数组、不标 Changed。
    /// 目标非 Alive 时返回 None，原子升级与最后强释放仲裁，禁止复活。
    pub fn upgrade(&self, index: usize) -> Result<Option<OwnedEntity<'w>>, crate::error::EcsError> {
        crate::owned_entity_inner::weak_mut_upgrade(self, index)
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
        crate::owned_entity_inner::weak_replace_from_inner(self, index, target)
    }
}
// 强/弱数组统一：逐位置替换成功后返回前标 Changed，同目标也标；只读/越界/拒绝不标。
// 同一访问对象只追加一次通知，重新取得可再登记；Weak 替换规则适用于后续逐位置操作。
// 固定数组由 EntityValues 的 strong_entities/weak_entities 专用请求初始化，不公开数组值构造。
// @to_design 请求安装/失败清理的授权表示、查询 GAT fetch 和注册前段编号协议待细化。
// 普通 &mut 描述 fetch 返回专用访问对象，不能用通用 Mut<T> 暴露内置头。
// 不宣称 Rust 能自动反射拒绝任意嵌套；框架必须封住普通构造、移出和任意值替换入口。
