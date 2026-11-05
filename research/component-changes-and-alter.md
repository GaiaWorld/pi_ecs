# 组件增删与 Alter：Bevy、hecs、Legion、pi_world

研究日期：2026-10-30。基线：**bevy_ecs 0.20.0、hecs 0.11.2、Legion 0.4.0、本地 D:/dev/pi_world 0.2.17**（Cargo.toml:1–7）。

沿用 [Bevy 研究](bevy-ecs.md)、[hecs/Legion 研究](hecs-legion-comparison.md)、[pi_world 研究](pi-world-and-requirements.md) 和 [查询研究](query-and-entity-access.md) 的版本固定、事实/推论分离惯例。本文重新直接读取官方 docs.rs 发布源码及本地实现。**事实**表示已读 API/代码；**推论**表示机制分析；**待验证**表示尚未运行的测试/基准。仅新增本文，没有修改生产代码，没有编译、运行测试或基准。本地行号为研究时文件位置，不代表锁定了仓库修订。

## 1. 先给结论

1. **本地 Alter 不是可连续 insert/remove 的编辑事务。**它是固定增量的 `Alter<'w,Q,F,A,D>`：A 是本次加入的 Bundle，D 是删除类型集合；每实体调用一次 alter(e, A)。多组件加入与删除合在这一次调用中，直接计算最终目标。改变原型后，在提交前再次改变该实体通常返回 RepeatAlter，不会自动合并。[P1][P2]
2. **一次最终迁移不是 Alter 独有。**Bevy 一次 insert(bundle)、remove::<Bundle>()、take::<Bundle>() 已按整个 bundle 处理；hecs 一次 insert/remove 也一样，且 **exchange::<S,T>(e, values)** 明确跳过先删后加的中间实体迁移。与 Alter 同时增删最接近的是 hecs exchange，不是两个单组件调用。[B1][H1][H2]
3. **Alter 的真实特点是：固定计划复用 + 立即写目标新增值 + 源结构暂留 + Drop 按列集中迁移/发布 + 更晚的统一填洞。**它既不是把所有操作都推迟到帧末，也不是每 insert/remove 搬一次，更不是“先把所有新值放临时 map，Drop 才决定目标”。[P2][P3][P5]
4. **Legion 普通 Entry 增删是单组件结构操作。**连续 add_component/remove_component 每次确有集合变化时各自 transfer_archetype；同一个 Entry 作用域不会合并。CommandBuffer 只是 FIFO 延迟执行；extend 的批量创建不能当作批量修改已有实体。[L1][L2][L3]
5. **缓存、延迟执行、一次迁移、物理整理是四件事。**Bevy 和 hecs 都有转换缓存；三库的普通命令队列没有自动将同实体命令折叠为最终状态。Bevy 仅改变 SparseSet 组件而 Table 不变时，甚至无需搬 Table 行，不能把每次原型变化都计成一次全组件搬运。[B2][B3][B4][H2][H3]
6. **不能直接照抄本地所有权实现。**标准派生 Bundle 最终用 ptr.write；替换已有组件未见旧值析构，跨原型替换也未进入 removing 列。另有 A/D 重叠、panic 中途提交和持有迭代结果时调用 alter 等边界，见第 6 节。上述是源码推导，不冒充动态复现或完整安全审计。[P2][P4][P6]

## 2. 比较口径与总表

“迁移一次”指一个实体的**保留组件从源存储转入最终目标存储的一轮结构迁移**，不是只执行一次 memcpy；各列仍需逐项处理，也不包含后续填洞搬动其他实体、hooks 或用户代码带来的额外结构操作。比较以下常规路径，不假设外部自行实现合并器。

