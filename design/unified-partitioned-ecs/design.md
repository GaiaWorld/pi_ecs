# 统一分区 ECS：实现设计

本文描述仓库当前的 RC 实现及跨模块约束。[概念](concepts.md)是使用入口和术语索引。精确的类型签名与安全前提以链接的源码为准。

## 支持范围与包边界

启用根 crate 的 `rc` feature 运行单线程 ECS。RC 的 `Executor::new(ExecutionMode::Parallel)` 返回 `Unsupported`，不自动降级。默认配置可用于编译检查，不代表非 RC 后端已具备运行支持。快照捕获与恢复、自动判区插件、并行执行、运行中动态系统请求和外层执行图不属于当前运行契约。宿主显式迁区可用。

| crate | 职责 |
| --- | --- |
| `pi_ecs_types` | 实体键、位置、组件与资源编号、变化时钟及错误报告 |
| `pi_ecs_storage` | Table、列、物理成员、稀疏值及擦除指针与缓冲 |
| `pi_ecs_command` | 命令 Hub、就绪队列、定时与取消 |
| `pi_ecs_core` | World、注册表、逻辑原型、组件策略、运行授权、查询、通知及销毁 |
| `pi_ecs_schedule` | 系统、参数状态、阶段排序、串行 Executor 和 Schedule |
| `pi_ecs_app` | World 所有权、插件、命名 Schedule 与有序关闭 |
| `pi_ecs_macros` | 对外注册 `Bundle` 与 `QueryData` 派生 |
| `pi_ecs_partition` | 自动判区的独立接口边界；不承担宿主显式迁区 |
| `pi_ecs` | facade、模块重导出和 prelude |

成员和 feature 传播见 [Cargo.toml](../../Cargo.toml)，公开导出见 [src/lib.rs](../../src/lib.rs) 与 [宏入口](../../crates/pi_ecs_macros/src/lib.rs)。Core 依据注册表决定逻辑结构和释放策略，Storage 管理物理内存；命令服务不持有 World，通知目录和订阅游标由 Core 管理。

## 实体身份、原型与存储

- `Entity` 是 World 本地完整代际键。索引高 12 位为出生域，低 20 位为域内槽；不会因迁区改变。World 默认单分区，构造时可固定配置，最多 4096 个。出生域不是当前分区；显式迁区只修改位置中的当前分区，不搬值、不产生组件变化。
- `EntityLocation` 包含当前分区与原型成员行、Table 物理行、Table/variant 编号三个字。只有三个字全为最大值才是 `NULL`。原型成员与 Table 物理行是不同索引；整理各自更新相应索引和实体位置。裸实体键不证明所属 World，跨 World 同号键不能靠定位识别。
- 基础 Table 和组件类型先登记，再冻结。`ArchetypeId { table, variant }` 以 variant 0 表示基础原型。运行中的创建与结构变更从完整组件集合选表：表组件是该集合的子集，优先覆盖最多的表，同覆盖数按登记顺序；显式原型登记可指定基础表。相同的组合复用，不隐式登记类型。
- `SparseOnly` 总走稀疏；`TableEligible` 仅在选中的基础 Table 覆盖它时进入列，否则走稀疏。值是 World 全局存储中的唯一实例，不按当前分区复制。
- RC 稀疏以出生域和本地槽定位，使用完整 owner 键验证代际。稀疏预留不等于发布；发布前槽不能作为活值读取。删除修复紧凑 owner/索引，不移动其他活组件槽。表删除和成员移出保留洞；`settle` 是宿主显式的物理整理，不是每阶段固定动作，不析构已移出的值，不制造业务变化。
- 取值计划可缓存稳定容器根，不缓存跨结构修改的值和 tick 缓冲。RC 增长可搬移缓冲；维护须等冲突借用结束。实际引用只在取值时对单个槽构造，不能用整列或整 World 的可变引用覆盖已交出的结果。

源码：[实体编码](../../crates/pi_ecs_types/src/entity.rs)、[注册表](../../crates/pi_ecs_core/src/registry.rs)、[物理存储](../../crates/pi_ecs_storage/src/storage.rs)、[列](../../crates/pi_ecs_storage/src/column.rs)、[稀疏映射](../../crates/pi_ecs_storage/src/sparse_set.rs)。

## 门禁、拥有责任与组件策略

