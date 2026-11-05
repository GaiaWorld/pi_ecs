//! 固定增删集合的声明化批量变更；闭包入口负责发布，不依赖 Alter 的 Drop。
use crate::{
    component::ComponentId,
    entity::{ArchetypeId, Entity, EntityLocation},
    entity_values::EntityValues,
    error::EcsError,
    world::World,
};
use std::collections::HashMap;
/// 长期复用的增删声明和源到最终原型映射，不持有业务值或存储指针。
/// 第一版手写组件编号集合；静态 Bundle A/D 适配不在本次伪造实现。
pub struct AlterState {
    /// 严格新增类型；规范化且无重复，必须已登记。
    pub(crate) added: Box<[ComponentId]>,
    /// 严格删除类型；必须已登记，与 added 不相交。
    pub(crate) removed: Box<[ComponentId]>,
    /// 每源原型的逻辑转换计划，不长期缓存物理行或列指针。
    pub(crate) changes: HashMap<ArchetypeId, crate::structural_change::ArchetypeChange>,
    // @to_design 与 QueryState 共用的 World 身份/注册兼容校验，系统访问集合实际表示待闭合。
}
impl AlterState {
    /// world 已冻结，added/removed 为本 World 的登记编号，无重复且不相交；空集合允许。
    /// 成功复制固定声明，后续不得改变；未知编号或非法集合返回错误，不改业务状态。
    /// 不隐式登记目标原型，目标组合须已显式 register_archetype。
    pub fn new(
        world: &World,
        added: &[ComponentId],
        removed: &[ComponentId],
    ) -> Result<Self, EcsError> {
        crate::alter_inner::new_inner(world, added, removed)
    }
    /// world 必须与本状态兼容；通过共享 World 内部取得维护/结构排他授权，run 只接收 Alter。
    /// 宿主 OwnedEntity/EntityValues 可跨普通维护；活组件/头/guard 冲突则入口拒绝且不执行 run。
    /// 系统内拥有引用仍绑定运行授权，不能逃到阶段外；不因共享签名放宽借用或恢复隔离。
    /// 宿主校验/授权/批次准备成功后、调用 run 前分配一次非零 tick，全部请求及发布共用；系统使用 this_run。
    /// 耗尽不执行 run、不接受写入，释放本次授权并报错；不逐请求/实体分配 tick。
    /// run 正常退出后统一发布所有成功接受的变更，返回 run 的值；入口校验失败不执行 run。
    /// 每次 alter 的普通错误仅拒绝本次请求；此前成功请求仍在正常退出时提交，不做整批回滚。
    /// run panic 或执行失败直接传播 panic，不提交补偿或继续使用 World；不在 Drop 再次提交。
    /// 高阶闭包阻止借用本次 Alter 的引用作为 R 逃逸；R 不是整批提交成功的替代标记。
    pub fn scope<R>(
        &mut self,
        world: &World,
        run: impl for<'a> FnOnce(&mut Alter<'a>) -> R,
    ) -> Result<R, EcsError> {
        /* 设计已确认：
        校验所属 World 和固定声明，内部取得维护/结构排他授权创建批次，冲突报错不等待。
        宿主准备成功后取得批次 tick，再调用 run；系统可信路径注入 this_run。
        run 中逐请求完成校验和预留，新增值当场写入目标隐藏行并记录初始化状态；同 Table 路径单独处理。
        正常退出后按源/目标计划集中搬保留列，处理删除值，维护两种成员行并发布。
        同 Table 保留共有列；Sparse→Sparse 保留值；完成后才返回闭包结果。
        panic 直接传播，不回滚，不通过 Alter::drop 隐式发布。
        */
        crate::alter_inner::scope_inner(self, world, run)
    }
}
/// 闭包内的受限批量变更句柄，不能独立构造、取消或取出 World。
/// 隐藏目标行及初始化责任由批次拥有，遗忘句柄不能暴露未初始化值。
pub struct Alter<'a> {
    /// 内部批次状态由 scope 保活，不向调用者暴露其写权限。
    pub(crate) batch: &'a mut AlterBatch,
}
impl Alter<'_> {
    /// entity 为完整存活键，values 精确覆盖固定 added，不能替换共有组件。
    /// 源必须包含全部 removed 且不含任一 added；目标已登记，同作用域重复 entity 返回错误。
    /// 普通校验/准备错误在任何本请求写入前返回，不影响先前已接受请求；values 被消费。
    /// 接受时当场写入目标隐藏存储，分配不移动其他存活值，新增集合尚不可经普通查询/Lookup 观察。
    /// scope 正常退出或系统参数 finish 才搬移保留值、处理删除并发布；不靠 Drop 提交。
    /// 本入口不返回移出值；移出的值在提交中用登记能力显式析构，执行失败直接 panic。
    pub fn alter(&mut self, entity: Entity, values: EntityValues<'_>) -> Result<(), EcsError> {
        crate::alter_inner::alter_inner(self, entity, values)
    }
}
/// 一次接受的请求，拥有隐藏存储初始化责任，不是可回滚事务对象。
pub(crate) struct AlterRequest {
    /// 完整身份及接收时的源位置；本次独占作用域内不能被外部结构修改。
    pub(crate) source: (Entity, EntityLocation),
    /// 显式登记的最终原型，不表示已发布。
    pub(crate) target: ArchetypeId,
    /// @to_design 隐藏目标位置及逐槽初始化/释放责任，不能假设每次转换都新建 Table 行。
    pub(crate) _pending_hidden: (),
}
/// 闭包持有期间的请求集合，批次始终由 scope 所有，不依赖可被遗忘句柄的 Drop。
pub(crate) struct AlterBatch {
    /// 宿主 scope 分配或系统 this_run 注入；隐藏新增/逻辑发布的 Added/Changed/Removed 共用。
    pub(crate) tick: crate::change_detection::ChangeTick,
    /// 接收顺序；重复身份禁止，分组内保持接收顺序，不按哈希遍历决定执行顺序。
    pub(crate) requests: Vec<AlterRequest>,
    // @to_design 与受限存储访问通道、全部请求准备以及按列执行壳的连接待闭合。
}

