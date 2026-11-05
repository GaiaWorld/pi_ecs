# pi_world 与 pi_ecs 需求对齐前调研

初次调研：2026-10-22；模块全貌、需求进展与 crate 提案合并：2026-10-28。旧工程：D:/dev/pi_world，Cargo.toml 包版本 0.2.17；README 的 v0.x 章节视为开发记录，不当作当前包版本。Bevy 基线 0.20.0，详见 [官方资料调研](bevy-ecs.md)。本文为源码静态阅读，未执行基准或复现 README 问题。

口径：源码机制属于事实；用户确认属于需求；模块与 crate 划分属于待确认设计提案。本文提供研究依据和讨论快照，不替代 [活动设计](../design/unified-partitioned-ecs/design.md) 或源码契约。原来的第一轮问题仅保留为研究起点，后续进展优先。

## pi_world 已核实机制

- 身份和位置分离：Entity 由 new_key_type! 定义，World.entities 为 SlotMap<Entity, EntityAddr>，地址记录 ArchetypeIndex 与 Row。没有把行号直接作为实体身份。键的位宽和代数策略尚未继续审计 pi_slot/pi_key_alloter 依赖。证据：[world.rs:53](../../pi_world/src/world.rs#L53)、[world.rs:123](../../pi_world/src/world.rs#L123)、[world.rs:789](../../pi_world/src/world.rs#L789)。
- 原型按排序后的 ComponentIndex 集合计算哈希；名称由组件类型名称拼接。它表达组件组成，未见独立的业务 EntityKind 或 PartitionId 维度。证据：[archetype.rs:368-402](../../pi_world/src/archetype.rs#L368)。这里只限定于已读核心结构，不排除应用层自己实现分类。
- 一个 Archetype 持有一个 Table；Table 存实体行与组件列的共享引用；World 按组件登记 Column，Column 再按原型索引分配 BlobTicks。列对象共享不表示不同 World 的组件数据隔离共享。证据：[archetype.rs:182](../../pi_world/src/archetype.rs#L182)、[table.rs:20](../../pi_world/src/table.rs#L20)、[column.rs:25-28](../../pi_world/src/column.rs#L25)、[column.rs:67-75](../../pi_world/src/column.rs#L67)。
- 结构变更采用新行分配、旧行标记移除、阶段末统一整理的设计；Table.settle 调整行后调用 World.replace_row 更新地址。World.settle 要求没有其他线程访问。证据：[table.rs:1-5](../../pi_world/src/table.rs#L1)、[table.rs:231-271](../../pi_world/src/table.rs#L231)、[world.rs:698-731](../../pi_world/src/world.rs#L698)。
- archetype.rs 顶部提到 Cow，语境是 Alter 迁移与旧引用有效性；不能据此声称已有 World 快照或跨分支存储 COW。证据：[archetype.rs:1-12](../../pi_world/src/archetype.rs#L1)。
- QueryState 保存 fetch/filter 状态、QState、World/SystemMeta 指针，以及最近原型的 fetch/filter 缓存；不是可直接原样复制到另一 World 的纯查询描述。证据：[query.rs:358-414](../../pi_world/src/query.rs#L358)。
- 调度记录组件关系和资源关系，执行图有 ArchetypeComponent 节点；可借鉴精细访问粒度，但分区加入后需要重新定义访问域、结构迁移和全局资源冲突。证据：[system.rs:330-341](../../pi_world/src/system.rs#L330)、[exec_graph.rs:997-1003](../../pi_world/src/exec_graph.rs#L997)。
- ComponentInfo 包含类型信息、drop_fn、set_fn、尺寸及 tick 信息，未包含通用 clone_fn/map_entities 契约；其 derive Clone 复制的是元数据，不能证明组件实例能被克隆。证据：[archetype.rs:410-446](../../pi_world/src/archetype.rs#L410)。
- README 记录性能不满意、多线程 schedule 卡住、测试覆盖不足等历史问题。此次没有复现，不能当作当前可复现缺陷或已测得结论。证据：[README.md:1-18](../../pi_world/README.md#L1)。

## 两者对照与设计判断

| 维度 | pi_world 核心实现 | Bevy 0.20.0 | pi_ecs 待明确 |
|---|---|---|---|
| 实体身份 | SlotMap 键与原型行间接层 | World 局部 Entity 与位置间接层 | 迁区、克隆、分叉后的身份 |
| 存储 | Archetype/Table + 按组件组织的多原型 Blob | Archetype + Table/SparseSet | 类型、分区与物理块的关系 |
| 业务类型 | 原型描述组件集合 | marker/组件组合，可自行建模 | 类型是否固定 schema、能否改变 |
| 分区 | 已读核心结构无独立维度 | 分区值不自动形成物理分区 | 是否互斥、迁移、局部遍历和并行 |
| 拷贝 | 元数据无通用组件克隆钩子 | EntityCloner，不是 World::clone | 对象范围、独立性、复制策略 |
| 运行态 | 资源、事件、tick、查询/系统状态 | 同样存在 World 外系统状态 | 快照包含范围与可见时点 |

推论：保留身份/位置分离、SoA、查询计划缓存、访问元数据等机制；不要在“快速拷贝”语义明确前选择整列或分页 COW。普通 Clone 也不承诺 Arc<Mutex<T>> 这类内部共享可变数据的独立性。

## 第一轮需求入口（未确认）

1. 快速拷贝最典型操作：单实体/实体图复制、批量类型复制、分区复制、整个 World 分叉、快照回滚？源和目标是否同时继续修改？
2. 分区最典型业务单位：地图区域、战斗副本、租户、场景、内存管理或其他？实体是否唯一归属，是否迁区，是否跨区引用？
3. 分类型的类型是逻辑身份、固定组件 schema、Rust 编译期类型，还是可动态组合组件的分组？
4. 后续再确认：数据规模、写入比例、复制频率、时延和内存预算、平台、资源/事件/系统状态范围、旧 API 兼容要求。

## 后续对齐：已确认的身份与恢复需求

以下更新优先于前文的未确认入口，不代表具体存储方案已确定。

- 游戏与渲染共用一个 World、查询与调度体系；目标是降低跨 ECS 数据搬运和衔接代码的维护成本，同时容纳两者结构变化、资源约束、执行节奏和开发节奏的差异。性能收益尚未实测。
- 句柄可以区分类型，查询可以统一；用户认为标记组件也可用于分类，核心诉求是独立、稳定的 ID 分配域。强类型句柄及组件归属约束暂未冻结。
- 游戏地图分区与渲染实体应具有隔离的分配状态；不同分配域的完整身份应避免冲突。具体算法、位宽和域生命周期待定。
- 实体迁区保留身份；当前所在地图分区与身份分配来源分离。
- 新实体默认使用出生时所在地图区的游戏分配器。例如角色由 A 区迁入 B 区后创建子弹，子弹默认由 B 区分配身份。
- 采用相同模拟序列产生相同 ID 的确定性目标。同域创建删除以确定顺序直接执行，立即得到最终 ID；删除后立即可复用，LIFO。创建失败允许消耗代际，但必须清理半实体；没有确认同域并发分配或预留提交协议。
- 快照独立，首版整体捕获客户端本地游戏实体、相关游戏分配状态和按规则保存的资源；渲染实体、资源及表现绑定保留。先采用独立复制 SlotMap 和原型数据，不先全存储 COW；低成本与运行开销目标尚无性能实测。
- 支持恢复当前 ECS 游戏状态，也支持在新 ECS 实例恢复并独立推演。
- 首版不支持任意地图区或实体子集独立回退；客户端恢复本地游戏分配状态，不要求扩大网络关注范围以维持来源域闭合。服务端持续权威运行，客户端分歧交 pi_rewind 校正。

### pi_rewind 已读契约

依据 [host.rs:6-27](../../pi_rewind/src/host.rs#L6)、[host.rs:30-79](../../pi_rewind/src/host.rs#L30) 和 [需求:11-14](../../pi_rewind/design/partitioned-rollback-netcode/01-requirements/needs.md#L11)：

- Simulation::capture 接收 &self，返回不要求 Clone 或序列化的不可变快照；后续修改不得使历史漂移。restore 恢复状态与快照帧号。
- restore 与重演不回退真实渲染实体或覆盖当前绑定；游戏校验排除渲染句柄及绑定。
- reconcile_render 比较上次已协调快照和当前最终状态，通过稳定身份协调同 ECS 渲染实体及多对多绑定，不修改未来模拟依赖的数据。
- 一个预测域可包含多个地图分区，域内统一保存、恢复和重演。

### 设计推论与待确认边界

快照应包含影响未来模拟的 ID 分配状态，但不能回退仍在范围外继续使用的分配器。迁区保留来源身份，故当前成员集合不自动等同于分配来源集合；出生、删除、复用、分区加载和范围切换需要共同定义。恢复到新 World 时还需注册兼容性与实体引用规则。游戏重演不能借助统一查询意外读取渲染状态；这是模拟约束，尚未决定由系统权限还是模块约定实施。

## 完整模块源码地图（2026-10-28 补充）

此前身份／分区／快照六项仅为专题范围，不是完整 ECS 分层。下表将本轮阅读与前次报告对齐；文件集合不意味着逐行审计或每个功能都需原样继承。

| 职责 | pi_world 源码落点 | 已核实关系及迁移提醒 |
|---|---|---|
| 身份、类型和世界 | [world.rs](../../pi_world/src/world.rs)、[insert.rs](../../pi_world/src/insert.rs)、[archetype.rs](../../pi_world/src/archetype.rs) | Entity、地址、Component／Bundle 和类型元数据分布其中；类型登记与结构操作应区分职责 |
| 存储及结构修改 | [table.rs](../../pi_world/src/table.rs)、[column.rs](../../pi_world/src/column.rs)、[alter.rs](../../pi_world/src/alter.rs)、[editor.rs](../../pi_world/src/editor.rs) | 原型行列、增删组合、实体编辑和 settle 共同影响位置与旧访问有效性 |
| 查询 | [query.rs](../../pi_world/src/query.rs)、[fetch.rs](../../pi_world/src/fetch.rs)、[filter.rs](../../pi_world/src/filter.rs)、[entry_query.rs](../../pi_world/src/entry_query.rs) | 匹配、获取、过滤和缓存协作；逻辑模块不必一文件一 crate |
| 资源 | [single_res.rs](../../pi_world/src/single_res.rs)、[multi_res.rs](../../pi_world/src/multi_res.rs) | 单例资源与多例资源代码均存在，但 multi_res 的 prelude 重导出被注释，部分可变实现也是注释；不能表述成完整公开能力已验证 |
| 系统及参数 | [system.rs](../../pi_world/src/system.rs)、[system_params.rs](../../pi_world/src/system_params.rs)、[function_system.rs](../../pi_world/src/function_system.rs)、[param_set.rs](../../pi_world/src/param_set.rs)、[param_unready.rs](../../pi_world/src/param_unready.rs) | SystemMeta、SystemParam、Local、函数适配和参数状态组成系统接入层 |
| 调度与执行 | [schedule.rs](../../pi_world/src/schedule.rs)、[schedule_config.rs](../../pi_world/src/schedule_config.rs)、[exec_graph.rs](../../pi_world/src/exec_graph.rs) | 配置、系统集合、条件和执行图协作；图具有依赖与环检查相关逻辑，未验证所有并行执行路径 |
| 延迟命令 | [commands/command_queue.rs](../../pi_world/src/commands/command_queue.rs) | 异构命令被缓冲，apply 接收 &mut World；录制与应用是不同阶段 |
| 事件及监听 | [event.rs](../../pi_world/src/event.rs)、[listener.rs](../../pi_world/src/listener.rs) | EventVec 保存消息及独立读取长度；ListenerMgr 组织按类型登记的通知式监听，不能混成同一种机制 |
| 应用与插件 | [app.rs](../../pi_world/src/app.rs)、[plugin.rs](../../pi_world/src/plugin.rs)、[plugin_group.rs](../../pi_world/src/plugin_group.rs) | AppInner 持有 World、普通／启动调度及异步运行时，提供运行入口；不同于 Bevy 将应用层拆到独立 crate |
| 异步与支持 | [async_function_system.rs](../../pi_world/src/async_function_system.rs)、[dot.rs](../../pi_world/src/dot.rs)、[debug.rs](../../pi_world/src/debug.rs)、[macros](../../pi_world/macros/) | 存在异步适配、图输出、诊断及宏；不等于跨 await 借用安全、平台覆盖和性能已验证 |

入口证据：[lib.rs:19-84](../../pi_world/src/lib.rs#L19)。本轮关键片段包括 system.rs:20-80、330 起的系统元数据与 trait；event.rs:20-85；listener.rs:21-32、143-165；exec_graph.rs:130-187；commands/command_queue.rs:19-34、96 起；app.rs:11-16、59-104。

### 容易混淆的三处机制

1. **访问关系不是实体关系。**system.rs 的 Relation／Related 描述 With、Without、Read、Write 等访问与过滤关系，不能当作父子或实体关联模块。访问冲突要求不能同时运行，但不自动提供确定的先后；Bevy 对照见报告第 5 节。
2. **缓冲事件不是即时监听。**event.rs 的 EventVec 维护每个监听者的读取长度，get_iter 推进消费位置；listener.rs 的 notify 路径调用监听者。Bevy 0.20 则明确区分 Message 与 Event／Observer，术语不能按旧名字直接对应。
3. **命令录制不是实体创建已经执行。**已有“有序创建执行时立即获得真实 ID”的决定，不能推出录制延迟创建命令时就返回真实 ID；预留身份、命令应用顺序和失败处理仍需单独设计。

## 后续已确认需求补充与本地依赖事实

### 已确认设计约束摘要

- 使用非策略泛型 World；World 持有身份状态，插件安装实现和配置。EntityStore 基本 Interface 仅 allocate／locate／relocate／remove；const N 留在具体实现。
- 来源域与类别在迁区后不变，当前分区另存；布局定义可共享，实际原型存储按当前分区与类别隔离。纯迁移保持 Key、组件集合和值，不伪造 Added／Removed／Changed，并修复源行换位影响的各来源域映射。
- 自动判区由普通系统执行，显式阶段有序驱动迁移；compact 和 shrink_to_last_occupied 同步同序且可重演，shrink_to_fit 可本地执行。截尾丢失历史后的旧 Key 再现问题仍待约束。
- 快照复制规则由插件显式登记，使用 Clone 或自定义可失败的同类型独立复制；不要求所有组件 Clone，也不支持首版 T→S→T。快照不可变、可重复恢复，不要求 Clone 或序列化。
- 派生组件先有合法初始值，再由普通系统组重建；保存资源替换，派生资源清旧重建，非游戏资源保留。恢复先校验和准备，失败不替换当前状态；安装或重建失败停止模拟，不承诺撤销。
- 恢复安装不逐实体触发普通业务创建销毁／Added／Removed 通知，内部修复和正确析构仍要执行。历史期间保存／恢复注册规则固定，变更由宿主清不兼容历史重建基线。

这些摘要只说明研究问题已有何种答案；具体契约以活动设计及 src/world.rs、src/entity_store.rs、src/snapshot_rules.rs 等当前声明为准。

### 最新本地依赖核对

- [pi_key/src/lib.rs](../../pi_key/src/lib.rs)：当前本地版本 0.4.0，32 位 index 与 32 位 version；空值要求两者均为 u32::MAX，不是仅判断 index。
- [pi_slotmap/src/basic.rs](../../pi_slotmap/src/basic.rs)：当前本地版本 0.5.0，SlotMap<K,V,const N:u32=0> 使用 version 低 N 位作扩展位，N < 32；本项目已选择用于来源域／类别，而非切 index 位。普通 Clone 保留完整槽、代际和 free 链状态。compact 升序重建 free 链、不移动值；截尾会丢代际历史。serde 反序列化重建 free 链，不能代替相同后续分配序列所需的内存快照。
- [pi_slot/src/lib.rs](../../pi_slot/src/lib.rs)：没有现成完整 Clone；共享引用并发插入及失败认领可消耗代际，耗尽时退休槽，与 pi_slotmap 的回绕处理不同。本项目没有选择其竞争分配协议。

这些是邻接依赖当前源码事实，不反向描述成 pi_world 原有 SlotMap 的相同保证，也不提供可复现的远程修订锁定。

## 模块与 crate 讨论结论快照：2026-10-28 初始提案

后续进展：用户已要求保留 pi_ecs 入口并创建五个子 crate。身份改为 core 固定实现，取消插件身份 trait；分区数量构造时通过 const 传入，World 非泛型，默认两区、末区渲染。下面保留初始提案作为研究推导，最新职责与真实路径以活动 design.md 的 Workspace 一节为准；尤其 partition 不再提供身份实现。

本轮先讨论完整逻辑模块，再讨论 crate。先前四包方案把存储、查询、系统和调度都留在 pi_ecs；在用户追问继续细分后，最新候选调整为下表。用户尚未确认该方案，尚未迁移源码或建立多包 workspace。

| 候选 crate | 职责与所有权 | 不能隐含承担的职责 |
|---|---|---|
| pi_ecs_core | World 数据、身份契约、类型／Bundle、存储、资源、查询、访问描述、变化追踪、快照数据准备安装 | 不依赖 System、SystemParam、Schedule、App |
| pi_ecs_schedule | System／SystemParam、函数适配、Local 与参数状态、Commands、调度／执行器、消息系统适配、系统式 Observer、恢复与重建协调 | 不自行实现另一套实体存储和位置修复 |
| pi_ecs | 核心与系统运行时的统一使用入口及重导出 | 不额外持有世界状态；是否默认重导出应用／扩展尚未决定 |
| pi_ecs_macros | 生成派生与适配代码，遵循运行时声明 | 不定义第二套运行语义；宏 crate 自身不反向依赖运行时 crate |
| pi_ecs_app | App、Plugin、应用阶段和宿主驱动，组合 World 与调度 | 不在插件内隐藏身份分配状态 |
| pi_ecs_partition | SlotMap 身份实现、分区配置、自动判区和迁区阶段组装 | 纯搬移与位置修复仍归 core；具体实现归包方式待确认 |

候选依赖方向（A → B 表示 A 依赖 B）：

```text
pi_ecs_partition → pi_ecs_app → pi_ecs_schedule → pi_ecs_core
pi_ecs_partition → pi_ecs_schedule / pi_ecs_core / pi_slotmap
pi_ecs → pi_ecs_schedule / pi_ecs_core
pi_ecs_core → pi_key
需要派生的运行时 crate → pi_ecs_macros（按需）
宿主 → pi_rewind + ECS（具体对接待定）
```

### 拆分理由和必须闭合的接口

- 存储与查询留在 core，共同维护布局、引用有效期和缓存失效，避免过早公开大量底层指针协议。系统和调度同包，共同处理参数状态、延迟操作与执行约束。
- core 提供查询执行及访问描述，schedule 提供系统参数注入和系统观察窗口；参数包装类型归属、Rust trait 实现约束及跨包 unsafe 契约需要编译设计验证。
- 核心变化标记与系统 last_run 可以分属两层，但其捕获恢复方式没有确认。消息缓冲／游标的具体归属也未完成；不能把“系统适配归 schedule”当成所有消息状态归属已定。
- 核心生命周期一致性与上层观察者系统需要明确衔接，避免反向依赖或用回调掩盖循环。实体关系／层级、异步系统、多例资源、反射／序列化／编辑器的首版范围尚未确认。
- 数据恢复在 core，完整恢复协调在上层属于提案；现有 World::restore_game 契约包含重建，需要先确认入口归属，不能仅移动文件。Plugin 当前面向 World，迁至 app 后如何登记调度同样待设计。
- Bevy 实际将 core 与 schedule 相关能力放在同一个 bevy_ecs crate；本次拆分是 pi_ecs 的选择，不是照搬的官方包结构，也不是已验证的独立设计。

## 仍需继续研究／决定

- Added／Changed、删除记录、tick、last_run、Local、消息读游标、延迟命令的保存恢复，以及捕获静止点与并发访问协议。此前助手建议没有获得确认。
- 域生命周期、代际回绕／截尾后的旧引用、跨 World 类型和存储 ID 兼容、图外引用、运行时缓存修复。
- 共享资源和跨区关系下的访问证明、确定执行顺序与分区并行；不能仅凭分区值不同就放开写冲突。
- Send／Sync、线程平台、panic／析构失败、规模和快照频率、历史长度、峰值内存与性能预算。现有资料没有提供实测结论。
