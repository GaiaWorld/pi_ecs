//! 并发设计以 &World 为基础，大部分操作共享接收者；修改依靠受限授权和内部可变性。
//! 共享引用不授予任意写权限，不允许 shared→整个 &mut World 强转；旧独占壳逐项迁移。

#[cfg(test)]
mod shared_maintenance_compile_checks {
    use crate::{
        alter::AlterState,
        entity::{ArchetypeId, Entity},
        entity_container::EntityQueueMut,
        entity_values::EntityValues,
        error::EcsError,
        world::World,
    };
    // 仅编译函数体，不作为运行测试调用失败桩；验证签名组合，不证明内部授权安全。
    fn host_resources<T: Send + Sync + 'static>(world: &World) -> Result<(), EcsError> {
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
        world: &World,
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
    fn host_pop_then_change(
        world: &World,
        owner: Entity,
        entity: Entity,
        archetype: ArchetypeId,
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
            let mut values = EntityValues::new();
            if let Err(rejected) = values.strong_entities::<1, ()>([owned]) {
                return Err(rejected.error);
            }
            world.change_archetype(entity, archetype, values)?;
        }
        Ok(())
    }
    fn scope_pop<'w>(
        world: &'w World,
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
        world: &'w World,
        owner: Entity,
    ) -> Result<Option<crate::owned_entity::OwnedEntity<'w>>, EcsError> {
        world.read_scope(|pin| {
            let Some(queue) = pin.get::<crate::entity_container::EntityQueue<()>>(owner) else {
                return Ok(None);
            };
            queue.front_owned()
        })?
    }
    fn acquire_then_change(
        world: &World,
        entity: Entity,
        target: Entity,
        archetype: ArchetypeId,
    ) -> Result<(), EcsError> {
        let owned = world.try_acquire(target)?;
        let mut values = EntityValues::new();
        if let Err(rejected) = values.strong_entities::<1, ()>([owned]) {
            return Err(rejected.error);
        }
        world.change_archetype(entity, archetype, values)?;
        world.flush_destruction()
    }
    fn acquire_then_scope(
        world: &World,
        state: &mut AlterState,
        entity: Entity,
        target: Entity,
    ) -> Result<(), EcsError> {
        let owned = world.try_acquire(target)?;
        let mut values = EntityValues::new();
        if let Err(rejected) = values.strong_entities::<1, ()>([owned]) {
            return Err(rejected.error);
        }
        state.scope(world, |alter| alter.alter(entity, values))??;
        Ok(())
    }
    fn pop_then_change<T: 'static>(
        world: &World,
        mut source: EntityQueueMut<'_, '_, T>,
        entity: Entity,
        archetype: ArchetypeId,
    ) -> Result<(), EcsError> {
        let Some(owned) = source.pop_front()? else {
            return Ok(());
        };
        drop(source);
        let mut values = EntityValues::new();
        if let Err(rejected) = values.strong_entities::<1, ()>([owned]) {
            return Err(rejected.error);
        }
        world.change_archetype(entity, archetype, values)
    }
}

