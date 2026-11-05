# 查询与已有 Entity 的点访问：Bevy、hecs、Legion、pi_world

研究日期：2026-10-30。基线：**bevy_ecs 0.20.0、hecs 0.11.2、Legion 0.4.0、本地 D:/dev/pi_world 0.2.17**（Cargo.toml:1–5）。

已阅读 [Bevy 报告](bevy-ecs.md)、[hecs/Legion 报告](hecs-legion-comparison.md)、[pi_world 报告](pi-world-and-requirements.md)。它们仅提供背景；下文重新依据固定版 docs.rs 发布源码、官方 rustdoc、本地源码，不以其他代理总结或旧报告代替证据。**事实**是已读实现/契约；**推论**是机制成本与便利性判断。未执行或修改生产代码，未编译、运行基准，仅新增本文。本地行号对应研究时文件，未锁定提交。

## 1. 总览

**已有 Entity 不要求先遍历整个 World。四者都有身份→位置→组件的入口，但准备成本、过滤与借用语义不同。**

| 关注点 | Bevy 0.20.0 | hecs 0.11.2 | Legion 0.4.0 | pi_world 本地 |
|---|---|---|---|---|
| 一个组件 | world.get::<T>(e)，Option<&T> | world.get::<&T>(e)，Result<Ref<T>, _> | world.entry_ref(e)?.into_component::<T>() | world.get_component::<T>(e) |
| 一个实体、多组件 | QueryState::get/get_mut；系统 Query 同名入口 | query_one::<Q>(e) 得 guard 再 get；query_one_mut 直接返回结果 | <Q>::query() 后 get/get_mut | World::query/make_query → QueryState；系统 Query |
| 点取值是否扫描所有实体 | 否；缓存原型 bitset + 本实体过滤 | 否；query_one 只 prepare 目标原型；View 建立可能处理多个原型 | 否；加过滤后可能求值原型缓存并线性检查原型列表 | 否；实体地址 + 原型 bitset + 行访问 |
| 点查询过滤 | 缓存结构条件，仍调用本实体 filter_fetch | query_one 每次检查目标原型；View 用已准备 fetch | **get 不执行动态过滤**，附加布局过滤仍校验 | 普通 get 每次 init_filter + F::filter |
| 共享世界写访问 | 普通 World 写要求 &mut；系统声明权限/调度 | &World 动态 borrow guard；&mut World 免动态检查 | 安全写用 &mut EntityStore；SubWorld 加权限检查 | Relation 元数据 + Ptr/unsafe，点访问未见 hecs 式 guard |
| 指定多实体可变结果 | get_many_mut 查重；unique 变体；iter_many_mut 借出式遍历 | query_disjoint_mut / View::get_disjoint_mut，重复 panic | 所查 Query 无同名 get_many_mut 数组入口；有 iter_mut | 所查 Query/QueryState 无 get_many_mut / iter_many |
| 查询缓存 | QueryState 缓存匹配 table/archetype 与准备状态 | std 普通查询也缓存；PreparedQuery 是额外显式准备态 | Query 内按 WorldId 缓存布局匹配 | QState 原型/bitset；catch 入口另缓存最近原型 |

表格便利性指调用步骤、错误类型和组合能力，**不作速度排名**。逐项证据如下。

## 2. Bevy

### 2.1 World 入口确实存在，但不能沿用旧版固定签名

- **World::get<T: Component>(&self, Entity) → Option<&T>**：实现是 get_entity(entity).ok()?.get()。实体不存在或缺组件均为 None。
- **World::entity<F: WorldEntityFetch>(&self, entities: F) → F::Ref<'_>**：委托 get_entity，不存在时 panic。单 Entity 对应实体引用；此版泛型 fetch 还支持相应多实体形态。
- **World::get_entity<F: WorldEntityFetch>(&self, entities: F) → Result<F::Ref<'_>, EntityNotSpawnedError>**：不是旧版固定单实体签名。
- **World::query<D: QueryData>(&mut self) → QueryState<D, ()>**；过滤入口为 query_filtered<D,F>()。构造的是准备态，不是实体结果列表。

