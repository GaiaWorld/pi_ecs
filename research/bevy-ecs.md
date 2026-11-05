# Bevy ECS 一手资料调研：供 pi_ecs 重写参考

初次调研：2026-10-22；模块全貌补充复核：2026-10-28。目标项目：D:/dev/pi_ecs；旧实现：D:/dev/pi_world。本文保留身份、存储、查询与复制专题，补入完整模块及 crate 关系；本地实现对照和 pi_ecs 的需求／拆分提案统一见 [pi_world 与需求对照报告](pi-world-and-requirements.md)。

## 1. 版本、证据与结论口径

**本次核实的当前稳定发布版为 Bevy 0.20.0 / bevy_ecs 0.20.0，发布于 2026-10-08。**

- 官方发布公告明确宣布 Bevy 0.20 发布：[bevy.org 发布公告](https://bevy.org/news/bevy-0-20/)。
- 调研时 [bevy latest](https://docs.rs/bevy/latest/bevy/) 和 [bevy_ecs latest](https://docs.rs/bevy_ecs/latest/bevy_ecs/) 均显示 0.20.0、08 October 2026；正文证据全部固定到 0.20.0，未把 main 或预发布版作为稳定基线。
- 通过 gh 查询官方 GitHub releases/latest 的尝试因本机未安装 gh 而失败；没有用其结果推断版本。版本以官方公告和 docs.rs 已发布 crate 交叉核实。
- 官方源码采用 docs.rs 中随 0.20.0 发布的源码镜像；它比指向 main 的 GitHub 链接更适合复核此版本行为。

标记含义：**【事实】**为官方文档明确陈述或该版本源码直接可见；**【边界】**说明证据覆盖范围与不能外推之处；**【推论】**为面向 pi_ecs 的设计判断，不是 Bevy 官方承诺。本文未改生产代码，未运行 Bevy 基准或编译实验；pi_world 静态阅读另见对照报告，未完成逐项兼容审计。原推论保留其研究性质；pi_ecs 当前决定以活动设计和源码契约为准。

## 2. 摘要

| 关注点 | 0.20.0 的确定能力 | 对 pi_ecs 的关键提醒 |
|---|---|---|
| Entity | World 内使用的 opaque ID，index + generation；代数可回绕 | 不是跨 World / 跨运行的业务主键 |
| 存储 | archetype 按完整组件集合分类；table 存 table 组件；SparseSet 单独存储 | archetype、table、业务类型、运行时分区应分清 |
| 查询缓存 | QueryState 缓存匹配存储及 fetch/filter 状态，绑定 WorldId | 不是业务条件的实体结果缓存 |
| 访问冲突 | FilteredAccessSet 按读写及 With/Without 互斥证明判定；调度器缓存系统冲突 | 不能从两段代码处理不同分区值自动得到并行性 |
| 实体克隆 | EntityCloner 配置组件克隆、映射、linked relationship 递归 | 不等于任意组件必能深拷贝，也不是跨 World 整体复制器 |
| 整体快照 | World 未实现 Clone；有实体/场景级构造能力 | 必须定义“数据复制”还是“完整运行状态回滚” |
| COW | ResolvedScene 官方明确有缓存模板 COW | 不能外推为 World / table / sparse-set 的运行时 COW |
| 局部并行 | 指定实体列表查询；唯一 EntitySet 的局部可变并行迭代 | 已有有用基础，但不等于分区权限成为调度的一等概念 |

## 3. Entity 身份、代数与跨 World

### 3.1 身份和存储位置分离

**【事实】**Entity 只是实体标识，不是实体本身，也不保证所指实体还活着。该版暴露 EntityIndex、EntityGeneration、index()/generation()，to_bits() 返回 u64，但明确不保证位布局。generation 随该 index 的实体 despawn 而递增；index 会被复用。

来源：[Entity](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/struct.Entity.html)，[发布源码 entity/mod.rs](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/entity/mod.rs.html#424-432)。

**【事实】**EntityLocation 分别含 archetype_id、archetype_row、table_id、table_row。这些是位置元数据，不能当成长期实体身份；同一实体改变组件集合时会改变 archetype，必要时改变 table。

来源：[EntityLocation](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/struct.EntityLocation.html)，[archetype::Edges](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/archetype/index.html)。

### 3.2 代数不能提供无限期唯一性

**【事实】**EntityGeneration 文档说明内部是 u32，能够 wrap / alias。旧 Entity 长期保留后，可能与后来不同概念实体的 ID 相等；Entity 文档专门说明此风险。不能表述成“generation 永远杜绝 ABA”。其 cmp_approx 对相距很远的代数可能非传递，因此 EntityGeneration 不实现 Ord/PartialOrd。

来源：[EntityGeneration / Aliasing](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/struct.EntityGeneration.html#aliasing)，[Entity / Aliasing](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/struct.Entity.html#aliasing)。

### 3.3 World 作用域与持久化

**【事实】**MapEntities 文档明确：Entity ID 仅对其来源 World 有效。从另一 World 复制组件后，里面的 Entity 引用需要映射。Entity 本身不携带 WorldId；两个 World 的同位模式不能视为同一个业务对象，错误 World 中甚至可能碰巧对应另一个活实体。

**【事实】**World 有独立 WorldId，WorldId::new() 的标识跨时间不复用；这是 World 标识，不是 Entity 自动附带的命名空间。

**【事实】**Entity 的二进制表示、Hash/Ord 等细节不提供长期版本稳定保证；直接 Serialize/Deserialize 不保证长期 wire compatibility，to_bits() 明确不应用于跨运行序列化。from_index() 的官方说明建议同步时插入二级业务标识组件，不应靠让不同实例的 Entity 恰好一致来同步。

来源：[MapEntities](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/trait.MapEntities.html)，[WorldId](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/world/struct.WorldId.html)，[Entity / stability-warning](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/struct.Entity.html#stability-warning)，[Entity::from_index](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/struct.Entity.html#method.from_index)。

**【推论】**pi_ecs 若需要跨 World 引用、分叉身份或网络同步，应分离 LocalEntity、World/Branch 身份、稳定业务 ID。选用复合句柄或显式映射都可，但不要把 Bevy 的裸 Entity 当成全局 UUID；还应明确代数回绕策略、失效引用处理和快照恢复后的身份规则。

## 4. archetype / table / sparse-set

### 4.1 三者关系

**【事实】**一个 World 对每种“完整组件类型集合”只有一个 archetype。每个 archetype 对应一个 table，但多个 archetype 可共用一个 table，它们只在 SparseSet 组件集合上不同。空 archetype 不被清理，持续存在到 World 被 drop；archetype 模块说明 tables 也有类似的创建后不清理特征。

来源：[archetype 模块](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/archetype/index.html)。


    实体完整集合                          table 存储
    {Position, Velocity, SparseTagA} ─┐
                                     ├─ {Position, Velocity} 的 SoA 列
    {Position, Velocity, SparseTagB} ─┘

    SparseTagA / SparseTagB → 对应组件的 ComponentSparseSet

**【事实】**Table 是列式 SoA 存储。Tables 优化连续迭代，SparseSets 优化随机访问及频繁插入/删除；默认组件存储是 Table，可用 #[component(storage = "SparseSet")] 选择稀疏存储。SparseSet 不是“每个 archetype 一张独立稀疏表”：SparseSets 是按 ComponentId 索引的组件存储集合。

来源：[storage 模块](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/storage/index.html)，[bevy_ecs / component-storage](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/#component-storage)，[SparseSets](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/storage/struct.SparseSets.html)。

**【边界】**“增删快、遍历慢”是官方总体权衡，不是任意负载的实测保证。仅变更 sparse 组件仍改变完整组件集合，因而可能换 archetype，但 table 组件集合相同可以共用 table；不能说 SparseSet 消除了所有结构变更成本。

### 4.2 组件 ID 和业务类型不是一回事

**【事实】**ComponentId 是 World 局部的组件/资源类型标识。除 Rust TypeId 对应组件，还能通过 descriptor 注册不在 Rust 类型系统中的动态组件；不能把另一个 World 的 ComponentId 直接用于访问，官方文档明确告诫此类使用会导致未定义行为。

来源：[ComponentId](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/component/struct.ComponentId.html)。

**【推论】**若 pi_ecs 要支持“同组件组成的不同业务实体类型”或“同 schema 的多个运行时区块”，archetype 不能自然充当这些身份。大量 marker 类型可以制造更多 archetype；SparseSet marker 虽可能复用 table，仍有 archetype 数量和查询路径代价。需要针对业务负载测量，而非仅凭组件布局判断。

## 5. 查询缓存、访问冲突和调度

### 5.1 QueryState 缓存了什么

**【事实】**QueryState 保存 WorldId、archetype_generation、matched_tables / matched_archetypes、component_access、fetch_state、filter_state，以及 matched_storage_ids。is_dense 决定存储 ID 列表采用 TableId 还是 ArchetypeId。缓存跨 system 运行复用，主要避免重复匹配存储和构造元数据，不是每次结果 Entity 列表的物化快照。

**【事实】**update_archetypes 根据 archetype generation 处理新 archetype；当查询有必需组件时，源码还会通过 component_index 选择候选更少的必需组件，缩小新增匹配扫描范围。安全入口校验 WorldId；常规方法自动更新，manual 方法需要调用方维护缓存，否则可能漏掉新 archetype 的实体。

来源：[QueryState](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/query/struct.QueryState.html)，[state.rs 字段](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/state.rs.html#79-101)，[更新和 World 校验实现](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/state.rs.html#547-639)。

**【边界】**存在 Changed/Added 等实体级条件，不应把“缓存匹配 archetype”误解成所有查询过滤都只执行一次。也不应照搬某个旧版关于 archetype_component_access 的调度解释来描述本版。

### 5.2 冲突证明采用访问元数据

**【事实】**FilteredAccess 包含读写权限和组件存在/不存在的过滤条件；FilteredAccessSet 保存组合访问和各个过滤访问。兼容检查先做粗粒度读写检查；有冲突时，再比较 With/Without 等约束能否证明访问对象互斥。

来源：[FilteredAccess](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/query/struct.FilteredAccess.html)，[FilteredAccessSet::is_compatible](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/query/struct.FilteredAccessSet.html#method.is_compatible)。

**【事实】**一个 system 内两个可变查询若可能访问同一组件会在初始化时报 panic；With<Player> 和 With<Enemy> 不能单独证明互斥，因为实体可以同时有两个 marker。加入相应 Without 可证明不重叠。ParamSet 可允许分时访问冲突参数，但不是把它们变成可同时使用的可变借用。

来源：[Query / disjoint-queries](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/system/struct.Query.html#disjoint-queries)。

### 5.3 调度器实际使用什么

**【事实】**0.20.0 MultiThreadedExecutor 初始化时对 system.access.is_compatible() 做两两检查并缓存 conflicting_systems；运行时将就绪系统与正在运行的冲突系统集合比较，还考虑依赖、条件、exclusive、非 Send 等约束。

来源：[multi_threaded.rs 冲突构造](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/schedule/executor/multi_threaded.rs.html#180-227)，[can_run](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/schedule/executor/multi_threaded.rs.html#531-590)，[schedule 模块](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/schedule/index.html)。

**【事实】**调度顺序需要显式依赖来规定；不能把冲突导致串行等同于业务上确定的先后。Commands 等延迟缓冲通过 ApplyDeferred 应用，运行某个 system 与它的结构变更对后继可见并非同一时点。

来源：[bevy_ecs / schedules](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/#schedules)，[ApplyDeferred](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/schedule/struct.ApplyDeferred.html)。

**【推论】**pi_ecs 若引入“分区 P 内写 Position”的权限，需要让分区权限进入访问证明/借用模型；不能只在循环内判断 PartitionId 后就绕过组件级可变冲突。共享 Resource、跨分区关系访问、结构迁移仍可能使原本独立的任务冲突。

## 6. EntityCloner、组件克隆与引用重映射

### 6.1 克隆不是普遍深拷贝

**【事实】**EntityCloner 可通过 opt-out / opt-in builder 选择组件，支持局部 clone behavior 覆盖、组件自定义 clone behavior、默认 handler 覆盖。默认情况下，derive Component 且实现 Clone 或 Reflect（配合 ReflectComponent）的组件可被克隆，优先 Clone。手动实现 Component 或条件 Clone 有额外 handler 选择注意事项；缺少可用 handler 时不能假设组件必被复制。

**【事实】**handler 优先级是：局部 override → Component::clone_behavior → builder 默认函数 override → 按 bevy_reflect feature 选择的反射或 noop 默认函数。Clone 的含义仍由组件自己决定；含 Arc 等共享对象时不能假设底层数据独立。

来源：[EntityCloner 默认策略与 Clone Behaviors](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/struct.EntityCloner.html#default-cloning-strategy)。Arc 的独立性判断是 Rust Clone 语义下的设计推论，不是 Bevy 替应用保证的深拷贝。

**【事实】**move_components 模式存在，并会触发生命周期 hooks / observers。不能把移动当作无副作用 memcpy，也不能把克隆配置理解为完整 World 的原样恢复机制。

来源：[EntityCloner / moving-components](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/struct.EntityCloner.html#moving-components)。

### 6.2 映射接入点与未映射引用

**【事实】**Component::map_entities 是组件引用映射的接入点。derive Component 时需用 #[entities] 标记含实体引用的字段；字段要实现 MapEntities。也可用 #[component(map_entities)] 委托组件自身 MapEntities，或指定映射函数。仅独立实现 MapEntities 不应被当成“组件自动接入”的充分条件。

来源：[Component::map_entities](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/component/trait.Component.html#method.map_entities)，[MapEntities](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/trait.MapEntities.html)。

**【事实】**克隆 handler 的 typed write_target_component 在写入前调用 C::map_entities；源码也提供 unsafe pointer 写入路径，不能推断所有自定义原始指针 handler 都自动做同样的 typed 映射。

来源：[clone_entities.rs / write_target_component](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/entity/clone_entities.rs.html#180-231)。

**【事实】**EntityMapper 解耦映射策略。EntityHashMap<Entity> 的实现对缺失项返回原 source Entity；不是自动报错或自动生成目标。SceneEntityMapper 则提供为源集合外引用分配不指向存活实体的目标引用的机制，两者不能混为一谈。

来源：[EntityMapper](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/trait.EntityMapper.html)，[map_entities.rs 对照实现](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/entity/map_entities.rs.html#297-376)。

**【推论】**复制任意实体图时，先为目标集合建立完整 source→target 映射，再复制和修复引用，有助于处理环与前向引用；对图外引用必须显式选择保留、失效、拒绝、转为业务 ID 或额外扩展复制范围。不能把缺失映射保留旧 ID 的默认行为直接用于跨 World 复制，否则可能误指目标 World 的无关实体。

### 6.3 递归图与跨 World 边界

**【事实】**linked_cloning 围绕 RelationshipTarget::LINKED_SPAWN 递归产生 linked 关系树复制；不等于追踪所有 Entity 字段做通用可达图深克隆。clone_entity_mapped 允许控制目标映射。

**【事实】**本版 EntityCloner::clone_entity / spawn_clone / clone_entity_mapped 接口均只接收一个 &mut World；源码从该 World 同时读取 source 和写入 target。它不是现成接收 source_world 和 destination_world 的跨 World 克隆 API。MapEntities / EntityMapper 能用于跨 World 引用转换，不意味着 EntityCloner 本身已经完成跨 World 数据搬运。

来源：[EntityCloner 方法签名](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/entity/struct.EntityCloner.html#implementations)，[单 World 克隆实现](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/entity/clone_entities.rs.html#494-575)。

## 7. World 整体克隆、快照与 COW

### 7.1 World 不是一个 Clone 值

**【事实】**0.20.0 World 的 rustdoc trait implementations 不含 Clone。World 源码除实体和组件存储外，还包括独立 allocator、组件注册信息、资源实体索引、archetype、bundle、observer、removed component messages、change ticks、command queue 等。

来源：[World trait implementations](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/world/struct.World.html#trait-implementations)，[World 定义](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/world/mod.rs.html#98-127)。

**【边界】**可确定的是“此版 World 没有实现 Clone”，以及所查官方公开接口没有给出保留完整运行态的一键 fork/snapshot/COW 契约；不是声称所有外部 crate、特定数据约束或用户自定义复制方案都不可能做到。

**【推论】**实体数据复制不等于完整快照。pi_ecs 应明确是否包含资源/非 Send 数据、未应用命令、消息消费位置、observer 注册、系统 Local/调度状态、变更 tick、外部资源句柄、allocator 的后续分配序列。Schedule / system state 还可能由 World 之外的调用方持有，仅复制 World 数据不足以保证确定性回滚。

### 7.2 0.20 场景能力必须按本版核实

**【事实】**本版 bevy_scene 索引提供 Scene trait、SceneList、ResolvedScene、ScenePatch、Template 和 BSN。Scene 的语义是描述单个根实体应如何生成/修改，相关实体可组合；不是“持有一个完整 World 的通用运行时快照”。

来源：[bevy_scene 0.20.0 索引](https://docs.rs/bevy_scene/0.20.0/bevy_scene/)，[Scene trait](https://docs.rs/bevy_scene/0.20.0/bevy_scene/trait.Scene.html)，[0.19→0.20 迁移说明](https://bevy.org/learn/migration-guides/0-19-to-0-20/)。

**【边界】**本次访问该版 DynamicScene / DynamicSceneBuilder / struct.Scene 的旧路径返回 404，并从当前索引确认了不同 API。不能因此单凭 404 断言所有历史场景保存能力在所有包中绝对消失；但不能把旧版 DynamicScene::from_world / write_to_world / Scene::clone_with 当作已核实的 0.20.0 接口。本文不以这些旧接口证明本版快照支持。

### 7.3 存在 COW，但对象是缓存场景模板

**【事实】**ResolvedScene 文档明确写有 copy-on-write：请求缓存场景里已有的 Template 时，通过 Template::clone_template 克隆到当前 ResolvedScene；应用时先应用缓存场景，再应用本地场景。

来源：[ResolvedScene](https://docs.rs/bevy_scene/0.20.0/bevy_scene/struct.ResolvedScene.html)，[get_or_insert_template](https://docs.rs/bevy_scene/0.20.0/bevy_scene/struct.ResolvedScene.html#method.get_or_insert_template)。

**【边界】**这是构造/复用场景描述的 COW。它不承诺已实例化实体的组件页共享、World 分叉、存储页复制、首次可变访问自动分离，或快照回滚。Column 的官方模型是 type-erased 连续数据及独立 change tick 缓冲；所查 World/Table/Column API 未暴露运行时存储 COW 契约。

来源：[Column](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/storage/struct.Column.html)，[storage](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/storage/index.html)，[World](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/world/struct.World.html)。

**【推论】**如果 pi_ecs 的核心目标是低成本 World 分叉，应独立设计页/块的所有权与写屏障、结构变更时的共享拆分、change ticks、drop、外部共享句柄和跨分支引用语义。组件内使用 Arc 可以实现局部共享，但并不自动形成存储层 COW，也不能隔离 Arc<Mutex<T>> 等共享可变状态。

## 8. 实体逻辑类型、运行时分区、局部查询与分区并行

### 8.1 逻辑实体类型不是 archetype，也不是 Bundle

**【事实】**Bevy 通过组件组合和 marker 表达类别；Bundle 是一组静态组件类型，用于插入/删除。官方明确反对把 Bundle 当作行为抽象单元，并有意不提供按“拥有某 Bundle”直接匹配的 Query。运行时 schema 可以用动态 ComponentId / QueryBuilder 表达组件存在条件。

来源：[Bundle](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/bundle/trait.Bundle.html)，[QueryBuilder](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/query/struct.QueryBuilder.html)。

**【推论】**若 pi_ecs 需要稳定的 EntityKind（例如角色/场景块/业务实例），应明确其是否允许随组件变化而保持不变，以及同组件集合的不同 Kind 是否共享布局。可以通过 marker/普通字段建模，但这些方案不自动获得“逻辑类型 ID 就是物理分区 ID”的保证。

### 8.2 运行时值不会自动形成物理分区

**【事实→推论】**archetype 分类依据组件类型集合，不依据组件值。因此 PartitionId(1) 与 PartitionId(2) 若其他组件集合相同，会属于同一 archetype；给每个实体一个 PartitionId 字段不会自动分成两张 table。这是从官方 archetype 定义直接得出的结构推论。

来源：[archetype 定义](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/archetype/index.html)。

**【边界】**本次检索的公开 ECS API / 查询访问实现中，未见类似“按任意 PartitionId 值建立持久分区、分区级借用、自动分区调度”的统一一等接口。不是断言用自定义 SystemParam、调度器或专用数据结构无法实现；也不把 hierarchy/relationship 当成已经保证连续存储的物理分区。

### 8.3 已有真正的局部查询和局部并行

**【事实】**Query 支持 get / get_many / iter_many，以及 unique 实体集合变体。官方复杂度表给出 get 为 O(1)、iter_many 为 O(k)，因此已有外部分区成员索引时可以只访问 k 个成员，而不必先全 World 扫描再按分区值筛选。成员索引的维护成本不包含在这个 O(k) 中。

**【事实】**本版还有 par_iter_many、par_iter_many_unique、par_iter_many_unique_mut。可变 unique 版本要求 EntitySet，返回 QueryParManyUniqueIter，通过 for_each 消费；不保证顺序/线程分配。没有 multithreaded feature 时退化为相应顺序执行行为。不存在或不满足组件条件的实体生成 QueryEntityError，调用者可按接口处理或只取 matched 项。

来源：[Query / performance](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/system/struct.Query.html#performance)，[iter_many_mut](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/system/struct.Query.html#method.iter_many_mut)，[par_iter_many_unique_mut](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/system/struct.Query.html#method.par_iter_many_unique_mut)。

**【边界】**“指定成员集合的局部可变并行迭代”已经存在，不能说 Bevy 只能全局查询、不能局部并行。EntitySet 的唯一性用于防止一个迭代内部对同实体重复产生可变访问；不等于调度器已证明两份独立分区列表互不重叠。

### 8.4 不同分区任务何时能并行

| 方案 | 可确认或合理预期的能力 | 仍需承担的成本/边界 |
|---|---|---|
| 单查询 par_iter_mut | 查询内部数据并行 | 不是按业务分区预先建立独占权限 |
| 成员索引 + par_iter_many_unique_mut | 单 system 内指定实体子集并行 | 成员唯一性、索引更新、随机访存；跨任务仍需借用证明 |
| With<A> / Without<A> 分开查询 | FilteredAccess 可证明互斥，供系统并行 | 类型/组件存在约束；不同 With marker 本身未必互斥 |
| PartitionId 值判断 | 应用能选择处理哪些实体 | 标准访问元数据不读取分区字段值，不能据此自动放开写冲突 |
| 动态 marker ComponentId + QueryBuilder | 运行时构造组件存在过滤 | 不是同一个组件的不同值；仍有注册、archetype 和系统参数接入成本 |
| 多 World | 存储和本地查询状态分离，可由上层调度 | Entity / ComponentId 命名空间、跨 World 通信、资源共享和迁移另行处理 |

表内“标准访问元数据不读取分区字段值”的依据为 FilteredAccessSet 及 MultiThreadedExecutor 实现；应用如何组合多 World 或索引是设计推论，而非现成的分区调度产品承诺。

来源：[FilteredAccessSet](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/query/struct.FilteredAccessSet.html)，[多线程执行器源码](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/schedule/executor/multi_threaded.rs.html)，[QueryBuilder](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/query/struct.QueryBuilder.html)。

## 9. 对 pi_ecs 重写的设计启示（均为推论）

1. **至少分清五种概念**：实体身份、组件 schema/archetype、业务逻辑类型、运行时分区、World/分支。允许组合，但不应在 API 上默认它们等价。
2. **保留 identity→location 间接层**：结构迁移不改变本地实体身份；对外不暴露可永久缓存的 row 指针契约。
3. **查询计划和成员索引分层**：借鉴 QueryState 缓存结构匹配；若局部遍历是核心需求，再增加 Partition→members/blocks 索引，维护迁入迁出、despawn、结构变更后的有效性。
4. **若分区并行是一等需求，权限也要一等化**：可考虑 (component, partition) 访问域或不可重叠的 partition lease；同时定义跨区访问、共享资源、关系遍历和迁移阶段。不能让用户通过任意 unsafe 声称列表互斥。
5. **区分三种复制接口**：同 World 实体/关系图克隆、跨 World 数据导入、完整运行态快照/分叉。每类各有身份映射、不可克隆组件、外部引用和生命周期副作用策略。
6. **快照优先界定语义，再决定 COW 粒度**：组件值、列、块/页、分区具有不同复制和迁移成本。Bevy 场景模板 COW 可作为描述层复用的参考，不是运行时存储 COW 的现成答案。
7. **以工作负载验证而非借用标签**：重点衡量同 schema 多分区的局部顺序迭代、随机成员访问、分区迁移、标签抖动、archetype 长期增长、稀疏组件 churn、分支小写集和共享资源造成的串行比例。

## 10. 证据完整性与待后续验证项

- 已使用官方发布公告、固定版本 rustdoc、固定版本发布源码交叉查证核心结论；没有依赖第三方博客。
- 对 World COW、完整快照、任意运行时分区一等支持的负面结论，均限定为所查 API/实现未提供对应契约；未将搜索不到等同于绝对不可实现。
- 部分大型源码网页会受抓取长度限制；本文源码引用仅针对实际读取到的字段或函数片段，不以整文件关键词未命中证明功能不存在。World 无 Clone 结论另有完整 trait implementations 作为依据。
- 本文不确认旧版场景序列化迁移到何处、第三方回滚库覆盖范围或自定义 SystemParam 扩展的完备性；这些都不改变当前已核实接口的边界。
- 后续若决定采用 Bevy 或移植其设计，应对指定目标和 feature 组合编写最小编译验证：图内/图外引用、自定义 clone handler、分区过滤系统并行、局部 unique 可变迭代、复制后的 change detection；本文没有把待验证项写成实验结果。

## 11. 完整模块与应用层关系（2026-10-28 补充）

### 11.1 官方模块地图

**【事实】**以下分组用于阅读；表内分组名称不是 Bevy 的额外 crate。除应用层及宏包外，所列运行时模块都在 bevy_ecs 中。

| 职责组 | Bevy 0.20.0 模块／能力 | 关键关系 |
|---|---|---|
| 数据定义与存储 | entity、component、bundle、archetype、storage、resource | 身份、类型、组件组合和物理存储各有职责，World 持有世界数据 |
| 世界操作与查询 | world、query、change_detection | 直接访问不要求先构造 App；查询匹配、访问描述和变化过滤相互配合 |
| 系统与运行 | system、schedule；Commands 位于 system 下 | SystemParam 抽取参数，函数适配为 System，Schedule 组织执行与延迟操作同步 |
| 通信与反应 | message、event、observer、lifecycle | 缓冲消息、触发事件、观察者系统和组件生命周期相关但不同 |
| 实体关联 | relationship、hierarchy、traversal、spawn | 提供关系组件、父子关系、遍历与关联生成，不等于物理地图分区 |
| 扩展与支持 | reflect、entity_disabling、name、template、batching、error、label、intern 等 | 反射、禁用、命名、模板、批次、错误与标签等支持能力，不必逐项映射为 pi_ecs 的独立 crate |
| 应用组装 | 独立 bevy_app crate：App、Plugin、PluginGroup、应用阶段及 runner | 应用层依赖 ECS；ECS 可单独配合 World 和 Schedule 使用 |
| 代码生成 | bevy_ecs_macros crate | 为运行时声明生成代码；不等于另一套存储或调度实现 |

来源：[bevy_ecs 模块索引](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/)、[system](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/system/index.html)、[schedule](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/schedule/index.html)、[bevy_app 索引及依赖](https://docs.rs/bevy_app/0.20.0/bevy_app/index.html)。此次重新读取了这些固定版本页面；不是依据旧版本名称补全。

### 11.2 消息、触发与生命周期不能混称

**【事实】**MessageWriter／MessageReader 形成缓冲通信；Observer 是监听特定 Event 触发的系统，官方将其与按日程读取的消息区分，描述为触发时立即响应。lifecycle 则提供对组件插入移除和实体创建销毁的反应能力。触发内部细节和延迟命令的应用时机仍需看具体入口，不能把“立即响应”外推成所有后续结构修改都立即可见。

来源：[消息说明](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/#messages)、[观察者说明](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/#observers)、[lifecycle](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/lifecycle/index.html)。

**【推论】**pi_ecs 需要分别定义队列内容、读游标、触发反应与恢复安装通知。缓冲消息、普通业务反应和不可省略的内部一致性维护不能共用一个模糊的“事件是否恢复”开关。

### 11.3 模块划分不是 crate 拆分结论

**【事实】**Bevy 将存储、查询、系统与调度放在同一个 bevy_ecs crate，将应用层放在 bevy_app。它没有采用本次讨论的 pi_ecs_core／pi_ecs_schedule 拆分。

**【推论】**本项目可以按数据所有权与运行状态划分 crate，但必须自行设计跨包数据访问、查询参数适配、生命周期反应和恢复协调界面。尤其需要检查引用有效期、unsafe 前提、缓存失效、延迟操作和系统观察窗口；不能以 Bevy 的内部模块可协作为证据，宣称这些跨 crate 契约已成熟。pi_ecs 候选边界及未决项统一记录在对照报告中。