| 问题 | Bevy 0.20.0 | hecs 0.11.2 | Legion 0.4.0 | pi_world 0.2.17 本地 |
|---|---|---|---|---|
| 已有实体加入多个组件 | EntityWorldMut::insert(bundle) | World::insert(e, DynamicBundle) | Entry::add_component 每次一个类型 | Alter 的 A Bundle，一次 alter(e,A) |
| 删除多个组件 | remove::<B>()；take::<B>() 返回值 | remove::<B>() 返回 B | remove_component::<T>() 每次一种 | D Bundle 指定删除集合，不接收 D 值 |
| 同时删一组、加一组 | 所查 EntityWorldMut 无 exchange；remove + insert 为两次操作 | exchange::<S,T>()，跳过中间实体迁移 | 普通 Entry 无对应集合交换入口 | A/D 合成最终目标，一次计划/迁移 |
| 一个作用域内连续操作自动融合？ | 否 | 否；须主动用 bundle/exchange | 否 | 否；改变原型后再改通常 RepeatAlter |
| 已有同类型值 | insert 替换；insert_if_new 保留旧值 | insert 替换并析构旧值 | 赋值替换并析构旧值 | 意图为替换；标准写入链未见旧值析构 |
| 缺失删除类型 | remove 忽略；take 缺任一项则 None、不删 | remove/exchange 缺任一待移出项则 Err、不完成该删除 | remove_component 无操作 | 忽略缺失类型 |
| 移出返回 | take → Option<B> | remove/exchange → Result<B,_> | remove_component → () | alter → Result<bool,QueryError>，无移出值 |
| 转换计划复用 | Archetype Edges 按 BundleId | insert_edges/remove_edges；静态 key 命中 | layout 索引查找/复用目标；所查路径非相同 edge cache | 每个 AlterState、每源原型的 ArchetypeMapping |
| 系统中延迟 | Commands，ApplyDeferred 时顺序执行 | CommandBuffer::run_on | CommandBuffer::flush | 新增值当场写目标；Alter Drop 才搬保留值并发布 |
| 旧行物理回收 | 常规路径 swap_remove 并修位置 | 常规路径 swap_remove 并修位置 | transfer 中 swap_remove 并修位置 | clear 留洞，Table::settle 后续统一填洞 |

证据：Bevy [B1–B4]；hecs [H1–H3]；Legion [L1–L3]；pi_world [P1–P6]。API 名相似不代表错误/析构/可见性等价。

## 3. 三个公开库的真实机制

### 3.1 Bevy：bundle 级转换，原型变化不必搬 Table

**事实：**EntityWorldMut::insert<T:Bundle>(&mut self,T)->&mut Self 覆盖同类型已有值；insert_if_new 用 Keep 模式。一次 insert 创建一个 BundleInserter，传完整 bundle。remove<T:Bundle>() 删除交集，不要求所有类型存在；take<T:Bundle+BundleFromComponents>()->Option<T> 要求全部存在，否则不移出任何项。remove 丢弃删除值，take 把所有权交给调用方。[B1]

核心流程：

    insert(bundle)
      注册/取得 BundleInfo
      源 Archetype + BundleId → 查询/建立插入 edge
      SameArchetype：原位写入/替换
      NewArchetypeSameTable：更改原型成员位置，保留 table_row
      NewArchetypeNewTable：迁移 Table 行，修复被换位实体位置
      写入 bundle，执行相应生命周期逻辑，flush/update_location

Edges 分别保存 insert_bundle、remove_bundle、take_bundle；take 缺失结果也可缓存。缓存避免反复求集合/建原型，不会免除实际搬值和析构。[B2]

**稀疏变化例外：**插入路径明确比较新旧 table_id；相同 Table 的新原型分支只调整原型成员，继续用原 Table 行。删除路径同样只在 table_id 不同时设置 old_and_new_table 并移动 Table 行；SparseSet 的删除单独处理。因此连续增删稀疏组件可能多次改变原型，但不是多次搬全部 Table 列。[B3]

**延迟不是融合：**EntityCommands::insert 和 remove 各 queue 一个命令；Commands 在 ApplyDeferred 中按序执行。把 .insert(A).insert(B).remove::<C>() 写成链式，只是三个命令；主动写 .insert((A,B)) 才把加入集合放进一个 bundle。一个自定义命令里连续调用三次 World 操作也不自动变成一次迁移。[B4]

**范围：**所查固定版 EntityWorldMut 公共方法表没有 hecs 式 exchange；modify_component 是暂时移出单组件供闭包修改、触发生命周期但不迁移原型，move_components 是把组件转给另一个实体的克隆/删除流程，都不是同实体任意 A/D 集合交换。[B5] hooks/observers、Required Components、关系处理可增加工作，不能把一轮底层 bundle 转换宣称成完整业务恒为一次移动。

### 3.2 hecs：exchange 已经提供“直接最终目标”的等价机制

准确 API：[H1][H2]

- insert(&mut self,e,components:impl DynamicBundle)->Result<(),NoSuchEntity>；insert_one 是单元素 tuple 包装。
- remove<T:Bundle+'static>(&mut self,e)->Result<T,ComponentError>；remove_one<T:Component> 解包单元素 tuple。
- exchange<S:Bundle+'static,T:DynamicBundle>(&mut self,e,components:T)->Result<S,ComponentError>；另有 exchange_one。
- remove 与 exchange 的待移出集合缺任一项时返回错误；这不同于 pi_world D 的缺失忽略，也不同于 Bevy remove 的交集删除。