- RC 门禁记录宿主读授权、写授权、维护、运行和不可恢复停止等状态。`WorldPin` 与 `WorldWrite` 从宿主独占借用取得受限访问；冲突拒绝，不等待。Drop 释放授权，不刷新通知或隐式整理；泄漏授权不会提前解锁。生命周期保护和组件访问声明各有职责，复制受限描述不会获得新权限。
- `EntityScope` 要求已冻结且可维护的 World，在会话内提供 `try_acquire/get/get_mut/spawn/change_archetype`。操作只投影所需字段。组件结果短借当前操作，`OwnedEntity` 绑定整个作用域，可在组件短借结束后用于下一次操作，不得逃逸。组件缺失为 `Ok(None)`；停止或会话失效返回错误。空作用域不额外取 tick。普通错误保留先前成功的操作；局部不可恢复失败先标停，外层即使捕获 panic 也不能恢复为 Idle。
- `WorldWrite` 成功取得授权时分配一次写 tick。单组件点查、组合点查和资源写复用该 tick。`query_one_mut` 先校验访问组合，只准备当前目标原型；令牌持有验证后的声明，返回值短借令牌，允许开放 fetch 保留原授权。该入口无变化观察窗口，拒绝实际活 tick 观察。
- `OwnedEntity` 表示一份 strong 责任，`try_clone` 新增一份；`forget` 移交释放责任给调用方。Drop 归还已有责任，strong 归零只登记销毁候选，不递归销毁。Pending 实体在位置隐藏前仍可被正常查询，但不能重新增持。裸键、临时借用与拥有责任不互相替代。
- `Component::Access` 指定 `ComponentPolicy`。`PlainAccess` 给出普通读引用和 `Mut`；关系容器及强弱数组给出各自的访问对象。两种结果寿命区分组件短借与拥有结果有效期。容器头留在组件槽内、节点由 World 池管理；不同头通过节点池短借操作，不构造整个池的独占借用。裸读取不增持，owned 读取增持，pop 移交原责任。
- `OwningPtr` 只携带活值的唯一转移/释放责任，不拥有分配，也不自动析构擦除值。移动使用已登记布局；回调前先移交责任并结束冲突借用，来源分配保活至清偿结束。普通释放与整体清偿均经按实际组件类型登记的唯一策略；没有第二套底层析构路径。

源码：[World](../../crates/pi_ecs_core/src/world.rs)、[作用域](../../crates/pi_ecs_core/src/entity_scope.rs)、[拥有引用](../../crates/pi_ecs_core/src/owned_entity.rs)、[策略](../../crates/pi_ecs_core/src/component.rs)、[关系容器](../../crates/pi_ecs_core/src/entity_container.rs)、[擦除指针](../../crates/pi_ecs_storage/src/ptr.rs)。

## 查询、匹配边界与错误

`QueryState` 保存长期声明、匹配计划及稳定容器定位，实际取值由本次授权派生的 `FetchAccess` 承担。普通 `Query` / `QueryMut` 支持遍历及点查；`Lookup` 只定位，不提供成员遍历。写批量入口在构造结果前拒绝重复实体；流式可变批量结果短借游标。必要组件缺失不匹配；可选组件缺席返回 `None`；有实际槽时仍按真实访问判断冲突。开放 `QueryData` 和 `QueryFilter` 必须如实报告组件、跟踪和活 tick 读取，不能以结构表达式代替用户的动态过滤。

`init_fetch_state` 同时完成范围匹配和稳定来源绑定。Table 范围可返回 `Ready`、`NotMatched` 或 `NeedsVariants`；前两项要求对全表成立，不能拿当前某个变体当证明。原型范围只允许前两项。字段组合保留真实错误；无错误时，不匹配优先于需要逐变体。增量查询按布局版本和已发布原型后缀同步，不能把潜在排序目标当实际缓存项。准备本次访问器时转发原授权和范围，取值时核对目标，不从临时 Fetch 借用扩大结果寿命。

`#[derive(QueryData)]` 接受具名字段结构体，生成同可见性的 `<Name>Item`。字段名和可见性保留，结果类型来自字段真实的 `QueryData::Item<'a, 'w>`。支持生命周期、类型与 const 泛型，`#[query_data(crate = path)]` 可指定 Core 路径。宏将声明、准备和取值委托给同序字段元组，仅重组结果；不语法猜测只读性，不遗漏重复访问。只有全部字段实现 `ReadOnlyQueryData` 时组合才实现它。

可恢复错误用带分类的 `EcsError` 报告。组合点查的非法别名为 `InvalidInput`；实体不存在为 `NotFound / NoSuchEntity`，存在但不匹配为 `InvalidInput / NotMatched`。后两者以 `QueryEntityContext` 保存完整实体键。组件引用、位置或授权上的内部不变量失效不伪装成普通缺值。

