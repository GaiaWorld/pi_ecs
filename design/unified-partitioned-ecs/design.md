# pi_ecs 持续设计稿

实现根：D:/dev/pi_ecs。当前为 zcode 设计期：声明、薄委托、中文契约及明确失败桩，不实现业务、不提交/发布。源码是具体契约单一来源；本文只保存需求、取舍和跨模块协作。旧设计归档 ../../archive/dev-design-lite/unified-partitioned-ecs/ 不继续维护。

检查命令：cargo fmt --all --check；cargo check --workspace --locked --all-targets --message-format=short；cargo doc --workspace --locked --no-deps。每轮以 checkRound 证据为准，编译仅验证设计声明。原有 pi_slotmap 66 条警告未压制，源码壳有未使用警告，不运行触发 todo 的业务测试；尚未独立轻审，未授权实现。

## rc / 非 rc 统一基准

| 项目 | rc | 非 rc |
| --- | --- | --- |
| 线程模式 | 单线程，不因 wasm 自动选择 | 多线程能力，串行执行也保持此模式 |
| 数值同步 | pi_share::ShareU32/U64/Usize 的非原子 AtomicCell | 同名 Share 类型对应标准原子 |
| 稀疏一级 | 本库 Vec | 必须 pi_buckets::Pages |
| 二级/列存储 | pi_arr 连续 RawArr，增长可搬移，结束旧借用并重绑 | pi_arr 分段 RawArr，普通增长保持旧值地址 |
| 域条目创建 | 按需 | 按需；同条目冲突由调用者/ECS 排除，不另加域初始化锁 |
| owner/下标 | ShareU64/ShareU32，null/空码相同 | 同名类型，Release 发布、Acquire 读取 |
| 删除 | 立即填 owner 洞并修下标，不搬移组件值 | 失效并追加退休紧凑下标，整理前旧洞不复用 |
| compact | 空操作 | 收束写者/借用后填洞、修映射并清退休列表 |
| 同位置冲突 | 调用者/既有访问管理 | 相同，底层不重复仲裁 |

优先复用 pi_share 的 Share 数值/指针/所有权/同步原语，不自行制造原子与普通值的运行时 enum。Share 类型仅切换基础能力，不消除算法差异，不凭容器线程安全推导组件访问权限。事件通知、节点队列、命令入口后续遵循同一原则：只共用有真实复用的分页、追加、发布与收束能力，保留各自通知游标/旧值析构、节点拥有责任等语义。不得以 ShareMutex 绕过已确认无结构锁的要求。rc 必须由 workspace 对外统一 feature 贯通 pi_share/pi_arr/各 ECS crate；未贯通前不宣称支持 rc，检查须分别覆盖默认与 rc，Cargo feature 合并可能使全图单线程。ThreadSend/ThreadSync/Share 仅在具体线程/外逃契约核对后逐项采用，不全局机械替换 Arc/Send/Sync。

## 并发核心原则

为支持并发，World 的大部分操作以 &World 为基础。共享接收者不等于只读，也不自动授权任意修改：内部可变性、原子/容器同步、参数访问声明与运行/维护门禁共同保证安全，禁止把共享引用强转整个 &mut World。SystemParam 在执行期从共享 World 与受限授权临时构造访问对象，长期存储不保存本次授权。现有部分 &mut World 壳是待迁移声明，不能据此推导整体串行；只对已闭合入口逐项更新。

## 目标及范围

游戏 World 与渲染 World 分离，均默认单分区，支持固定多分区、确定性身份和自动迁区。游戏 World 整体快照恢复，渲染通过类似 Svelte 的反应式投影桥接。服务器权威持续运行、客户端尽力预测，分歧交宿主校正，不扩网络关注区，不把 ECS 错误当自动重拉基线。

| 要求 | 可观察验收 |
|---|---|
| 双 World 桥接 | 游戏可回滚，渲染独立；通过稳定 key/值投影和净差异协调，不共享拥有型实体句柄 |
| 独立分配 | 其他来源域创建/删除不改变本域有序操作得到的 ID；完整键拒绝普通旧代际 |
| 立即确定身份 | 创建返回持最终键的强引用；最终槽回收后 LIFO 复用，失败允许消耗代际 |
| 迁区 | 身份、集合、值和变化元数据保持，源/目标只有一个有效当前位置 |
| 恢复范围 | 整个游戏 World 及分配状态，另一个渲染 World 不被恢复，不做任意分区子集回退 |
| 历史独立 | 后续修改不污染历史，同快照可重复恢复到兼容实例，不要求序列化 |
| 派生恢复 | 未保存组件先有合法初值，资源依规则替换/清旧/保留，生产者先于消费者 |
| 失败保证 | 准备失败不安装半状态；执行搬移或恢复安装/重建失败停止，不事务回滚 |
| 通知 | 恢复不逐实体发普通增删通知，但析构/内部修复不可省略 |
| 规则生命周期 | 历史期间规则固定，宿主变更时清不兼容历史并重建基线 |
| 确定维护 | compact/截尾按同步模拟顺序重演；shrink_to_fit 可本地进行，截尾不保证旧键永不重现 |

## Workspace

Rust 2021，九包，均 publish=false。根 pi_ecs 仅重导出和 prelude；types 基础定义、storage 物理存储、command 队列服务；core 数据/查询/类型化通知与 World 集成；schedule 系统参数/调度/消息/完整恢复；app 组装；partition 普通自动判区系统；macros 暂无派生入口。宏在对应手写 trait、生命周期和展开目标稳定后逐项补齐，不必等待全部 core 实现完成；开发时必须参考 Bevy 对应宏的源码、展开结构及诊断/编译测试，记录参考版本，适配本项目契约而非照搬安全假设。依赖向下，core 不依赖调度；芯不得调用其他芯。

依赖：本地 pi_key 0.4.0、pi_slotmap 0.5.0（Cargo.lock 固定解析但 path 本身可变）；crates.io pi_arr =0.33.0、pi_buckets =0.5.0、pi_share =2.1.0，不用本地 path；papaya =0.2.5。参考文献在 ../../research/，研究反映固定来源，项目接口以当前壳为准，不以源码分支数推断性能。

## App 与插件

一个 App 对应一个 World，内部稳定 Arc 保活；目标由 App 持有 Stage，Stage 持有系统状态，命名 Schedule 引用动作/子流程；现存持有关系是待替换临时壳。客户端由宿主组合游戏 App、渲染 App 及桥接，服务器仅游戏 App。无固定 update，不因登记调度自动运行或接图；宿主显式运行启动、模拟、表现和渲染，回滚继续由 GameRecovery 协调，不自动重演外部副作用。