exchange 核心流程：

    定位源实体
    S::get 从源取得待移出值；缺失则返回错误
    remove_target::<S>(源) → 逻辑中间原型 ID
    insert_inner(e, T, graph_origin=中间ID, loc=真实源位置)
      求最终插入目标
      最终 == 真实源：原位写值
      否则：分配最终行，写入新值，搬保留值，回收真实源行
    返回 S

**关键区别：**源码仍调用 remove_target，可能查找/建立“删后未加”的中间原型元数据；但实体数据不先搬到该原型。官方所说 skip intermediate archetype 指跳过中间实体迁移，不能扩写成从不产生中间布局元数据。[H2]

**复用：**World 持有 insert_edges: (源原型,静态 bundle key)→InsertTarget 和 remove_edges: (源原型,TypeId<T>)→目标。exchange 复用这两段图查找，没有单独 exchange_edges。DynamicBundle::key 为 None 时仍能按类型集合查找/复用 ArchetypeSet 的原型，但不走这个静态插入 edge 命中路径。insert_inner 在最终原型等于真实源时原位更新，否则只分配最终行；保留列逐项转移，并修正源末行换入实体的位置。其 Guard 明确用于即使被覆盖值 Drop panic 也完成新值安装和源清理；这不等于整个 World 的任意 panic 都可继续使用。[H2]

**命令：**CommandBuffer 支持 insert/insert_one/remove/remove_one，run_on 顺序解释；所查没有专用 exchange 方法，但可 queue 闭包调用 World::exchange。insert 的新值先拥有在带 TypeInfo、offset、对齐的擦除存储内；执行后清理消耗记录，clear/Drop 丢弃未执行新值，不提交。remove 命令调用 World::remove 并忽略 Result，所以多项 bundle 缺一项时并非“其余存在的全部删掉”。缓冲区可重复使用已有容量，不能因此声称多个命令被融合。[H3]

### 3.3 Legion：Entry 单类型增删，extend 是创建

Entry::add_component<T> 先尝试 get_component_mut；存在则普通赋值替换。不存在时，基于源 layout 加一个类型，get_archetype_for_components 查 layout 索引或新建，transfer_archetype 搬源公共组件，再将新 T 写入目标。remove_component<T> 缺失则直接返回，否则构造减一个类型的布局并 transfer；无移出返回值。[L1]

transfer_archetype：源实体数组 swap_remove，目标 push，修复本实体和被换位实体的位置；遍历源类型，目标仍有的调用 storage.move_component，目标没有的 storage.swap_remove。**同一 Entry 连续新增两个缺失类型、再删一个存在类型，普通路径会执行三轮迁移，不因 Entry 活到作用域末才统一提交。**目标原型可复用，但所查 entry→world 路径没有 Bevy/hecs 同形的“源+bundle→目标”转换 edge 缓存，不能说 Legion 完全没有索引/复用。[L1][L2]

CommandBuffer 的 add_component/remove_component 分别排入 writer，flush 按 FIFO 执行；writer 再进入 world.entry(...).expect(...).add/remove_component。失效实体在这些 writer 中会 panic，不能照 hecs 的忽略错误语义推断。exec_mut 允许自定义 World 闭包，但闭包内部连续 Entry 调用仍是逐次迁移。[L3]

World::extend 接收 IntoComponentSource，支持 tuple/SoA 批量新实体，集中取得目标原型并写数据；CommandBuffer::extend 同样是排队创建新实体。**批量新建相同布局 != 已有一个实体多组件增删合并，也 != 一批已有实体的统一结构变更接口。**[L2][L3]

## 4. 本地 Alter 完整链：构造、写入、clear、Drop

### 4.1 入口与类型集合

直接入口 World::make_alter::<Q,F,A,D>() 返回 QueryAlterState；它建立 QueryState、注册 A/D 的组件信息，并为当前查询源原型建立空映射。get_param 得 Alter。系统入口 SystemParam::init_state 同样建立状态，get_param 先 align 新增源原型，再构造 Alter。直接 QueryAlterState::get_param 本身没有这一 align 步骤，不能据系统路径推断长期复用的直接入口一定自动刷新所有新原型。[P1]

**没有 Alter::insert / Alter::remove / 公开 Alter::clear。**真实调用是 alter(e, components:A)；D 通过泛型给出，纯删除用 A=()。clear 是 AState 的内部提交函数；Drop 无条件调用它。alter.rs:37–152 的 Alterer/delay_alter 整段是注释，不算已实现能力。[P1][P2]

