# 统一分区 ECS：概念

本目录由 zcode 持续维护，源码壳是具体契约单一来源。旧 dev-design-lite 位于 ../../archive/dev-design-lite/unified-partitioned-ecs/，不再维护；research 是固定来源研究，不是当前设计规范。

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

## 身份和分区

- World 非泛型，默认单分区，按需固定配置多分区。游戏与渲染为独立 World，无全局/本地域。
- Entity 为完整 pi_key，来源域/类别编码在版本扩展位；每域独立分配状态、同域有序操作、强归零阶段末分批排序销毁，Dead 且强弱均零只登记候选；安全收束末按域/完整键排序压入 LIFO，不在线程释放时立即回收。当前分区与来源域不同，迁区不改身份。
- DOMAIN_BITS 是版本扩展位数，不是分区数量。Clone 保存槽/代际/free 链，serde 不等价；同步 compact/截尾需同序重演，shrink_to_fit 可本地执行，截尾丢历史可能使旧键重现。
- EntityLocation 为三个 u32：分区12位+成员行20位，Table行32位，Table编号16位+变体编号16位；分区不带本地域哨兵，保留码/最大数量待定。见 crates/pi_ecs_types/src/entity.rs。

## Table 与原型统一编号

- ArchetypeId 仅含 table/variant，Table 无域，每 World 各自登记一张空 Table，variant=0 为基础。
- 增补原型声明完整组件集合并固定绑定一个基础 Table；Table 必须是组件子集，自动选覆盖最多者，同分先登记优先，register_archetype_on 可指定基础表。
- 类型和 Table 初始化后冻结，运行期允许显式声明已登记类型的组合；相同集合复用，不隐式登记组件。原型成员在每分区独立保存 Entity 与 table_row，查询先匹配原型再遍历成员。
- Table 行搬移更新身份 table_row 和成员记录；原型成员搬移更新身份 archetype_row。表布局跨区一致，物理数据独立。见 crates/pi_ecs_core/src/{archetype,registry}.rs 与 crates/pi_ecs_storage/src/{archetype,table,storage}.rs。

## 三种组件来源

| 策略与覆盖 | 来源 | 所有权 | 纯迁区 |
|---|---|---|---|
| SparseOnly | SharedSparse | 每组件每 World 一份，跨分区共享 | 不搬值/元数据 |
| TableEligible 且 Table 覆盖 | Table(column) | 分区 Table 列 | 搬至目标 Table |
| TableEligible 但未覆盖 | PartitionSparse | 当前分区后备 | 移至目标分区后备 |

- 同一实体同一组件只有一份活值。迁区所有路径都保留变化元数据，不伪造 Added/Removed/Changed。
- SparseSet 是后端协议，SparseSetStorage 枚举分派，不用 dyn。get/get_mut 返回 Ptr/PtrMut，remove 返回受来源生命周期约束的 OwningPtr，不设 settle、不强制 dense/owners 或末值填洞。
- papaya 0.2.5 为哈希保底实现，可用于共享或分区实例，不代表每分区默认后备算法已选定。两级 SparseArray 已选为稀疏存储方向，页长/身份解码和登记工厂仍待闭合。
- papaya guard 保活节点，ECS 权限约束组件读写/移出；remove 返回 &V 而非 V，SparseEntry 移出 OwningPtr 的排他性仍待证明，不手工扩大 Send/Sync。
- Column 采用 crates.io pi_arr 0.33.0，接口使用 Ptr/PtrMut/OwningPtr，不提前类型化；分配归容器，OwningPtr 只负责值转移，普通 drop 不自动析构未知值，须显式消费。缓冲对齐及条目分配仍待闭合。扩展期间不承诺全列单切片。

- 非 rc 普通增删不搬移其他存活值地址；rc 为单线程模式，允许容量增长搬移旧值，增长前结束受影响借用并重绑旧地址缓存。两种模式普通增删都不夹带组件值填洞压缩；rc 删除仅立即填 owner 映射洞；搬移型维护仍须排他授权，地址稳定不延长被删值寿命。