App::new(world) 进入构建，类型化 ScheduleLabel 标识进程内调度；字段私有，通过 add_schedule/schedule_mut/world/world_mut 配置。add_plugin 立即 build，具体类型唯一，可装子插件，依赖 require_plugin 显式要求已成功安装，不自动解依赖。finish 按成功安装顺序调用插件 finish，补连接并统一验证后冻结，不执行启动系统；Ready 再次 finish 幂等。finish 期间不能装插件或重入运行。前置拒绝不改登记，部分构建/不可恢复执行失败停止，不承诺事务；不支持卸载、热插拔或解冻。

Plugin 只 build/finish，无每帧行为，配置可有字段，运行数据归资源/组件/Local。系统配置变化需清历史重组装。具体契约见 app/{app,plugin}.rs。Arc 独占配置及稳定绑定发布后的维护/恢复入口仍待闭合；不公开可替换 Arc，不以独占 App 推导独占 World，不增加公开 Builder/RunSession 或用不安全强转绕过。插件回调借用、类型去重的内部表示待细化。

## World 关闭

关闭先拒绝运行/维护和命令提交，收束任务/参数/guard；释放系统参数状态，再整体清理业务值（组件、资源、通知旧值、payload 等）及剩余节点，最后释放身份控制和底层分配。不等待强计数归零，强环也整体释放。清理模式不递归按归零再销毁，不发普通业务通知，各值一次析构；不保证业务值间 Drop 顺序，Drop 不访问其他业务数据或发起新工作。控制数据和节点池必须活到最后一份可能使用它们的拥有值释放。关闭实现和外部安全句柄接口仍待细化，见 core/destruction.rs。

独立 Sender 可晚于 World 存活：关闭接受截点前成功项由 World 清理，截点后拒绝并返还 payload。临时 OwnedEntity 以生命周期绑定 World，不能晚于 World 释放；内置记录由 World 特殊释放。已放弃独立 Arc 控制块及无生命周期外逃句柄。恢复/关闭须等待临时引用结束，具体授权接线待闭合。

## 宿主读写访问授权

World::write/write_scope 提供宿主独占组件访问 WorldWrite，冲突报错不等待；与 pin/运行/其他写/维护互斥，Drop 归还，forget 只泄漏授权。get/get_mut/query_one_mut 只访问组件，不开放结构维护，先释放写授权再维护。ComponentAccess 与 QueryData::Item 已分离组件借用期和拥有结果有效期，宿主 OwnedEntity 可在 pin/write 释放后保留，系统不得越过运行授权；现有系统短借用壳保守使用相同寿命。world.rs 仅编译样例覆盖真实 write→pop→释放授权→结构修改，以及作用域返回拥有引用。实际门禁/guard/fetch 未实现。

World::pin 返回显式 WorldPin，read_scope 复用同一读授权；不是 std::pin::Pin，不增实体计数。多个只读 pin 可共存，首版排斥系统运行、组件写、结构维护/销毁/settle/恢复；强计数增减可继续，归零只登记。get 的结果借用 pin，不保留无授权 World::get。Drop 释放授权，panic 展开同样释放，forget 只泄漏授权令冲突持续拒绝，不得提前解锁。OwnedEntity 不等于 pin；系统 Query/Lookup 沿用运行权限，不重复申请宿主 pin。宿主组件写点查已迁入 WorldWrite，资源值访问也已迁入 pin/write，剩余结构入口需检查同一门禁，实际原子表示及全入口授权接线仍未实现。

## World 运行与维护入口

单一 World 内部门禁为 Idle/Running/Maintaining/停止态；根运行与宿主维护排他，冲突前置报错不隐式等待，子 Schedule 复用根授权不重新抢门禁。任务/借用/提交完全收束后回 Idle，不可恢复执行或安装失败停止，不能正常释放成可继续运行的 Idle。门禁不替代组件权限或内部可变性安全表示。

world_mut 仅绑定未发布的构建期允许，Arc 唯一不足以证明无内部绑定。运行后通过具体资源插入/移除、flush_destruction、settle、快照入口取得维护权限，不公开 with_world_mut 或可移动/替换整个 World 的引用；不能共享转可变。系统和宿主复用底层操作壳但授权来源不同。具体维护声明与内部门禁存储接线仍待细化。

App::capture_game/restore_game 仅 Ready，协调全部 Stage，每个 Stage 状态只捕获/恢复一次；委托 GameRecovery 不重复恢复逻辑。校验/准备失败保持状态，开始清旧后失败 App/World 停止，不自动重演或渲染。现存单 Schedule/&mut World 的 GameRecovery 是待迁移壳，需结合动作表接口衔接，不宣称 Arc 下恢复已实现。

## 执行图与动作归属

Stage 是完整收束的系统动作节点，System 实例及 Local/last_run/订阅等状态归一个 Stage。暂不做跨 Stage 实例共享或全局 System 表；函数复用可以创建独立实例。正常模拟/重演通过调用同一 Stage 复用状态。

目标结构为 App 统一拥有 Stage，Schedule 是命名执行图，引用 Stage/子 Schedule；调用关系不复制实例。图由明确的流程所有者组合，插件提供 Stage/子图，必要时经公开扩展点贡献；插件依赖只检查能力，不隐含执行顺序，不依靠零散依赖自然拼图。具体标签/扩展/节点 API 须对接用户执行图库后确定。

阶段内计划负责系统访问依赖及条件执行；外层图负责阶段顺序、分支、循环/子流程，首版阶段不重叠，返回前释放任务/借用并完成提交与销毁。条件不成立不是 ECS 执行错误，不能由选择分支吞掉错误。阶段是否支持跨帧图暂停及保存控制游标仍待协议确认。公开 Executor 不再扩展，线程派发归属随图库能力确定。

源码当前 Schedule 持有 stages/executors、线性 run 及 App 的调度持有关系是待替换临时壳，尚未迁移为最终图接口，不能把编译通过当作图能力已支持。见 schedule/{schedule,stage}.rs 与 app/app.rs。此前线性阶段或固定循环的接口建议不作为正式声明。

## 身份、位置和统一编号

World 非泛型，with_partitions::<N> 配置本 World 固定分区数，默认1，不含全局/本地恢复域。12位分区编码的保留值/最大可配置数待定。固定 EntityStore 按来源域持独立槽/代际/free链，四操作 allocate/locate/relocate/remove。来源域和类别用版本扩展位，DOMAIN_BITS 不是 N。

EntityLocation 三个 u32：高12位分区+低20位 archetype_row，32位 table_row，高16位 table+低16位 variant。所有分区位都用于本 World 分区解释，不再有本地哨兵；保留码、无位置状态及校验方法待闭合。见 crates/pi_ecs_types/src/entity.rs。