另有已实现的 EntityEditor::alter_components_by_index，一次传入 (ComponentIndex,bool) 列表，缺失新值由 set_fn 初始化，随后当场 alter_row；它不是延迟到 Drop 的动态值编辑器。add_components<B> 包装临时 make_alter/get_param/alter，在该语句的临时值析构时提交，也不是多次调用自动聚合。本文不把这些不同入口混称 Alter，也不把注释中的接口算作能力。[P7]

### 4.2 何时计算目标、何时分配、有没有新值暂存

1. AState::make 将 A 标记 true、D 标记 false，按 (ComponentIndex,bool) 排序。
2. alter(e,A) 调 check，只校验实体存在及源原型是否在映射中；该调用没有再执行 F 的逐实体 filter。迭代入口则来自 QueryIter，先执行相应过滤。[P2][P5]
3. 首次遇到一个源原型，find_mapping 调 Archetype::alter 合并源类型与增删集合，分出 adding/moving/removing；在此时通过 world.find_archtype 查找或创建最终目标原型，并初始化 A::Item。后续该状态同源原型复用映射。[P2][P3]
4. 目标等于源：立刻 A::insert 写源行，返回 Ok(false)，不加入 moves；这条分支位于 RepeatAlter 检查之前。
5. 目标不同：检查地址 mark，重复则 Err(RepeatAlter)；否则标记地址、**立即 alloc 目标行**，立即 A::insert 把新增/替换值写目标列；记录 (src_row,dst_row,e)，返回 Ok(true)。此刻旧地址仍指源原型/源行，保留组件还没搬。[P2]

因此，bundle_vec:Vec<MaybeUninit<A::Item>> 存的是**每源原型的写入准备态**，标准派生对应列 Share/原型索引等，不是暂存每实体 A 值的队列。A 被本次调用消费并直接写进目标列。额外暂存的是 moves、dirty 映射列表和列计划；目标未发布行也占用存储。[P2][P4]

### 4.3 Drop 的提交与更晚的填洞

    Alter::drop
      AState::clear
        对 dirty 的每个源→目标映射
          源 mark_remove；若实体已 destroy，清掉其目标新增列并取消迁移
          move_columns：外层按保留列，内层遍历该映射所有实体
          remove_columns：析构明确删除列的值，记录 Removed
          目标行 set(entity)
          world.replace(entity,目标原型,目标行)，同时清 mark
          清 moves
    更晚 Table::settle
      汇总洞 → 统一列填洞动作 → 修复换位实体 row → 整理存储

实现顺序以 clear 为准，不照搬 table.rs:1–5 的概括性旧注释。clear 先令源行不可见，再搬保留列、析构删除列，最后逐实体发布目标并改地址；不是先改地址再完成所有组件初始化。[P2][P5]

**可见性分三层：**

- 调用后至 Drop 前：改变原型的实体仍从源读取/迭代；目标新行按空 Entity 设计保持查询不可见，新增值已经写入目标。EntityAddr 的 mark 编码负原型号，但 archetype_index() 仍解出源，Query 点访问不因 mark 自动改读目标。
- Drop/clear 后：地址指最终目标，后续有序观察者可见新集合；并不等于必须等下一帧。
- settle 后：洞被物理填补，位置再修复；这不是逻辑增删首次生效时点。[P2][P5]

目标空行依赖底层 append/数组初始化约定：Table::alloc 自身仅转发 entities.alloc；本地 patch 的 pi_append_vec::alloc 先增加长度再 load_alloc。不能据此宣称只要长度递增就已安全发布完整行，更不能把本地另外的 safe_vec 实现当成该实际依赖路径。[P5][P8]

**destroy 也不是全延迟：**Alter::destroy 立即调用 destroy_row，Table::destroy 当场析构源所有组件、置空源 Entity 并移除世界身份。若此前已有待迁移记录，clear 发现源已为空后只销毁目标已写入的 adding 列，避免再搬源。Drop 是结构迁移提交点，不是所有变更操作统一延迟点。[P2][P5]

## 5. 一次最终迁移的严格等价性

假定源有 X、Y、Keep，要删 X/Y，加 A/B：

| 写法 | 常规实体迁移轮次 | 语义区别 |
|---|---|---|
| Bevy remove::<(X,Y)>(); insert((A,B)) | 至多两轮 Table 迁移；同 Table 变化可为零 | 两个发布/生命周期阶段；remove 忽略缺失 |
| hecs remove::<(X,Y)>(); insert((A,B)) | 通常两轮 | remove 返回旧值，缺失则 Err |
| hecs exchange::<(X,Y),_>(e,(A,B)) | 至多一轮，直接源→最终 | 返回 X/Y；要求待移出值全存在；可能建立中间元数据 |
| Legion 两次 remove_component、两次 add_component | 最多四轮 | 每个缺失删除/已有插入可减少结构迁移，但不融合 |
| pi_world Alter<A=(A,B),D=(X,Y)>::alter(e,(A,B)) | 一轮源→最终，Drop 执行保留列搬移 | 缺失删除忽略；不返回 X/Y；提交前读旧集合 |

