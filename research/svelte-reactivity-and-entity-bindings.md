# 借鉴 Svelte 5：游戏状态到本地渲染状态的绑定与回滚边界

## 1. 结论先行

**可以借鉴 Svelte 的「响应式源 → 派生投影 → 渲染副作用」，但不能把它理解成「两边没有引用」或「框架使用弱引用，所以跨回滚计数自然安全」。**

针对“两者之间是否建立引用关系”，必须拆成三种关系：

1. **更新依赖关系**：Svelte 会记录哪些响应式源被派生计算、effect 读取，从而使源变化能够触发更新。这个关系确实存在。[S2][S3]
2. **数据对象引用关系**：是否共享对象取决于投影表达式。返回源对象，就可能持有同一对象；构造只含值字段的新对象，才可以在这些字段上脱离源对象引用。Svelte 明确说明，$derived 不会自动把结果转换成深层响应式代理，也不会自动复制结果。[S2]
3. **Entity 生命周期计数关系**：JS 对象引用和响应式依赖不等于 Rust Entity 的 strong/weak 计数。要避免渲染层的跨回滚 weak 计数问题，需要在架构上取消渲染层对可回滚 Entity 计数对象的长期持有，改用值投影和经校验的身份键。这是本报告的设计建议，不是 Svelte 已经实现的 ECS 功能。[S6][S7]

**推荐方向：游戏状态可回滚；本地渲染状态由它单向投影，在回滚切换后重新协调。渲染层长期保存值、逻辑身份和本地资源，而不保存旧世界的 Entity weak handle。** 这能消除“渲染绑定导致的跨回滚弱引用增减”这一类问题，不能据此宣称 ECS 内部所有引用计数问题已解决。

## 2. 研究范围与证据口径

研究对象为 Svelte 5（用户原拼写“Sevtle”）。API 语义以本次读取的官方文档为准；用于解释引用图和重绑机制的源码固定在 **svelte@5.25.0** 标签，避免将随 main 变动的内部实现当作稳定 API。文档可能包含该标签之后增加的行为，本文不以旧源码证明新功能。[S1–S5][C1–C3]

下文使用三类标识：

- **事实**：文档明确声明或指定版本源码可直接观察。
- **推导**：由已核实语义推出的例子和边界；示例未在本项目中运行。
- **建议**：面向 ECS 回滚的可借鉴方案，不是 Svelte 或当前项目已经具备的能力。

本次没有审计本项目 Entity、快照、回滚或计数实现；因此涉及“计数回滚”的具体故障模型均为条件性分析，不认定本项目已经采用某种实现。

## 3. Svelte 实际建立了什么关系

### 3.1 $state proxy：拦截访问，不是弱 Entity 句柄

**事实。** 对数组和简单对象使用 $state，会得到深层响应式 proxy。读取、写入属性会经过代理逻辑，使更新可以细化到具体属性；递归代理遇到非简单对象时停止，类实例不会自动被代理。[S1，Deep state / Classes]

官方还特别指出：通过 proxy 更新属性时，原始对象不会被修改。指定版本源码中，proxy 有属性到信号的普通 Map，属性读取经相应信号取得值，写入更新相应信号；代理仍围绕目标对象工作。这说明“响应式代理”不等于“每次写回原始对象”，也不等于“没有对象引用”。[S1][C1]

**边界。** 不能从上述行为推出：

- 包一层 $state 就完成任意对象图的深拷贝、序列化或回滚隔离；
- 外部 Rust/WASM 世界内存变化会被 JS proxy 自动观察；
- 类实例、原生 Map 的内部变化都会像普通对象属性一样被追踪。

类实例代理边界已有文档说明；Svelte 另行提供 SvelteMap 等响应式内建类型，且 SvelteMap 的值也不会自动变成深层响应式对象。外部事件系统则有显式适配 API createSubscriber。[S1][S5]

### 3.2 $derived：记录读取依赖，结果可能仍是原引用

**事实。** $derived / $derived.by 把表达式同步读取的响应式值视为依赖；依赖变化后将派生值标为 dirty，在下一次读取时重新计算。表达式应当无副作用。[S2，Understanding dependencies]