## 宿主读写访问授权

World::write/write_scope 提供宿主独占组件访问 WorldWrite，冲突报错不等待；与 pin/运行/其他写/维护互斥，Drop 归还，forget 只泄漏授权。get/get_mut/query_one_mut 只访问组件，不开放结构维护，先释放写授权再维护。ComponentAccess 与 QueryData::Item 已分离组件借用期和拥有结果有效期，宿主 OwnedEntity 可在 pin/write 释放后保留，系统不得越过运行授权；现有系统短借用壳保守使用相同寿命。world.rs 仅编译样例覆盖真实 write→pop→释放授权→结构修改，以及作用域返回拥有引用。实际门禁/guard/fetch 未实现。

World::pin 返回显式 WorldPin，read_scope 复用同一读授权；不是 std::pin::Pin，不增实体计数。多个只读 pin 可共存，首版排斥系统运行、组件写、结构维护/销毁/settle/恢复；强计数增减可继续，归零只登记。get 的结果借用 pin，不保留无授权 World::get。Drop 释放授权，panic 展开同样释放，forget 只泄漏授权令冲突持续拒绝，不得提前解锁。OwnedEntity 不等于 pin；系统 Query/Lookup 沿用运行权限，不重复申请宿主 pin。宿主组件写点查已迁入 WorldWrite，资源值访问也已迁入 pin/write，剩余结构入口需检查同一门禁，实际原子表示及全入口授权接线仍未实现。

## 查询及批量点查

- WorldPin::get 单组件只读定位，WorldWrite::get_mut 提供短借用写访问；WorldWrite::query_one_mut 只准备目标原型，不构建全局缓存。
- QueryState 的 query/query_mut/get/get_mut 直接接受共享 &World，不改为 pin/write 参数；四入口均 unsafe，不申请门禁/动态访问租约；调用者保证全结果期不冲突及存储有效，系统由调度器履行前提。安全维护无法发现未登记查询；后端 guard 仍必需，其保活表示待闭合。
- QueryState 在所属 World 保存一份 u16 两级索引及紧凑 fetch 数组；不存在独立 View。Query/QueryMut 是本次访问权限和借用包装，提供 get/iter。
- 两跳索引先 Table 取变体段偏移，再 variant 取 fetch+1，0 不匹配；总索引长度和偏移另作 u16 容量校验。恢复后清所属 World 全部计划，不影响另一 World。
- 阶段前同步/重绑 fetch，点查只定位身份、解压偏移、选分区访问器、过滤并取值；长期状态不能保留失效的组件指针或 papaya guard。实际 FetchState 与授权期 Fetch 的绑定仍待闭合，不恢复 View API。
- 迭代器每实体最多产出一次、跳洞，允许保留不同实体的结果；不能靠重新借用整列破坏先前结果。见 core/{query,query_fetch,query_iter}.rs。

- Lookup<Q,F> 只点查已发布值，可含可变 Q；get_mut 短借用，值权限/过滤不豁免，无成员遍历。可与同系统隐藏目标追加兼容，仍拒绝源值可变别名。

宿主无门禁可写 QueryState 绑定在校验/准备成功后分配一次非零写 tick，由 QueryMut 持有，全部 get_mut/iter_mut 派生写包装共用；直接 get_mut 每次成功取值使用一个新 tick。只读 query/get 不分配写 tick。无实际修改也允许消耗，取得 tick 不产生变化；耗尽在交出可写结果前报错，不回绕。系统可信绑定提供 this_run，不走宿主分配路径；fetch/ChangeMarker 的实际接线仍未实现，不改变 unsafe 外部同步责任。

## 结构修改和执行