同理，**只新增 A/B 时，Alter 的“一轮”应与 Bevy/hecs 一次 bundle 比，不能与两次单组件调用比来制造独占优势。**若最终集合不变，三者都存在原位处理路径；“有改值”不能等同于“迁移了原型”。

这些是正常路径结构计数，不是速度测量。Alter 的按列批处理、hecs 的紧凑即时搬移、Bevy 的 Table/SparseSet 分流和 Legion 的 packed storage 有不同常数、内存与生命周期成本，本文不作速度排名。

## 6. Alter 边界：便利性不能替代所有权契约

### 场景 A：同一实体在一个 Alter 生命周期内再改一次

源 {Keep}，A={New}，第一次 alter 写未发布目标并 mark；第二次仍通过源地址定位，目标不同，返回 RepeatAlter，第二次传入的新 A 按普通参数所有权退出/析构。Drop 只提交第一次。**不是 New 的“最后一次值覆盖”，也不是两次编辑融合。**[P2]

若源本就有全部 A 且 D 没有任何存在类型，目标等于源，提前原位写入并返回 false，不设置 mark；同一作用域可以再次走这条原位路径。故“同 system 绝对只准调用一次”不够准确，限制主要针对待提交的原型变化。来自另一个同源 Alter 的原位分支又在 mark 检查之前，安全组合还依赖调度/权限约束，不能只靠 mark 推导全局互斥。[P2]

### 场景 B：缺失删除与已存在组件替换

源 {Keep}，A=()、D={Missing}：目标不变，Ok(false)，无删除析构。源 {T,Keep}，A={T}、D=()：同集合立刻写 T；若再加 New，目标不同，T 被归入 adding 而非 moving/removing，旧 T 不在明确删除列中。[P2][P3]

**静态发现：**标准 Component 派生调用 TypeItem::write→BlobRef::write→ptr.write(val)，这会覆盖字节而不 drop 原值；跨原型替换时旧 T 也未进入 remove_columns，源实体行之后置空，Table::drop 仅析构非空行。因此，按这条标准链，带析构资源的旧 T 可能泄漏，不能称为与 Bevy/hecs/Legion 一样的正确替换析构语义。自定义 Bundle 可以执行不同逻辑，本文结论限定于已读标准派生链；尚未用计数 Drop 测试复现。[P2][P3][P4][P6]

### 场景 C：同一类型同时出现在 A 与 D，或 Bundle 重复类型

排序的是 (ComponentIndex,bool)，false 在 true 前；Archetype::alter 遇相同 ComponentIndex 仅处理第一个，所以 A/D 重叠时**删除优先**，不是“先删再加、最终保留”。但 A::insert 仍写 A 的值，标准 TypeItem 又不检查目标集合包含该列。这存在向非目标布局写值/初始化状态失配的风险，不能推荐把重叠当作 exchange 的同类型替换。[P2][P3][P4]

A 内重复类型元数据在计算目标时被去重，不等于值写入去重；tuple Bundle 仍逐项 insert，可能覆盖前一个值。应在未来接口中拒绝重复类型、明确 A/D 交集，而非依赖 sort/dedup 自然得到正确所有权。[P3][P4]

### 场景 D：先 alter 再 destroy；Drop/panic 中断

先 alter 再 destroy：源已有值即时析构；Drop 取消该实体迁移并销毁目标新值，目标行进入移除列表，不发布已销毁身份。这一路明确存在实现，不应说 destroy 只排队到 Drop。[P2][P5]

正常 Drop 会提交，而不是取消；mem::forget(Alter) 不执行 clear，也没有隐含的替代提交保证。A::insert 如果在 mark/目标分配之后、moves.push 之前 panic，目标行和源标记已经有副作用，未见局部清理 guard。clear 又先移动公共列、再析构删除列；删除组件 Drop 若 panic，目标还未完整发布、地址可能仍旧，源码没有完整事务回滚/展开恢复协议。**不能据 RAII 一词推断 panic 安全、可继续使用或自动回滚。**[P2]