**推导。** 假设 row 是响应式状态中读取到的对象，以下表达式的数据引用关系不同：

- $derived(row) 返回 row 当前的值；当它是对象时，不会自动克隆。
- $derived({ x: row.x, y: row.y }) 在求值时创建新对象；若 x/y 是数值，这两个字段没有指向 row 的对象引用。
- 若写成 $derived({ position: row.position })，position 仍可能是共享对象引用。新建一层外壳不等于彻底解耦。

这是由“derived 值保持原样”的文档语义推导的；官方示例更直接地说明：从深层响应式 items 中派生 selected = $derived(items[index]) 后，修改 selected 的属性会影响 items 内的对应对象。[S2，Deriveds and reactivity]

**因此，“渲染状态由游戏状态派生”不代表“两者只剩值关系”。必须检查投影输出的每个字段是否包含 Entity 句柄、共享对象或捕获旧世界的闭包。**

### 3.3 referential identity：用于传播优化，不是实体身份解析

**事实。** Svelte 采用 push-pull reactivity：状态变化推送失效信息，派生值被读取时再计算。如果派生结果与原结果 referentially identical，跳过下游更新。[S2，Update propagation]

**推导。**

| 派生输出 | 对引用同一性的影响 | 对渲染桥接的意义 |
| --- | --- | --- |
| 布尔值或数值 | 值未变可避免下游更新 | 适合可见性、坐标等细粒度投影 |
| 每次新建的对象 | 即使字段相同，对象身份也不同 | 不会自动得到按字段相等的传播优化 |
| 同一个深层 proxy | 派生结果本身可能同一 | 下游直接读取的响应式属性仍可独立形成依赖 |
| 同一个普通对象，原地改内部字段 | 普通字段不自动成为响应式源 | 不能指望只返回这个对象就观察到外部变化 |

这里的同一性比较不是“这个 Entity 在回滚前后是否仍是同一逻辑实体”。Svelte 不会凭对象字段推断跨世界、跨分支实体身份；这种业务身份需要适配层定义。[S2][S5]

### 3.4 源码证据：图中存在普通 JS 引用，不能说全是弱引用

**事实，限 svelte@5.25.0。**

- runtime.js 的 get 在活动 reaction 中读取信号时收集依赖；update_reaction 比较本次读取和旧依赖，移除不再需要的关系，并登记新关系。
- 依赖以 reaction.deps 数组记录，反向关系以 dependency.reactions 数组记录；代码直接向数组放入对象，并非以 WeakRef 包装这些边。
- deriveds.js 创建的派生节点含 fn、deps、reactions、v、parent 等字段，update_derived 保存计算结果到 v，并按相等判断更新版本。[C2][C3]

**推导。** 至少这些被核查的路径包含普通对象引用和缓存值；它们能参与 JS 对象的可达性。依赖图清理与作用域销毁有实际意义，不能因为 JS 有 GC 就认为旧状态不会被持有。

**证据限制。** 本报告没有证明 Svelte 全仓库绝无 WeakRef，也没有声称它全局不存在强引用。这里能够确认的是：**被检查的核心依赖登记不是“以 WeakRef 代替所有引用”的机制。** 不应把代码中的 WeakSet（例如错误去重用途）误解成整个响应式图使用弱引用。[C2][S6]

## 4. 整体替换对象后，依赖怎样重绑

### 4.1 关键不是“旧指针自动迁移”，而是“重新读取当前源”

以下为语义示意，可放在受 Svelte 编译的组件脚本中；不是 ECS 实现，也不是本次运行过的测试：

```svelte
<script>
  let game = $state({ player: { x: 1, y: 2 } });

  // 每次求值都从当前 game 入口读取
  let player = $derived(game.player);
  let position = $derived({ x: player.x, y: player.y });

  // 只在初始化时读取一次，保存旧对象
  const oldPlayer = game.player;

  function replaceGame() {
    game = { player: { x: 10, y: 20 } };
  }

  $effect(() => {
    console.log(position.x, position.y);
  });
</script>
```

**推导。**