源码：[查询与声明](../../crates/pi_ecs_core/src/query.rs)、[取值协议](../../crates/pi_ecs_core/src/query_fetch.rs)、[批量](../../crates/pi_ecs_core/src/query_batch.rs)、[派生](../../crates/pi_ecs_macros/src/query_data.rs)、[错误类型](../../crates/pi_ecs_types/src/error.rs)。

## 结构写与值安装

| 入口 | 目标与输入 | 时钟 |
| --- | --- | --- |
| `World::spawn` / `EntityScope::spawn` | 从 Bundle 完整集合推原型；显式出生域也是初始分区 | 成功安装使用宿主操作 tick |
| `World::change_archetype` / `EntityScope::change_archetype` | 严格增删；新增缺席，移除存在；拒绝重复或交集 | 非空成功修改使用宿主操作 tick |
| `AlterState::scope` | 宿主预定编号增删集，在会话内执行 | 复用写作用域 tick |
| `Spawn<B>` | 独立系统参数；完整目标由 B 声明，固定配置分区默认 0；越界在登记时拒绝 | 系统 `this_run` |
| `QueryAlter<Q, F, Add, Remove>` | Q/F 选候选，增删类型确定固定行为；Remove 只描述类型，`()` 表示空集合 | 系统 `this_run` |

系统入口依据实际 RunAccess 叶验证 World、布局与访问范围。`SystemBundle::Shape` 只约束调用签名，允许同形短借输入；运行还需比对实际有序类型清单和输入来源。Bundle 直写已准备的目标槽，writer 独立检查实际写入类型、布局、顺序和完成度。可信 `ComponentInput::install` 不能展开；无法确定半槽责任时终止进程。一般的前置错误不修改目标，已安装前缀与未写值各有唯一清偿责任，不为 panic 恢复运行。

成功创建和转换在调用返回前发布位置及成员。首次需要的变体在结构操作中发行；保留值和 tick 随来源迁移，同一物理列或同一稀疏槽不用重写。系统空 Bundle 仍可创建成员；空增删的固定 Alter 仍运行零槽 writer，不搬行、不发通知、不另取 tick。查询式转换游标借用本次匹配配置，目标优先遍历，源留洞、目标尾部追加；不积攒批处理，不在 Drop 提交。ParamSet 再取成员时可重绑已发布的新变体。

Spawn 在发布实体后追加通知时仍有独立停止保护：失败展开先标停，再由局部拥有引用归还 strong；已发布存储由关闭流程唯一清理。

源码：[Bundle](../../crates/pi_ecs_core/src/bundle.rs)、[类型化输入](../../crates/pi_ecs_core/src/bundle_input.rs)、[Spawn](../../crates/pi_ecs_core/src/spawn.rs)、[Alter](../../crates/pi_ecs_core/src/alter.rs)、[宿主结构入口](../../crates/pi_ecs_core/src/world_inner.rs)。

## 系统、阶段与 App

1. App 唯一拥有 RC World；插件按具体类型安装，`build` 即时执行，可嵌套安装；依赖显式检查。`finish` 完成插件回调、组合验证和注册冻结，不提前初始化系统参数，不消费 Local/多例初值，不分配系统 tick。Ready 的 App 按标签显式运行 Schedule，可在两次运行之间管理系统。Stage 拥有系统实例；Schedule 拥有 Stage 和 Executor。
2. Schedule 建立 World 根运行会话，顺序运行 Stage。Stage 先初始化活动系统、登记参数责任，再应用追踪需求和准备系统。初次初始化失败的实例留在失败状态，需注销替换；不会反复消耗配置。系统绑定同一 World，已成功初始化不重复登记。
3. 阶段入口根据访问声明建图。潜在原型闭包以已发布原型和 Spawn 目标为种子，再把 Alter 目标作为潜在源反复推导；逻辑序号不是 `ArchetypeId`，也不提前注册实体或目标。排序缓存按 World 身份、布局代次和原型总量复用；系统增删会撤销缓存。同一 Stage 内刷新后续系统的真实匹配，不重排已经选定的顺序。
4. 读写实际重叠的值、活 tick 和资源按写先读推导；对组件来说，双方共享的逻辑原型还必须实际含有该组件。结构成员变化与通知发布分别按成员范围和流判断；纯实体查询也有成员访问。两个结构写系统按登记顺序，双向要求按登记顺序，多系统成环时整个 Stage 按登记顺序执行，不重跑系统。冲突图及拓扑处理复用 `pi_graph`。
5. `AccessTree` 区分叶、同时组合与独占分时组合。同时组合拒绝结构写与独立成员/定位、值、活 tick、相关通知读取或另一结构写的冲突；`ParamSet` 用独占短借依次取成员，逐次重绑，对外声明全部潜在访问。系统在真实绑定上使用 `last_run/this_run`，成功才推进运行观察点；跳过或准备失败不推进。
6. Stage 成功退出后统一回收通知和销毁候选；失败不补正常维护，不继续后续 Stage。普通错误结束健康的根会话，原先完成的修改保留；不可恢复失败保持 Stopped，App 进入失败态。宿主重试是新一轮调用，不从失败处续跑。