额外的状态资源边界：bundle_vec 用 MaybeUninit 保存 A::Item，没有逐项 assume_init_drop；源码以“Bundle state 不需释放”为假定，但标准 TypeItem 本身包含 Share<Column>。至少不能将其宣传为已证明无资源泄漏的通用 Bundle 状态容器；自定义 Item 的析构责任也需明确。[P2][P4]

### 场景 E：迭代时变更与别名

Alter::get/get_mut 借用接收者，普通连续调用有 Rust 借用限制；但 AlterIter::Item 是 Q::Item<'w>，调用 alter(&mut self, A) 不要求前一次 next 的结果生命周期结束。推迟移动公共组件可以避免迭代中立刻搬走源数据，这是实际设计收益；却不是完整别名证明：同原型替换可当场写源值，destroy 可当场析构源值，调用者可能仍持有迭代结果。[P1][P2][P5]

实现使用 Ptr、transmute、共享引用下的内部写入及手工 Send/Sync；顶部关于执行图隔离的注释表达的是所依赖的约束，不是本文已独立审计完整调度器的证明。还需验证源/目标所有列访问权限、同原型分支、保留引用、同实体跨参数调用及新原型加入后的冲突更新。不能把“组件批量移动”扩写成“安全地允许任意多个引用与结构修改并存”。

## 7. 真优势与条件成本

### 值得保留的机制优势

- **静态 A/D 一次描述最终集合。**相对 Legion 普通逐类型 Entry 和拆开的 remove/insert，可省中间数据迁移；相对 hecs exchange 则不是独占优势。
- **长期源→目标映射和列计划。**首次计算 adding/moving/removing，后续同源实体复用；比每实体重建类型差异更有利于重复相同变换。Bevy/hecs 也有转换缓存，应比较缓存作用域及命中代价，而不是“有/无缓存”。
- **按列批处理多个实体。**clear 外层保留列、内层 moves，可减少来回切列和重复准备；这是可测的局部性假设，不是已测速度结论。
- **提交与填洞拆开。**变更期不立即搬末行实体来填每个洞，保留源行在查询期的位置；Drop 后即可看到新集合，阶段末再集中填洞。适合已有阶段调度和可跳洞查询的项目。[P2][P5]

### 必须同时计入的成本

- **空间与值存放：**源旧行、目标未发布行同时存在，外加每源映射、列 Share、moves、dirty 列表；这不是零暂存。原生 Alter 无逐实体 A 临时 map，但若要扩展为动态、连续编辑并最终合并，就需要额外拥有新值的类型擦除缓冲、对齐、替换析构与取消规则。
- **类型动态性：**静态 A/D 快速计划不直接等价于运行时任意类型混合。EntityEditor 有按索引的已实现操作，但依赖 set_fn 构造新增值，不提供任意值暂存与多调用合并语义。[P7]
- **可见性：**Drop 前新值不可正常按实体查询，旧值仍可能修改；Drop 时才搬公共值，所以最后一次源值修改会被搬走。需要明确后继系统的提交边界，不能笼统写“全部 deferred”。
- **异常与所有权：**ptr.write 替换、重复类型、重叠 A/D、部分初始化、析构 panic、忘记 Drop，以及 Item 释放责任都必须单独设计；禁止将未验证分支当成性能收益。
- **混合存储：**Bevy 的同 Table 稀疏变化已避免 Table 搬移。本项目采用预设 Table + 稀疏后备后，结构转换计划必须区分实际存储变化，不能每次改组合都搬全列。

## 8. 对 pi_ecs 的设计建议（推论，不修改已确认契约）

依据当前 [design.md](../design/unified-partitioned-ecs/design.md):68–84、128–150：已选混合存储、作用域提交、阶段末整理；直接 World::change_archetype 返回时提交；EntityValues、计划及存储入口仍属设计声明/失败桩。**本文不把它们当作本项目已经实现的能力。**