每 World 独立登记 Table，一张零号空 Table，冻结后不新增布局。ArchetypeId 仅 table16位/variant16位，variant=0 为基础。register_archetype 最大子集选表，同分先登记优先，不存在推域歧义；register_archetype_on 为本 World 显式选表入口，不改变组合幂等原则。编号不能跨 World 混用，跨 World 转换不是迁区。

各分区独立 Table 和原型成员，成员包含 Entity/table_row。Table 填洞同步修复身份 table_row 及原型成员；成员填洞同步修复身份 archetype_row。精确声明见 crates/pi_ecs_core/src/{archetype,registry}.rs 与 crates/pi_ecs_storage/src/{archetype,storage,table}.rs。

## 内置实体引用与容器：当前确认方案

StrongEntities<N,T>/WeakEntities<N,T> 是内置组件记录，仅保存身份数组，不使用节点池，不持 World/EntityControl，不执行普通 Rust Drop。实体容器头也不持绑定，节点仍由 World 统一池管理；撤销 Arc<EntityControl> 句柄及 ContainerWriter/SlotWriter 的建议。内置类型占组件注册前段，泛型 N/T 对应不同类型，具体容量与冻结登记策略待细化，不能无依据固定保留数量。

spawn 返回 OwnedEntity<寿命>，初始一份强责任；临时引用借用共享 World，Drop 检查完整身份/控制记录并交还责任，归零只登记。容器 push 消费临时引用，pop/remove 返回原责任的临时引用，不重复增减计数，返回值不独占借住容器头。OwnedEntity 不能存入 static 组件；计数检查不能替代 World 存活和授权证明。

Query 的 &mut 内置容器描述返回临时专用访问对象（EntityListMut/EntityQueueMut/EntityDequeMut），fetch 绑定头写借用与共享 World 受限节点/计数通道；不实现到裸头的 DerefMut，不再每次传 owner 或 EntityLinks。普通组件仍返回 Mut。已声明 Component::Access 与框架封闭 ComponentAccess 的 GAT 读写映射；只写一个通用引用 QueryData 实现，不使用重叠特化。普通组件显式选 PlainAccess，不给所有 static 类型 blanket 实现。WorldPin::get 与 WorldWrite::get/get_mut 已采用 ComponentRead/ComponentWrite，Option/元组/Lookup/批量复用映射；实际 QueryData/fetch 仍未实现，专用写对象已绑定共用 ChangeMarker 与独立 marked 状态；实际授权/标记通道证明仍待闭合。

容器无公开 new，通过 EntityValues::empty_list/empty_queue/empty_deque 在 spawn/Alter 中登记创建请求。EntityValues::insert 只接受 Component<Access=PlainAccess>，不能插入内置头；内置类型无公开值构造、复制或移出入口。注册从同一策略生成分类与特殊释放元数据，不构造值。强弱数组初始值通过带寿命的专用请求转移；嵌套无法靠 Rust 自动反射识别。删除时按 ComponentId 查 ComponentInfo，选择普通 drop_value 或具体类型登记的 release_builtin，互斥执行。内置释放函数在登记时绑定 N/T 和布局，不在释放时反射字段或查 TypeId；builtin 分类与函数必须一致。释放时传入受限 World 上下文，组件本身不存绑定。见 core/{owned_entity,entity_container,entity_values,component}.rs。

只读容器提供 len/is_empty/front/iter，遍历按头至尾返回裸 Entity，不增持、不标 Changed、不授权目标组件。迭代器借用访问对象，存活时禁止同对象修改及节点回收；不同容器仍可并行操作。RemovedNotice 的 value 使用 ComponentRead：从独立旧头构造只读容器对象，不按 owner Entity 查询当前组件。实际节点保护期与 Removed 的 World 授权接线仍待闭合。

显式增持：宿主 World::try_acquire、OwnedEntity::try_clone 共用完整身份/状态校验与原子 strong 增持协议，PendingDestroy/Dead 不复活，溢出不回绕。系统通过容器 Ref/Mut 的 front_owned/iter_owned、Deque 的 back_owned/get_owned 对成员增持，不需整个 World；强数组 Ref/Mut 提供 get/get_owned，弱数组提供 get/upgrade。裸读取不计数，owned 新增责任，pop 转移原责任，三者区分。增持不改成员/顺序、不标 Changed，不授予目标组件权限。iter_owned 创建不增持，next 逐项成功后推进，拒绝不跳过当前项、不撤销已有结果，提前结束不处理余项。返回 OwnedEntity 的寿命绑定 World/运行授权而非方法或迭代器短借用；实际授权/原子实现仍待闭合。

数组初始化使用 EntityValues<寿命> 专用请求：strong_entities 消费 N 个 OwnedEntity 并转移原责任，拒绝返回完整输入；weak_entities 借用 N 个强引用准备逐份 weak，失败清理临时 weak 且不消费 strong。不公开临时 WeakEntity；重复目标按份计、N=0 合法。安装前校验登记/目标 World/分支，未安装请求释放剩余责任，安装不重复计数；载体不得擦除为 static。WeakEntitiesMut::replace_from 先取得新 weak 再替换并交还旧 weak，成功标 Changed，同目标也标，前置拒绝不改位置。具体声明见 core/{entity_values,owned_entity}.rs；宿主 change_archetype、AlterState::scope、flush_destruction 已改共享 World 接收者，以内部门禁和结构权限保证排他；OwnedEntity 只保活身份，可跨普通维护，活组件/头/guard 仍阻止冲突操作，恢复/关闭另行排除外部责任。world.rs 中仅编译样例覆盖增持后结构修改/Alter 及 pop 释放头后转入新组件，不运行失败桩；实际门禁、WorldPin 读授权的原子状态与全入口冲突检查及内部可变性证明仍待闭合。

## 内置关系组件变化追踪

专用写对象使用与普通 Mut 相同的 ChangeMarker 和本对象 marked 状态，方法成功修改后返回前按 TrackingDemand 写 tick/追加通知，不在 Drop 或系统末尾延迟标记。只读、空 pop/clear、无效键/越界及前置拒绝不标；非空 clear、成功 push/pop/remove/insert 标记，强弱数组替换同目标也标记。每个访问对象只追加一次 Changed，重新取得可再追加。Changed 指成员/顺序变化，不传播目标组件变化。写入或标记中不可恢复失败停止，不承诺回滚。具体操作契约见 core/{entity_container,owned_entity,change_detection}.rs，实际通道/授权尚未实现。

## 实体计数与阶段末销毁

strong 归零登记 PendingDestroy，禁止增持/升级，本阶段已有借用保持有效；任务/参数/guard 收束后销毁并处理新归零项。weak 尚存时保留控制槽；强环正常运行不自动回收，关闭整体释放。pi_slot LIVE/FREE link 的计数与空闲链协议、版本统一及特殊释放接线仍待细化。容器 DequeKey 非拥有，校验 World/容器/节点代际，不能绕过头写权限。

