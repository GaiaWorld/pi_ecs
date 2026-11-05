# hecs、Legion、pi_world 与 Bevy：pi_ecs 选型研究

调研日期：2026-10-28。目标是为 pi_ecs_core 的表示、结构操作与访问界面选型提供依据，不替用户确认方案。本次仅文档与源码静态研究，没有安装依赖、编译候选库或运行基准。

## 1. 版本与证据范围

| 对象 | 本次基线 | 证据与限制 |
|---|---|---|
| hecs | 0.11.2 | 本次 docs.rs latest 指向该版；以下引用固定版本 [H1] |
| Legion | 0.4.0 | 本次 docs.rs latest 指向该版；以下引用固定版本 [L1] |
| Bevy ECS | 0.20.0 | 沿用已核实报告，不以旧版或 main 代替 [B1] |
| pi_world | 本地包 0.2.17 | 本地源码静态阅读，路径内容可能变化 [P1] |

latest 解析是本次可访问已发布文档的版本事实，不是长期版本承诺。本次未通过 crates.io API、仓库提交和发布历史独立审计维护活跃度，不根据版本号或旧印象判断 Legion 已停止维护。docs.rs 页面展示的日期也不用于推断维护状态。hecs 与 Bevy 标示 MIT OR Apache-2.0，Legion 标示 MIT；直接复制代码时还需按实际来源履行许可证要求 [H1][L1][B1]。

标记：**事实**来自固定版文档或实际读取源码；**推论**是本项目适配判断；**待验证**不是已经做过的实验。Bevy 和 pi_world 专题细节复用两份原报告，不重复宣称完整源码审计：

- [Bevy 专题与模块报告](bevy-ecs.md)。
- [pi_world 源码、需求与模块对照](pi-world-and-requirements.md)。
- [当前活动设计](../design/unified-partitioned-ecs/design.md)。

## 2. 本项目不可混淆的约束

- 一个非泛型 World 统一游戏与渲染。构造时 const PARTITION_COUNT 默认 2，至少 2；0 为全局游戏区，末区本地渲染，其余地图游戏区。身份和分区机制为 core 固定实现。
- 各来源域分配独立，同域有序执行、删除后 LIFO 立即复用；执行创建即返回真实 ID。迁区保持身份，来源域与当前位置分离。
- 组件布局可共享，实际存储按当前分区和类别隔离；纯迁移不伪造 Added/Removed/Changed。
- 首版整体恢复本地游戏状态，保留渲染实体资源和表现绑定；显式登记同类型独立复制，可失败，不要求所有组件 Clone。
- 历史不可漂移、可以重复恢复；复制值与恢复未来分配序列、系统观察状态是不同问题。
- core 与 schedule 已拆包；查询安全、系统参数适配和恢复协调的跨包界面仍需设计。DOMAIN_BITS 与 PARTITION_COUNT 不等价。

以上是选型前提，不是要求候选库必须原生提供所有功能。需要分别评估直接采用依赖、修改其实现和借鉴其设计的成本。

## 3. hecs：轻量数据世界

### 3.1 范围与组件

**事实：**hecs 自我定位是库而非框架，不定义显式 System 抽象；业务可直接通过 World 查询。优先目标包括遍历、简单界面、较小依赖闭包，以及把外部可实现功能留给外部。Component 自动覆盖 Send + Sync + 'static 类型；Bundle、Query 等宏通过可选 hecs-macros 提供 [H1]。

这与“core 可独立使用、schedule 外置”方向接近，但不是已替我们定义好 SystemParam 安全协议。其公开模块主要是 serialize，其他类型多在包入口重导出；不应照其内部文件数量拆包。

### 3.2 身份与确定分配

**事实：**Entity 源码包含 u32 id 与 NonZeroU32 generation。普通 alloc 从 pending.pop 取空槽，free 增代际并 pending.push；这一直接有序路径是栈式复用。代际 wrapping_add，回绕至零时改为 1，并非永远杜绝旧身份重现 [H2]。

reserve_entity/reserve_entities 使用共享引用和原子游标预留，后续 flush 分配位置并使实体进入存储；预留 ID 与查询可见性不是同一时点 [H2][H3]。不能把这套预留流程自动引入本项目的立即创建契约。

**重要事实：**World 暴露 freelist 与 set_freelist。官方明确说明：相同存活实体和相同 freelist 可保持不同 World 后续 spawn 的确定分配；重建时先通过 spawn_at 等恢复存活实体，再安装保存的 freelist。set_freelist 要求存活 ID 集合与捕获时一致；期间 spawn、despawn 或 reserve 会使旧 freelist 不再适用 [H3]。

