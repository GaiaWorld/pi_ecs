//! 双模式：rc 单线程用 WorldRef=&mut World，pin/write/结构修改直接持有独占借用；
//! 非 rc 多线程用 WorldRef=&World，修改依靠授权、原子/锁与并发存储，不做 shared→&mut 强转。
//! 参数层仍凭运行授权受控 unsafe 取值；宿主 WorldPin/WorldWrite 在 rc 下是普通安全借用。
//! RC 自动原型创建/变更通过 Bundle 直接写入；非 RC 结构修改延期。
#[path = "world_inner.rs"]
mod world_inner;

#[cfg(test)]
mod shared_maintenance_compile_checks {
    use crate::{
        alter::AlterState, component::ComponentId, entity::Entity,
        entity_container::EntityQueueMut, error::EcsError, world::World,
    };
    // 仅编译函数体，不作为运行测试调用失败桩；验证签名组合，不证明内部授权安全。
    fn host_resources<T: Send + Sync + 'static>(world: &mut World) -> Result<(), EcsError> {
        {
            let pin = world.pin()?;
            let value = pin.resource::<T>();
            let items = pin.multi_resource::<T>();
            drop((value, items));
        }
        {
            let mut access = world.write()?;
            let value = access.resource_mut::<T>();
            drop(value);
            let items = access.multi_resource::<T>();
            drop(items);
        }
        let removed = world.remove_resource::<T>()?;
        drop(removed);
        Ok(())
    }
    fn pin_read<C: crate::component::Component>(
        world: &mut World,
        entity: Entity,
    ) -> Result<(), EcsError> {
        let pin = world.pin()?;
        {
            let value = pin.get::<C>(entity);
            drop(value);
        }
        drop(pin);
        world.read_scope(|pin| {
            let value = pin.get::<C>(entity);
            drop(value);
        })?;
        world.flush_destruction()
    }
    #[cfg(not(feature = "rc"))]
    fn host_pop_then_change(
        world: &World,
        owner: Entity,
        entity: Entity,
        removed: &[ComponentId],
    ) -> Result<(), EcsError> {
        let owned = {
            let mut access = world.write()?;
            let Some(mut queue) = access.get_mut::<crate::entity_container::EntityQueue<()>>(owner)
            else {
                return Ok(());
            };
            queue.pop_front()?
        };
        if let Some(owned) = owned {
            let values = crate::bundle_input::StrongArrayInput::<1, ()>::new([owned])
                .map_err(|rejected| rejected.error)?;
            world.change_archetype(entity, values, removed)?;
        }
        Ok(())
    }
    #[cfg(not(feature = "rc"))]
    fn scope_pop<'w>(
        world: &'w mut World,
        owner: Entity,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, EcsError> {
        world.write_scope(|access| {
            let Some(mut queue) = access.get_mut::<crate::entity_container::EntityQueue<()>>(owner)
            else {
                return Ok(None);
            };
            queue.pop_front()
        })?
    }
    fn pin_owned<'w>(
        world: &'w mut World,
        owner: Entity,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, EcsError> {
        world.read_scope(|pin| {
            let Some(queue) = pin.get::<crate::entity_container::EntityQueue<()>>(owner) else {
                return Ok(None);
            };
            queue.front_owned()
        })?
    }
    #[cfg(not(feature = "rc"))]
    fn acquire_then_change(
        world: &World,
        entity: Entity,
        target: Entity,
        removed: &[ComponentId],
    ) -> Result<(), EcsError> {
        let owned = world.try_acquire(target)?;
        let values = crate::bundle_input::StrongArrayInput::<1, ()>::new([owned])
            .map_err(|rejected| rejected.error)?;
        world.change_archetype(entity, values, removed)?;
        world.flush_destruction()
    }
    #[cfg(not(feature = "rc"))]
    fn acquire_then_scope(
        world: &World,
        state: &mut AlterState,
        entity: Entity,
        target: Entity,
    ) -> Result<(), EcsError> {
        let owned = world.try_acquire(target)?;
        let values = crate::bundle_input::StrongArrayInput::<1, ()>::new([owned])
            .map_err(|rejected| rejected.error)?;
        state.scope(world, |alter| alter.alter(entity, values))??;
        Ok(())
    }
    #[cfg(not(feature = "rc"))]
    fn pop_then_change<T: 'static>(
        world: &World,
        mut source: EntityQueueMut<'_, '_, T>,
        entity: Entity,
        removed: &[ComponentId],
    ) -> Result<(), EcsError> {
        let Some(owned) = source.pop_front()? else {
            return Ok(());
        };
        drop(source);
        let values = crate::bundle_input::StrongArrayInput::<1, ()>::new([owned])
            .map_err(|rejected| rejected.error)?;
        world.change_archetype(entity, values, removed)
    }
}

#[cfg(not(feature = "rc"))]
use std::sync::atomic::{AtomicU32, AtomicU64};