- World 直接修改返回前发布；Alter 固定严格增删集合、禁止交集/重复、拒绝同作用域重复 Entity，通过 scope 正常闭包退出统一发布，不依赖 Drop。请求错误不撤销此前请求。
- 查询式 Alter 只改 Q/F 对应源，声明源/目标结构写；新增值当场写隐藏目标，借用释放后 finish 迁移发布。AlterIter::alter 修改 fetch_next 最近项，先结束项借用；具体状态接线待定。
- 共用 ArchetypeChange 及 OwningPtr 搬移能力；同 Table 共有列不动。先校验准备，开始移动后失败直接 panic，不回滚、不补发失败行或恢复运行；展开仍须内存安全。
- settle 只物理填 Table/成员洞并修位置、合并 pi_arr 桶，不重复析构，不参与稀疏逻辑删除。不固定每阶段必做。
- Schedule 顺序运行 Stage，阶段不重叠；Stage 前同步原型和访问计划，Executor 可复用，仅决定阶段内串行/并行。系统访问默认受限 &World。首版不支持同 World 多 Schedule 重叠驱动，运行门禁待定。
- core/access.rs 的声明用于参数冲突和系统排序；权限和 guard 不能互相替代。SystemParam::Config 按参数位置拥有初值/恢复能力，ParamConfig 持有配置，initialize 消费配置；条件跳过不推进窗口/游标，真实适配接线待定。

- LocalConfig/MultiResConfig 先 new(initial)/Default，再选择 save_clone/save_with/rebuild/preserve；策略选择返回 Result、重复拒绝，初始化消费初值并保留恢复能力，未选策略的游戏捕获拒绝。

- Local 普通解引用不记 tick；Res/ResMut/MultiResMut 提供窗口变化检查，可写访问统一标记。MultiRes 逐项返回只读 Res，结构增删以版本判断而非数量，首次视为变化。

## 变化与通知

宿主变化查询显式使用 QueryState::query_with_window 或 query_mut_since；前者传完整 ChangeWindow，后者传 last_run 并以新写 tick 作为上界，QueryMut::this_run 可读取。World::read_change_tick 仅读取已分配时钟，不是并发写者完成屏障。宿主自行同步截点、在处理成功后推进独立观察点；观察点的 World/恢复分支正确性完全由调用者负责，不增加身份/epoch 包装或分支校验，QueryState 不保存/消费进度，不新增 clear_trackers 全局基线。无窗口 query/query_mut/get/get_mut 遇到 Added/Changed（含组合分支）报错；单点可先绑定显式窗口包装再 get。系统 Query/Lookup/Alter 仍由调度器注入 last_run/this_run、复用运行授权，不增加业务参数/宿主门禁/重复分配；具体接口见 core/query.rs，绑定仍为失败桩。

结构写 tick 按上下文而非实体分配：宿主单次 spawn/change_archetype 在校验准备成功后、写入前取得一次；宿主 Alter scope 在调用闭包前取得一次，所有接受请求共用，系统路径沿用 this_run。阶段末/显式销毁收束取得一次供全部级联批次共用，无工作不分配；耗尽在物理修改前拒绝，不丢失待处理责任。强归零只登记、不取 tick/发 Removed，Removed 使用实际移出上下文时间；旧值最终回收不重复 Removed，新归零交后续收束。迁区/settle 保留原 tick、不制造业务变化。具体流程见 core/{world,alter,destruction,notification}.rs；实际分配/授权接线未实现。

WorldWrite 成功取得独占授权后分配一次非零 ChangeTick；所有组件/资源/容器修改共用，write_scope 不另分配。冲突拒绝不消耗 tick，成功但无修改允许消耗而不产生变化/通知。耗尽报错并释放本次授权，不返回可写对象、不回绕，既有停止态不因清理解除。Drop 只释放授权，不分配 tick 或延迟发布。重新取得写包装可产生多条同 tick 的 Changed，以流序号区分；系统参数沿用 this_run。源码主体见 World::write/WorldWrite 及 ChangeMarker；实际分配与授权实现仍未接通。

