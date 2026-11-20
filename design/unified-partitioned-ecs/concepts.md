# 统一分区 ECS：概念

本文解释当前 RC 的常用概念。实现顺序、安全边界和源码索引见 [实现设计](design.md)。

## World、实体与分区

| 概念 | 含义 |
| --- | --- |
| World | 实体、组件、资源和时钟的所有者；实体键在本 World 内解释。 |
| Entity | 带槽和代际的裸身份。复制它不会增加计数；不能拿另一 World 的同号键当同一实体。 |
| AllocationDomain | 出生域，编码在实体键中，分配后不变。 |
| PartitionId | 当前位置中的分区。显式迁区只改这个字段，不改变实体键、组件位置或变化记录。 |
| OwnedEntity | 一份 strong 拥有责任。可显式增持或转交；Drop 归还责任。 |

strong 归零后先登记待销毁。实体在位置隐藏前仍可能被查询，但不能重新增持。关闭 World 会整体清偿，包括尚未归零的拥有环。

## 原型和物理存储

**原型**是一个完整组件组合，带一个成员集合；**Table** 为所选列提供物理存储。一张 Table 可以承载多个原型变体，因此实体的原型成员行与 Table 物理行不一定相同。`SparseOnly` 组件始终在稀疏存储；`TableEligible` 组件仅在所选 Table 覆盖它时进入列，否则仍在稀疏存储。无论当前分区如何，每个实体的同种组件只有一份活值。

组件类型和基础 Table 先登记再冻结；创建或严格增删可按需发布原型。普通移除可能留洞；宿主可在静止点调用 `settle` 整理存储。

## 访问和查询

- **宿主借用**：`WorldPin` 读组件或资源；`WorldWrite` 访问现有槽，写对象共享本令牌的 tick；`EntityScope` 还可创建、增删和取得拥有引用。需要把某次操作取得的拥有责任交给下一次操作时，用同一个 `EntityScope`，先结束组件短借。
- **查询**：数据描述 `QueryData` 决定返回哪些值，`QueryFilter` 决定选哪些实体。`QueryState` 保存长期匹配状态，`Query` 和 `QueryMut` 使用本次授权，`Lookup` 只点查。可选组件缺席给出 `None`，不排除实体。
- **具名查询**：`#[derive(QueryData)]` 可把元组式查询写成具名结构体，例如 `struct Motion { entity: Entity, position: &'static Position }`。`Query<'_, Motion>` 返回 `MotionItem`，可访问 `item.entity` 和 `item.position`。可写字段使用 `QueryMut`；字段的实际类型由组件策略决定，不一定是普通引用。
- **点查错误**：组合点查区分不存在（`NotFound / NoSuchEntity`）与存在但不匹配（`InvalidInput / NotMatched`）。重叠的可变访问在取值前拒绝。

## 创建、增删与关系组件

**Bundle** 描述本次写入的组件类型和对应值。宿主可直接创建或严格增删；系统内用 `Spawn<B>` 创建固定形状的实体，用 `QueryAlter<Q, F, Add, Remove>` 查询并修改结构。严格增删要求新增组件原先不存在、移除组件原先存在。成功结果在该调用返回前发布。

**关系组件**包括 List、Queue、Deque 以及强弱引用数组。它们提供专用读写对象；读裸键不增持，owned 读取新增一份责任，pop 移出并转交容器原有的一份责任。普通组件使用 `PlainAccess`；通用宿主和查询入口通过同一组件策略返回恰当的访问对象。

## 时钟、变化与通知

**ChangeTick** 是变化时钟，**ChangeWindow** 表达观察区间；系统自动使用自己的上次成功运行与本次运行时间，宿主显式提供窗口。读取值或取得可写对象本身不算修改。Added/Changed 查询过滤观察活槽的 tick；通知读者消费独立的历史流，两者可以独立使用。

**Added/Changed** 通知记录身份与时刻；**Removed** 还拥有移出的旧值，读到的值只能在批次借用期使用。每个读者有自己的游标，慢读者会保留其尚未确认的历史。Stage 成功后进行通知回收与归零销毁；这不是 `settle`。命令使用独立的 **SimulationTick** 控制定时可见性，不等同于变化时钟。

## 系统、阶段与应用

**System** 声明数据访问，**Stage** 检查并排序，**Schedule** 顺序运行各 Stage；当前 RC 用串行 **Executor**。**ParamSet** 让同一个系统分时使用参数，每次只借用当前成员。**Local** 归系统参数所有；唯一资源和多例资源归 World，生产者占用自己的登记槽。

**App** 拥有 World 和命名 Schedule。插件负责组装，宿主选择何时完成配置、运行哪个 Schedule 以及何时关闭。快照恢复、自动判区、非 RC 运行和外层执行图不属于当前支持范围。