证据：[world/mod.rs:753–757](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/world/mod.rs.html#753-757)、[934–941](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/world/mod.rs.html#934-941)、[1344–1368](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/world/mod.rs.html#1344-1368)、[1864–1888](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/world/mod.rs.html#1864-1888)。

**推论：**一个已知实体的一个组件直接 get 即可，不必每次临时构造 QueryState；多组件组合、过滤、系统注入时查询描述更方便。

### 2.2 QueryState / Query 的准确 API

| 方法 | 已核实签名要点 |
|---|---|
| QueryState::get | &mut self、&World、Entity → Result<只读 Item, QueryEntityError> |
| QueryState::get_mut | &mut self、&mut World、Entity → Result<D::Item, _> |
| QueryState::get_many_mut | &mut self、&mut World、[Entity;N] → Result<[D::Item;N],_>；D: IterQueryData |
| Query::get / get_mut | &self / &mut self、Entity；不再传 World |
| Query::get_many | &self、[Entity;N] → Result<[只读 Item;N],_>；允许重复 |
| Query::iter_many | 输入 Entity 列表，按列表顺序返回；此版每项为 Result，.matched() 可跳过失败 |

QueryState 的 get → self.query(world).get_inner(entity)，其他可变方法通过 query_mut 对应委托。

证据：[state.rs:932–978](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/state.rs.html#932-978)、[1023–1079](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/state.rs.html#1023-1079)、[system/query.rs:918–968](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/system/query.rs.html#918-968)、[1722–1777](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/system/query.rs.html#1722-1777)、[iter.rs:1601–1621](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/iter.rs.html#1601-1621)、[2231 起](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/iter.rs.html#2231)。

### 2.3 执行流程与过滤

Query::get_inner(e)：

    entities.get_spawned(e) → location
      → matched_archetypes.contains(archetype_id)
      → D/F::init_fetch → D/F::set_archetype
      → F::filter_fetch(e, row) → D::fetch(e, row)

不遍历其他实体。**缓存匹配原型不等于完全不执行过滤器**：With/Without 的 filter_fetch 为 true，结构条件此前已匹配；非原型条件仍须对目标实体判定。官方性能说明明确区分 With/Without 等原型过滤与 Added/Changed/Spawned 等变化过滤。

证据：[get_inner:1873–1927](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/system/query.rs.html#1873-1927)、[QueryFilter 契约:90–112](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/filter.rs.html#90-112)、[With:215–226](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/filter.rs.html#215-226)、[Without:326–337](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/filter.rs.html#326-337)、[Query 性能表](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/system/struct.Query.html#performance)。

QueryState 保存 WorldId、generation、matched_tables/archetypes、matched_storage_ids、fetch_state/filter_state。常规 query/get 会 update_archetypes，处理新增原型；有必需组件时缩小候选。manual 入口不自动更新，可能漏掉新原型。**直接实体定位无扫描，不等于冷启动/结构变化后的整个调用没有原型匹配成本。**官方 get 标为 O(1)，分析端到端成本时仍应把前置缓存更新分开计。

证据：[state.rs:79–101](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/state.rs.html#79-101)、[354–392](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/state.rs.html#354-392)、[547–639](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/state.rs.html#547-639)、[1129–1148](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/state.rs.html#1129-1148)。

### 2.4 借用与可变多实体

World 直接读写依靠 &World / &mut World；系统 Query 依靠声明访问、初始化时的参数冲突检查、调度保证。不是“全部编译期证明”，也不是每次 get 返回 hecs 式动态组件借用 guard。见 [Query 冲突说明](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/system/struct.Query.html#disjoint-queries)、[get_inner 安全注释](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/system/query.rs.html#1873-1876)。

get_many_mut_inner 双层循环查重，重复返回 AliasedMutability，然后逐实体 get_inner；普通数组查重 O(N²)。只读 get_many 不查重；UniqueEntityArray 变体依赖唯一集合保证。iter_many_mut 允许输入重复时，fetch_next 将结果生命周期限制在当前借用，不能同时持有前后重复实体的可变引用。unique 变体只证明当前集合内部唯一，不自动证明两个系统的实体集合互斥。

证据：[system/query.rs:2097–2181](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/system/query.rs.html#2097-2181)、[iter.rs:1601–1608](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/query/iter.rs.html#1601-1608)。

## 3. hecs

### 3.1 单点 API：特别注意 0.11.2 返回值

- **World::get<'a,T: ComponentRef<'a>>(&'a self,e) → Result<T::Ref,ComponentError>**。泛型传 &T 或 &mut T；结果分别是 Ref<T> / RefMut<T>，不是 Bevy 的裸 &T / Mut。
- **World::entity(&self,e) → Result<EntityRef<'_>,NoSuchEntity>**；只创建实体引用不立即借用组件，随后 get 才获得 guard。
- **World::query_one<Q>(&self,e) → QueryOne<'_,Q>**，此版本身**不返回 Result**。再调用 guard.get() → Result<Q::Item,QueryOneError> 才实际取值和动态借用。不要照抄旧版对 query_one 本身用 ? 的写法。
- **World::query_one_mut<Q>(&mut self,e) → Result<Q::Item<'_>,QueryOneError>**，直接返回组合结果，跳过动态 borrow 检查，但仍 assert_borrow 检查查询自身组合。

证据：[world.rs:460–573](https://docs.rs/hecs/0.11.2/src/hecs/world.rs.html#460-573)、[官方 World 签名](https://docs.rs/hecs/0.11.2/hecs/struct.World.html#method.query_one)、[entity_ref.rs:64–91](https://docs.rs/hecs/0.11.2/src/hecs/entity_ref.rs.html#64-91)、[319–350](https://docs.rs/hecs/0.11.2/src/hecs/entity_ref.rs.html#319-350)。

单点流程：entities.get(e) → 目标 archetype/row → Q::Fetch::prepare(archetype) →（共享 World 路径 borrow）→ execute → 按行 Q::get。**不扫描全世界或全部原型**。With/Without 在 prepare 中检查目标原型；不是 Bevy 式内置 Changed 的逐实体 tick 过滤。

证据：[world.rs:500–510](https://docs.rs/hecs/0.11.2/src/hecs/world.rs.html#500-510)、[query_one.rs:40–50](https://docs.rs/hecs/0.11.2/src/hecs/query_one.rs.html#40-50)、[query.rs:488–499](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#488-499)、[567–579](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#567-579)。

动态借用计数在 **archetype 的组件存储粒度**，不是每实体锁。不同实体、同原型同组件的两个可变 guard 也可能冲突 panic。QueryOne::get 成功后持有借用，再次获取会 panic；借用持续到 guard drop，而非 get 返回就释放。

证据：[entity_ref.rs:376–449](https://docs.rs/hecs/0.11.2/src/hecs/entity_ref.rs.html#376-449)、[query.rs:160–224](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#160-224)、[query_one.rs:33–50](https://docs.rs/hecs/0.11.2/src/hecs/query_one.rs.html#33-50)。

### 3.2 QueryBorrow::view 与普通查询缓存

World::query<Q>(&self) → QueryBorrow；**QueryBorrow::view(&mut self) → View<'_,Q>**：先 self.borrow() 得到 CachedQuery，对匹配的非空原型借用组件，再建立随机访问 fetch 表。View::get（Q: QueryShared）/get_mut 按 entity.id 取 meta、校验 generation、按原型索引取 fetch、按行获取。

**建立 View 会有匹配原型级准备/借用成本；建立后单次点取值不再全局匹配 With/Without。**少数点查询可直接 query_one，反复随机访问可复用 View；是否获益仍需测量。

证据：[query.rs:659–710](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#659-710)、[View:1400–1494](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#1400-1494)、[fetch_all:2079 起](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#2079)。

**版本校正：0.11.2 的 std 普通查询也缓存。**CachedQuery 按 Fetch TypeId 在 World 的 query_cache 查 Arc 状态，核对 archetypes_generation；缺失/失效时遍历原型 prepare 重建。无 std 分支另走逐原型准备，不能把 std 缓存结论外推到所有 feature。

证据：[query.rs:1888–1964](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#1888-1964)、[2028–2050](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#2028-2050)。

### 3.3 PreparedQuery 与唯一性

PreparedQuery 保存 memo=(World id, archetype generation)、匹配原型/Fetch::State、fetch 缓冲；memo 不同重建。query(&World) 动态借用，query_mut(&mut World) 免动态借用，view_mut 与 PreparedQueryBorrow::view 提供 PreparedView，后者 get/get_mut 用 meta 与原型 fetch 索引。

**官方明确：PreparedQuery 较不方便，通常无可测性能改善；除非端到端测量支持，否则优先普通查询。**这不是保证所有场景都无收益。

证据：[query.rs:1186–1273](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#1186-1273)、[1284–1325](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#1284-1325)、[1637–1682](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#1637-1682)。

**World::query_disjoint_mut<Q,N>([Entity;N]) → [Result<Item,_>;N]**；**View / PreparedView::get_disjoint_mut → [Option<Item>;N]**。重复实体 panic，不像 Bevy 返回整体 AliasedMutability。View 文档说明 N>3 查重需克隆数组并 O(N log N)。旧名字 query_many_mut/get_many_mut 仍存在，但自 0.11.0 deprecated，改名 disjoint。

证据：[world.rs:513–540](https://docs.rs/hecs/0.11.2/src/hecs/world.rs.html#513-540)、[query.rs:1496–1542](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#1496-1542)、[1688–1711](https://docs.rs/hecs/0.11.2/src/hecs/query.rs.html#1688-1711)。

## 4. Legion

### 4.1 Entry 直接取组件

**entry_ref / entry_mut 是 EntityStore trait 方法**，World、SubWorld 实现；需 trait 在作用域（例如 legion::EntityStore）。分别 &self / &mut self，返回 Result<EntryRef / EntryMut,EntityAccessError>。World 由 entities 位置表取 location，不扫描实体。

证据：[world.rs:45–60](https://docs.rs/legion/0.4.0/src/legion/internals/world.rs.html#45-60)、[845–876](https://docs.rs/legion/0.4.0/src/legion/internals/world.rs.html#845-876)。

EntryRef/EntryMut 的 **get_component<T>(&self)** 返回受 entry 借用约束的引用；**into_component<T>(self)** 消费 entry，返回与底层 world 借用期关联的引用，便于链式带出临时 entry。EntryMut 另有 get_component_mut / into_component_mut。实现查组件权限后，按 storage<T> → archetype slice → row 取值，不跑 Query 过滤。

证据：[entry.rs:76–150](https://docs.rs/legion/0.4.0/src/legion/internals/entry.rs.html#76-150)、[216–337](https://docs.rs/legion/0.4.0/src/legion/internals/entry.rs.html#216-337)。另有 **World::entry(&mut self,e) → Option<Entry>** 可修改组成，不要与 EntryRef/EntryMut 混称：[world.rs:307–327](https://docs.rs/legion/0.4.0/src/legion/internals/world.rs.html#307-327)。

### 4.2 Query::iter / get 均存在，但过滤语义不同

通过 IntoQuery（如 <(&Position,&Velocity)>::query()）创建 Query。**iter(&store)** 要求只读 Fetch，**iter_mut(&mut store)** 允许可变 View；底层 iter_chunks 再 flatten。

证据：[query/mod.rs:419–454](https://docs.rs/legion/0.4.0/src/legion/internals/query/mod.rs.html#419-454)、[511–543](https://docs.rs/legion/0.4.0/src/legion/internals/query/mod.rs.html#511-543)、[Query 官方文档](https://docs.rs/legion/0.4.0/legion/query/struct.Query.html)。

**get(&mut self,&T:EntityStore,e)** 要求 ReadOnlyFetch；**get_mut(&mut self,&mut T,e)** 允许写，返回 Result<View::Element,EntityAccessError>。底层 get_unchecked：

1. entry_ref(e)?.location() 直接定位。
2. get_component_storage::<View>().unwrap() 获取权限受限的 storage accessor。
3. 构造只含目标 archetype 的 QueryResult 并 fetch。
4. 若 !is_view_filter（调用 .filter(...) 添加了额外过滤），evaluate_query 后检查 result.index().contains(target_archetype)。
5. fetch.accepted() 触发相应版本更新，再取目标行。

**get 明确不执行动态过滤器**，包括 maybe_changed 的过滤状态判定/消费。附加布局过滤仍匹配原型，不能据此声称完全不检查 filter；也不能将其语义等同于 Bevy 带 Changed 的 get。迭代才 prepare filter、对 fetch 调用 matches_archetype，且为原型/切片级，不是精确逐实体值比较。

证据：[get 及明确警告:245–327](https://docs.rs/legion/0.4.0/src/legion/internals/query/mod.rs.html#245-327)、[迭代 prepare:386–395](https://docs.rs/legion/0.4.0/src/legion/internals/query/mod.rs.html#386-395)、[动态判定:893–898](https://docs.rs/legion/0.4.0/src/legion/internals/query/mod.rs.html#893-898)。

### 4.3 缓存及点访问扫描边界

Query 含 layout_matches:HashMap<WorldId,Cache>、filter:Mutex<F>。缓存或引用 group/subgroup 的 ordered 原型片段，或保存 unordered Vec + seen。evaluate_query 用 layout_index.search_from(filter,seen) 增量更新新原型，并遍历匹配原型做权限校验。

所以默认 View 的 get 不对整个查询匹配集合求值；附加过滤的 get 可能增量搜索布局、遍历匹配原型权限，并线性 contains。**不扫描所有实体，但不能无条件称 O(1)。**Mutex/filter 状态也不等于 hecs 组件动态借用计数。

证据：[query/mod.rs:128–165](https://docs.rs/legion/0.4.0/src/legion/internals/query/mod.rs.html#128-165)、[172–229](https://docs.rs/legion/0.4.0/src/legion/internals/query/mod.rs.html#172-229)、[285–294](https://docs.rs/legion/0.4.0/src/legion/internals/query/mod.rs.html#285-294)。

### 4.4 SubWorld 权限、借用与多实体

安全写入口依靠 &mut EntityStore；SubWorld 还持有 ComponentAccess 与可选 archetype BitSet。entry_ref/entry_mut 校验目标原型，把组件权限带入 Entry；具体 get_component 再校验组件读写。query 的 get_component_storage 校验 View 权限，失败返回 AccessDenied，但 Query 内 unwrap，**不保证所有权限失败都变成 get 的 Err，也可能 panic**；evaluate_query 的原型权限失败也会 panic。

证据：[subworld.rs:161–182](https://docs.rs/legion/0.4.0/src/legion/internals/subworld.rs.html#161-182)、[248–287](https://docs.rs/legion/0.4.0/src/legion/internals/subworld.rs.html#248-287)、[query/mod.rs:172–176](https://docs.rs/legion/0.4.0/src/legion/internals/query/mod.rs.html#172-176)、[262–263](https://docs.rs/legion/0.4.0/src/legion/internals/query/mod.rs.html#262-263)。

所查 [完整 Query 实现/方法清单](https://docs.rs/legion/0.4.0/legion/query/struct.Query.html) 未见 Bevy 式 get_many/get_many_mut 同名指定数组入口。连续 get_mut 不会绕过 &mut store 的借用；iter_mut 或权限 split 可组织多实体访问。unsafe get_unchecked 需自行证明无可变别名，不是同等安全的便利替代。没有该方法不代表多实体操作不可能。

## 5. pi_world 本地实际符号

以下链接均指 D:/dev/pi_world，不是远程同名包。

### 5.1 World 入口

- **get_component<T:'static>(&self,e) → Result<&T,QueryError>**。
- **get_component_mut<T:'static>(&mut self,e) → Result<Mut<'static,T>,QueryError>**。源码确实声明 'static，不能替它改述为 Bevy 的 World 借用期签名。
- TypeId→ComponentIndex，entities.get(e)→EntityAddr，Column::blob_ref(archetype)→row；不遍历其他实体，也不执行 Query 的 F。
- **World::query<Q,F>() / make_query<Q,F>() → QueryState<Q,F>**，构造时设置 default_system_meta.this_run、初始化并 align。
- **contains(e)** 检查存在，**get_entity_prototype(e)** 返回名字/原型索引。所查 World 公开实现未见 Bevy/hecs 同名 World::entity/get_entity/get<T> 通用实体引用 API。QueryIter::entity() 只是当前迭代实体，不能混称。

证据：[world.rs:234–249](../../pi_world/src/world.rs#L234)、[324–337](../../pi_world/src/world.rs#L324)、[517–572](../../pi_world/src/world.rs#L517)、[query.rs:722–724](../../pi_world/src/query.rs#L722)。

### 5.2 Query / QueryState 点取值与两种缓存

Query 包装 &mut QueryState；get(&self,e) 得只读 Item，get_mut(&mut self,e) 得 Q::Item。QueryState::get(&self,_world:&World,e) / get_mut(&mut self,_world:&World,e) 存在，但 **_world 被忽略，实际用状态保存的 Ptr<World>**，没有 Bevy 式此入口 WorldId 校验。不能因同名就说可对另一 World 重用。

证据：[query.rs:103–156](../../pi_world/src/query.rs#L103)、[358–414](../../pi_world/src/query.rs#L358)、[425–450](../../pi_world/src/query.rs#L425)。

普通 get/get_mut → get_by_tick：

    entities.load(e) → 原型 bit_set 检查
      → 每次 Q::init_fetch + F::init_filter
      → F::filter(row,e) → Q::fetch

没有实体扫描；本实体 filter 每次执行，true 表示排除并返回 NoMatchEntity。With/Without 登记结构关系，原型匹配先缓存，实体 filter 默认不排除。

证据：[query.rs:498–541](../../pi_world/src/query.rs#L498)、[filter.rs:19–52](../../pi_world/src/filter.rs#L19)、[55–125](../../pi_world/src/filter.rs#L55)。

**实际拼写是 get_by_catch/get_mut_by_catch**，委托 get_by_tick_catch：同最近原型复用 fetch/filter，换原型检查 bitset 并重建；仍每实体 F::filter。普通 get_by_tick 的最近原型复用分支已注释，不能说所有 get 都自动走该缓存。

证据：[query.rs:143–156](../../pi_world/src/query.rs#L143)、[453–494](../../pi_world/src/query.rs#L453)、[510–538](../../pi_world/src/query.rs#L510)。

QState 缓存匹配 archetypes、bit_set、已处理原型数量；align 仅对新增原型运行 relate。普通 get 本身不调用 align；World::make_query 创建时 align，系统 Query 的 SystemParam::align 更新。长期持有 state 后发生结构变化，须遵守同步协议，不能假设每次 get 自动刷新。全遍历则在匹配原型逐行扫描，跳 null Entity，每实体执行 F::filter。

证据：[query.rs:185–203](../../pi_world/src/query.rs#L185)、[394–397](../../pi_world/src/query.rs#L394)、[597–672](../../pi_world/src/query.rs#L597)、[725–775](../../pi_world/src/query.rs#L725)。

### 5.3 EntryQuery 存在，但点取值未完成

**有效 EntryQuery 定义在 query.rs:252**，prelude 重导出；get/get_mut 调用 get_by_tick1，后者只查实体，核心实现注释，最后 **todo!()**。所以有效实体进入该入口会到未实现路径；不存在实体先返回 NoSuchEntity。不能推荐为当前可用快路径。独立 entry_query.rs 是注释版本，不是有效运行实现。

证据：[lib.rs:25](../../pi_world/src/lib.rs#L25)、[query.rs:252–320](../../pi_world/src/query.rs#L252)、[545–592](../../pi_world/src/query.rs#L545)、[entry_query.rs:1–28](../../pi_world/src/entry_query.rs#L1)。这是静态路径判断，未运行复现。

### 5.4 安全与唯一性边界

所查 Query/QueryState 无 get_many_mut/iter_many/get_disjoint_mut 同名批量入口，不能凭空补写存在性或唯一性保证。FetchComponents::init_state 登记 SystemMeta 访问，内部使用 Ptr、transmute、SyncUnsafeCell、手工 Send/Sync。已读点访问没有 hecs 式 borrow guard，**不能说多个独立 QueryState 自动动态检查互斥**。World::get_component_mut 返回 Mut<'static,T>、QueryState 保存 World 指针等需要独立安全审计；本文不将其认定为和三库相同的安全契约。

证据：[fetch.rs:15–67](../../pi_world/src/fetch.rs#L15)、[105–127](../../pi_world/src/fetch.rs#L105)、[query.rs:106–156](../../pi_world/src/query.rs#L106)、[358–399](../../pi_world/src/query.rs#L358)、[world.rs:522–569](../../pi_world/src/world.rs#L522)。

另：Query::contains → QState::check 仅检查实体和原型 bitset，**不执行 F::filter**，不能用来证明 Changed 等查询 get 必成功。[query.rs:416–418](../../pi_world/src/query.rs#L416)、[654–672](../../pi_world/src/query.rs#L654)。

## 6. Bevy SparseSet 与本项目 HashMap trait 不直接等价

先区分两个默认：**Bevy 默认组件存储是 Table**；显式选 SparseSet 后，内建表示为 ComponentSparseSet：dense:Column、entities:Vec、sparse:SparseArray<EntityIndex,TableRow>。SparseArray 是按整数索引的 values，先找到 dense 行再取 Column 值。文档 “HashMap-like” 指抽象行为，不代表实现是 HashMap<Entity,T>。

证据：[StorageType 默认 Table](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/component/enum.StorageType.html)、[storage 概述](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/storage/index.html)、[sparse_set.rs:80–105](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/storage/sparse_set.rs.html#80-105)、[157–178](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/storage/sparse_set.rs.html#157-178)、[278–284](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/storage/sparse_set.rs.html#278-284)。

本项目 **pi_ecs**（不是旧 pi_world）[crates/pi_ecs_core/src/sparse_set.rs:9–38](../crates/pi_ecs_core/src/sparse_set.rs#L9) 声明 SparseSet trait，get/get_mut 取完整 Entity；HashMapSparseSet 保存 **HashMap<Entity,Blob>**，无 dense owners/局部索引映射，remove 契约不搬动其他实体；枚举分派而非 dyn trait。当前文件还注明 trait impl 待 Blob 所有权闭合，**是声明，不是已完成可测后端**。

**推论：**两者可共享以实体取组件的上层概念，但身份校验位置、哈希成本、稀疏索引空间、dense 遍历、删除搬行、Blob 所有权/引用契约不能直接等价。不能用 Bevy 的两次数组索引与连续列性质给本项目 HashMap 后端背书，也不能以 HashMap 稀疏占用直觉推断 Bevy SparseArray 内存。

## 7. 使用便利性与性能推论

1. 已有一个 Entity、一个组件时直接访问即可；不必为点取值先全局遍历。
2. 多组件组合可复用 Query；hecs 独占 query_one_mut 返回值直接，共享 query_one 需要 guard；Legion get 不执行动态过滤；pi_world 普通 Query 可用，但 EntryQuery 当前未完成。
3. 这些缓存主要是**匹配存储/原型与准备信息**，不是实体结果快照。存活、位置、权限、变化过滤、生命周期仍按入口处理。
4. 成本拆成**准备/更新、定位、过滤、借用、唯一性**：hecs View 可扩大准备/借用域，Bevy 更新新原型，Legion 附加过滤 get 检查原型集合，pi_world catch 才复用最近原型。
5. 没有基准，不给速度排行。官方复杂度不能替代组件数量、原型规模、随机访存、冷热缓存、结构变更、feature、多线程与成员索引维护成本的实测。

### 证据限制

- 固定版本 docs.rs 源码链接，不用 latest/main 替代；宏展开可能重复显示，行号以原源码锚点为准。
- 尝试过 Bevy world/entity_ref.rs、world/entity_ref/mod.rs、storage/sparse.rs 源码路径，返回 404；不据此判定 API 消失。本文 World 入口依据成功读取的 world/mod.rs，SparseArray 依据 sparse_set.rs。
- “未见便利入口”仅限已查类型与实现，不否定第三方扩展或用户封装；未审计所有 panic、unsafe 不变量和自定义 Query/Fetch/Filter 性能。