宿主资源值访问统一复用 WorldPin/WorldWrite：resource 共享读取，resource_mut 写已有唯一资源，multi_resource 返回只读紧凑视图（len/is_empty/iter），结果借用访问对象。移除无授权 World::resource/resource_mut；insert_resource/remove_resource 以 &World 内部取得独立维护授权，与 pin/write/运行互斥，不向访问对象开放。多例生产槽增删仍归阶段前系统参数登记/注销；系统 Res/MultiRes 继续原运行授权，tick 规则不变，不增加资源通知流。实际存储/门禁及标记通道仍未实现，见 core/resource.rs。

- World tick 非零 u64（初始1），ChangeTick 使用 NonZeroU64，恢复拒绝零；属执行上下文，不是帧号；窗口左开右闭，首次 None 无下界，当前修改立即可观察，跳过不推进，恢复历史窗口。u64 耗尽停止报错。TrackingDemand 按需启用，启用/重启 tick 以维护时刻初始化现存值，不补通知。
- Mut 只读不标记，可写解引用/显式标记当场登记通知；观察者必须声明与对应写入排斥的观察权限，无额外系统末发布屏障。
- 每查询独立订阅游标，全部有效读者确认且无活借用才释放记录；Changed 不全局合并，慢读者保留历史。
- Removed 包含 despawn 的旧组件值，由通知存储独立拥有分配，多读者共享读取，最后回收析构；不长期保存 OwningPtr。Added/Changed 仅身份和时间，不保证消费时实体仍存在。
- 流/游标一同保存恢复，历史期订阅配置固定；Reader 统一 read/is_empty/clear，返回即消费，clear 只放弃本读者当前上界以前记录；动态订阅与回绕仍待定，见 core/{change_detection,notification}.rs。

## App 与插件

- 单 App 单 World，客户端组合游戏/渲染 App，服务器只装游戏 App；宿主掌握主循环、回滚及跨 World 桥接。
- 命名 Schedule 显式运行，不自动 update；目标 Stage 归 App、System 状态归 Stage，执行器归属待图库对接。类型标签只进程内有效。
- 插件按具体类型唯一，立即 build、可装子插件；显式依赖检查，不自动安装/排序。全部 build 后按成功安装顺序 finish 并统一验证冻结；运行数据不是插件字段。
- Building→Finishing→Ready，部分构建/不可恢复运行失败停止；无热插拔/解冻，初始化系统显式运行。Arc 独占与恢复访问安全入口仍待设计，见 app/app.rs。

- 执行图目标：App 拥有 Stage，Schedule 引用 Stage/子 Schedule；System 实例归唯一 Stage，暂不跨 Stage 共享，函数可以复用。图所有者显式组合，插件依赖不决定运行顺序。当前线性 Schedule/Executor 仍为临时壳，正式接口等待图库协议。

- World 单一门禁排他运行/维护，子图复用根授权，冲突报错不等待，致命失败停止。绑定发布后不返回整个可变 World；App 快照入口协调所有 Stage 一次，具体动作表/维护授权仍待接线。

- 内置强弱记录不持控制块；临时 OwnedEntity 持生命周期和 World 借用，普通 Drop 归还责任。
- 关闭整体释放含强环，不递归归零、不发通知；特殊释放不递归登记，实际授权待细化，正常销毁/槽回收采用固定排序，临时引用须先结束。

## 内置关系组件当前方案