spawn_at 会销毁同 id 的现有实体；复活旧 handle 可能令旧引用重新有效，官方明确提醒碰撞风险 [H3]。因此它是保留身份导入能力，不是自动安全的历史恢复事务。

**推论：**hecs 直接证明了“复制分配状态”不是只能修改私有字段才能做到，值得参考其公开契约。但它仍是单套 World 分配器，没有本项目固定多域路由与版本扩展位协议。仅用游戏/渲染 marker 不会使分配状态独立。

### 3.3 存储与结构操作

**事实：**相同组件集合的实体存为连续组件序列。Archetype 源码维护各类型数据和借用状态；TypeInfo 包含 TypeId、Layout、析构函数，按对齐等规则组织。remove 使用末行填补，返回被搬到该行的实体 id，供调用者修正位置 [H3][H4]。

源码有析构展开保护注释，说明不能仅把删除视作普通字节复制。这里未完整审计 World 中每个 panic 路径，不承诺所有操作事务性或分配失败可恢复 [H4]。

insert/remove/exchange 提供组件集合修改；remove 文档明确：所要求的任一组件不存在时，不移除任何组件并返回错误 [H3]。这种已明确的局部保证不能推广为其他操作相同保证。

**推论：**适合作为“紧凑原型实例 + 末行换位 + 身份位置修复”的较小参考。跨分区同布局实例、批量迁区和恢复安装仍由本项目扩展；末行换位可能改变遍历顺序，确定性逻辑不能把无契约的行序当成稳定业务顺序。

### 3.4 查询与访问

**事实：**PreparedQuery 缓存查询准备信息；query(&World) 使用动态借用检查，违反唯一引用约束会 panic；query_mut(&mut World) 使用独占世界借用，避免该动态检查成本。还提供 View/PreparedView 随机访问与 With/Without 过滤 [H1][H5]。

hecs 暴露批次迭代，可作为外部工作划分基础；本次没有验证其本身提供 Legion/Bevy 那样的自动系统并行执行契约，不能把批次接口称为现成调度器。

**推论：**需要选择直接访问走动态借用、系统执行走预先证明的访问，还是统一采用另一套方式。两条路径可共存，但不能让外置 schedule 绕开安全契约后重复制造可变引用。

### 3.5 变化检测与快照

**事实：**ChangeTracker<T>::track 要求 T: Clone + PartialEq，通过保存前值并比较实现精确的值变化检测；需要访问可能变化的实体。官方要求每个 World 每种 T 使用一个 tracker；同型多个或跨 World 使用结果不可预测。Changes::removed 明确排除已 despawn 实体 [H6][H7]。

这与 Bevy“变化标记加观察窗口”不是同一种语义。普通可变借用但值未变化、销毁实体的删除通知、历史恢复后的前值如何处理，都必须分别讨论。

World 的公开 trait 列表没有通用 Clone；EntityBuilderClone/BuiltEntityClone 是可克隆组件组合，不等于完整 World 快照。serialize 支持按行/列能力，freelist 重建能帮助恢复分配序列，但不能自动覆盖资源、系统 Local、命令、业务通知及失败状态 [H1][H3]。

## 4. Legion：packed archetype 与执行权限

### 4.1 范围

**事实：**Legion 包含 world、storage、query、systems、serialize 模块。系统访问查询和独立 Resources；Schedule 在 World 与 Resources 上执行。parallel feature 使用 Rayon，另有 serialize、crossbeam-events、codegen 等可选能力，文档列出默认项 [L1]。

与 hecs 不同，它已覆盖系统、资源、调度和命令运行层；与当前 pi_ecs 的所有权选择也不同：独立 Resources 不应不经讨论搬入我们 World 持有资源的设计。

### 4.2 身份：不是代际 SlotMap

**事实：**Entity 包装 NonZeroU64。Allocate 从进程静态 NEXT_ENTITY: AtomicU64 以 16 个一块领取 ID，再从块内递增；LocationMap 是独立的位置表。删除位置不意味着把该身份放回 LIFO free 链 [L2]。

Entity 的 Clone 实现还读取线程局部 ID_CLONE_MAPPINGS，命中时返回重写身份，未命中保留原值 [L2]。这是复制上下文的一部分，不是“任意组件字节中 Entity 都会被自动重写”的保证。

**推论：**默认身份算法与本项目独立域、LIFO、回滚后重现分配序列差异很大。不能通过复制某个 World 回退进程共享分配器，因为其他 World 可能继续分配。直接采用需要另设身份层或修改实现，不宜把现成 ID 分配算法作为主参考。