## 内置组件释放与级联

pop/remove 回收节点并将原强责任转给 OwnedEntity，不减计数；clear 释放所有节点及其责任并保留空头；删除组件则移除头。只交还自身持有份数，不强制删除仍有其他拥有者的目标，重复目标逐份释放。强/弱数组逐位置交还对应计数。搬移、迁区、settle 只转移责任。

有 Removed 订阅时，旧头连同节点/引用责任移入通知存储，不先释放、不只复制头；全部读者通过且无借用时，按同一 ComponentInfo 特殊释放。无订阅直接释放。慢读者会延迟目标归零。旧值存储记录 ComponentId，注册元数据及节点/身份能力必须保活至清理结束。

普通释放将新归零实体追加迭代工作队列，阶段任务/借用完全收束后处理至队列为空，不在组件释放栈或 OwnedEntity Drop 内递归销毁；归零项唯一登记，责任份数不去重。关闭/旧分支整体清理不递归登记、不发业务通知，各值由整体遍历唯一处理。快照历史副本不操作活动分支计数，准备失败只清理非活动临时责任。正常级联按批次确定性排序；具体类型释放函数及回收授权仍未实现。

## 确定性销毁与身份回收

并发归零只登记，正常安全收束按 AllocationDomain/完整 Entity 键升序分批销毁，新归零进入下一批；组件、数组和链释放采用固定顺序。Removed 回收固定组件流与流内记录顺序。具体顺序主体见 core/destruction.rs 的 World::flush_destruction，通知映射见 core/notification.rs。

Dead 后 strong=weak=0 也只登记候选，不由释放线程直接修改 free 链。级联全部收束后按域/完整键升序回收并逐个压入各域 LIFO，因此后续复用不依赖线程抢先释放。快照静止点须完成本次确定性销毁与候选槽回收再捕获分配状态。此协议不保证用户 Drop 副作用、其他并发结构操作或不同收束输入自动确定；批次截点的授权与原子实现仍待闭合。

## 组件来源与稀疏所有权

ComponentSource 分 Table(ColumnId)、SharedSparse、PartitionSparse：SparseOnly 禁止入表且跨分区共享；TableEligible 被选定表覆盖则入列，否则放当前分区后备。WorldStorage.shared_sparse 仅显式稀疏有实例，PartitionStorage.fallback_sparse 仅普通组件有后备，不重复保存活值。

迁区搬 Table 列和分区后备，显式共享稀疏完全不动；所有路径保留值及变化元数据，不生成业务 Added/Removed/Changed。同区改变原型时按实际来源搬移，同物理列和共享/后备原位路径不自搬。生命周期、恢复准备和跨区后备移交须一起修复，不能沿用“所有稀疏均不动”的假设。

SparseSet trait 保留 get/get_mut/prepare/insert/remove，remove 返回 OwningPtr，不设 settle；SparseSetStorage 枚举分派不使用 dyn。目前仅有 papaya 哈希保底声明，不代表默认后备实现已选定；稀疏存储已选择两级 SparseArray，一级 Pages/rc Vec、二级 RawArr，页长/身份解码、构造工厂和登记选择仍需闭合。papaya guard 只保活，不能授权可变或无读者的 OwningPtr 移出；remove 返回 &V，节点内移交和回收不能二次析构，签名/证明尚未闭合。reserve 只是估计容量，不保证执行中零分配。

沿用 Bevy 命名：Ptr/PtrMut 为擦除借用，OwningPtr<寿命> 仅转移值拥有权，不拥有底层分配，不自动析构未知类型；调用方必须移交、读取或显式调用登记析构能力。Column 采用 pi_arr 拥有缓冲，SparseEntry 自己管理分配，不能长期保存 OwningPtr 或伪造 static。列间即时搬移无需额外单值分配；分配、对齐和移出保护期仍待闭合。此为本地接口声明，不直接依赖 bevy_ptr。见 crates/pi_ecs_storage/src/{ptr,column,sparse_set}.rs。资料：https://docs.rs/pi_arr/0.32.0/pi_arr/ 、https://docs.rs/papaya/0.2.5/papaya/struct.HashMap.html 。

## 查询：紧凑 QueryState，无独立 View

共享单组件读取迁入 WorldPin::get；写点查迁入 WorldWrite::get_mut/query_one_mut；后者不创建全局查询缓存。重复点查和遍历统一走 QueryState 和本次 Query/QueryMut，不再提供独立 View/ViewMut 或 view/view_mut 方法。

QueryState 每 World 一份连续 u16 两级索引（Table→变体段，variant→fetch+1，0不匹配）及紧凑 fetch 数组，校验每表数量及索引容量。游戏 World 恢复后失效该 World 全部 Query/Alter/访问计划，重新绑定；渲染 World 缓存独立不受影响。

QueryData::FetchState/QueryFilter::FetchState 保存内置准备状态，授权期 Fetch 负责绑定本阶段地址和保护期；不通过伪造 static 或永久 guard 实现长期缓存。阶段入口刷新，settle/恢复后旧地址失效；实际分区 fetch 组织与重绑定协议仍待细化。Query 只包装权限和生命周期，不恢复单独 View。

点查：身份定位→压缩偏移→当前分区 fetch→实体过滤→fetch_at。组件组合共用定位，papaya 稀疏值仍需各自按 Entity 哈希。QueryIter 遍历紧凑匹配原型成员，跳洞、不重访；可变结果可并存但不能重借用整列破坏旧引用。点查和迭代必须遵守相同范围/过滤，不以 Added/Changed 条件证明实体互斥。见 core/{query,query_fetch,query_iter}.rs。

引用/元组/Option/Entity 实现、OwningPtr 转换、变化时间窗口、批量多实体唯一性及性能仍待设计/验证。QueryState::query/query_mut/get/get_mut 均直接接收共享 &World，不要求传 WorldPin/WorldWrite；QueryMut 同样只绑定共享 World。系统复用运行授权，不逐查询重抢宿主门禁。四个共享直连入口均为 unsafe，不申请/登记宿主门禁、不维护动态借用表或长期访问租约；调用者负责包装及全部结果有效期内的冲突排除、存储有效性与回收同步。安全 pin/write/维护入口无法发现未登记查询，不能依靠它们自动拒绝冲突。系统由调度器履行前提后内部绑定，业务参数仍安全。类型/计划/过滤及单查询别名校验保留，必需后端 guard 不因省门禁而移除；其保活/遗忘安全表示仍待闭合。