唯一资源的插入、替换、移出是 RC 宿主维护操作，读取和修改经过 `WorldPin/WorldWrite` 或系统授权。Local 由系统参数拥有，不记录变化 tick。多例生产槽按系统身份和参数叶序登记，由稳定槽身份重绑；物理偏移随结构版本验证，不能靠当前数量判断。系统注销清偿生产者、Local、跟踪需求和订阅。

源码：[App](../../crates/pi_ecs_app/src/app.rs)、[Schedule](../../crates/pi_ecs_schedule/src/schedule.rs)、[Stage](../../crates/pi_ecs_schedule/src/stage.rs)、[排序](../../crates/pi_ecs_schedule/src/stage_plan_inner.rs)、[系统参数](../../crates/pi_ecs_schedule/src/system_param.rs)、[访问声明](../../crates/pi_ecs_core/src/access.rs)、[资源](../../crates/pi_ecs_core/src/resource.rs)。

## 变化、通知、销毁与关闭

- `ChangeTick` 是不回绕的非零时钟。`ChangeWindow` 为左开右闭，首次观察无下界；宿主变化查询显式传窗口或传 `last_run`。`World::read_change_tick` 读取时钟，不是写入屏障。追踪按登记需求应用，启用时为现存值初始化 tick，不补历史通知。取得写对象不等于修改；可写解引用、显式标记或成功的容器变更按协议更新 tick 和通知。
- Added、Changed、Removed 是按组件和种类独立的流。每流序号单调、不回绕，前缀回收不重编号。Reader 持自己的订阅游标；Added/Changed 返回通知头，不保证消费时实体还存在。RemovedBatch 的旧值短借批次，旧值不从当前实体槽重取。
- Removed 发布先准备通知头容量，再转移旧值；取责成功后完整发布头和尾界。无订阅时把原 `OwningPtr` 交回调用方释放，不复制析构责任。同流动态借用冲突、容量或发布不变量失败走停止策略；组件 Changed 的通知通道携带同源门禁，即使作用域内部捕获失败也不恢复。旧值回收只释放所有读者游标之前的前缀，慢读者保留历史。
- 成功的销毁收束先处理通知前缀，再取待销毁批次。每批按出生域和完整键排序；销毁先隐藏位置，按登记顺序移出组件，转交 Removed 或释放，没有回调递归销毁。新归零进入后续批次；回收身份候选按首次登记顺序去重，等整轮清理完成后回收。同一销毁轮共享写 tick；只有通知/身份回收不另取销毁 tick。单独 `reclaim_notifications` 不销毁实体。
- App 关闭先注销调度系统与参数，释放调度和插件，再关闭 World。World 先关闭命令服务，再用 `WholeState` 清偿 Table、稀疏、Removed、唯一资源、多例及默认值；强环不必先归零。整体清偿逐份交还责任，不新增待销毁/回收登记，不产生业务通知。已移出的值不会由底层补析构。重复关闭幂等，Drop 复用关闭；不能安全完成的关闭故障终止进程。

源码：[变化检测](../../crates/pi_ecs_core/src/change_detection.rs)、[通知流](../../crates/pi_ecs_core/src/notification.rs)、[Reader](../../crates/pi_ecs_schedule/src/notification_reader.rs)、[销毁](../../crates/pi_ecs_core/src/destruction.rs)、[关闭](../../crates/pi_ecs_core/src/shutdown.rs)。

## 类型化命令

`CommandHub` 管理类型队列、容量、服务代次和模拟时间。`send` 接受后进入就绪队列；`schedule_at` 的期限已到则直接就绪，否则由单调递增的 `advance` 推进可见性。`cancel` 只取消仍待定时交付且与队列匹配的项，不能取消已就绪项；失败发送把原 payload 交回。接收资格唯一；`read` 固定当前已就绪尾界，批次只消费该尾界前的项，取出的 payload 归调用方，未取出的项留待下次读取。Sender/Receiver 持独立队列和控制状态，不保活 World；Hub 关闭撤销旧句柄并清偿剩余已接收 payload。RC 句柄不跨线程；模拟时间跟组件变化 tick 是不同的时钟。

源码：[命令队列](../../crates/pi_ecs_command/src/command.rs)、[World 命令入口](../../crates/pi_ecs_core/src/command.rs)。