### 4.3 存储：packed 不等于固定页

**事实：**Legion 将每种组件的各原型切片组织在组件存储中。PackedStorage<T> 可把部分切片打包到同一分配；稳定一段时间且预计有收益的切片更适合打包。GroupDef 为常见查询提供组件组合提示，影响打包排列 [L3]。

源码中的身份块大小 16、查询 ChunkView 和组件 packed allocation 是三种不同概念；没有证据支持“Legion 是固定 16 实体组件页”或“原生分区 COW”这样的说法。

**推论：**这比每原型各自独立分配多了一层打包/解包与稳定性判断。可能缓解多原型碎片造成的缓存损失，但本项目频繁迁区、结构变化和快照是否受益，需要负载验证。首版无需预设引入 pack。

### 4.4 查询、权限和调度

**事实：**Query 支持布局过滤、单实体 get、iter_chunks 和 Rayon 并行版本。安全可变迭代要求可变 EntityStore；unchecked 版本允许共享引用下取可变访问，调用者必须证明不产生可变别名 [L4]。

World::split / split_for_query 返回 SubWorld，对数据访问权限进行划分。文档例子按 Position 的读写权限划分，不是自动按地图分区值建立不重叠实体子集 [L5]。

Legion 的 EntityStore trait 是可查询存储访问界面，与我们已取消的身份分配四方法 trait 含义不同，不应因同名混用。

**事实：**官方概述描述 Schedule 是按可观察副作用顺序组织的系统线性序列，能够在保持系统视角表观顺序的前提下自动并行。它与 Bevy 默认未显式排序系统不保证业务先后的使用模型不同 [L1]。具体条件、命令 flush 和资源冲突仍应在后续调度专题逐项核实。

**推论：**SubWorld 的权限载体值得研究 core→schedule 的跨包访问界面；不能直接将任意分区编号包装成 SubWorld 就声称完成安全证明。跨区引用、资源和结构修改必须纳入权限。

### 4.5 变化、事件与复制

**事实：**maybe_changed 是保守粗粒度过滤，通过的实体不保证每个值实际修改；storage 文档描述组件切片 Version 在可变访问时增加 [L3][L6]。这与 hecs 前值比较、Bevy 逐组件实例标记不能按一个 Changed 名字等同。

World::subscribe 提供原型创建、实体进入/离开原型等事件 [L5][L7]；这种原型成员事件不能直接映射成业务创建销毁或组件 Added/Removed，尤其在保留身份的迁区中。

**事实：**World::clone_from 接收 source、布局过滤器和 Merger，返回被重映射的 ID；Duplicate 显式注册 copy、clone 或 convert。未登记的组件在示例契约中会被忽略。register_convert 返回目标值而不是 Result；World::clone_from 也不提供我们的可失败准备/安装事务契约 [L5][L8]。

World::move_from 允许世界间搬移；serialize 通过注册信息映射稳定类型标识，未知组件可被省略。这些能力都不是完整运行态快照，无法据此保证分配器、资源、查询过滤状态和系统进度一并恢复 [L5]。

**推论：**值得借鉴“显式登记复制能力”，但我们的规则缺失必须报错/派生重建，不能继承静默忽略；首版同类型可失败复制也不能直接等同于 Legion 的转换注册。

## 5. 四者机制对照

下表为已读范围摘要；pi_world/Bevy 的逐项证据见原报告 [P1][B1]。

| 维度 | hecs 0.11.2 | Legion 0.4.0 | pi_world 本地 | Bevy 0.20.0 |
|---|---|---|---|---|
| 定位 | 精简数据世界，外部组织逻辑 | 数据与系统运行层齐备 | 数据、调度、App 在同包 | ECS 与 App 分包，ECS 内含调度 |
| 默认身份 | index + generation；直接路径栈式复用 | 进程级分块递增 u64 | SlotMap→原型/行 | World 局部 index + generation |
| 分配状态重建 | 明确公开 freelist/set_freelist 契约 | 默认进程分配器不随单 World 回退 | 本次旧实现尚未证实完整协议 | 未提供本项目多域整体快照契约 |
| 存储 | 连续原型组件列，末行填补 | 每类型原型切片，可按组打包 | 每组件跨原型 Blob；阶段整理 | Archetype + Table/SparseSet |
| 结构可见性 | 直接可变 World 操作；另有预留/命令 | Entry 修改；系统命令另有应用点 | 新旧行与 settle 共同约束 | 独占结构操作与 ApplyDeferred |
| 访问组织 | 动态借用 / 独占快路径 | View/权限/SubWorld | Relation/Related 与执行图 | FilteredAccess 与系统访问检查 |
| 查询缓存 | PreparedQuery | Query/搜索索引；缓存实现未逐字段审计 | QueryState 含 World/System 指针 | QueryState 存储匹配与 fetch/filter |
| 变化语义 | Clone + PartialEq 前值比较 | 切片级保守版本过滤 | tick/组件事件等机制 | 组件/资源变化标记与观察窗口 |
| 复制 | 克隆组合、序列化、保留 ID 导入 | Merger/Duplicate 跨 World 复制 | 元数据无通用值复制契约 | EntityCloner 与场景构造 |
| 现成完整回滚 | 未提供本项目完整契约 | 未提供本项目完整契约 | 未证实 | 未提供本项目完整契约 |