宿主无门禁可写 QueryState 绑定在校验/准备成功后分配一次非零写 tick，由 QueryMut 持有，全部 get_mut/iter_mut 派生写包装共用；直接 get_mut 每次成功取值使用一个新 tick。只读 query/get 不分配写 tick。无实际修改也允许消耗，取得 tick 不产生变化；耗尽在交出可写结果前报错，不回绕。系统可信绑定提供 this_run，不走宿主分配路径；fetch/ChangeMarker 的实际接线仍未实现，不改变 unsafe 外部同步责任。

## 查询可选项、组合与批量访问

Option 首版仅支持单组件 Option<&T>/Option<&mut T>，不存在返回 None，不约束原型匹配；必需组件缺失不匹配，实体无效仍报错。可变项输出 Mut，不因 optional 而豁免别名检查，重复只读允许，冲突可变组合拒绝。具体引用/Option/数据元组 fetch 实现尚待接线，不把契约注释当作已支持实例化。

With/Without 只判断结构存在性不读值，元组为 AND、Or 为 OR，当前过滤声明含 1 至 4 项组合。初始化拒绝静态矛盾（包含数据必需项与过滤条件的矛盾），不把没有当前匹配实体当矛盾；OR 保留分支，缺一分支组件不拒绝其他成立分支。Added/Changed 不用于互斥证明，OR 需各可能分支均证明不交才能放行；通用 Not 后置。见 core/query_filter.rs。

core/query_batch.rs 提供 get_many/get_many_mut 固定数组保序输出，N=0 合法，全部校验后交出引用，失败无部分结果且不标 Changed。只读输入允许重复，可变重复 Entity 一律 AliasedEntity（可选缺失也不放宽）。iter_many 普通迭代逐项返回错误，iter_many_mut 使用 fetch_next 短借用，允许输入重复但禁止前后可变结果并存。唯一输入包装后置；不匹配不隐式略过，PendingDestroy 本阶段仍有效。查重算法/阈值不固定，不声称已验证性能。

## 结构修改双入口与整理

World 直接修改返回前发布，Alter 固定 A/D 声明按源/目标计划分组在 scope 正常退出发布，共用差异及 OwningPtr 移交。严格新增/严格删除、拒绝交集和重复类型、同作用域重复 Entity 拒绝；一次请求失败不撤销此前接受请求。不靠 Alter Drop 提交，不设置取消/回滚事务。目标原型必须显式登记。

先校验并准备容量/记录，开始移动后失败直接 panic，不补发失败行、不支持捕获后继续 World；展开仍须内存安全。Alter 接受时当场写目标隐藏存储，批次拥有隐藏行初始化责任；具体操作仍未实现。共享执行壳及并发参数接线仍待闭合。见 core/{world,alter,structural_change}.rs。

CAS 每次加一认领行，认领不等于完成初始化；连续发布前缀不能越过未完成行，内存序和访问约束见 table.rs。不承诺 lock-free。pi_world 的实际 append 依赖和未导出 safe_vec 示例须分开，不能据 CAS 自动推导整体并发安全。

settle 在指定静止点应用统一动作填 Table 洞、修位置，再整理原型成员及合桶；不重复析构已移出/销毁值，Table 整理与 SparseArray 的映射 settle 分开：非 rc 稀疏映射在安全收束点填登记洞，rc 映射 compact 为空。阶段结束是维护机会，不固定每阶段必做；维护编排接口待闭合。参考 pi_world/src/table.rs:235，具体定义见 crates/pi_ecs_storage/src/settle.rs 与 crates/pi_ecs_core/src/world.rs。

## 访问、阶段与执行器

Schedule 按登记顺序持有多个 Stage 和可复用 Executor 池。Stage::prepare 同步最新已发布原型、增量更新 Query/Alter 及权限/冲突图，Executor 只执行 PreparedStage，不管理阶段顺序或重复同步。串行/并行只作用阶段内部，始终使用受限 &World。

上一阶段任务、作用域发布、引用和 guard 全部收束后才进下一阶段。首版不支持同 World 多 Schedule 重叠驱动，安全入口运行门禁尚待实现；不引入全局跨执行器阶段授权。panic 收束工作线程后传播，不能遗留后台写访问。见 schedule/src/{schedule,stage,executor}.rs。

AccessDeclaration 分组件读写、实体范围、结构访问；同查询先检查可变别名，同系统参数冲突拒绝，跨系统冲突串行/排序。阶段前可展开实际原型范围，动态条件不当互斥证明。Alter 覆盖保留值搬移、源/目标成员/Table/身份及分区后备，未证明兼容的结构操作保守串行，不改变默认并发模型。

RunAccess unsafe 构造只表达跨包证明责任，不是已实现授权；FetchAccess 短借用不能扩大权限，papaya guard 必须覆盖所有返回引用而非只活于临时 fetch。真实系统参数和守卫持有层尚未闭合，不手工 Send/Sync 掩盖。见 core/access.rs。

## System 与 SystemParam 生命周期

schedule/system.rs 拥有 System 身份、FunctionSystem 的函数/参数长期状态/last_run 和本次 SystemRunContext。system_param.rs 的封闭 SystemParam 协议区分 State、Pending 和 Item<run>：initialize 阶段前登记；prepare 刷新计划并保留参数声明边界；begin 准备独立工作；fetch 在授权下借用；用户返回并释放借用后 finish 显式提交；unregister 在阶段前解除订阅责任。Pending 不借用用户参数，授权持续到提交及 guard 收束。成功后更新 last_run，panic 不回退游标、不回滚、不继续模拟。

阶段前先应用控制变更/取消订阅，再汇总需求和同步计划；取消后仍需运行的必需参数不能拿失效订阅执行或自动重新订阅。部分初始化失败清理本次登记，不把全部阶段控制变更做成事务。Query/Reader/Alter 分别适配，具体实现/函数推导及受限维护授权尚未接通，不创建空适配模块。

执行顺序约束：同组件先写后读，多个写者按登记顺序；先前将所有无显式顺序冲突按登记顺序定向的建议撤销。执行图及整个全阶段运行由后续专门执行图推动，本轮仅保留现有阶段入口，不确定图数据结构、跨阶段图边界或改造驱动器。

## 点查与查询式结构变更

非 rc 普通添加/删除不搬移其他存活值地址；rc 单线程模式允许容量增长搬移旧值，但须结束受影响借用并重绑旧地址缓存，两种模式均不夹带 settle/压缩；显式迁移与恢复仍受排他授权，删除不延长被删值寿命。Lookup<Q,F> 专门按 Entity 点查已发布值，不扫描成员；支持含 &mut T 的 Q，get_mut 短借用防止重复实体可变结果并存，只读 Q 另有 get。过滤/值别名检查不因点查豁免，配置为 ()，复用查询计划但无成员遍历权限。