use crate::{
    archetype::Archetype,
    component::{ComponentId, ComponentInfo, StoragePolicy},
    entity::ArchetypeId,
    entity::{Entity, EntityLocation},
    error::EcsError,
    partition::PartitionMove,
    registry::TypeRegistry,
    table::TableLayout,
};
/// 宿主/调度入口的世界借用：rc 单线程用独占可变借用，非 rc 多线程用共享只读借用。
/// 函数体内需要共享读时写 &*world，两种模式都成立；参数层仍可凭运行授权受控 unsafe 取值。
#[cfg(feature = "rc")]
pub type WorldRef<'a> = &'a mut World;
/// 非 rc 共享 World；并发结构与原子负责同步，不把共享引用强转可变。
#[cfg(not(feature = "rc"))]
pub type WorldRef<'a> = &'a World;
pub use world_inner::reborrow_world;
/// 宿主访问门禁模式；转换由 Stage 和宿主入口驱动，World 只保存当前值。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccessMode {
    /// 无授权：可进入多个只读 pin，或由 Stage 进入 Running/Maintaining。
    Idle,
    /// 宿主只读 pin 计数；计数为零回到 Idle。
    Pinning(usize),
    /// 系统运行中；排斥宿主 pin/write 与其他维护。
    Running,
    /// 结构维护中；排斥 pin/write 与系统运行。
    Maintaining,
    /// 不可恢复失败；不再进入任何运行/写/维护或组件读授权态。
    /// 仅允许不读取实体组件内容的只读诊断元数据；引用释放/关闭清理可继续，不能复活。
    Stopped,
}
/// rc 的借用负责独占；状态负责记录运行/维护及不可恢复停止，不能以借用替代失败状态。
#[cfg(feature = "rc")]
pub(crate) struct AccessState {
    /// 初始 Idle；实体会话进入 Maintaining，普通退出回 Idle，不可恢复失败保持 Stopped。
    pub(crate) mode: std::cell::Cell<AccessMode>,
}
/// 非 rc 门禁单元：原子字打包模式与 pin 计数，不使用 UnsafeCell 或手写 Sync。
/// 转换点只有宿主入口与 Stage 维护点；状态读写通过原子，工作线程只读已发布模式。
#[cfg(not(feature = "rc"))]
pub(crate) struct AccessState(
    /// 低 8 位模式码，其余位为 pin 计数；具体编解码在门禁芯实现。
    pub(crate) AtomicU32,
);
/// 支持分区的非泛型 World；游戏与渲染使用不同实例；持有核心固定身份实现和分区状态。
/// 组件/Table/原型/通知全局共享；实体按出生分区分配，当前分区只存于位置用于分类系统过滤。
pub struct World {
    /// 全 World 长期查询的 tick 需求登记，通知需求由 notifications.subscriptions 单源提供。
    pub(crate) tracking: crate::tracking::TrackingRegistry,
    /// RC 整体关闭的单向状态；独占访问，不与 AccessState 的运行/停止门禁混为一谈。
    #[cfg(feature = "rc")]
    pub(crate) shutdown: crate::shutdown::ShutdownState,
    /// 本 World 唯一且不复用的身份；由初始化入口从全局计数分配，恢复不改变。
    /// QueryState 用它拒绝跨 World 复用，不能只凭 layout_epoch 判断兼容。
    pub(crate) identity: u64,
    /// 唯一身份定位入口；rc 为 RefCell 局部可变身份表，非 rc 保留原子后端。
    /// 身份表借用不得跨业务析构/用户回调或逃逸到组件结果，不共享转可变 World。
    pub(crate) entities: crate::entity_store::EntityIdentityStore,
    /// 统一类型化命令服务，payload 原地延迟可见，外部入口不提供组件权限。
    pub(crate) commands: crate::command::CommandHub,
    /// 不随游戏恢复回退，旧后台结果必须拒绝。
    pub(crate) epoch: crate::command::WorldEpoch,
    /// 显式推进的模拟时间，不等于 change_tick。
    pub(crate) simulation_tick: crate::command::SimulationTick,
    /// 全 World 唯一资源实例。
    pub(crate) resources: crate::resource::ResourceStore,
    /// 每 World 的查询默认来源，独占维护修改，查询仅共享读取。
    pub(crate) query_defaults: crate::query_data::QueryDefaults,
    /// 生产者多例紧凑数组；结构变动更新 core 映射/版本，各参数在下次 prepare 惰性刷新偏移。
    pub(crate) multi_resources: crate::resource::MultiResourceStore,
    /// 强引用归零只登记；阶段末无组件借用时收束物理销毁。
    pub(crate) pending_destruction: crate::destruction::PendingDestruction,
    /// 单向/双向容器节点统一存储；节点不是实体组件，不带用途 T。
    pub(crate) entity_nodes: crate::entity_container::EntityNodePools,
    /// 构造后固定的分区数，默认1；上限为 MAX_PARTITIONS，当前分区编号须小于该值。
    pub(crate) partition_count: usize,
    /// 非零 u64 变化时钟，初始为 1，不是模拟帧；系统/宿主上下文取得一次 tick。
    /// AtomicU64 无非零类型，封装必须保持非零不变量，恢复拒绝零，耗尽报错不回绕。
    /// rc 为普通 u64，由 &mut 独占写；非 rc 为原子。
    #[cfg(feature = "rc")]
    pub(crate) change_tick: u64,
    /// 非 rc 原子时钟，支持共享读取与并发分配。
    #[cfg(not(feature = "rc"))]
    pub(crate) change_tick: AtomicU64,
    /// Added/Changed/Removed 的可靠多读者通知及历史旧值所有权。
    pub(crate) notifications: crate::notification::Notifications,
    /// 本 World 恢复后递增，所有布局相关派生计划须失效重建。
    pub(crate) layout_epoch: u64,
    /// 本 World 独占的类型、Table 布局及动态原型登记；不与另一 World 共用可变登记状态。
    pub(crate) registry: TypeRegistry,
    /// 冻结前为 None；全局 Table/稀疏/原型成员准备成功后一次安装。
    pub(crate) storage: Option<crate::storage::WorldStorage>,
    // 单一内部门禁：Idle 可进入多个宿主只读 pin 或 Running/Maintaining 排他态。
    // pin 存活时拒绝运行、组件写与维护；全部读授权释放后才可转入冲突状态。
    // 子 Schedule 复用根运行授权；完成任务/借用/提交收束后才回 Idle，不可恢复失败进停止态。
    // 宿主运行后使用具体资源/flush/settle/恢复入口，不返回整个 &mut World，也不共享转可变。
    // 停止态传播：entity_scope/write/pin/query/try_acquire/spawn/change_archetype、
    // 资源/默认值修改、通知订阅/回收、原型登记、flush/settle/迁区以及阶段运行均不得重新进入。
    // 各自 Result 入口须在数据访问与时钟分配前返回错误；停止不解释为缺组件/空集合。
    // 允许只读诊断元数据、拥有责任释放和整体关闭清理；禁止新增强/弱责任或重置停止状态。
    // RC EntityScope 与 WorldWrite 的组件访问先检查本次授权；本地通知失败不能被闭包捕获后解除。
    // World 只保存 AccessState；模式转换、等待与失败停机的驱动在 Stage 和宿主入口。
    /// # 用途
    /// 共享授权门禁及停止状态；不授予业务值访问，转换受宿主或根运行协议约束。
    pub(crate) access: AccessState,
}
// 非 rc 下 World 必须可跨线程共享；字段不满足 Sync 时此处编译失败，禁止用 unsafe impl 绕过。
// 依赖 storage 的 RowCell、SparseArray/RawArr 自身 Sync 不变量；运行期协议仍待实现验证。
#[cfg(not(feature = "rc"))]
const _: fn() = || {
    fn assert_sync<T: Sync>() {}
    assert_sync::<World>();
};
/// 宿主只读访问授权，不是 std::pin::Pin，也不增加实体强计数；无 Copy/Clone/提前解锁接口。
/// 可并存多个只读 pin；首版排斥系统运行、组件写、结构修改/销毁/settle/恢复。
/// Drop 归还一份授权；forget 只泄漏授权使冲突操作持续拒绝，不得失去旧借用保护。
pub struct WorldPin<'w> {
    /// World 保活借用，不向调用者暴露未经授权的组件入口；Drop 经 AccessState 归还读计数。
    pub(crate) world: &'w World,
}
impl<'w> WorldPin<'w> {
    /// entity 须为本 World 完整键；未知类型/无效实体/缺组件返回 None，不隐式登记。
    /// 普通值返回共享引用、内置类型返回只读对象，均借用 self，禁止越过 pin 解锁期。
    /// 裸读取不增持；成员 *_owned 所得寿命绑定 World，可在 pin 释放后存在，不携带读权限。
    pub fn get<C: crate::component::Component>(
        &self,
        entity: Entity,
    ) -> Option<crate::component::ComponentRead<'_, 'w, C>> {
        Self::pin_get_inner::<C>(self, entity)
    }
}
impl Drop for WorldPin<'_> {
    /// 释放读授权而不销毁业务值；panic 展开也须执行，不启动隐式维护。
    fn drop(&mut self) {
        Self::drop_pin_inner(self)
    }
}
/// 宿主独占组件访问，不是整个可变 World；不开放结构维护，无 Copy/Clone/提前解锁。
/// 与 pin、运行、其他写访问和维护互斥，Drop 释放，forget 只泄漏授权使冲突持续拒绝。
pub struct WorldWrite<'w> {
    /// rc 持有从 &mut World 取得的独占借用；普通安全代码直接写，不靠内部可变性。
    #[cfg(feature = "rc")]
    pub(crate) world: &'w mut World,
    /// 非 rc 只持有共享 World，内部访问由运行授权与并发存储约束。
    #[cfg(not(feature = "rc"))]
    pub(crate) world: &'w World,
    /// 成功取得写授权后分配的非零时间，全部组件/资源/容器标记共用，不在每项写入时重新分配。
    pub(crate) tick: crate::change_detection::ChangeTick,
    /// 本次点查已验证的声明；结果独占短借令牌，期间声明不能替换或释放。
    #[cfg(feature = "rc")]
    pub(super) point_declaration: Option<crate::access::AccessDeclaration>,
}
impl<'w> WorldWrite<'w> {
    /// 共享读取借用本访问对象；rc 结果只能绑定本次借用，不能越过 &mut World。
    #[cfg(feature = "rc")]
    pub fn get<C: crate::component::Component>(
        &self,
        entity: Entity,
    ) -> Option<crate::component::ComponentRead<'_, '_, C>> {
        Self::write_get_inner::<C>(self, entity)
    }
    /// 非 rc 共享读取；结果可绑定 World 寿命。
    #[cfg(not(feature = "rc"))]
    pub fn get<C: crate::component::Component>(
        &self,
        entity: Entity,
    ) -> Option<crate::component::ComponentRead<'_, 'w, C>> {
        Self::write_get_inner::<C>(self, entity)
    }
    /// 短借用防止冲突结果并存；rc 从独占 World 重借，结果绑定 &mut self。
    #[cfg(feature = "rc")]
    pub fn get_mut<C: crate::component::Component>(
        &mut self,
        entity: Entity,
    ) -> Option<crate::component::ComponentWrite<'_, '_, C>> {
        Self::write_get_mut_inner::<C>(self, entity)
    }
    /// 非 rc 写读取；结果可绑定 World 寿命。
    #[cfg(not(feature = "rc"))]
    pub fn get_mut<C: crate::component::Component>(
        &mut self,
        entity: Entity,
    ) -> Option<crate::component::ComponentWrite<'_, 'w, C>> {
        Self::write_get_mut_inner::<C>(self, entity)
    }
    /// 只准备本实体的组合点查，先检查 Q 内部别名/类型及目标，失败不改值、不创建全局缓存。
    /// RC：非法组件访问组合报 InvalidInput；无实体为 NotFound/NoSuchEntity，不匹配为 InvalidInput/NotMatched。
    /// 已停止的授权报 InvalidState。结果绑定 &mut self；不开放任意结构操作。
    #[cfg(feature = "rc")]
    pub fn query_one_mut<Q: crate::query::QueryData>(
        &mut self,
        entity: Entity,
    ) -> Result<Q::Item<'_, '_>, EcsError> {
        Self::write_query_one_inner::<Q>(self, entity)
    }
    /// 非 rc 结果可绑定 World 寿命。
    #[cfg(not(feature = "rc"))]
    pub fn query_one_mut<Q: crate::query::QueryData>(
        &mut self,
        entity: Entity,
    ) -> Result<Q::Item<'_, 'w>, EcsError> {
        Self::write_query_one_inner::<Q>(self, entity)
    }
}
impl Drop for WorldWrite<'_> {
    /// 仅释放独占授权，不分配 tick、不延迟标记/发布、不隐式维护；展开亦需释放，致命失败不能解除停止态。
    fn drop(&mut self) {
        Self::drop_write_inner(self)
    }
}
impl World {
    /// # 用途
    /// 从同源 cell 的 entity 和当前 location 读取 C；组件缺失返回 None，不分配 tick。
    /// 组件结果借用 'a；身份责任能力仍绑定原来源 'w。
    /// # Safety
    /// 调用者保证位置已发布且有效，整个 'a 无冲突写入或结构修改。
    #[cfg(feature = "rc")]
    pub(crate) unsafe fn read_component_at<'a, 'w: 'a, C: crate::component::Component>(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
        entity: Entity,
        location: EntityLocation,
    ) -> Option<crate::component::ComponentRead<'a, 'w, C>> {
        unsafe { Self::read_component_at_inner::<C>(cell, entity, location) }
    }

    /// # 用途
    /// 为作用域内 entity 的 C 绑定独占短结果；缺组件不分配 tick，命中分配一次。
    /// cell 和 location 必须同源；时钟耗尽标停并 panic，结果保留 'w 的身份责任能力。
    /// # Safety
    /// 调用者持 Maintaining，保证 location 当前有效且整个 'a 无冲突槽或结构访问。
    #[cfg(feature = "rc")]
    pub(crate) unsafe fn scope_write_component_at<'a, 'w: 'a, C: crate::component::Component>(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
        entity: Entity,
        location: EntityLocation,
    ) -> Option<crate::component::ComponentWrite<'a, 'w, C>> {
        unsafe { Self::scope_write_component_at_inner::<C>(cell, entity, location) }
    }

    /// # 用途
    /// 统一创建安装主体；host/scope 由 Maintaining 提供授权，系统由 Running 提供授权。
    /// cell 来自同源独占根；domain 是出生域；authorized/system_tick 是系统的有序声明和本轮时钟，宿主传 None。
    /// values 按 Bundle 协议直写，普通拒绝不发布实体；成功保持调用方持有的门禁模式。
    /// 行与身份移交后，通知发布失败在展开中标停；已发布值留给关闭清理。
    /// # Safety
    /// 调用者持匹配模式的结构权限，无冲突值、节点或布局借用；系统已验证有序声明。
    #[cfg(feature = "rc")]
    pub(crate) unsafe fn install_spawn<'w, B: crate::bundle::Bundle>(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
        domain: crate::entity::AllocationDomain,
        authorized: Option<&[ComponentId]>,
        system_tick: Option<crate::change_detection::ChangeTick>,
        values: B,
    ) -> Result<crate::owned_entity::OwnedEntity<'w>, EcsError> {
        unsafe { Self::install_spawn_inner(cell, domain, authorized, system_tick, values) }
    }

    /// # 用途
    /// 对 entity 严格增删组件：values 是新增 Bundle，removed 是待移除编号，均先校验。
    /// 空操作不分配 tick；成功发布一次修改，普通错误保留源实体与原门禁。
    /// # Safety
    /// cell 来自独占根，调用者已通过 Idle 或持有 Maintaining 结构授权，无冲突业务借用。
    #[cfg(feature = "rc")]
    pub(crate) unsafe fn change_archetype_in_cell<B: crate::bundle::Bundle>(
        cell: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
        entity: Entity,
        values: B,
        removed: &[ComponentId],
    ) -> Result<(), EcsError> {
        unsafe { Self::change_archetype_in_cell_inner(cell, entity, values, removed) }
    }

    /// # 用途
    /// 分配进程内非零且永不复用的身份；全局 checked 分配，耗尽后永久返回 ResourceExhausted。
    /// 不允许计数回绕恢复可用，不消耗失败后再次分配历史编号；并发构造须保持唯一性。
    pub(crate) fn allocate_identity() -> Result<u64, EcsError> {
        Self::allocate_identity_inner()
    }
    /// # 用途
    /// 在独占实体操作作用域中运行 run；返回 R 不得携带本作用域拥有引用或组件借用。
    /// run 可跨操作保存 OwnedEntity，但组件借用须在下一次冲突操作前结束。
    /// 普通入口错误不调用 run；闭包错误原样返回，已完成操作不回滚；panic 不得捕获后继续使用 World。
    /// 正常/展开退出释放局部拥有责任和授权；不自动 flush_destruction、settle 或推进模拟时间。
    /// rc 由 &mut World 排除外部访问；非 rc 同样需要唯一宿主维护会话。
    /// 仅在 Idle、registry 已冻结且 storage 已安装时执行 run；拒绝不取 tick、不修改状态。
    /// 入口从宿主独占根派生同源能力，进入 Maintaining 并由私有 EntityScopeSession 管理退出。
    /// run 返回 Ok/普通 Err 后结束字段借用并归还会话；已停止的状态不得因正常返回恢复 Idle。
    /// panic 展开由外层会话置 Stopped；局部引用只交还责任，不隐式物理销毁，不吞掉 panic。
    /// EntityScope 本身不持有门禁 guard，forget 访问对象不能跳过外层退出保护。
    /// 首次创建修改后的失败须先停止再 panic；闭包内捕获不能允许后续操作或恢复 Idle。
    /// 写/维护入口均须遵守停止态拒绝及内部先停止后 panic；实际接线仍为配对芯实现责任。
    pub fn entity_scope<R>(
        &mut self,
        run: impl for<'s> FnOnce(&mut crate::entity_scope::EntityScope<'s>) -> Result<R, EcsError>,
    ) -> Result<R, EcsError> {
        /* 设计已确认：
        只读检查冻结/存储/Idle，失败不执行 run、不分配 tick。
        建立外层 EntityScopeSession 并进入 Maintaining；门禁字段与业务字段分开借用。
        用同源受限能力建立高阶寿命 EntityScope，执行 run；组件短借用不得外逃，拥有责任只在会话内。
        返回 Ok/普通 Err：结束 EntityScope 字段借用，完成会话；保持已停止状态，否则返回 Idle。
        panic 展开：局部拥有责任正常释放，外层会话 Drop 置 Stopped，继续展开，不回滚。
        即使闭包内部捕获了操作 panic，停止标记仍有效；后续读写操作拒绝，退出不得复活。
        不自动销毁/整理/登记原型或分配 tick，操作各自负责既定发布，非事务。
        */
        Self::entity_scope_inner(self, run)
    }
    /// 共享读取当前已分配的非零变化时钟，不递增、不代表并发写入已完成，不是快照/内存完成屏障。
    /// 宿主在相关写者收束后取观察上界；恢复后进度沿用已安装时钟，旧分支游标须另校验。
    pub fn read_change_tick(&self) -> crate::change_detection::ChangeTick {
        Self::read_change_tick_inner(self)
    }
    /// # 用途
    /// RC 静态独占不替代状态检查：仅 Idle/未关闭且无遗留授权时取得写授权。
    /// Running/Pinning/Maintaining/Stopped/关闭均在数据访问和分配 tick 前拒绝。
    /// 成功后 checked 分配一次非零 tick；耗尽不交付对象，释放本次授权，不回绕。
    /// 之后写包装共用该 tick，实际标记才产生变化；展开由原守卫标停，不能恢复运行。
    #[cfg(feature = "rc")]
    pub fn write(&mut self) -> Result<WorldWrite<'_>, EcsError> {
        Self::write_inner(self)
    }
    /// 非 rc 申请宿主独占组件访问，冲突前置报错不等待且不消耗 tick。
    /// 取得授权后分配一次非零 ChangeTick；耗尽报错并释放本次授权，不返回写对象、不回绕。
    /// 成功但未修改值可消耗 tick，不产生值变化或通知；普通/内置/资源全部写包装共用该 tick。
    #[cfg(not(feature = "rc"))]
    pub fn write(&self) -> Result<WorldWrite<'_>, EcsError> {
        /* 设计已确认：
        检查运行状态并取得独占访问授权，冲突立即拒绝，不推进时钟。
        原子分配本次非零 tick；耗尽则清理本次授权并报错，不发布可写对象。
        成功构造持授权与固定 tick 的 WorldWrite；后续实际标记复用 tick。
        无写入也不退回 tick，不生成业务通知；失败耗尽后的停止状态按全局时钟契约处理。
        */
        Self::write_inner(self)
    }
    /// # 用途
    /// RC 完整复用 World::write 的状态检查与一次 tick 分配；前置拒绝不调用 run。
    /// run 仅短借写对象，R 不得含本次组件引用；正常结束释放授权，panic 标停并原样传播。
    /// Changed 发布失败局部标停，闭包内捕获 panic 后仍拒绝组件与资源访问，退出不恢复 Idle。
    /// 已发布写入不回滚，不补 finish，不自动 flush/settle。
    #[cfg(feature = "rc")]
    pub fn write_scope<'w, R>(
        &'w mut self,
        run: impl for<'a> FnOnce(&'a mut WorldWrite<'w>) -> R,
    ) -> Result<R, EcsError> {
        Self::write_scope_inner(self, run)
    }
    /// 非 rc 完整复用 write 的授权与一次 tick 分配；不为闭包另取 tick。
    /// R 可含 World 寿命的 OwnedEntity，不得含本次组件借用。
    /// 拒绝不调用 run，正常/展开释放授权，闭包中结构维护冲突须报错。
    #[cfg(not(feature = "rc"))]
    pub fn write_scope<'w, R>(
        &'w self,
        run: impl for<'a> FnOnce(&'a mut WorldWrite<'w>) -> R,
    ) -> Result<R, EcsError> {
        Self::write_scope_inner(self, run)
    }
    /// rc 取得唯一只读授权：&mut World 被重借为 &World，pin 期间不能再取得 &mut。
    #[cfg(feature = "rc")]
    pub fn pin(&mut self) -> Result<WorldPin<'_>, EcsError> {
        Self::pin_inner(self)
    }
    /// 非 rc 取得一份宿主只读授权，多读允许；运行/维护/停止态冲突前置报错。
    /// 所有冲突入口须检查同一读授权状态；计数增持/释放仍允许但归零只登记，不物理销毁。
    #[cfg(not(feature = "rc"))]
    pub fn pin(&self) -> Result<WorldPin<'_>, EcsError> {
        Self::pin_inner(self)
    }
    /// rc 作用域：独占借用期间执行，闭包结束自动归还。
    #[cfg(feature = "rc")]
    pub fn read_scope<'w, R>(
        &'w mut self,
        run: impl for<'a> FnOnce(&'a WorldPin<'w>) -> R,
    ) -> Result<R, EcsError> {
        Self::read_scope_inner(self, run)
    }
    /// 非 rc：run 仅在 pin 授权期执行，R 不得携带本次 pin 的借用。
    /// 申请拒绝不执行闭包，闭包 panic 展开释放授权；闭包内冲突维护应拒绝，不能提前解锁。
    #[cfg(not(feature = "rc"))]
    pub fn read_scope<'w, R>(
        &'w self,
        run: impl for<'a> FnOnce(&'a WorldPin<'w>) -> R,
    ) -> Result<R, EcsError> {
        Self::read_scope_inner(self, run)
    }
    /// rc 增持身份：&mut 独占，计数更新无需原子。
    #[cfg(feature = "rc")]
    pub fn try_acquire(
        &mut self,
        entity: Entity,
    ) -> Result<crate::owned_entity::OwnedEntity<'_>, EcsError> {
        Self::try_acquire_inner(self, entity)
    }
    /// 非 rc 原子增持完整身份；不授予组件访问。
    /// 未知键/代际失效/PendingDestroy/Dead/计数溢出拒绝，不改变计数、不复活。
    #[cfg(not(feature = "rc"))]
    pub fn try_acquire(
        &self,
        entity: Entity,
    ) -> Result<crate::owned_entity::OwnedEntity<'_>, EcsError> {
        Self::try_acquire_inner(self, entity)
    }

    // 宿主共享单组件读取迁入 WorldPin::get，不保留脱离读授权的 World::get。
    // 宿主写点查迁入 WorldWrite，禁止脱离独占组件授权取得写结果。
    /// 创建无额外过滤的长期准备态；self 须完成登记，不持有组件引用或隐式注册类型。
    /// 查询组合冲突或未登记类型返回错误；本态只与所属兼容 World 使用。
    pub fn query<Q: crate::query::QueryData>(
        &mut self,
    ) -> Result<crate::query::QueryState<Q>, EcsError> {
        Self::query_inner::<Q>(self)
    }
    /// 创建带 F 过滤的长期准备态；要求与 query 相同，不产生实体结果快照。
    pub fn query_filtered<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        &mut self,
    ) -> Result<crate::query::QueryState<Q, F>, EcsError> {
        Self::query_filtered_inner::<Q, F>(self)
    }

    /// # 用途
    /// 从 values 的完整组件类型集合推导原型并创建实体，不要求调用者预登记原型。
    /// domain 是出生分区，同时决定初始当前分区；必须小于本 World 的分区数。
    /// values 为保留具体值/寿命的 Bundle，包含普通叶子及内置输入；所有类型及 Table 已登记冻结。
    /// 先只读校验类型、域、请求归属及容量边界；非法输入返回错误并释放未安装输入。
    /// 校验后自动复用或登记原型，按最大覆盖 Table（同分首次登记）确定数据源；
    /// 自首次原型/物理创建修改起失败 panic，不回滚、不捕获后继续使用 World。
    /// 成功返回初始 strong=1 的拥有引用；初始分区为 domain，组件 Added/Changed 共用本次 tick。
    /// rc 宿主入口仅 Idle 进入 Maintaining，正常退出回 Idle，修改后展开保持 Stopped，不在 Running 内重入。
    /// rc 返回值借住本次 World；需保留引用继续修改时使用 World::entity_scope。
    #[cfg(feature = "rc")]
    pub fn spawn<B: crate::bundle::Bundle>(
        &mut self,
        domain: crate::entity::AllocationDomain,
        values: B,
    ) -> Result<crate::owned_entity::OwnedEntity<'_>, EcsError> {
        /* 设计已确认：
        读取 B::component_types 并校验 domain、重复、登记和 B::validate 来源；不做值擦除。
        推导规范组件集合；在排他准备点复用或登记原型，不新增组件/Table。
        首次创建修改后失败 panic；以 domain 同时分配身份和设置当前分区。
        安装值与引用责任，发布成员和位置，最后发布 Added/Changed，返回初始拥有引用。
        */
        Self::spawn_inner(self, domain, values)
    }
    /// # 用途
    /// 从 values 的完整组件类型集合推导原型并创建实体，不要求调用者预登记原型。
    /// domain 是出生分区，同时决定初始当前分区；必须小于本 World 的分区数。
    /// values 拥有普通组件与内置请求；所有类型及 Table 必须预先登记并完成冻结。
    /// 先只读校验类型、域、请求归属及容量边界；非法输入返回错误并释放未安装输入。
    /// 校验后自动复用或登记原型，按最大覆盖 Table（同分首次登记）确定数据源；
    /// 自首次原型/物理创建修改起失败 panic，不回滚、不捕获后继续使用 World。
    /// 成功返回初始 strong=1 的拥有引用；初始分区为 domain，组件 Added/Changed 共用本次 tick。
    /// rc 返回值借住本次 World；需保留引用继续修改时使用 World::entity_scope。
    /// 非 rc 入口内部取得排他准备能力；不得在并行查询持借用时扩展原型。
    #[cfg(not(feature = "rc"))]
    pub fn spawn<B: crate::bundle::Bundle>(
        &self,
        domain: crate::entity::AllocationDomain,
        values: B,
    ) -> Result<crate::owned_entity::OwnedEntity<'_>, EcsError> {
        // 核心流程见 rc 的 World::spawn；差异为共享入口须取得运行/维护授权，不能强转 &mut World。
        Self::spawn_inner(self, domain, values)
    }

    /// # 用途
    /// 在已有系统运行结构授权下创建实体；access 是本 Spawn 位置的同源叶子，partition 为其固定分区。
    /// ordered_components 是该位置授权的有序完整清单；输入的 Shape 不构成安全证明。
    /// 逐项核对实际输入类型和来源，且只用授权清单计划目标与发布通知；前置拒绝不发布实体。
    /// 安装及发布顺序复用 World::spawn：默认不创建新 Table，直接写空槽，再发布成员、位置和 Added/Changed。
    /// 出生域与初始位置取 partition 的同一编号，tick 使用系统 this_run；不重入 Idle 门禁、不分配新 tick。
    /// 成功返回初始 strong，同系统授权寿命；展开沿已有停止和责任清理规则。
    /// # Safety
    /// 调用者须持完整目标的排他结构写授权，排除会被表/稀疏值搬移失效的活视图。
    #[cfg(feature = "rc")]
    pub(crate) unsafe fn spawn_from_system<'w, B: crate::bundle::Bundle>(
        access: &crate::access::RunAccess<'w>,
        partition: crate::entity::PartitionId,
        ordered_components: &[crate::component::ComponentId],
        tick: crate::change_detection::ChangeTick,
        values: B,
    ) -> Result<crate::owned_entity::OwnedEntity<'w>, EcsError> {
        Self::spawn_from_system_inner(access, partition, ordered_components, tick, values)
    }

    /// 交还 forget 转出的外部强引用，不无视其他持有者强制销毁。
    /// 返回是否触发 strong 归零登记；false 也已释放一份引用，不得重试。
    /// 归零后禁止增持/升级，仅登记 PendingDestroy；本阶段查询仍可见，flush_destruction 才物理销毁。
    /// weak 尚存则保留身份槽，销毁收束且 weak=0 才回收，不能立即覆盖 link。
    /// # Safety
    /// entity 必须属于本 World，且调用者拥有一份 forget 转出的未释放强引用责任。
    /// 每份责任只调用一次，复制裸 Entity 不增加责任；重复释放不是幂等操作。
    pub unsafe fn despawn(&mut self, entity: Entity) -> Result<bool, EcsError> {
        /* 设计流程：
        中文伪码：
          校验外部责任，交还一份 strong；非零返回 false。
          归零禁止增持/升级，登记 PendingDestroy 并返回 true，不隐藏本阶段组件。
          阶段末或宿主显式 flush_destruction 才收束物理销毁。
        */
        Self::despawn_inner(self, entity)
    }

    /// # 用途
    /// 按严格增删修改 entity 的组件组合；目标为（源集合 − removed）∪ added 的类型集合。
    /// entity 为本 World 活键；added 是类型化 Bundle，新增类型已登记且源中全缺，值直接写槽。
    /// removed 是本 World 的组件编号切片，必须无重复、全在源中，且与 added 不相交；空增删为无操作。
    /// 普通输入校验错误在修改前返回，原实体不变；输入被消费，未安装责任正常释放。
    /// 校验后自动复用/登记目标原型，不要求调用者预登记；不新增组件或 Table。
    /// 自首次原型/物理创建修改起失败 panic，不回滚；成功返回时新组合与位置已发布。
    /// 保留共有值及变化 tick；移除值按订阅转交 Removed 或释放，新增/移除共用本次结构 tick。
    /// rc 宿主仅 Idle 认领 Maintaining，正常退出回 Idle，修改后展开保持 Stopped。
    /// 调用前必须结束冲突组件借用；保留拥有引用时从 EntityScope 调用，Running 内使用 QueryAlter 而非重入宿主入口。
    #[cfg(feature = "rc")]
    pub fn change_archetype<B: crate::bundle::Bundle>(
        &mut self,
        entity: Entity,
        added: B,
        removed: &[ComponentId],
    ) -> Result<(), EcsError> {
        /* 设计已确认：
        校验完整身份和严格增删；空增删返回，不分配 tick、不发通知。
        计算目标规范集合，排他准备并自动登记缺少的原型。
        首次修改后失败 panic；按 structural_change::ArchetypeChange 搬共有值和 tick，
        安装新增责任，转交/释放移除值，发布成员与位置后追加通知；旧 Table 行仅退休。
        同 Table 保留行，Sparse→Sparse 不搬值，其他来源互转不得丢变化 tick。
        */
        Self::change_archetype_inner(self, entity, added, removed)
    }
    /// # 用途
    /// 按严格增删修改 entity 的组件组合；目标为（源集合 − removed）∪ added 的类型集合。
    /// entity 必须为本 World 的存活完整键；added 拥有新增值/内置请求，类型必须已登记且源中不存在。
    /// removed 是本 World 的组件编号切片，必须无重复、全在源中，且与 added 不相交；空增删为无操作。
    /// 普通输入校验错误在修改前返回，原实体不变；输入被消费，未安装责任正常释放。
    /// 校验后自动复用/登记目标原型，不要求调用者预登记；不新增组件或 Table。
    /// 自首次原型/物理创建修改起失败 panic，不回滚；成功返回时新组合与位置已发布。
    /// 保留共有值及变化 tick；移除值按订阅转交 Removed 或释放，新增/移除共用本次结构 tick。
    /// rc 调用前必须结束冲突组件借用；保留拥有引用时从 EntityScope 调用。
    /// 非 rc 内部取得结构排他与原型登记能力，冲突在修改前报错。
    #[cfg(not(feature = "rc"))]
    pub fn change_archetype<B: crate::bundle::Bundle>(
        &self,
        entity: Entity,
        added: B,
        removed: &[ComponentId],
    ) -> Result<(), EcsError> {
        // 核心流程见 rc 的 World::change_archetype；共享入口不能构造整份 &mut World。
        Self::change_archetype_inner(self, entity, added, removed)
    }
    /// 在初始化登记阶段显式登记 T 与 storage；不要求 Clone 或 Default。
    /// 同类型同策略返回原 ComponentId，冲突或冻结后登记错误且登记不变。
    /// T 显式实现 Component，框架从 T::Access 生成一致的分类/释放元数据；不构造业务值。
    /// 不提供 static 类型自动普通组件适配，实际稀疏存储初始化由冻结阶段协调。
    pub fn register_component<T: crate::component::Component>(
        &mut self,
        storage: StoragePolicy,
    ) -> Result<ComponentId, EcsError> {
        Self::register_component_inner::<T>(self, storage)
    }

    /// 登记 components 对应的 Table；覆盖数量相同时，首次成功登记者优先。
    /// 输入必须是本 World 已登记且非显式稀疏的无重复组件；顺序不影响规范布局。
    /// 相同集合复用首次编号且不改变顺序；非法集合或冻结后登记错误，现有登记不变。
    /// 每 World 一个零号空 Table，基础身份 variant=0，不声明恢复域。
    pub fn register_table(&mut self, components: &[ComponentId]) -> Result<ArchetypeId, EcsError> {
        Self::register_table_inner(self, components)
    }

    /// 完成类型和 Table 登记、同号基础原型及各分区存储初始化；冻结后不再新增类型或布局。
    /// 在系统参数初始化和实体创建之前调用；重复成功冻结无副作用。
    /// 校验或准备失败不冻结，不发布部分可运行状态。
    /// 顺序：若已安装存储则幂等返回；先按当前登记构建全部 Table/列/稀疏/原型成员，
    /// 再提交 registry.freeze（失败保持未冻结），最后一次性安装 storage。
    /// 构建失败丢弃局部存储、不修改 registry；提交后发布可运行状态，不回滚。
    /// 分配失败/panic 由分配器或展开直接终止，不暴露半 World。
    pub fn finish_registration(&mut self) -> Result<(), EcsError> {
        Self::finish_registration_inner(self)
    }

    /// 冻结后显式声明完整 components 集合；复用相同集合，基础原型与 Table 同号。
    /// 动态原型仅追加声明，不新增 Table；选最大覆盖的子集 Table，同分选先登记者。
    /// 未登记或重复组件、未冻结或编号耗尽返回错误且不公开半声明；不创建实体。
    /// 内部顺序：registry.plan_archetype → storage.prepare_archetype → registry.commit；
    /// 普通错误在提交前返回；RC 成功只增加原型数量，不推进布局替换代次，查询/Alter 增量扫描。
    /// RC 此公开宿主入口仅 Idle；Running 中新增原型只由已有排他结构操作的内部准备路径完成，不能重入宿主门禁。
    pub fn register_archetype(
        &mut self,
        components: &[ComponentId],
    ) -> Result<ArchetypeId, EcsError> {
        Self::register_archetype_inner(self, components)
    }

    /// 显式选择本 World 基础 Table；base 必须 variant=0 且 components 包含其全部组件。
    /// components 必须符合已有集合幂等规则；失败不登记半原型，不允许跨 World 使用编号。
    /// 与 register_archetype 共用内部 plan → storage prepare → commit 及 RC 宿主 Idle 门禁；base 必须 variant=0。
    pub fn register_archetype_on(
        &mut self,
        base: ArchetypeId,
        components: &[ComponentId],
    ) -> Result<ArchetypeId, EcsError> {
        Self::register_archetype_on_inner(self, base, components)
    }
    /// 只读取得 id 的组件元信息，未知编号返回 None；不能用于绕过显式登记。
    pub fn component_info(&self, id: ComponentId) -> Option<&ComponentInfo> {
        Self::component_info_inner(self, id)
    }
    /// 共享只读通知管理器；订阅增删仍走专用入口。
    pub fn notifications(&self) -> &crate::notification::Notifications {
        &self.notifications
    }
    /// rc：宿主 Idle 静止点登记通知订阅；组件须已登记，Running/Stopped/关闭拒绝。
    /// 系统阶段初始化改走 ParameterRegistration::subscribe_notice，不复用本宿主入口放宽门禁。
    #[cfg(feature = "rc")]
    pub fn subscribe_notice(
        &mut self,
        component: ComponentId,
        kind: crate::notification::NoticeKind,
    ) -> Result<crate::notification::NoticeSubscription, EcsError> {
        self.subscribe_notice_inner(component, kind)
    }
    /// rc：宿主 Idle/Stopped 静止点注销通知订阅；不推进其他读者游标，Running/关闭拒绝。
    /// 系统失败清理/注销经 ParameterRegistration::unsubscribe_notice；关闭由其清偿作用域承担。
    #[cfg(feature = "rc")]
    pub fn unsubscribe_notice(
        &mut self,
        subscription: crate::notification::NoticeSubscription,
    ) -> Result<(), EcsError> {
        self.unsubscribe_notice_inner(subscription)
    }
    /// 只读取得公共 Table 布局，未知编号返回 None，不暴露分区数据。
    /// # 用途
    /// 返回全部冻结 Table 声明，按稳定 Table 编号排列；未冻结返回 InvalidState。
    /// 结果只借本次 self，不含物理存储；借用期间禁止布局/原型目录结构修改。
    pub fn table_layouts(&self) -> Result<&[TableLayout], EcsError> {
        self.table_layouts_inner()
    }
    /// # 用途
    /// 返回全部已发布完整原型声明，未冻结返回 InvalidState，不含逻辑待建目标。
    /// 供阶段闭包和外部查询准备读取，不访问实体成员，不分配或推进版本。
    pub fn archetypes(&self) -> Result<&[Archetype], EcsError> {
        self.archetypes_inner()
    }
    /// 只读取得公共 Table 布局，未知编号返回 None，不暴露分区数据。
    pub fn table_layout(&self, id: ArchetypeId) -> Option<&TableLayout> {
        Self::table_layout_inner(self, id)
    }
    /// 只读取得原型声明，未知编号返回 None，不返回独立成员列表。
    pub fn archetype(&self, id: ArchetypeId) -> Option<&Archetype> {
        Self::archetype_inner(self, id)
    }
    /// 创建默认单分区 World；等价于 World::with_partitions::<1>()。
    /// 游戏和渲染分别构造独立 World，不执行系统；失败不发布部分实例。
    pub fn new() -> Result<Self, EcsError> {
        Self::with_partitions::<1>()
    }

    /// 创建分区数为 PARTITION_COUNT 的 World，数量构造后固定。
    /// 合法范围 1..=MAX_PARTITIONS(4096)；0 或超限在分配前返回错误。
    /// 出生分区与当前分区共用编号：AllocationDomain < MAX_DOMAINS，迁区不改出生分区。
    /// 首版 EntityCategory 恒为 0，不写入实体键。
    /// 返回相同的非泛型 World；非法数量或初始化失败返回错误，不发布部分实例。
    pub fn with_partitions<const PARTITION_COUNT: usize>() -> Result<Self, EcsError> {
        Self::with_partitions_inner::<PARTITION_COUNT>()
    }

    /// 校验完整身份后返回位置副本，不持有组件借用，不修改身份映射。
    ///
    /// # Arguments
    /// * `entity` - 完整键，包含代际和出生分区位。
    ///
    /// # Returns
    /// 无效键返回 None；有效键返回当前存储实例与行，状态不变。
    pub fn locate(&self, entity: Entity) -> Option<EntityLocation> {
        Self::locate_inner(self, entity)
    }

    /// # 用途
    /// 宿主静止点执行单个纯迁区请求；组件全局不动，只提交 EntityLocation.partition。
    /// self 必须已冻结且为 Idle，无活宿主授权、查询结果或存储视图；&mut World 不代替状态检查。
    /// Running/Stopped/Closing/Closed 拒绝；系统不能重入本入口，首轮不提供系统内迁区能力。
    /// request.entity 必须属于本 World、完整身份有效且位置非 Null；RC strong 必须大于零。
    /// request.target 必须小于本 World 固定 partition_count；出生域、完整键与计数保持。
    /// 在全部前提检查后，同区请求幂等返回，不改任何位置或分配 tick。
    /// 成功仅改变当前位置分区；table/table_row/archetype/archetype_row、成员顺序、
    /// 所有值及 added/changed tick 保持，不认领/搬移/移除物理行，不登记 settle 工作。
    /// 当前查询与排序均不按分区收窄，首轮没有额外“分区系统实体集合”需要同步。
    /// 后续宿主 locate/查询观察新分区；不发 Added/Removed/Changed，不触发自动判区插件。
    /// # Errors
    /// 状态、授权、身份/strong 或目标检查失败返回分类错误，保留完整原状态。
    /// Idle 认领 Maintaining 后经 EntityStore::relocate 更新位置，正常回 Idle；
    /// 提交前普通错误释放门禁并回 Idle，提交中意外展开标 Stopped，不宣称事务回滚。
    pub fn move_partition(&mut self, request: PartitionMove) -> Result<(), EcsError> {
        /* 设计已确认：
        检查冻结、Idle 与无活授权，认领 Maintaining；校验完整身份、非 Null、strong>0 与目标范围。
        全部校验后同区直接释放门禁返回；否则复制当前位置，仅改 partition。
        通过 EntityStore::relocate 提交位置，不触碰物理行、tick 或通知；释放门禁回 Idle。
        普通失败保留原位置并回 Idle；提交期展开标 Stopped，不能从 Running 重入。
        */
        Self::move_partition_inner(self, request)
    }
    /// 回收通知系统中已无读者借用且通过最小游标的 Removed 旧值；统一经 release_value 选择特殊释放。
    /// 调用者须处于无通知批次/旧值借用的收束点；普通 Added/Changed 头不在此释放。
    /// RC 仅宿主 Idle 进入 Maintaining，成功回 Idle，异常标停；不在 Running 内嵌套调用。
    /// 不分配业务 tick、不物理销毁归零实体/回收身份槽；新归零仅登记，宿主可随后 flush_destruction。
    /// 阶段末统一 flush 主体在已有 Running 授权内执行相同旧值释放协议，不重入此宿主门禁。
    pub fn reclaim_notifications(&mut self) -> Result<(), EcsError> {
        Self::reclaim_notifications_inner(self)
    }

    /// 在阶段末整理结构修改留下的待移除 Table 行，并修正换行影响的身份位置。
    /// self 必须处于无其他线程、查询、组件引用或存储视图访问的独占同步点。
    /// 整理本身不改变实体身份和逻辑组件集合，不伪造 Added/Removed/Changed。
    /// 成功后待整理行已处理，所有存活实体指向正确的当前分区、Table 和行。
    /// settle 负责物理填洞及位置修复，不作为结构修改首次可见的发布点。
    /// 已移出或已析构的值不得再次析构；移除值的处理在对应修改提交路径完成。
    /// 按 ArchetypeId 升序处理；同 Table 用尾部存活行填前方洞，全部列共用动作表。
    /// 每次移动修复对应来源域的位置，之后按新长度合并 pi_arr 桶并重置认领/发布长度。
    /// 无待移除行时仍可合并扩展桶；重复调用不产生逻辑状态变化。
    /// Table 整理须同步修正原型成员的 table_row；原型成员填洞须修正 EntityLocation::archetype_row。
    /// SparseSet 不参与阶段末整理；其 remove 只返回 OwningPtr，不搬动其他实体。
    /// 整理前准备动作表和所需空间；开始搬行后失败直接 panic，不回滚。
    /// RC 当前公开入口仅宿主 Idle 认领 Maintaining，正常返回回 Idle，搬行后展开保持 Stopped。
    /// 调度不得在 Running 内直接重入此宿主入口；需要运行内整理时另有显式设计后才提供能力。
    /// 不随每个 Stage 自动执行，避免隐式搬行。
    /// RC 委托 WorldStorage::settle，接收 SettledLocation 后经身份表无分配修正全部位置；
    /// 应用时保留当前分区，交接完成前不可返回 Idle 或恢复业务读取，失败保持 Stopped。
    pub fn settle(&mut self) -> Result<(), EcsError> {
        /* 设计已确认：沿用 pi_world 的阶段末整理，先准备动作表再执行。
        中文伪码：
          在独占同步点处理待移除行；不对纯迁区的共享稀疏值执行操作。
          按区号和 ArchetypeId 升序生成尾部存活行到前洞的动作表；全部列应用同表。
          更新行标记和各来源域的位置；无活值区截尾，按新长度合并 pi_arr 桶。
          清理待整理记录并重置认领/发布长度；中途失败直接 panic，不恢复或继续使用 World。
          整理原型成员并修正 archetype_row；不调用 SparseSet 整理。
        */
        Self::settle_inner(self)
    }
}