1. 为 game 重新赋值改变响应式入口，使读取 game 的派生计算失效。
2. 当 player / position 被重新读取时，计算沿当前 game 访问新对象；依赖集合随重新执行更新。
3. oldPlayer 仍然是旧对象引用，不会因为变量 game 被重赋值就自动指向新 player。
4. 旧对象是否可回收取决于是否还有可达引用；闭包、缓存、历史快照都可能继续持有它。依赖解绑不等于立即释放所有旧对象。[S1，Passing state into functions / Deep state][S2][C2][S6]

普通解构也只在当时取值，不会自动跟随后续替换；若希望解构变量保持派生关系，应使用 $derived 解构声明，或使用每次读取当前入口的 getter / 访问函数。[S1][S2，Destructuring]

### 4.2 对整体快照发布的启示

**事实。** $state.raw 不建立深层响应式对象更新机制；要让变化触发更新，应替换整个对象或数组。它允许内部包含另外的响应式状态，因此 raw 也不等于深度不可变。[S1，$state.raw]

**建议。** 游戏世界通过外部桥接发布不可变渲染快照时，可以借鉴“稳定的响应式入口 + 替换入口值”。在 JS/Svelte 场景可考虑 $state.raw(snapshot)；在 Rust 原生渲染场景则只借鉴这个入口模型。两者都需要桥接方显式通知快照切换。

如果把 Rust 世界包装成一个长期不变的对象，而内部世界指针在回滚后悄悄变化，Svelte 不会仅凭这个事实自动发现变化。需要发布新投影、更新被跟踪的 revision，或者用显式订阅适配。**同样的响应式入口，是重新查找当前世界的入口，不应是永久缓存旧世界实体地址的入口。**[S1][S3][S5]

$state.snapshot 提供的是深层 proxy 的静态快照，适合传给不接受 proxy 的外部 API；文档没有把它定义为任意 Rust 世界、GPU 资源或 ECS 生命周期计数的快照协议。不能用它替代 ECS 回滚设计。[S1，$state.snapshot]

## 5. $effect：动态依赖与 teardown 能借鉴到哪里

**事实。** $effect 会追踪函数体同步读取的响应式值，包括同步调用函数中的读取；其依赖取决于上一次实际执行走过的路径。分支不再读取某值后，该值不再是该 effect 的依赖。读取对象本身与读取对象某属性也不是一回事。[S3，Understanding dependencies]

一般 effect 在组件挂载后运行，状态变化后的重跑在 microtask 中批处理，并在 DOM 更新后进行。setTimeout 中、await 之后的异步读取不按普通 effect 同步读取规则追踪。不要把其他语法（如编译器处理的顶层 await 派生表达式）的特殊规则泛化成 effect 的规则。[S2][S3]

**事实。** effect 返回的 teardown 在它重跑前和销毁时执行；父作用域销毁、父 effect 重跑也涉及子 effect 清理。高级 API $effect.root 的作用域不会自动清理，需要调用它返回的销毁函数。[S3，Understanding lifecycle / $effect.root]

**建议。** 将 GPU 对象、订阅、音效实例等视为本地资源：绑定时登记明确的释放动作，在绑定被替换或销毁时执行。不要依靠 GC 终结回调安排决定性的资源回收时刻。[S3][S6]

但 teardown 只约束框架管理的作用域，**并不保证清理函数里拿着的 Entity weak handle 一定属于当前世界**。如果清理代码跨过回滚边界后才 decrement 已恢复的计数，绑定清理得再完整也不能自动修正计数账本。需要取消这种持有，或明确规定计数对象所在的回滚域与生命周期协议。这是 ECS 侧设计问题。

官方建议用 $derived 表达派生值，避免用 effect 在两份状态之间反复同步。可借鉴的分工是：**纯投影描述“应渲染什么”；effect 或渲染协调器执行“如何操作本地资源”。**[S3，When not to use $effect]

## 6. keyed each：稳定身份不依赖对象地址

**事实。** keyed each 的 key 必须唯一标识列表项；Svelte 据此插入、移动和删除项。key 可以是对象，但官方推荐字符串或数字，因为对象本身被替换时，这类 key 仍能保留身份。[S4，Keyed each blocks]