- World 为并发使用大部分 &World 操作，内部可变性不替代访问授权，禁止共享强转整个可变 World；旧独占壳逐项迁移。
- StrongEntities/WeakEntities 只存身份数组，容器头无绑定、节点在 World 统一池；撤销 Arc 控制块 RAII 存储句柄。
- spawn/pop 返回借用 World 的 OwnedEntity，push 转移其责任；Component 显式选择封闭访问策略，普通组件选 PlainAccess，内置类型框架实现。Query/Lookup/直接点查使用同一 GAT 读写映射，内置容器读写分别返回 Ref/Mut 对象，无 Writer/EntityLinks 参数，无裸 DerefMut；普通值输入只接受 PlainAccess。容器仅 spawn/Alter 特定输入创建，释放由 ComponentInfo 的具体类型函数分派，与普通析构互斥。Removed 保留节点及引用责任至最终回收，无订阅则直接释放；归零级联阶段末按域/完整键分批排序，新归零进入下一批；组件/数组/链按固定顺序释放，Removed 按组件流/记录序号回收。不递归强删仍有拥有者的目标。

- 内置写对象成功修改后返回前标 Changed，同对象只追加一次通知；只读/拒绝/空操作不标，数组同目标替换仍标。共用普通 Mut 标记通道，变化只归属关系组件，不传播到目标组件。

只读容器提供 len/is_empty/front/iter，遍历按头至尾返回裸 Entity，不增持、不标 Changed、不授权目标组件。迭代器借用访问对象，存活时禁止同对象修改及节点回收；不同容器仍可并行操作。RemovedNotice 的 value 使用 ComponentRead：从独立旧头构造只读容器对象，不按 owner Entity 查询当前组件。实际节点保护期与 Removed 的 World 授权接线仍待闭合。

显式增持：宿主 World::try_acquire、OwnedEntity::try_clone 共用完整身份/状态校验与原子 strong 增持协议，PendingDestroy/Dead 不复活，溢出不回绕。系统通过容器 Ref/Mut 的 front_owned/iter_owned、Deque 的 back_owned/get_owned 对成员增持，不需整个 World；强数组 Ref/Mut 提供 get/get_owned，弱数组提供 get/upgrade。裸读取不计数，owned 新增责任，pop 转移原责任，三者区分。增持不改成员/顺序、不标 Changed，不授予目标组件权限。iter_owned 创建不增持，next 逐项成功后推进，拒绝不跳过当前项、不撤销已有结果，提前结束不处理余项。返回 OwnedEntity 的寿命绑定 World/运行授权而非方法或迭代器短借用；实际授权/原子实现仍待闭合。

数组初始化使用 EntityValues<寿命> 专用请求：strong_entities 消费 N 个 OwnedEntity 并转移原责任，拒绝返回完整输入；weak_entities 借用 N 个强引用准备逐份 weak，失败清理临时 weak 且不消费 strong。不公开临时 WeakEntity；重复目标按份计、N=0 合法。安装前校验登记/目标 World/分支，未安装请求释放剩余责任，安装不重复计数；载体不得擦除为 static。WeakEntitiesMut::replace_from 先取得新 weak 再替换并交还旧 weak，成功标 Changed，同目标也标，前置拒绝不改位置。具体声明见 core/{entity_values,owned_entity}.rs；宿主 change_archetype、AlterState::scope、flush_destruction 已改共享 World 接收者，以内部门禁和结构权限保证排他；OwnedEntity 只保活身份，可跨普通维护，活组件/头/guard 仍阻止冲突操作，恢复/关闭另行排除外部责任。world.rs 中仅编译样例覆盖增持后结构修改/Alter 及 pop 释放头后转入新组件，不运行失败桩；实际门禁、WorldPin 读授权的原子状态与全入口冲突检查及内部可变性证明仍待闭合。

## 快照与 crate 边界

成员字段已收敛：RawSparseArray 仅持两级寻址/分配，不再带 len、退休列表或每域重复下标。SparseArray<Entity> 通过关联成员类型独占 SparseOwners（紧凑 owner、唯一稀疏下标数组、预留范围、退休列表），SparseArray<ChangeTick> 的成员字段为零大小 ()；values/added/changed 按域/槽保存数据，以同一 owners 集合判定有效。维护入口移至 SparseOwners::compact，不再给每个基础数组提供独立成员整理。不为 RawSparseArray 提供成员 compact，也不保存独立 dense_indices 副本。