## 6. 按 pi_ecs 需求评估适配成本

| 本项目要求 | 研究结论 |
|---|---|
| 独立域有序 LIFO 身份 | hecs 直接分配路径最接近，且分配重建契约可借鉴；仍须自建多域与扩展位。Legion 默认算法明显不同 |
| 同布局多分区物理隔离 | 四者所查默认模型均未直接提供我们的完整维度；需将布局定义与实际存储实例分开 |
| 纯迁区不伪造变化 | 必须有专用搬移路径并保留变化元数据；不能简单销毁重建，或直接翻译原型出入事件 |
| 游戏恢复、渲染保留 | 仅按当前区筛选实体不够，必须同时处理来源域状态和跨区位置；现成 clone/import 不闭合此问题 |
| 独立且可失败的同类型复制 | Legion 的登记设计、Bevy clone behavior 可借鉴；都需按本项目错误和准备安装阶段重做契约 |
| 非 Clone 组件与派生重建 | 规则缺失必须拒绝或具备合法初始值和重建，不继承静默省略 |
| core/schedule 分离 | hecs 提供最小世界参考；Legion SubWorld 提供权限视图参考；Bevy 的访问描述/参数体系提供完整参照，但其内部界面不等于可直接拆包的稳定协议 |
| 生命周期与失败 | 四者都需逐入口核实；没有一份资料足以替我们承诺任意 panic、析构失败和部分搬移的事务性 |

**没有性能排名。**以上是机制匹配与工程复杂度判断，不是实测证明 hecs 最快、Legion 最慢或 Bevy 过重。

## 7. 待选方向：选择机制，不必整库照搬

### A. 紧凑原型数据核（主参考 hecs）

- 每个存储实例保存连续列；删除/搬移采用明确的末行换位及位置修复。
- 身份仍用本地 pi_key/pi_slotmap，独立域状态显式保存；参考 hecs 的 freelist 一致性要求。
- 从一开始区分布局与实例；以 Bevy 的访问元数据和变化窗口补齐上层需要。
- 代价：大列扩容与频繁结构变化的搬移，按分区导致的小实例碎片；仍需自建注册、可失败复制和系统接入。

### B. 切片集中管理与可选打包（主参考 Legion/pi_world）

- 按组件类型管理多个实例切片；参考 Legion 稳定切片打包以及 pi_world 的列组织。
- 保留本项目固定身份，不继承 Legion 进程分配器。
- 代价：打包/解包、跨实例借用、迁移和快照表示复杂，收益依赖多原型局部性；阶段整理协议必须重新明确。

### C. 混合存储（主参考 Bevy）

- 主体列式，频繁增删的特定组件可选稀疏存储，完整组件集合与表布局分别建模。
- 代价：位置层级、结构迁移、查询路径、变化追踪和快照都须覆盖两种后端；分区维度让组合更多。

**研究建议而非定案：**先比较 A 与 B，C 作为确有组件高频切换负载后的候选。A 的核心表示较少，适合作为小实验基线；是否采用要看业务负载，不等于现在批准重写为 hecs 或新增该依赖。

直接依赖任一现成 World，再外加身份映射/多 World 分区，会引入额外 ID 间接层或跨 World 查询、资源、调度问题。应把“直接采用成品库”与“自有 core 借鉴算法”分别估算，不能称简单封装即可满足全部约束。

## 8. 选型前的最小验证建议（尚未执行）