1. **优先一个“最终结构变更”内核。**静态 Bundle、显式 EntityValues 和未来系统包装复用同一差异计划；明确加入/替换/移出/丢弃四类所有权。静态调用可对标 hecs exchange，系统包装再提供 pi_world 风格作用域提交。若以后要连续编辑合并，应作为独立新契约，不能用现有 Alter 名称暗示已有该功能。
2. **先定重复/交集/缺失规则。**重复类型拒绝；A/D 同类型推荐明确“移出旧值再加入新值”或直接拒绝；缺失删除区分宽松 remove 和严格 take/exchange。替换旧值只能析构一次，返回移出值则由调用方拥有，不能既返回又析构。
3. **计划按源/最终原型和实际存储来源分路。**固定注册表下缓存结构差异，并区分 Column→Column、Column→Sparse、Sparse→Column、Sparse→Sparse。相同 Table 的纯稀疏变化保留 table_row；仍维护 archetype_row 与原型成员，不能省掉逻辑发布。计划不长期缓存可能失效的实体行号或裸组件地址。
4. **清楚定义两个提交形态。**独占 World 调用返回即完整发布；系统作用域包装在确定边界统一发布；settle 只填洞。原型成员、Table 行、SparseSet 拥有值及身份位置必须共同达到可观察一致，尤其多个原型共享 Table 时不能照搬 pi_world“一原型一 Table”。
5. **沿用项目的失败约束，但保住展开内存安全。**普通准备错误发生在认领和搬值前；执行/析构失败停止模拟，不承诺恢复/回滚。仍要追踪哪些槽已初始化、哪些值已移出/退休，确保展开时不读未初始化值、不二次析构。这不要求重新引入已否定的事务对象或“捕获 panic 后继续 World”。
6. **安全借用必须覆盖原位分支和 destroy。**未来 View/Query 活借用与结构编辑应有排他协议；若允许“迭代时请求变更”，让请求与真正写源/析构的时点分离，或使用限制结果存活的借出式接口。不能仅靠调度器禁止其他 system 就认为同一迭代器内部没有别名。
7. **后续验证建议：**用 Drop 计数覆盖已有类型替换、同类型交换、A/D 交集、重复类型、缺失删除、alter 后 destroy、忘记/取消编辑；测试提交前后可见性与同时维护两种行的定位。性能基准分别测冷/热计划、bundle 对单项、exchange 对联合变更、同 Table 稀疏变化、多源映射、多实体按列搬移、洞比例与 settle 成本，并记录峰值内存和析构成本。不要仅测一条热路径后宣布某库最快。

## 9. 一手证据索引

以下官方链接均固定到指定版本；本地链接供定位，精确证据以标注行号为准。

### Bevy 0.20.0