同一稀疏位置并发修改的正确性由调用者负责，已有 ECS 访问冲突管理/调度负责避免这种重叠；稀疏数组后端不再做逐槽检测或仲裁，不增加槽锁、CAS 认领或安装中编码。原子下标与 owner 只表达已确认的跨位置发布可见性，不替代访问授权、借用保护及实体整体结构发布协议。见 crates/pi_ecs_storage/src/sparse_set.rs。

稀疏下标发布已确认：非 rc 使用原子紧凑下标/不可用编码，rc 使用普通下标。安装先完成组件值/tick/映射准备，再 Release 发布 owner，最后 Release 发布稀疏下标；点查 Acquire 读下标，再 Acquire 读 owner 并核对完整 Entity。删除先失效下标、再置空 owner/登记洞，不能以这些原子操作代替借用保护。compact 在独占收束点修复下标。SparseDenseIndex 固定 u32、u32::MAX 为空码；非 rc AtomicU32、rc 普通 u32，最终编译分派须保证槽为 4 字节而非运行时 enum。预留范围最多 u32::MAX 项，退休洞 compact 前仍占容量；有界认领在耗尽前拒绝，不能回绕或分配空码。模式编译分派仍待闭合，见 crates/pi_ecs_storage/src/sparse_set.rs。

新增发布以 owner null 为不可用标记，取消独立 SparsePublication。逻辑 owners 仍为 SparseArray<Entity>，紧凑槽通过 SparseElement 映射为 SparseOwner：非 rc 原子完整键（u64），rc 普通 Entity。pi_key 完整键 ffi 编码 null=u64::MAX；预留可见前槽初始化为 null，全部安装后 Release 发布，读取 Acquire 后判空，删除原子置空并登记下标。null 不区分未发布/退休，不授权旧洞提前复用，不替代借用/回收保护。64 位原子目标约束及编译期 rc 分派待接线，当前不宣称已实现无锁存储。

非 rc owners 并发新增区分预留下标与安装完成：预留范围不是可遍历的已初始化前缀。组件值、owner、启用 tick 和双向映射全部准备完成后才发布；读取先检查发布可见性，跳过未发布/退休位置，不能先读未初始化 Entity 再判断。允许多写者乱序完成，不加结构修改锁；compact/捕获仍等待全部写者和借用收束。rc 顺序安装后更新有效范围，不增加并发发布机制。具体原子载体及失败清理接线待闭合，见 crates/pi_ecs_storage/src/sparse_set.rs。

稀疏映射维护入口为 SparseOwners::compact，rc compact 为空；已有稀疏映射 settle 描述均指此入口，不改变 World/Table 的维护名称。非 rc 待整理列表载荷固定为退休紧凑下标 usize，同一活成员删除只登记一次；compact 前旧洞绝不复用，删除后重加追加新位置，安全收束完成填洞后才清记录/截短。命名区分：底层数组分段合并用 make_contiguous，保留范围且移除范围外元素用 retain_range_and_compact；本轮未修改外部依赖公共 API，也不把映射 compact 说成组件值连续化。