1. **正确性切片**：多域同序创建删除，穿插渲染分配，跨区迁移并触发末行换位；验证键不变、映射完整和后续 ID 序列。包含组件构造及复制失败清理。
2. **恢复切片**：存活值相同但 free 链不同的两份状态，检查未来分配是否不同；恢复同历史两次、恢复至新实例；渲染数据及绑定不被覆盖。
3. **访问切片**：不同区写同类型组件、跨区关系访问、共享资源、迁移同步点；证明哪些能安全并行，哪些必须冲突。检查 core→schedule 实际 Rust 借用/trait 约束。
4. **表示对比**：A 与 B 在全局遍历、单区遍历、小区多实例、增删组件、集中迁区、快照复制与恢复峰值内存上的差别。先由用户给代表规模/频率，避免拍脑袋评分。
5. **变化语义**：只取得可变访问但不修改、改后还原、销毁实体、同布局纯迁区、历史恢复后的首次 Changed 读取；分别验证想要的语义。

## 9. 未核实与限制

- 没有候选库实测、编译、Miri 或 panic 注入实验；最小 Rust 版本、wasm/无标准库目标及线程 feature 组合未逐平台验证，不仅凭依赖表声称支持。
- hecs 查询缓存具体失效字段、Legion 搜索缓存与调度冲突图没有完成逐字段源码审计。
- Legion 增删组件的内部搬移和全部析构异常路径未完整审计；hecs 仅实际读取关键身份、原型源码和公开契约，不代表整库安全审查。
- 实体重映射是否覆盖用户自定义 clone、容器、图外引用需要具体实验。没有把普通 Clone、serde、跨 World 合并当作本项目可重复完整快照。
- 不默认继承 hecs 的组件 Send+Sync 限制或 Legion 的独立 Resources；本项目线程平台与非 Send 资源政策仍待定。

## 10. 一手来源

- [H1] [hecs 0.11.2 索引](https://docs.rs/hecs/0.11.2/hecs/)；本次 [latest](https://docs.rs/hecs/latest/hecs/) 指向该版。
- [H2] [hecs entities.rs](https://docs.rs/hecs/0.11.2/src/hecs/entities.rs.html)：Entity、alloc/free、reserve/flush、freelist。
- [H3] [hecs World](https://docs.rs/hecs/0.11.2/hecs/struct.World.html)：spawn_at、remove、reserve、freelist/set_freelist、公开 trait 列表。
- [H4] [hecs archetype.rs](https://docs.rs/hecs/0.11.2/src/hecs/archetype.rs.html)：TypeInfo、列借用、remove 末行填补与析构保护。
- [H5] [hecs PreparedQuery](https://docs.rs/hecs/0.11.2/hecs/struct.PreparedQuery.html)。
- [H6] [hecs ChangeTracker](https://docs.rs/hecs/0.11.2/hecs/struct.ChangeTracker.html)。
- [H7] [hecs Changes](https://docs.rs/hecs/0.11.2/hecs/struct.Changes.html)。
- [L1] [Legion 0.4.0 索引](https://docs.rs/legion/0.4.0/legion/)；本次 [latest](https://docs.rs/legion/latest/legion/) 指向该版。
- [L2] [Legion internals/entity.rs](https://docs.rs/legion/0.4.0/src/legion/internals/entity.rs.html)：Entity、分块分配、线程局部重映射与 LocationMap。
- [L3] [Legion storage](https://docs.rs/legion/0.4.0/legion/storage/index.html)：packed archetype、GroupDef、PackedStorage、Version。
- [L4] [Legion Query](https://docs.rs/legion/0.4.0/legion/query/struct.Query.html)：安全/unchecked 查询及并行迭代。
- [L5] [Legion World](https://docs.rs/legion/0.4.0/legion/world/struct.World.html)：split、subscribe、pack、move_from、clone_from、序列化。
- [L6] [Legion maybe_changed](https://docs.rs/legion/0.4.0/legion/query/fn.maybe_changed.html)。
- [L7] [Legion Event](https://docs.rs/legion/0.4.0/legion/world/enum.Event.html)。
- [L8] [Legion Duplicate](https://docs.rs/legion/0.4.0/legion/world/struct.Duplicate.html)、[Merger](https://docs.rs/legion/0.4.0/legion/world/trait.Merger.html)。
- [B1] [Bevy 0.20.0 官方模块索引](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/)，详细固定源码链接见本目录 Bevy 报告。
- [P1] [pi_world 本地入口](../../pi_world/src/lib.rs)、[存储表](../../pi_world/src/table.rs)、[列](../../pi_world/src/column.rs)、[查询](../../pi_world/src/query.rs)、[系统访问](../../pi_world/src/system.rs)，细节见本目录 pi_world 报告。