/// 查询式系统 Alter，Q/F 限制可修改源原型，A/D 首版由固定编号配置，不新增 Bundle 实现。
/// 源及对应目标均为结构写，另计 Q 的值权限；不能修改查询声明外实体。
pub struct QueryAlter<
    'a,
    Q: crate::query::QueryData,
    F: crate::query::QueryFilter = crate::query::NoFilter,
> {
    /// 当前查询与本次批次的受限组合，不持整个可变 World。
    pub(crate) _pending_access: std::marker::PhantomData<(&'a mut (), Q, F)>,
}
/// 查询式 Alter 的本次游标，结果短借用结束后才可再次取值或变更当前实体。
pub struct AlterIter<
    'a,
    Q: crate::query::QueryData,
    F: crate::query::QueryFilter = crate::query::NoFilter,
> {
    /// 借用查询式参数，禁止另一游标同时访问同批状态。
    pub(crate) source: &'a mut QueryAlter<'a, Q, F>,
    /// 最近成功返回且可操作的实体；耗尽后清空，失败不得沿用旧当前项。
    pub(crate) current: Option<Entity>,
}
impl<Q: crate::query::QueryData, F: crate::query::QueryFilter> QueryAlter<'_, Q, F> {
    /// entity 须通过源声明及过滤；短借用返回 Q 结果，内部别名检查不可豁免。
    pub fn get_mut(&mut self, entity: Entity) -> Result<Q::Item<'_, '_>, EcsError> {
        crate::alter_inner::query_get_inner(self, entity)
    }
    /// 借用本参数创建源查询游标；不遍历目标隐藏行，不允许重复实体结构修改。
    pub fn iter_mut(&mut self) -> AlterIter<'_, Q, F> {
        crate::alter_inner::iter_inner(self)
    }
    /// entity 必须属于查询对应合法源；values 精确覆盖固定新增集合，语义同 Alter::alter。
    pub fn alter(&mut self, entity: Entity, values: EntityValues<'_>) -> Result<(), EcsError> {
        crate::alter_inner::query_alter_inner(self, entity, values)
    }
}
impl<Q: crate::query::QueryData, F: crate::query::QueryFilter> AlterIter<'_, Q, F> {
    /// 返回下一项短借用结果；旧结果释放后可 alter 当前实体或继续前进，耗尽为 None。
    /// 首版不实现长寿命 Iterator，避免迭代器同时修改已交出的组件结果。
    pub fn fetch_next(&mut self) -> Option<Q::Item<'_, '_>> {
        crate::alter_inner::iter_next_inner(self)
    }
    /// 修改最近成功取出的当前实体，values 精确覆盖 A；未取项/已耗尽/同批重复为错误。
    /// 当场写隐藏行，延后迁移/删除/发布；必须先释放当前项借用。
    pub fn alter(&mut self, values: EntityValues<'_>) -> Result<(), EcsError> {
        crate::alter_inner::iter_alter_inner(self, values)
    }
}
// @to_design QueryAlter 与宿主 Alter 的统一命名、长期 QueryState/AlterState 组合及 SystemParam 接线待细化。

// 宿主 scope 通过共享 World 取得内部排他授权；系统查询式参数见 QueryAlter，同批新增值当场转入隐藏目标存储。
// 普通拒绝先于写入；已开始写入/搬移失败 panic 停止，清理须记录初始化责任且不能重复析构。