稀疏数组区分 SparseArray<T> 与 RawSparseArray，类型化数组复用擦除数组的两级寻址/分配，二级仍使用 RawArr。组件后端 values 为 RawSparseArray，owners/added/changed 分别是必存 SparseArray<Entity>、Option<SparseArray<ChangeTick>>、Option<SparseArray<ChangeTick>>；每个元数据数组自带完整域/槽寻址，不是域内 Arr，也不将字段嵌入值数组域条目。owners 一律保留，包括 Lookup-only；它同时承担紧凑反向映射，不新增注册用途开关。SparseArray<T>/RawSparseArray 共用稀疏槽到紧凑下标及紧凑 pi_arr 反向地址映射，有效前缀/双向一致性定义活槽；rc 删除立即用尾部 owner 填映射洞；非 rc 删除只登记退休下标，独占 compact 再填洞；均不搬移其他组件值。反向映射以 owners 的紧凑完整 Entity 为单一来源，非 rc 不加结构修改锁，SparseOwners 持有待接入的 pi_append_vec 退休下标列表，删除只失效并登记，安全收束时 compact 才用尾部 Entity 填洞并修复域/槽下标；compact 前有效范围允许有洞，遍历必须跳过，rc compact 是空操作。rc 删除立即用尾部 owner 填洞并修复下标、缩短范围，不搬移组件值；rc compact 为空。待整理列表选用 pi_append_vec，不自行实现链表；registry 0.6.3 独立编译被旧 pi_share 的 stable 编译错误阻塞，未接入项目依赖，需兼容发布版本。追加完成后才在静止点读/清空，不以 len 作为完成发布屏障。已删除重复通用 domain/slot 反向地址表。非 owners 数组的最小字段表示及并发提交接线仍待闭合；不豁免 World 身份校验，不能以零值伪造合法 ChangeTick。见 crates/pi_ecs_storage/src/sparse_set.rs。

存储方向已确认：Table 擦除列使用 pi_arr::RawArr；稀疏组件使用两级 SparseArray，默认一级使用 pi_buckets::Pages，rc 一级由本库用 Vec 实现，二级统一使用 pi_arr 无类型数组。registry 固定 pi_arr 0.33.0 / pi_buckets 0.5.0，不切本地 path；0.34.0 发布包未包含本次 RawArr 接口，不按版本号盲升。源码定义见 crates/pi_ecs_storage/src/{column,sparse_set}.rs；rc 已确认是单线程配置，允许一级 Vec 和二级/列连续 RawArr 增长时搬移旧值，增长前须结束受影响借用、之后重绑地址缓存；一级直接按来源域索引，二级直接按域内槽索引，无 B 分块；值和完整身份/按需 tick 元数据分离，代际不参与地址计算。多线程一级必须 Pages，rc 才使用 Vec，不再将两者作为多线程可替换选项。普通删除不收缩域内数组。Pages 页长、身份解码、rc feature 与地址重绑定接线仍待闭合，当前非可运行后端。分配槽按 MaybeUninit 语义保持合法，不要求组件 Default，活组件析构由 ECS 管理，不交 RawArr 重复析构。

- core 拥有数据及 GameDataSnapshot；schedule 拥有系统状态和 GameSnapshot/GameRecovery；app 组装；partition 自动判区；macros 等手写契约稳定再补。
- 首版整体恢复游戏 World 和分配状态；渲染 World 通过稳定 key、值投影和净差异反应式协调，不共享跨 World 引用计数，不提供任意分区/实体子集回退。
- 组件/资源显式登记 Copy、Clone 或 T→S→T 捕获恢复能力，不默认 memcpy，不先 COW；历史独立不可变且可重复恢复，特殊 S 清理不操作运行计数。未保存组件先合法初始化，再由普通系统重建。
- 阶段末静止点捕获，不强制 settle；强弱计数原样保存恢复，不从拥有关系重建，调用方负责保存范围及外部责任一致。裸 Entity 非拥有副本可保留；外部强弱句柄/forget 责任建议恢复前清偿后重取。
- 可恢复到兼容登记的空 World；重绑目标且不额外增持，安装动态原型及身份分配状态，不做任意编号重映射。完整游戏恢复还需兼容 Schedule。
- 先准备非活动副本，再整体清旧（只操作旧计数，不递归归零销毁），再安装激活、推进 epoch 和失效缓存、派生重建。准备失败不替换当前状态；安装/重建失败停止模拟，不承诺回滚。恢复不逐实体触发普通生命周期通知，仍须正确析构和修复。规则历史期固定，变更由宿主清历史重建基线。
- 保存模拟时间、变化 tick、last_run、未回收通知及读者游标、未消费游戏命令/模拟定时项；Local/资源显式分类，多例按系统+参数身份恢复。Preserve 由目标预备，缓存重建，epoch 不回退且旧 Sender 不刷新。
- 系统及订阅配置历史期固定；宿主负责外部输入截点、重放去重及现实副作用隔离，渲染只收最终净差异。具体复制登记/存储/安装接线仍待实现，壳可编译不证明业务已实现。