查询式 Alter 持 Q/F 的合法源范围和固定 A/D，源及对应目标为结构写，另计 Q 的值访问。接受时校验/预留后当场写隐藏目标存储，未发布行不能点查；同 Table 路径不假设另分配一行。finish 在全部用户借用释放后搬保留值、处理删除、更新位置/成员并发布。初始化责任逐槽跟踪，失败不能重复析构，不依赖 Drop 提交。撤销此前批次暂存新增值的选择。

系统内 Lookup 与仅隐藏目标追加可兼容，包括可变点查已有目标实体，但重叠源查询的值读写仍必须拒绝或证明范围互斥。跨系统结构写首版仍保守排斥，不把点查当任意结构变更豁免。AlterIter::alter 修改当前实体，先释放 fetch_next 返回的短借用再提交，首版不提供能长期保留结果的标准 Iterator。宿主 AlterState::scope 保留，查询式参数暂命名 QueryAlter，统一命名及真实长期状态/授权绑定尚待细化，见 core/{alter,lookup,access}.rs 与 crates/pi_ecs_storage/src/storage.rs；均为失败壳。

## 参数集、显式构建与命名组合

SystemParam 首版封闭实现；关联 Config 表达每参数位置的初值/恢复能力，ParamConfig<P> 实际拥有 P::Config，initialize 消费配置。组合配置与参数元组同形，同类型 Local 不合并；普通 Query/Res 无需额外配置。部分初始化失败清理本次成员，不能开放任意 State 工厂。State/Pending/Item 分别是长期状态、本次完成工作及用户借用；prepare→begin→fetch→用户调用→释放借用→finish→释放授权→推进 last_run，条件跳过不执行调用协议、不消费通知或推进窗口。函数首版返回 ()，不加异步挂起/业务 Result 适配，命名组合不扩大权限。具体参数配置类型与函数推导仍待细化。

schedule/param_composition.rs 的 ParamAccess 保留 Leaf/Together/Exclusive 边界：普通参数组合成员同时可用，ParamSet 成员分时短借用。单成员内部别名仍拒绝，集合内部允许成员间冲突，对组外及跨系统使用全部成员权限并集。不能先构造所有冲突引用再包装；p0/p1 按需借用，当前先落双成员访问壳，SystemParam GAT 适配尚待闭合。首版 BorrowOnlyParam 只面向无结构提交的 Query/资源，不加入 Alter。

NamedParam<P,M> 使用 ParamView 将既有参数结果重组为命名结构，长期状态和生命周期复用 P，不开放任意外部 SystemParam 实现。命名不自动分时，组合冲突照常检查。system_builder.rs 显式接收函数和 ParamConfig，build 返回未初始化 FunctionSystem，配置留到阶段前登记/初始化。Local/多例具体初值配置、元组协议和函数推导仍待补齐，宏后置；当前不是可用构建器。

SubWorld 仅作为权限收窄/拆分参考，本轮不新增公开 World 类型，不复制数据或创建分区。执行图仍暂缓。

LocalConfig/MultiResConfig 使用两步式：new(initial) 或 Default 只提供首次初始化值，再消费配置选择 save_clone/save_with/rebuild/preserve，策略选择返回 Result，重复选择报错而非覆盖；失败时配置正常析构，不改 World。不选策略可普通运行但游戏快照拒绝。初始化移入 initial，不逐次重置；保存捕获当前值，恢复不复用初值；Save/重建能力保留到长期状态。具体擦除能力与 SystemParam 适配仍待接通。见 schedule/{local,resource_param}.rs。

## 资源、多例与 Local

宿主资源值访问统一复用 WorldPin/WorldWrite：resource 共享读取，resource_mut 写已有唯一资源，multi_resource 返回只读紧凑视图（len/is_empty/iter），结果借用访问对象。移除无授权 World::resource/resource_mut；insert_resource/remove_resource 以 &World 内部取得独立维护授权，与 pin/write/运行互斥，不向访问对象开放。多例生产槽增删仍归阶段前系统参数登记/注销；系统 Res/MultiRes 继续原运行授权，tick 规则不变，不增加资源通知流。实际存储/门禁及标记通道仍未实现，见 core/resource.rs。

Local 提供 Deref/DerefMut，不记变化，不提供 take；可用 mem::replace。Res 只读解引用，ResMut/MultiResMut 可写解引用及 set_changed 按已有规则标记；三者提供 is_added/is_changed，首版无绕过标记写入接口。MultiRes 提供 len/is_empty/iter/is_structure_changed，iter 每项返回含窗口的 Res。结构版本与参数上次成功观察版本比较，首次视为变化，增删后数量相同仍为变化，查询不消费；不公开稳定下标定位。tick 观察与 Mut 标记通道的无别名表示仍待细化。具体声明见 schedule/{local,resource_param}.rs。

唯一资源与组件登记空间分开，全 World 每类型一个可缺失实例，类型初始化前登记，存在性变更在独占维护点。core/resource.rs 定义登记/插入返回旧值/移出/只读与 Mut 访问；schedule/resource_param.rs 定义 Res/ResMut 及 MultiRes/MultiResMut。资源和每个多例实例始终维护 added/changed tick，不按观察需求开关；新增同时设置，替换保持 added，可写解引用更新 changed。首版不扩资源通知流/分区资源/非 Send 资源。

多例为每系统每参数位置独占的生产者槽；不同槽可并行写，读全体排在写者后。紧凑数组不使用可变位宽映射，增删在阶段前静止点执行并全遍历更新生产者偏移缓存，执行中禁止搬移引用。读者按当前紧凑物理顺序遍历，不承诺登记顺序；稳定生产者身份用于恢复，数组位置不作身份。跳过保留值，读不清空，注销生产者移除实例普通析构；结构版本与每实例 tick 分开。

Local 为 System+参数位置私有值，不登记 World 访问或 tick，支持显式初值/Default，跳过保留，移除系统析构。普通资源 Send+Sync，Local Send 不要求 Sync，非 Send 参数后置。恢复分类为 Save/Rebuild/Preserve，游戏系统 Local 未配置则拒绝捕获；资源保存包括存在性。具体恢复复制/重建工厂、参数注入、Deref 和类型擦除存储仍待闭合，不代表接口可运行。

## tick 与可靠通知

宿主变化查询显式使用 QueryState::query_with_window 或 query_mut_since；前者传完整 ChangeWindow，后者传 last_run 并以新写 tick 作为上界，QueryMut::this_run 可读取。World::read_change_tick 仅读取已分配时钟，不是并发写者完成屏障。宿主自行同步截点、在处理成功后推进独立观察点；观察点的 World/恢复分支正确性完全由调用者负责，不增加身份/epoch 包装或分支校验，QueryState 不保存/消费进度，不新增 clear_trackers 全局基线。无窗口 query/query_mut/get/get_mut 遇到 Added/Changed（含组合分支）报错；单点可先绑定显式窗口包装再 get。系统 Query/Lookup/Alter 仍由调度器注入 last_run/this_run、复用运行授权，不增加业务参数/宿主门禁/重复分配；具体接口见 core/query.rs，绑定仍为失败桩。