```svelte
{#each renderRows as row (row.renderKey)}
  <RenderItem data={row} />
{/each}
```

**推导。** 同一批协调更新中，重建 row 对象不一定意味着重建对应渲染实例；只要 key 身份相同，框架可以按该身份匹配。反之，用 row 对象自身作 key，会使新对象具有新身份。key 保留实例也不保证组件内部一次性保存的旧 data 别名会自动刷新，应让读取路径跟随当前 props / 派生值。[S1][S2][S4]

**限制。** 稳定 key 不是跨任意历史的实例复活机制：某项已从列表中移除并完成销毁后，再出现相同 key，不能凭 keyed each 推出旧组件及旧本地状态会复活。需要跨消失阶段保留资源时，必须另设有界缓存和明确回收策略。[S3][S4]

**建议。** 游戏里的 renderKey 表达“应保留本地渲染连续性的逻辑身份”，不要直接采用内存地址或数组 index：

- 只用可复用槽位 index，可能把槽位后来创建的另一实体误认成旧实体。
- index + generation 只有在其生成规则能区分需要区分的实体时才够用；如果 generation 也随世界回滚，分支重放可能重用相同组合。
- 必须定义：回滚重演出的实体是要沿用同一逻辑身份，还是视为新实体。可以采用稳定逻辑 ID，或在需要区分分支时加入不随模拟回滚的分配域/出生身份。
- 不宜机械地把每次快照 revision 加进 renderKey，否则每次发布都改变 key，会破坏原本希望保留的本地连续性。

**区分两个概念：renderKey 管实例连续性；publication token 管某次异步结果是否仍可提交。** 它们可以包含相关信息，但不应默认是同一个东西。上述身份和防陈旧规则属于 ECS 适配方案，不是 Svelte 提供的 Entity 代际保证。

## 7. 为什么 JS GC 与 Rust Entity strong/weak 不是同一种关系

| 关系 | 已核实语义 | 对回滚的含义 |
| --- | --- | --- |
| 普通 JS 对象引用 | 普通 Map 等容器能使其键和值继续存活；闭包等也可能保留对象。回收由 GC 决定。[S6] | 不存在“Svelte 的图天然不持有数据”的前提 |
| JS WeakRef | 弱引用本身不足以保活目标；目标何时被收集、何时 finalizer 执行没有及时性保证。[S6] | 即使采用，也不能让资源清理或世界回滚得到确定性时序 |
| Rust 标准库 Rc / Arc 类所有权 | Rc clone 指向同一分配；最后一个 Rc 销毁时内部值被 drop；Arc 是多线程原子引用计数对应类型。[S7] | 是具体分配的所有权机制，不会按逻辑 ID 自动迁移到另一世界分配 |
| Rust 标准库 Weak | 不保活内部值，但保留 backing allocation；upgrade 在值已经 drop 时返回 None。[S7] | “不保活值”不等于没有计数和分配生命周期，也不意味着可跨快照重新定位 |
| 自定义 Entity strong/weak | 需要由 ECS 契约定义；不能只因名称相同就等同 std::rc::Weak | 是否阻止槽位复用、clone/drop 修改哪里、是否进入快照，必须单独决定 |

**条件性故障模型。** 若一个 ECS 把 weak 计数纳入可回滚状态，而渲染层句柄不随它一起回滚，那么“计数的历史”与“外部句柄的 clone/drop 历史”可能分离。例如恢复的计数没有包含某次外部持有，而该句柄稍后释放仍会扣减。反过来，恢复出的计数也可能对应已经释放的外部持有。这是两个生命周期域不一致的问题，不能用响应式依赖图直接证明安全。

如果自定义句柄的计数保存在不可回滚的独立控制块中，问题又不同，需要证明旧分配如何退出、实体身份如何解析；不能将上面的假设直接套用。Svelte 文档不回答这些 ECS 实现问题。

## 8. 可借鉴方案：按身份投影，当前世界解析，本地资源独立