- [B1] [EntityWorldMut rustdoc](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/world/struct.EntityWorldMut.html)；[insert 与实际调用链 world_mut.rs:997–1103](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/world/entity_access/world_mut.rs.html#997-1103)；[take/remove:1269–1369](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/world/entity_access/world_mut.rs.html#1269-1369)。
- [B2] [Edges 定义、插入/删除/移出缓存 archetype.rs:195–322](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/archetype.rs.html#195-322)；[删除缓存及严格 take 缺失检查 bundle/remove.rs:391–444](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/bundle/remove.rs.html#391-444)。
- [B3] [BundleInserter 目标/同 Table 分流 insert.rs:55–104](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/bundle/insert.rs.html#55-104)；[成员变化与 Table 迁移分支:197 起](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/bundle/insert.rs.html#197)；[BundleRemover 新旧 Table 判断 remove.rs:90–105](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/bundle/remove.rs.html#90-105)；[SparseSet 删除及 Table 行迁移:231 起](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/bundle/remove.rs.html#231)。
- [B4] [Commands 的顺序执行契约](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/system/struct.Commands.html)；[EntityCommands::insert queue commands/mod.rs:1410–1413](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/system/commands/mod.rs.html#1410-1413)；[remove queue:1701–1704](https://docs.rs/bevy_ecs/0.20.0/src/bevy_ecs/system/commands/mod.rs.html#1701-1704)。
- [B5] [modify_component](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/world/struct.EntityWorldMut.html#method.modify_component)、[move_components](https://docs.rs/bevy_ecs/0.20.0/bevy_ecs/world/struct.EntityWorldMut.html#method.move_components) 及同页方法表。

### hecs 0.11.2

- [H1] [World rustdoc](https://docs.rs/hecs/0.11.2/hecs/struct.World.html)；[insert/insert_one world.rs:621–767](https://docs.rs/hecs/0.11.2/src/hecs/world.rs.html#621-767)；[remove/remove_one:769–859](https://docs.rs/hecs/0.11.2/src/hecs/world.rs.html#769-859)。
- [H2] [World edge 字段:42–51](https://docs.rs/hecs/0.11.2/src/hecs/world.rs.html#42-51)；[insert_inner、Guard、key 缓存与最终行分配:633–755](https://docs.rs/hecs/0.11.2/src/hecs/world.rs.html#633-755)；[remove_target:831–852](https://docs.rs/hecs/0.11.2/src/hecs/world.rs.html#831-852)；[exchange/exchange_one:861–901](https://docs.rs/hecs/0.11.2/src/hecs/world.rs.html#861-901)。
- [H3] [CommandBuffer rustdoc](https://docs.rs/hecs/0.11.2/hecs/struct.CommandBuffer.html)；[记录 insert/remove/queue command_buffer.rs:74–126](https://docs.rs/hecs/0.11.2/src/hecs/command_buffer.rs.html#74-126)；[run_on 顺序执行与清理:144–239](https://docs.rs/hecs/0.11.2/src/hecs/command_buffer.rs.html#144-239)。

### Legion 0.4.0

- [L1] [Entry rustdoc](https://docs.rs/legion/0.4.0/legion/world/struct.Entry.html)；[add_component/remove_component entry.rs:452–513](https://docs.rs/legion/0.4.0/src/legion/internals/entry.rs.html#452-513)。
- [L2] [World::extend world.rs:236–257](https://docs.rs/legion/0.4.0/src/legion/internals/world.rs.html#236-257)；[entry:294–311](https://docs.rs/legion/0.4.0/src/legion/internals/world.rs.html#294-311)；[transfer_archetype 与 layout 索引:364–430](https://docs.rs/legion/0.4.0/src/legion/internals/world.rs.html#364-430)。
- [L3] [CommandBuffer rustdoc](https://docs.rs/legion/0.4.0/legion/systems/struct.CommandBuffer.html)；[writer 调 Entry command.rs:187–224](https://docs.rs/legion/0.4.0/src/legion/internals/systems/command.rs.html#187-224)；[FIFO flush、exec_mut:281–307](https://docs.rs/legion/0.4.0/src/legion/internals/systems/command.rs.html#281-307)；[extend/add/remove 命令:329–372](https://docs.rs/legion/0.4.0/src/legion/internals/systems/command.rs.html#329-372)。

### pi_world 本地 0.2.17

本地引用根为 D:/dev/pi_world；附加依赖源码根 D:/dev/pi_append_vec。不是把 docs.rs 上同版本号当成本地修订证明。

- [P1] [world.rs](../../pi_world/src/world.rs):329–360，make_query/make_alter/editor；[alter.rs](../../pi_world/src/alter.rs):155–290，真实方法表、SystemParam、Drop；313–367，直接 get_param、状态与 align。
- [P2] [alter.rs](../../pi_world/src/alter.rs):369–425，find_mapping/原位/mark/alloc/写 A/登记 moves；439–449，A/D 排序；472–564，clear、取消目标新增、按列搬移、删除析构与记录；566–620，映射缓存与 alter_row；623–672，destroy/check；699–712，dirty 分组；767–800，AlterIter。37–152 为被注释的旧 Alterer。
- [P3] [archetype.rs](../../pi_world/src/archetype.rs):241–313，源类型与增删合并、去重、adding/moving/removing；[world.rs](../../pi_world/src/world.rs):590–629，目标原型查找/创建/通知/初始化；638–640，地址替换；789–819，mark 编解码。
- [P4] [insert.rs](../../pi_world/src/insert.rs):179–203，Bundle、TypeItem 和 write；206–237，tuple Bundle；[macros/src/lib.rs](../../pi_world/macros/src/lib.rs):423–449，派生 Bundle；467–489，派生 Component 的写链。
- [P5] [query.rs](../../pi_world/src/query.rs):498–541，点读源地址与 filter；694–765，迭代与跳空行；[table.rs](../../pi_world/src/table.rs):115–144，alloc/destroy/mark_remove；231–271，阶段整理与位置修复；274–294，最终按存活实体析构。
- [P6] [column.rs](../../pi_world/src/column.rs):350–358，added 标记；428–434，ptr.write 不析构原值；449–468，字节迁移与显式析构。结合 P2/P3/P4/P5 支撑替换值析构缺口的静态判断。
- [P7] [editor.rs](../../pi_world/src/editor.rs):12–27，set_fn 初始化；47–146，动态增删列表与立即 alter_row；256–263，临时 Alter 包装；不是连续调用最终合并器。
- [P8] [Cargo.toml](../../pi_world/Cargo.toml):58–61，pi_append_vec/pi_arr 本地 patch；[pi_append_vec/src/lib.rs](../../pi_append_vec/src/lib.rs):79–82，alloc 的实际长度/槽位分配顺序。

**最终判断：**借鉴 Alter 的“联合结构计划、源查询稳定、作用域提交、集中填洞”有价值；把它称为其他 ECS 都没有的“一次迁移编辑器”不符合源码。对 pi_ecs 更稳妥的目标是吸收这些分层机制，同时保留 hecs 式集合交换的所有权清晰度和 Bevy 式同 Table 免搬移路径，并独立闭合本项目混合存储的安全协议。