结构写 tick 按上下文而非实体分配：宿主单次 spawn/change_archetype 在校验准备成功后、写入前取得一次；宿主 Alter scope 在调用闭包前取得一次，所有接受请求共用，系统路径沿用 this_run。阶段末/显式销毁收束取得一次供全部级联批次共用，无工作不分配；耗尽在物理修改前拒绝，不丢失待处理责任。强归零只登记、不取 tick/发 Removed，Removed 使用实际移出上下文时间；旧值最终回收不重复 Removed，新归零交后续收束。迁区/settle 保留原 tick、不制造业务变化。具体流程见 core/{world,alter,destruction,notification}.rs；实际分配/授权接线未实现。

WorldWrite 成功取得独占授权后分配一次非零 ChangeTick；所有组件/资源/容器修改共用，write_scope 不另分配。冲突拒绝不消耗 tick，成功但无修改允许消耗而不产生变化/通知。耗尽报错并释放本次授权，不返回可写对象、不回绕，既有停止态不因清理解除。Drop 只释放授权，不分配 tick 或延迟发布。重新取得写包装可产生多条同 tick 的 Changed，以流序号区分；系统参数沿用 this_run。源码主体见 World::write/WorldWrite 及 ChangeMarker；实际分配与授权实现仍未接通。

World.change_tick 为非零 u64 原子变化时钟，初始1、恢复拒绝零；ChangeTick 用 NonZeroU64 表示，首次窗口仍用 None。不等于模拟帧；每系统执行或宿主修改上下文取得一次 tick。last_run 在成功执行及作用域变更完成后更新，tick 顺序不替代依赖。TrackingDemand 分开控制 added_tick/changed_tick 和三种通知，未订阅项不写状态；查询/通知初始化注册并汇总需求。通知订阅在阶段前静止点生效，从流尾开始，覆盖全 World；阶段前可取消，最后订阅取消后不推送该类通知，不关闭仍被过滤器需要的 tick。窗口统一 last_run < value_tick <= this_run，首次 last_run=None 无下界，已启用 tick 的既有匹配值可见；本系统写入 this_run 后立即可观察。跳过不推进，恢复历史窗口，安装不算业务变化。u64 耗尽停止报错，不回绕。动态启用/重启 tick 时现存值对应槽初始化为维护时刻，保守视为变化，不补历史通知。

Mut<T> 读取不标记，可写解引用或显式 set_changed 按需求更新时间并当场登记 Changed 通知，不专门延迟到系统结束。调度器必须将通知观察声明为对应范围的观察访问，与写入/结构修改排斥；即使只消费 Entity 也不能绕开该规则。既有执行依赖提供可见性，不同步递归执行观察者。Added/Removed 跟随逻辑结构发布，settle/迁区/保留值搬移不伪造变化。

通知系统拥有按组件/种类的记录流，查询各自注册独立游标。Changed 不跨已发布记录全局合并；同一 Mut 首次可写访问/显式标记只登记一次，再取 Mut 可登记新记录。消费者可以在自己本次批次去重。记录在所有有效订阅者确认且没有读取借用后才回收，慢/跳过读者会保留记录，不按固定帧淘汰。注销解除该读者责任；序号不等于 change tick，读取批次固定上界，以 Iterator::next 成功返回为消费，不提供显式 ack；早停/drop 保留尚未返回的记录，已返回但业务尚未处理不恢复游标。RemovedBatch 返回 RemovedNotice<T>，旧值引用的保活独立于批次是否已 drop。

Removed 包含 despawn 并独立保存旧值，不要求 Clone。通知存储先准备对齐空间，通过 OwningPtr 转入旧值，不能长期保存源槽指针；多读者只读旧值，最终回收析构一次。无 Removed 订阅则直接析构。Added/Changed 不保存历史组件值，消费 Entity 后点查得到当前状态，可能已不存在。

状态过滤为 core/query_filter.rs 的 Added<T>/Changed<T>，实现 QueryFilter，只按系统窗口筛选当前活组件，不消费通知。独立通知参数为 schedule/notification_reader.rs 的 AddedReader/ChangedReader/RemovedReader，各参数实例独立订阅，Reader 不强制开启活组件 tick。RemovedReader 返回 RemovedBatch<T>，旧值仅共享读取；尚无 SystemParam 注入实现。

Reader 统一 read/is_empty/clear，配置为 ()。read 固定上界、返回即消费、早停保留；is_empty 不消费；clear 仅放弃调用时上界以前本读者记录，不影响其他读者或此后追加，活批次/旧值借用阻止 clear。失效订阅错误不推进，无 ack/peek/自动重试/去重或运行中退订，clear 不代表业务成功。

声明见 core/{change_detection,notification}.rs，WorldWrite::get_mut 使用 ComponentWrite：普通组件为 Mut，内置容器为专用访问对象。具体通知稳定页/并发追加、迭代旧值的稳定保活、Removed 对齐分配和运行期类型校验、窗口耗尽错误与 SystemParam 接线仍有 @to_design，不把占位结构当可用通知系统。

## 类型化命令与延迟可见

World 统一拥有命令服务，插件登记命令类型及处理系统，可用资源承载具体服务实现。crates/pi_ecs_command/src/command.rs 定义单消费者 CommandReceiver<C> 与多生产者 CommandSender<C>；next 转出 C 即消费，固定就绪批次上界，早停保留未返回项，无业务 ack/自动重试。暂时无消费者保留命令，消费者注销交还资格不清队列；关闭/容量/旧 epoch 等提交错误交还 payload。广播流沿用独立订阅游标及共享读取语义，不和唯一消费者混用，具体类型化广播接口后续补齐。

延迟 payload 从登记起已在同一队列中拥有，到期只更新可见状态/就绪索引，不复制、不重新入队，不产生第二次入队容量失败。未来项不阻塞即时项；即时按接受顺序就绪，定时到期按 deadline 及同期限登记序号就绪，推进只改变可见性不调用业务代码。TimerKey 为非拥有且校验归属/代际的取消凭据，到期变可见即失效，不能撤回已可见命令。模拟 SimulationTick 与变化 ChangeTick 分离；一次性定时优先，周期和漏 tick 策略后置。

WorldEpoch 不随游戏快照回退，发送端捕获派发时代次，不自动改成当前代次，旧后台结果拒绝。新外部输入由宿主按新请求规则获取发送端；后台任务只传拥有型数据，不携带 Query/RunAccess/组件借用。发送端仅共享独立命令入口，不强持整个 World，也不授予 ECS 写权限。定时项/payload 保存及 epoch 语义见运行状态恢复分类，消费资格重绑具体接口尚待闭合，不能把 Arc 保活当成访问授权。