以下整节为**建议**。Svelte 为它提供的是“派生、重新读取、稳定 key、作用域清理”的设计参照，不提供现成回滚安全协议。

```text
可回滚游戏世界
  └─ 在一致的读取边界生成值投影
       └─ 发布当前 RenderSnapshot + publication token
            └─ 按 renderKey 协调本地 RenderState
                 └─ 更新/创建/释放本地 GPU、音效与 UI 资源

交互命令携带逻辑身份
  └─ 提交时向当前世界重新解析并校验
```

### 8.1 渲染侧长期保存什么

- renderKey、逻辑实体 ID 等值标识；缺失/不可解析是正常结果。
- 坐标、朝向、颜色、可见性等渲染所需值；复杂数据必须明确拷贝、不可变共享或独立资源所有权边界。
- 本地插值状态、GPU handle、播放实例等渲染资源，生命周期由渲染协调器管理。

不长期保存指向可回滚 Entity 控制块的计数句柄，也不保存捕获旧 Entity 句柄的订阅闭包。**普通 ID 不是 Rust Weak：它不自动增减目标计数，也不承诺目标存在。** 它可能成为陈旧标识，因此每次需要回到模拟层时必须通过当前世界解析。

### 8.2 回滚发布与重新协调

```text
onRollbackComplete(currentWorld)
  在一致读取边界生成当前投影
  生成新的 publication token
  发布投影和 token
  按 renderKey 与本地实例集合协调
    匹配项：应用新值，按规则保留或重置插值
    新项：创建本地实例
    缺失项：清理或进入明确的短期淡出策略
  拒绝旧 token 的异步回写
```

publication token 应能区分回滚前后的发布，不能只依赖也会倒退、重复的模拟 tick。稳定 renderKey 可跨快照不变，而 token 随发布更新。读取过程中世界不能同时被替换到一半；使用快照、读取阶段或其他一致性边界是模拟/渲染桥接方的职责。

如果采用变化集而非全量投影，回滚切换必须有 reset/reconcile 或等价协议：不能默认旧世界的增量订阅和变更游标还有效。Svelte 的“重新执行时更新依赖”启示的是刷新当前读取关系，不是保留旧地址继续读。

### 8.3 这样能避免什么，不能直接解决什么

| 问题 | 此方案的效果 |
| --- | --- |
| 渲染绑定对可回滚 Entity weak 计数的长期增减 | 若边界确实不含这种句柄，则消除此来源 |
| 回滚后仍访问旧世界 Entity 地址 | 通过当前世界解析和值投影避免长期旧地址依赖 |
| 新快照对象替换导致所有本地实例重建 | 可用稳定 renderKey 做匹配 |
| 实体消失后资源不释放 | 明确差异协调与 teardown 处理；过渡/缓存另定寿命 |
| 旧异步结果污染新世界视图 | 用 publication token 校验提交 |
| ECS 内部引用计数、身份冲突、快照一致性 | 仍需 ECS 自己的契约与正确实现 |
| 确定性、网络回滚算法、插值跳变 | 不由 Svelte 响应式自动解决，需要独立策略 |

如果业务必须让渲染层阻止某实体或资源回收，就已经引入跨域生命周期需求，不能再声称“渲染层完全不参与保活”；需要明确的 lease / pin / 独立资源所有权协议，并证明它与模拟回滚的关系。这是选择该需求后的设计工作，而非 Svelte WeakRef 的推论。

## 9. 最终回答

**两者会不会建立引用关系？** 会建立响应式更新依赖；是否共享游戏数据对象由投影决定。Svelte 不自动把一切派生结果复制成独立渲染值。

**整体替换后能否重新绑定？** 可以让读取当前响应式入口的派生计算和 effect 在更新执行时重建依赖；旧别名、旧闭包、外部非响应式内存不会自动重定向。

**能否避免 ECS 跨回滚弱引用计数问题？** 可以借鉴其分层思想，通过“值投影 + 稳定逻辑身份 + 当前世界解析 + 明确本地清理”移除渲染层对可回滚 Entity weak 计数的依赖。**真正起作用的是生命周期解耦，不是 Svelte 使用了某种神奇弱引用。**