## 本轮三个 crate 的设计壳边界

依赖固定为 core → storage / command / types，storage → types，command → types；types 不依赖其他 ECS 包。没有新增其他 crate，crates/pi_ecs_core/src 保持平铺。核心 OwnedEntity、ComponentAccess、Query、类型化通知、逻辑原型/组件注册和非泛型 World 仍在 core；不增加 World 能力体系。

- types：entity.rs 单源定义 Entity、AllocationDomain、PartitionId、EntityCategory、ArchetypeId、EntityLocation、ArchetypeRow；component.rs/resource.rs 定义 ComponentId/ResourceId；change_detection.rs 定义 ChangeTick/ChangeWindow/TrackingDemand/ComponentTicks；error.rs 定义 EcsError。位置编码、tick 和资源编号内部字段不因拆包全部公开；Mut/ChangeMarker 留 core。
- storage：ptr、column 及配对芯、table 及布局配对芯、sparse_set 及配对芯、settle，以及 storage 物理聚合与配对芯；archetype.rs 仅保存 ArchetypeMember/ArchetypeStorage，不拥有逻辑组件集合或注册策略。TableLayout::new/id/columns、Column::new/get/get_mut/prepare/insert/remove、Table::new、SparseSetStorage::hash_map、PartitionStorage::new/prepare_archetype、WorldStorage::new 是实际构造/读取所需的最小壳入口，字段仍封装；构造只接收编号、Layout、空物理所有权，不接收 core::ComponentInfo/World。所有新操作薄委托至明确失败桩，未提供可运行存储。
- command：CommandHub/CommandEndpoint、Sender/Receiver/Batch、TimerKey、SimulationTick、WorldEpoch、CommandSendError 及队列配对芯迁入独立包；CommandHub::new/register/sender/receiver/advance 接收具体服务数据。core/command.rs 保留 impl World，core/command_inner.rs 只委托 CommandHub 壳；芯不调用其他芯，发送端不持整个 World，原 Send 约束不放宽。

原 core 的 entity/error/component::ComponentId/resource::ResourceId/change_detection::{ChangeTick,ChangeWindow,TrackingDemand}/table/command 公共路径通过重导出保持同一类型，World 命令方法签名不变；原私有 ptr/storage 路径保留必要适配，已删除无消费者的旧 column/settle/sparse_set 文件和迁走的配对芯。原型声明仍在 core/archetype.rs，物理成员只在 storage 定义。

registry 版本继续固定 pi_arr=0.33.0、pi_buckets=0.5.0、pi_share=2.1.0；pi_append_vec 0.6.3 的发布兼容阻塞及占位保留，没有接本地 path。rc 仍未全图贯通，当前目录枚举/线程和地址保护接线不是可用的 rc 支持；已确认 rc Vec/连续增长/即时映射填洞/compact 空，非 rc Pages/原子 Share/延迟填洞，不反转。

验证范围为 src、crates、design/unified-partitioned-ecs、Cargo.toml、Cargo.lock。新包首次 cargo check 更新锁后再 locked；最终执行 cargo fmt --all、cargo check --workspace --locked --all-targets --message-format=short、cargo doc --workspace --locked --no-deps，以 checkRound 的 success/current 证据为准。拆分前默认全目标检查已通过；这是设计稿编译验证，不运行触发失败桩的业务测试，未进行独立轻审。工作区无 Git，git diff 不能提供历史差异证据。