任意结构修改只是普通命令处理器的行为，在专门命令处理阶段执行，不设特殊结构命令队列。fory 为外部协议编解码计划选型，未核实 Rust 版本或引入依赖；本地类型化结果不强制序列化，World 指针/强弱句柄不能直接作为可恢复协议值。具体 SystemParam 适配、任务管理、背压、关闭排空及命令恢复具体接口仍待设计。

## 快照恢复

core 负责 GameDataSnapshot，schedule 的 GameRecovery 接 World+Schedule 协调整体 GameSnapshot。存储级独立复制身份及选定数据，不全存储 COW；组件/资源明确登记 save_*_copy<T:Copy>、save_*_clone<T:Clone> 或 save_*_with<T,S> 捕获/恢复函数。允许异表示 T→S→T，不要求序列化或全部 Clone。Copy/Clone 不证明指针/Arc/内部可变数据历史独立，未登记不得默认 memcpy。资源分保存替换、派生清旧重建、非游戏保留；未保存组件先合法初值再普通系统重建。依赖缺失/环/失败停止，不暗中忽略。

捕获失败不改当前模拟；恢复兼容检查和准备失败不替换当前游戏，安装/重建失败停止不回滚。渲染 World 不参与恢复；先保留参考状态，再恢复重演到相同模拟帧，对比最终净差异推动反应式渲染协调。恢复安装不逐实体发普通通知，重演的游戏反应与最终桥接发布须隔离，正确析构及内部修复不可省。历史期规则固定，宿主改规则时清历史重建基线。引用复制/安装遵循下述协议；桥接只传逻辑身份/投影值，不传游戏 World 的强弱拥有句柄或组件地址，稳定 key 的跨分支语义及差异比较接口待设计。

快照上下文不修改源 World；特殊历史 S 普通析构只清历史内存，不减运行计数。恢复准备期构造非活动绑定，安装时与身份计数整体激活；失败清理不得让临时 T 的普通 Drop 误减当前分支。框架内建适配记录引用及节点拥有关系，容器头与节点池一致捕获。旧分支析构不得影响新计数。真实内建适配、失败清理和激活协议仍待闭合，不以函数类型可编译宣称安全实现。见 core/snapshot_rules.rs。

捕获限定阶段末静止点：任务/参数借用/guard 全结束，待销毁实体与可回收身份槽均按确定性协议收束，不强制 settle。快照保存代际、free 链及强弱计数原值，不扫描引用重建；调用方保证选定组件/资源/队列及外部拥有责任与保存计数一致。框架仍负责无重复析构、历史隔离及失败清理。裸 Entity 可跨恢复保存并重新查询有效性；外部强弱句柄及 forget 留下的释放责任不能不加区分地对恢复后计数释放，简单约定为恢复前清偿、恢复后重取，不增加运行期自动扫描机制。

恢复先校验并完整准备非活动副本，准备失败只清临时数据且当前状态不变；之后进入整体清理模式，旧值仅操作旧计数，不根据归零再递归销毁已由整体遍历负责的值，不发布普通生命周期通知。旧状态清理完再安装身份原样计数及新值，激活目标绑定，推进不回退的 epoch，失效 Query/Alter/访问计划，再运行派生重建。开始清旧后的失败停止模拟，不撤销安装流程。

## 本轮三个 crate 的设计壳边界

依赖固定为 core → storage / command / types，storage → types，command → types；types 不依赖其他 ECS 包。没有新增其他 crate，crates/pi_ecs_core/src 保持平铺。核心 OwnedEntity、ComponentAccess、Query、类型化通知、逻辑原型/组件注册和非泛型 World 仍在 core；不增加 World 能力体系。

- types：entity.rs 单源定义 Entity、AllocationDomain、PartitionId、EntityCategory、ArchetypeId、EntityLocation、ArchetypeRow；component.rs/resource.rs 定义 ComponentId/ResourceId；change_detection.rs 定义 ChangeTick/ChangeWindow/TrackingDemand/ComponentTicks；error.rs 定义 EcsError。位置编码、tick 和资源编号内部字段不因拆包全部公开；Mut/ChangeMarker 留 core。
- storage：ptr、column 及配对芯、table 及布局配对芯、sparse_set 及配对芯、settle，以及 storage 物理聚合与配对芯；archetype.rs 仅保存 ArchetypeMember/ArchetypeStorage，不拥有逻辑组件集合或注册策略。TableLayout::new/id/columns、Column::new/get/get_mut/prepare/insert/remove、Table::new、SparseSetStorage::hash_map、PartitionStorage::new/prepare_archetype、WorldStorage::new 是实际构造/读取所需的最小壳入口，字段仍封装；构造只接收编号、Layout、空物理所有权，不接收 core::ComponentInfo/World。所有新操作薄委托至明确失败桩，未提供可运行存储。
- command：CommandHub/CommandEndpoint、Sender/Receiver/Batch、TimerKey、SimulationTick、WorldEpoch、CommandSendError 及队列配对芯迁入独立包；CommandHub::new/register/sender/receiver/advance 接收具体服务数据。core/command.rs 保留 impl World，core/command_inner.rs 只委托 CommandHub 壳；芯不调用其他芯，发送端不持整个 World，原 Send 约束不放宽。

原 core 的 entity/error/component::ComponentId/resource::ResourceId/change_detection::{ChangeTick,ChangeWindow,TrackingDemand}/table/command 公共路径通过重导出保持同一类型，World 命令方法签名不变；原私有 ptr/storage 路径保留必要适配，已删除无消费者的旧 column/settle/sparse_set 文件和迁走的配对芯。原型声明仍在 core/archetype.rs，物理成员只在 storage 定义。

registry 版本继续固定 pi_arr=0.33.0、pi_buckets=0.5.0、pi_share=2.1.0；pi_append_vec 0.6.3 的发布兼容阻塞及占位保留，没有接本地 path。rc 仍未全图贯通，当前目录枚举/线程和地址保护接线不是可用的 rc 支持；已确认 rc Vec/连续增长/即时映射填洞/compact 空，非 rc Pages/原子 Share/延迟填洞，不反转。

验证范围为 src、crates、design/unified-partitioned-ecs、Cargo.toml、Cargo.lock。新包首次 cargo check 更新锁后再 locked；最终执行 cargo fmt --all、cargo check --workspace --locked --all-targets --message-format=short、cargo doc --workspace --locked --no-deps，以 checkRound 的 success/current 证据为准。拆分前默认全目标检查已通过；这是设计稿编译验证，不运行触发失败桩的业务测试，未进行独立轻审。工作区无 Git，git diff 不能提供历史差异证据。