## 一手来源

### Svelte 官方文档

- [S1] [$state](https://svelte.dev/docs/svelte/$state)：Deep state、Classes、$state.raw、$state.snapshot、Passing state into functions / across modules。本文读取的[纯文本入口](https://svelte.dev/docs/svelte/$state/llms.txt)。
- [S2] [$derived](https://svelte.dev/docs/svelte/$derived)：Understanding dependencies、Deriveds and reactivity、Destructuring、Update propagation；包括 selected 共享底层对象及 referential identity 说明。
- [S3] [$effect](https://svelte.dev/docs/svelte/$effect)：Understanding lifecycle、Understanding dependencies、$effect.root、When not to use $effect。本文读取的[纯文本入口](https://svelte.dev/docs/svelte/$effect/llms.txt)。
- [S4] [{#each ...}](https://svelte.dev/docs/svelte/each#Keyed-each-blocks)：唯一 key、列表协调、推荐字符串/数字以跨对象替换保留身份。本文读取的[纯文本入口](https://svelte.dev/docs/svelte/each/llms.txt)。
- [S5] [svelte/reactivity](https://svelte.dev/docs/svelte/svelte-reactivity)：SvelteMap 的响应式边界、createSubscriber 外部事件适配和清理。本文读取的[纯文本入口](https://svelte.dev/docs/svelte/svelte-reactivity/llms.txt)。

### 语言与内存语义

- [S6] [TC39 WeakRefs 官方提案说明（已达 Stage 4）](https://github.com/tc39/proposal-weakrefs/blob/master/README.md)：Weak references、A note of caution、Finalizers；明确普通 Map 的保活行为、WeakRef 不足以保活目标、GC 和 finalizer 时序的不确定性。本报告以其作为委员会一手解释材料，而非把提案示例当作 Svelte 实现。
- [S7] [Rust 标准库 std::rc](https://doc.rust-lang.org/std/rc/index.html)：Rc clone/drop、Weak downgrade/upgrade、backing allocation、循环引用以及 Arc 的定位。仅用于比较标准库机制，不替代自定义 Entity 契约。

### 固定版本源码：svelte@5.25.0

- [C1] [proxy.js](https://github.com/sveltejs/svelte/blob/svelte%405.25.0/packages/svelte/src/internal/client/proxy.js)：proxy 的对象类型过滤、属性信号 Map、get/set traps 和目标对象关系。
- [C2] [runtime.js](https://github.com/sveltejs/svelte/blob/svelte%405.25.0/packages/svelte/src/internal/client/runtime.js)：get、update_reaction、remove_reaction / remove_reactions、update_effect；依赖收集、旧关系移除、新关系登记，以及重跑前 teardown。
- [C3] [reactivity/deriveds.js](https://github.com/sveltejs/svelte/blob/svelte%405.25.0/packages/svelte/src/internal/client/reactivity/deriveds.js)：derived 节点字段、execute_derived 经 update_reaction 求值、update_derived 的缓存与版本更新。

内部函数名只用于定位以上已读取版本的证据，不作为应用代码可调用的公共 API，也不保证未来版本维持相同实现。

[S1]: https://svelte.dev/docs/svelte/$state
[S2]: https://svelte.dev/docs/svelte/$derived
[S3]: https://svelte.dev/docs/svelte/$effect
[S4]: https://svelte.dev/docs/svelte/each#Keyed-each-blocks
[S5]: https://svelte.dev/docs/svelte/svelte-reactivity
[S6]: https://github.com/tc39/proposal-weakrefs/blob/master/README.md
[S7]: https://doc.rust-lang.org/std/rc/index.html
[C1]: https://github.com/sveltejs/svelte/blob/svelte%405.25.0/packages/svelte/src/internal/client/proxy.js
[C2]: https://github.com/sveltejs/svelte/blob/svelte%405.25.0/packages/svelte/src/internal/client/runtime.js
[C3]: https://github.com/sveltejs/svelte/blob/svelte%405.25.0/packages/svelte/src/internal/client/reactivity/deriveds.js