use crate::{
    archetype::Archetype,
    component::{ComponentId, ComponentInfo, StoragePolicy},
    entity::ArchetypeId,
    entity::{Entity, EntityLocation},
    entity_store::EntityStore,
    error::EcsError,
    partition::PartitionMove,
    registry::TypeRegistry,
    table::TableLayout,
};
/// 支持分区的非泛型 World；游戏与渲染使用不同实例；持有核心固定身份实现和分区状态。
/// 原型存储按当前分区和类别隔离，身份来源域不随迁区改变。
pub struct World {
    /// 唯一身份定位入口；业务系统不得绕过核心直接修改映射。
    pub(crate) entities: EntityStore,
    /// 统一类型化命令服务，payload 原地延迟可见，外部入口不提供组件权限。
    pub(crate) commands: crate::command::CommandHub,
    /// 不随游戏恢复回退，旧后台结果必须拒绝。
    pub(crate) epoch: crate::command::WorldEpoch,
    /// 显式推进的模拟时间，不等于 change_tick。
    pub(crate) simulation_tick: crate::command::SimulationTick,
    /// 全 World 唯一资源实例。
    pub(crate) resources: crate::resource::ResourceStore,
    /// 生产者多例紧凑数组，结构变动后全遍历刷新偏移。
    pub(crate) multi_resources: crate::resource::MultiResourceStore,
    /// 强引用归零只登记；阶段末无组件借用时收束物理销毁。
    pub(crate) pending_destruction: crate::destruction::PendingDestruction,
    /// 单向/双向容器节点统一存储；节点不是实体组件，不带用途 T。
    pub(crate) entity_nodes: crate::entity_container::EntityNodePools,
    /// 构造后固定的分区数，默认1；12位分区编码上限及保留值待校验契约闭合。
    pub(crate) partition_count: usize,
    /// 非零 u64 变化时钟，初始为 1，不是模拟帧；系统/宿主上下文取得一次 tick。
    /// AtomicU64 无非零类型，封装必须保持非零不变量，恢复拒绝零，耗尽报错不回绕。
    pub(crate) change_tick: pi_share::ShareU64,
    /// Added/Changed/Removed 的可靠多读者通知及历史旧值所有权。
    pub(crate) notifications: crate::notification::Notifications,
    /// 本 World 恢复后递增，所有布局相关派生计划须失效重建。
    pub(crate) layout_epoch: u64,
    /// 本 World 独占的类型、Table 布局及动态原型登记；不与另一 World 共用可变登记状态。
    pub(crate) registry: TypeRegistry,
    /// 冻结前为 None；全部分区 Table/后备及显式共享稀疏入口准备成功后一次安装。
    pub(crate) storage: Option<crate::storage::WorldStorage>,
    // 单一内部门禁：Idle 可进入多个宿主只读 pin 或 Running/Maintaining 排他态。
    // pin 存活时拒绝运行、组件写与维护；全部读授权释放后才可转入冲突状态。
    // 子 Schedule 复用根运行授权；完成任务/借用/提交收束后才回 Idle，不可恢复失败进停止态。
    // 宿主运行后使用具体资源/flush/settle/恢复入口，不返回整个 &mut World，也不共享转可变。
    // @to_design 门禁原子表示、受限内部可变存储及原型/规则安装状态尚待接线。
    pub(crate) _pending_runtime: (),
}
/// 宿主只读访问授权，不是 std::pin::Pin，也不增加实体强计数；无 Copy/Clone/提前解锁接口。
/// 可并存多个只读 pin；首版排斥系统运行、组件写、结构修改/销毁/settle/恢复。
/// Drop 归还一份授权；forget 只泄漏授权使冲突操作持续拒绝，不得失去旧借用保护。
pub struct WorldPin<'w> {
    /// World 保活借用，不向调用者暴露未经授权的组件入口。
    pub(crate) world: &'w World,
    /// 唯一持有的一份读授权；引用读取必须借用本 pin，不能只绑定 world。
    /// @to_design 多读计数/状态转换、授权凭据及并发注册的原子表示待闭合。
    pub(crate) _pending_read_lease: (),
}
impl<'w> WorldPin<'w> {
    /// entity 须为本 World 完整键；未知类型/无效实体/缺组件返回 None，不隐式登记。
    /// 普通值返回共享引用、内置类型返回只读对象，均借用 self，禁止越过 pin 解锁期。
    /// 裸读取不增持；成员 *_owned 所得寿命绑定 World，可在 pin 释放后存在，不携带读权限。
    pub fn get<C: crate::component::Component>(
        &self,
        entity: Entity,
    ) -> Option<crate::component::ComponentRead<'_, 'w, C>> {
        crate::world_inner::pin_get_inner::<C>(self, entity)
    }
}
impl Drop for WorldPin<'_> {
    /// 释放读授权而不销毁业务值；panic 展开也须执行，不启动隐式维护。
    fn drop(&mut self) {
        crate::world_inner::drop_pin_inner(self)
    }
}
/// 宿主独占组件访问，不是整个可变 World；不开放结构维护，无 Copy/Clone/提前解锁。
/// 与 pin、运行、其他写访问和维护互斥，Drop 释放，forget 只泄漏授权使冲突持续拒绝。
pub struct WorldWrite<'w> {
    /// 拥有结果的 World 寿命，与组件借用/写授权寿命分离。
    pub(crate) world: &'w World,
    /// 成功取得写授权后分配的非零时间，全部组件/资源/容器标记共用，不在每项写入时重新分配。
    pub(crate) tick: crate::change_detection::ChangeTick,
    /// 独占组件访问授权；禁止通过共享 World 伪造。
    /// @to_design 与单一门禁及变化上下文的实际原子表示待闭合。
    pub(crate) _pending_write_lease: (),
}
impl<'w> WorldWrite<'w> {
    /// 共享读取借用本访问对象；缺失返回 None，不登记类型或增持。
    pub fn get<C: crate::component::Component>(
        &self,
        entity: Entity,
    ) -> Option<crate::component::ComponentRead<'_, 'w, C>> {
        crate::world_inner::write_get_inner::<C>(self, entity)
    }
    /// 独占短借用防止冲突结果并存；普通值返回 Mut，内置返回专用对象，变化规则不变。
    /// 缺失返回 None；转出的 OwnedEntity 只绑定 World，不保留本次写权限。
    pub fn get_mut<C: crate::component::Component>(
        &mut self,
        entity: Entity,
    ) -> Option<crate::component::ComponentWrite<'_, 'w, C>> {
        crate::world_inner::write_get_mut_inner::<C>(self, entity)
    }
    /// 只准备本实体的组合点查，先检查 Q 内部别名/类型及目标，失败不改值、不创建全局缓存。
    /// 组件结果借用本对象，拥有结果绑定 World；不开放任意结构操作。
    pub fn query_one_mut<Q: crate::query::QueryData>(
        &mut self,
        entity: Entity,
    ) -> Result<Q::Item<'_, 'w>, EcsError> {
        crate::world_inner::write_query_one_inner::<Q>(self, entity)
    }
}
impl Drop for WorldWrite<'_> {
    /// 仅释放独占授权，不分配 tick、不延迟标记/发布、不隐式维护；展开亦需释放，致命失败不能解除停止态。
    fn drop(&mut self) {
        crate::world_inner::drop_write_inner(self)
    }
}
impl World {
    /// 共享读取当前已分配的非零变化时钟，不递增、不代表并发写入已完成，不是快照/内存完成屏障。
    /// 宿主在相关写者收束后取观察上界；恢复后进度沿用已安装时钟，旧分支游标须另校验。
    pub fn read_change_tick(&self) -> crate::change_detection::ChangeTick {
        crate::world_inner::read_change_tick_inner(self)
    }
    /// 申请宿主独占组件访问，冲突前置报错不等待且不消耗 tick；不返回整个 &mut World。
    /// 取得授权后分配一次非零 ChangeTick；耗尽报错并释放本次授权，不返回写对象、不回绕。
    /// 成功但未修改值可消耗 tick，不产生值变化或通知；普通/内置/资源全部写包装共用该 tick。
    pub fn write(&self) -> Result<WorldWrite<'_>, EcsError> {
        /* 设计已确认：
        检查运行状态并取得独占访问授权，冲突立即拒绝，不推进时钟。
        原子分配本次非零 tick；耗尽则清理本次授权并报错，不发布可写对象。
        成功构造持授权与固定 tick 的 WorldWrite；后续实际标记复用 tick。
        无写入也不退回 tick，不生成业务通知；失败耗尽后的停止状态按全局时钟契约处理。
        */
        crate::world_inner::write_inner(self)
    }
    /// 完整复用 write 的授权与一次 tick 分配；不为闭包另取 tick。
    /// R 可含 World 寿命的 OwnedEntity，不得含本次组件借用。
    /// 拒绝不调用 run，正常/展开释放授权，闭包中结构维护冲突须报错。
    pub fn write_scope<'w, R>(
        &'w self,
        run: impl for<'a> FnOnce(&'a mut WorldWrite<'w>) -> R,
    ) -> Result<R, EcsError> {
        crate::world_inner::write_scope_inner(self, run)
    }
    /// 取得一份宿主只读授权，多读允许；运行/维护/停止态冲突前置报错，不等待或隐式重入。
    /// 所有冲突入口须检查同一读授权状态；计数增持/释放仍允许但归零只登记，不物理销毁。
    pub fn pin(&self) -> Result<WorldPin<'_>, EcsError> {
        crate::world_inner::pin_inner(self)
    }
    /// run 仅在 pin 授权期执行，R 不得携带本次 pin 的借用；内部复用 pin 协议，不创建第二套门禁。
    /// 申请拒绝不执行闭包，闭包 panic 展开释放授权；闭包内冲突维护应拒绝，不能提前解锁。
    pub fn read_scope<'w, R>(
        &'w self,
        run: impl for<'a> FnOnce(&'a WorldPin<'w>) -> R,
    ) -> Result<R, EcsError> {
        crate::world_inner::read_scope_inner(self, run)
    }
    /// 宿主增持指定 World 中的完整身份，不授予组件访问；Alive 且 strong>0 才原子加一。
    /// 未知键/代际失效/PendingDestroy/Dead/计数溢出拒绝，不改变计数、不复活；与最后释放原子仲裁。
    /// 返回寿命绑定 World/授权，门禁与分支隔离必须覆盖 Drop；不作为系统取得任意 World 权限的入口。
    pub fn try_acquire(
        &self,
        entity: Entity,
    ) -> Result<crate::owned_entity::OwnedEntity<'_>, EcsError> {
        crate::world_inner::try_acquire_inner(self, entity)
    }

    // 宿主共享单组件读取迁入 WorldPin::get，不保留脱离读授权的 World::get。
    // 宿主写点查迁入 WorldWrite，禁止脱离独占组件授权取得写结果。
    /// 创建无额外过滤的长期准备态；self 须完成登记，不持有组件引用或隐式注册类型。
    /// 查询组合冲突或未登记类型返回错误；本态只与所属兼容 World 使用。
    pub fn query<Q: crate::query::QueryData>(
        &mut self,
    ) -> Result<crate::query::QueryState<Q>, EcsError> {
        crate::world_inner::query_inner::<Q>(self)
    }
    /// 创建带 F 过滤的长期准备态；要求与 query 相同，不产生实体结果快照。
    pub fn query_filtered<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        &mut self,
    ) -> Result<crate::query::QueryState<Q, F>, EcsError> {
        crate::world_inner::query_filtered_inner::<Q, F>(self)
    }

    /// 创建已显式声明 archetype 的实体，partition 为当前分区，domain 为身份来源域。
    /// values 必须恰好覆盖原型全部组件且类型已登记；不隐式生成默认值或新增原型。
    /// 同域按确定顺序执行；成功返回 OwnedEntity，借用 World 并拥有初始唯一强引用；World 不另持引用。
    /// &World 支持并发入口，内部授权校验分配来源/原型写与容量，不因共享签名允许任意并发。
    /// 所有普通检查/准备错误在认领行和写入前返回，不发布半实体；新身份失败可消耗代际但须释放。
    /// 宿主校验/容量准备成功后、认领写入前取得本次结构 tick，新增组件的 Added/Changed 共用。
    /// 系统可信创建路径复用 this_run；耗尽在写入前报错，不逐组件分配。
    /// 输入 values 被消费；写入开始后失败直接 panic，不回滚，不允许捕获后继续使用 World。
    /// @to_design domain 与 partition/类别的合法组合及 20位成员行/32位 Table 行容量边界待定。
    pub fn spawn(
        &self,
        partition: crate::entity::PartitionId,
        domain: crate::entity::AllocationDomain,
        archetype: ArchetypeId,
        values: crate::entity_values::EntityValues<'_>,
    ) -> Result<crate::owned_entity::OwnedEntity<'_>, EcsError> {
        /* 设计流程：
        中文伪码：
          校验冻结、原型、类型集合与域/区；准备值和目标稀疏容量。
          准备目标行列槽、稀疏索引和全部记录空间，所有普通错误在认领前返回。
           宿主分配本次结构 tick（系统注入 this_run），耗尽在认领写入前拒绝。
           认领 Table 行并取得最终身份，开始写入。
          按原型来源写列/稀疏，设置身份位置与 Live 标记，再逐行推进发布前缀。
          执行阶段失败直接 panic，不撤销或补发失败行。
        */
        crate::world_inner::spawn_inner(self, partition, domain, archetype, values)
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
        crate::world_inner::despawn_inner(self, entity)
    }

    /// 将 entity 改为已显式声明的 target 原型；当前分区与完整身份保持不变。
    /// added 必须恰好是 target 相对当前原型新增的组件；保留组件不接收替换值。
    /// target 不再含的组件销毁；共有组件移动所有权且保留变化元数据，不 Clone。
    /// 宿主校验/准备成功后、搬移前取得一个结构 tick，新增/移除通知共用；系统复用 this_run。
    /// 耗尽在任何本次写入前拒绝；Removed 使用实际移出上下文时间，不用较早的强归零时间。
    /// 校验和容量准备的普通错误保持原实体；开始搬移后失败直接 panic，不回滚。
    /// 直接调用成功返回时新组合可见；系统批量包装按既定作用域提交，不等 settle。
    /// 宿主入口内部取得 Maintaining 与所需结构排他授权；冲突在写入前报错，不等待。
    /// 可持有宿主 OwnedEntity/输入请求：它们保活身份但不借用组件；活组件引用/节点 guard 仍阻止冲突操作。
    /// 禁止将共享 World 强转整个可变引用；系统结构提交复用既有运行授权，不重入宿主维护入口。
    /// @to_design 维护授权、宿主共享只读引用的回收阻止机制及变化记录表示待闭合。
    pub fn change_archetype(
        &self,
        entity: Entity,
        target: ArchetypeId,
        added: crate::entity_values::EntityValues<'_>,
    ) -> Result<(), EcsError> {
        /* 共有组件的存储互转唯一流程主体：
        中文伪码：
          校验实体、目标和新增值集合；预先准备全部列/稀疏空间，不修改源值。
           宿主准备成功后分配本次结构 tick，系统可信路径注入 this_run；耗尽拒绝不搬移。
           同 Table 时保留行；不同 Table 时认领新行，按组件来源计划处理：
            Column→Column：移动值与变化元数据至目标列。
            Column→PartitionSparse：移到当前分区后备，按完整身份安装。
            PartitionSparse→Column：移出 OwningPtr 后移入目标列，不填其他实体的洞。
            同分区后备→同分区后备、SharedSparse→SharedSparse：共有值不动。
          写新增值，处理移除值；不设置事务容器或取消入口。
          组件互转计划见 structural_change::ArchetypeChange；同物理列不自搬。
          统一发布原型和位置；若更换 Table，则源行退休，阶段末物理整理。
          开始搬移后不再返回普通错误；失败直接 panic，不回滚或继续使用 World。
        */
        crate::world_inner::change_archetype_inner(self, entity, target, added)
    }
    /// 在初始化登记阶段显式登记 T 与 storage；不要求 Clone 或 Default。
    /// 同类型同策略返回原 ComponentId，冲突或冻结后登记错误且登记不变。
    /// T 显式实现 Component，框架从 T::Access 生成一致的分类/释放元数据；不构造业务值。
    /// 不提供 static 类型自动普通组件适配，实际稀疏存储初始化由冻结阶段协调。
    pub fn register_component<T: crate::component::Component>(
        &mut self,
        storage: StoragePolicy,
    ) -> Result<ComponentId, EcsError> {
        crate::world_inner::register_component_inner::<T>(self, storage)
    }

    /// 登记 components 对应的 Table；覆盖数量相同时，首次成功登记者优先。
    /// 输入必须是本 World 已登记且非显式稀疏的无重复组件；顺序不影响规范布局。
    /// 相同集合复用首次编号且不改变顺序；非法集合或冻结后登记错误，现有登记不变。
    /// 每 World 一个零号空 Table，基础身份 variant=0，不声明恢复域。
    pub fn register_table(&mut self, components: &[ComponentId]) -> Result<ArchetypeId, EcsError> {
        crate::world_inner::register_table_inner(self, components)
    }

    /// 完成类型和 Table 登记、同号基础原型及各分区存储初始化；冻结后不再新增类型或布局。
    /// 在系统参数初始化和实体创建之前调用；重复成功冻结无副作用。
    /// 校验或准备失败不冻结，不发布部分可运行状态。
    /// @to_design 与物理存储准备的提交接口、分配失败及 panic 边界仍待定。
    pub fn finish_registration(&mut self) -> Result<(), EcsError> {
        crate::world_inner::finish_registration_inner(self)
    }

    /// 冻结后显式声明完整 components 集合；复用相同集合，基础原型与 Table 同号。
    /// 动态原型仅追加声明，不新增 Table；选最大覆盖的子集 Table，同分选先登记者。
    /// 未登记或重复组件、未冻结或编号耗尽返回错误且不公开半声明；不创建实体。
    pub fn register_archetype(
        &mut self,
        components: &[ComponentId],
    ) -> Result<ArchetypeId, EcsError> {
        crate::world_inner::register_archetype_inner(self, components)
    }

    /// 显式选择本 World 基础 Table；base 必须 variant=0 且 components 包含其全部组件。
    /// components 必须符合已有集合幂等规则；失败不登记半原型，不允许跨 World 使用编号。
    pub fn register_archetype_on(
        &mut self,
        base: ArchetypeId,
        components: &[ComponentId],
    ) -> Result<ArchetypeId, EcsError> {
        crate::world_inner::register_archetype_on_inner(self, base, components)
    }
    /// 只读取得 id 的组件元信息，未知编号返回 None；不能用于绕过显式登记。
    pub fn component_info(&self, id: ComponentId) -> Option<&ComponentInfo> {
        crate::world_inner::component_info_inner(self, id)
    }
    /// 只读取得公共 Table 布局，未知编号返回 None，不暴露分区数据。
    pub fn table_layout(&self, id: ArchetypeId) -> Option<&TableLayout> {
        crate::world_inner::table_layout_inner(self, id)
    }
    /// 只读取得原型声明，未知编号返回 None，不返回独立成员列表。
    pub fn archetype(&self, id: ArchetypeId) -> Option<&Archetype> {
        crate::world_inner::archetype_inner(self, id)
    }
    /// 创建默认单分区 World；游戏和渲染分别构造独立 World，不执行系统。
    /// 失败返回错误；不发布部分初始化实例。
    pub fn new() -> Result<Self, EcsError> {
        crate::world_inner::with_partitions_inner::<1>()
    }

    /// 创建分区数为 PARTITION_COUNT 的 World，数量构造后固定。
    /// PARTITION_COUNT 至少为1，容量受12位分区字段限制；不预留渲染分区。
    /// 返回相同的非泛型 World；非法数量或初始化失败返回错误，不发布部分实例。
    /// @to_design 分区可表示上限、分配来源编码及其余初始化配置尚未确定。
    pub fn with_partitions<const PARTITION_COUNT: usize>() -> Result<Self, EcsError> {
        crate::world_inner::with_partitions_inner::<PARTITION_COUNT>()
    }

    /// 校验完整身份后返回位置副本，不持有组件借用，不修改身份映射。
    ///
    /// # Arguments
    /// * `entity` - 完整键，包含代际和来源扩展位。
    ///
    /// # Returns
    /// 无效键返回 None；有效键返回当前存储实例与行，状态不变。
    pub fn locate(&self, entity: Entity) -> Option<EntityLocation> {
        crate::world_inner::locate_inner(self, entity)
    }

    /// 执行纯迁区，保持完整键、组件集合与值，不发组件增删或修改业务通知。
    ///
    /// # Arguments
    /// * `request` - 存活实体与目标当前分区；调用时所有冲突借用须已结束。
    ///
    /// # Returns
    /// 成功后实体仅有一个有效位置；源端留下的待整理行在阶段末由 Self::settle 处理。
    /// 只在所属 World 内迁区；SharedSparse 不动，后备移到目标分区，跨 World 不是迁区。
    /// 可见性沿用 pi_world：目标行初始化完成后才发布，源行失效后查询跳过；
    /// 迁移作用域提交后，有序后继访问看到新位置，不等待阶段末 settle。
    /// 系统内批量变更在变更访问作用域结束时提交，源端冲突访问须由调度排除。
    /// 搬移前完成全部检查和容量准备；开始搬移后失败直接 panic，不回滚。
    /// @to_design 混合存储的访问包装待定。
    ///
    /// # Errors
    /// 无效实体、非法目标及容量不足在搬移前返回错误，保持原实体；执行中不返回普通错误。
    pub fn move_partition(&mut self, request: PartitionMove) -> Result<(), EcsError> {
        /* 设计已确认：先检查和准备，再执行；不是事务。
        中文伪码：
          定位实体并检查目标及访问条件；非法请求在搬移前返回错误。
          若当前区等于目标区，保持原状态；否则准备对应类别与布局的目标存储。
          搬移 Table 行至目标区相同 ArchetypeId，保持组件值与变化元数据；显式共享稀疏不动，分区后备移入目标分区但不产生业务变化通知。
          更新身份来源域映射，源行标记待整理；阶段末由 World::settle 整理并修正行号。
          成功后发布迁区结果；中途失败直接 panic，不撤销。
        */
        crate::world_inner::move_partition_inner(self, request)
    }

    /// 在阶段末整理结构修改留下的待移除 Table 行，并修正换行影响的身份位置。
    /// self 必须处于无其他线程、查询、组件引用或存储视图访问的独占同步点。
    /// 整理本身不改变实体身份和逻辑组件集合，不伪造 Added/Removed/Changed。
    /// 成功后待整理行已处理，所有存活实体指向正确的当前分区、Table 和行。
    /// settle 负责物理填洞及位置修复，不作为结构修改首次可见的发布点。
    /// 已移出或已析构的值不得再次析构；移除值的处理在对应修改提交路径完成。
    /// 按分区编号、ArchetypeId 升序处理；同 Table 用尾部存活行填前方洞，全部列共用动作表。
    /// 每次移动修复对应来源域的位置，之后按新长度合并 pi_arr 桶并重置认领/发布长度。
    /// 无待移除行时仍可合并扩展桶；重复调用不产生逻辑状态变化。
    /// Table 整理须同步修正原型成员的 table_row；原型成员填洞须修正 EntityLocation::archetype_row。
    /// SparseSet 不参与阶段末整理；其 remove 只返回 OwningPtr，不搬动其他实体。
    /// 整理前准备动作表和所需空间；开始搬行后失败直接 panic，不回滚。
    /// @to_design 调度接入协议待定。
    pub fn settle(&mut self) -> Result<(), EcsError> {
        /* 设计已确认：沿用 pi_world 的阶段末整理，先准备动作表再执行。
        中文伪码：
          在独占同步点处理待移除行；不对纯迁区的共享稀疏值执行操作。
          按区号和 ArchetypeId 升序生成尾部存活行到前洞的动作表；全部列应用同表。
          更新行标记和各来源域的位置；无活值区截尾，按新长度合并 pi_arr 桶。
          清理待整理记录并重置认领/发布长度；中途失败直接 panic，不恢复或继续使用 World。
          整理原型成员并修正 archetype_row；不调用 SparseSet 整理。
        */
        crate::world_inner::settle_inner(self)
    }
}
