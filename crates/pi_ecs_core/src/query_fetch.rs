//! 查询计划、访问令牌与取值协议；不实现原始指针读取。
use crate::{
    archetype::ComponentSource,
    component::ComponentId,
    entity::{ArchetypeId, Entity, EntityLocation, PartitionId},
    error::EcsError,
    query::QueryData,
    world::World,
};
use std::{marker::PhantomData, ptr::NonNull};
/// 一个查询项的已解析组件获取路径；可缓存，不保存物理地址。
pub struct ComponentFetchPlan {
    /// 登记后的组件编号。
    pub(crate) component: ComponentId,
    /// 必需组件的唯一来源；可选组件不存在时为 None。
    pub(crate) source: Option<ComponentSource>,
}
/// 已匹配原型的逻辑计划，同一个声明可用于各分区，不能缓存实体行号。
pub struct ArchetypeFetchPlan {
    /// 本计划对应的原型编号。
    pub(crate) archetype: ArchetypeId,
    /// 按查询数据描述顺序保存组件路径；Entity 查询项不消耗组件路径。
    pub(crate) components: Box<[ComponentFetchPlan]>,
}
/// 本次访问的内部权限令牌；仅可信查询入口构造，不提供安全的裸 World 逃逸。
/// 生命周期只约束存储存活；可变权限必须由独占入口或系统访问证明授予，不能从共享 World 制造。
/// 可变查询只授权已校验的组件集合；共享指针不自动授权可写访问。
pub struct FetchAccess<'w> {
    /// 稳定且存活的 World，不能在本次访问期间结构修改或整理。
    pub(crate) world: NonNull<World>,
    /// 关联实际借用生命周期；权限读写类型由不可外部构造的入口保证。
    pub(crate) lifetime: PhantomData<&'w World>,
    /// 已校验的权限范围；系统入口由 RunAccess 的短借用提供，不复制授权。
    pub(crate) declaration: &'w crate::access::AccessDeclaration,
    // @to_design papaya guard 应由覆盖所有返回引用的访问作用域持有；不能由局部 fetch 持有后提前释放。
    // guard 表与宿主独占适配签名待闭合，不据生命周期标记推导 Send/Sync。
}
/// 本次目标实体及其已校验位置，供过滤和取值共用，不跨结构修改保存。
pub struct FetchEntity {
    /// 完整身份，不能只用 index 访问 HashMapSparseSet。
    pub(crate) entity: Entity,
    /// 同时包含 Table 行及 archetype_row 的当前位置。
    pub(crate) location: EntityLocation,
}
/// 单个分区原型的可执行 fetch；内部持有列/稀疏访问器，不持有实体结果缓存。
/// 仅由核心 QueryData 关联实现，禁止外部伪造可变引用生命周期。
pub trait QueryFetch<'w, Q: QueryData>: super::query::private::Sealed {
    /// 从已校验逻辑计划准备本次访问器，不扫描实体，不重新解析类型。
    /// access 必须属于 plan 所属 World，partition 是实际当前区；错误不修改业务值。
    /// # Safety
    /// 调用方已校验 Q 内部冲突、World/原型兼容及权限，访问期间排除结构修改。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        partition: PartitionId,
        plan: &ArchetypeFetchPlan,
        state: &Q::State,
        fetch_state: &Q::FetchState,
    ) -> Result<Self, EcsError>
    where
        Self: Sized;
    /// 按 entity 的当前行取值，Column/Sparse OwningPtr 在此边界才转换为 Q 的引用组合。
    /// # Safety
    /// entity 属于已准备区/原型且存活、匹配；过滤已通过。返回可变值时，调用方必须保证
    /// 在所有仍存活结果及其他访问器间没有冲突；同一实体重复 fetch 不能自动成立。
    /// 结果不得借用 self 内部临时缓冲，应指向由 access 保活的存储。
    unsafe fn fetch_at(&mut self, entity: &FetchEntity) -> Q::Item<'w, 'w>;
}
/// 过滤与数据取值分离；状态不允许偷偷修改业务组件或产生未声明写访问。
pub trait FilterFetch<'w>: super::query::private::Sealed {
    /// entity 为已匹配原型内的有效目标，true 表示通过；点查和遍历调用相同语义。
    /// # Safety
    /// 访问器已为目标区/原型准备，访问令牌和过滤上下文有效，不产生冲突引用。
    unsafe fn matches(&mut self, entity: &FetchEntity) -> bool;
}
/// 同一视图内每个区/原型的已准备访问器；成员行和组件存储不可在此期间搬动。
pub(crate) struct PreparedFetch<'w, Q: QueryData, F: crate::query::QueryFilter> {
    /// 当前原型数据访问器。
    pub(crate) data: Q::Fetch<'w>,
    /// 同原型的实体过滤器。
    pub(crate) filter: F::Fetch<'w>,
}
