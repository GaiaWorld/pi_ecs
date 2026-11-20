//! 内置查询数据描述及共享状态；无实际存储读取实现。
use crate::{
    change_detection::ChangeWindow,
    component::{Component, ComponentId, PlainAccess},
};
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
};
/// 存在性查询；返回 bool，不要求原型包含 T，不读取组件值。
pub struct Has<T: Component>(PhantomData<fn() -> T>);
/// 缺组件时读取所属 World 的默认值，存在时读取组件；只读，不插入组件。
pub struct OrDefault<T: Component<Access = PlainAccess>>(PhantomData<fn() -> T>);
/// 同 OrDefault，返回带来源变化检测的 ValueRef，不为缺失实体制造组件 tick。
pub struct OrDefaultRef<T: Component<Access = PlainAccess>>(PhantomData<fn() -> T>);
/// 默认来源独立于普通 Res<T>；显式登记，不要求 T:Default，随组件统一要求 T: Send + Sync。
/// 不在 fetch 调用户工厂。
/// 运行期间不可替换，修改只走 World 独占维护；不增加独立默认权限表或锁。
pub struct QueryDefaults {
    /// 按 Rust 类型保存每 World 唯一默认值，独占维护点才能增删替换。
    pub(crate) values: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}
/// 只读值及变化状态；普通 Ref 使用组件，OrDefaultRef 缺失分支使用 World 默认值。
pub struct ValueRef<'a, T> {
    /// 值由本次授权保活，不可变访问，无 DerefMut。
    pub(crate) value: &'a T,
    /// 组件分支记录快照 tick；默认分支为 None，不冒充实体组件变化。
    pub(crate) ticks: Option<(
        crate::change_detection::ChangeTick,
        crate::change_detection::ChangeTick,
    )>,
    /// 显式宿主窗口或系统窗口；必须存在，缺失在绑定入口报错。
    pub(crate) window: ChangeWindow,
}
/// 查询组件变化的描述类型；结果是 ValueRef，登记 added/changed tick 需求，不订阅通知。
/// Ref/OrDefaultRef 必须绑定系统或宿主显式窗口，缺窗口报错，不保存或推进观察进度。
pub struct Ref<T: Component<Access = PlainAccess>>(PhantomData<fn() -> T>);
/// 单组件查询长期状态，不保留值地址；默认来源的授权独立于普通资源类型。
pub struct ComponentQueryState {
    /// 已登记组件身份，不隐式注册。
    pub(crate) component: ComponentId,
    /// 可选默认来源类型；OrDefault 系列声明组件 T 的可选读，不额外登记默认值访问权限。
    pub(crate) default_type: Option<TypeId>,
}
/// 单组件长期 fetch 状态；直接保存列/稀疏容器地址，不经统一缓存表、槽号或包装往返。
pub struct ComponentQueryFetchState {
    /// 本原型组件来源，None 表示可选缺失。
    pub(crate) source: Option<crate::archetype::ComponentSource>,
    /// 直接区分缺失、列地址和稀疏地址；World 身份由所属 QueryState 核对。
    pub(crate) storage: ComponentStorageBinding,
}
/// # 用途
/// 内置查询声明/组合的静态编排入口，不是运行 fetch；不持权限、后端或业务值。
pub(crate) struct QueryDataBinding<Q>(
    /// 仅用于关联静态方法到查询形状，不参与运行取值。
    pub(crate) PhantomData<fn() -> Q>,
);
/// # 用途
/// 单组件本次取值器；视图按值持有但借用原授权后端，不引用自身字段或临时缓冲。
/// Q 决定普通/可选/写/变化/默认结果映射，C 是真实组件类型；不可 Clone 或重置。
pub struct ComponentFetch<'w, Q, C: Component> {
    /// 原始授权、窗口和统一写 tick；不复制或取得额外权限。
    pub(crate) access: crate::query_fetch::FetchAccess<'w>,
    /// 本次范围，Table 允许全部变体，Archetype 要求完整原型一致。
    pub(crate) scope: crate::query_fetch::FetchScope,
    /// 直接取值的冻结后端描述；没有值引用或逐实体地址表。
    pub(crate) storage: ComponentView<'w>,
    /// prepare 从同源组件元数据绑定的屏障后需求，结果期不可切换追踪。
    pub(crate) demand: &'w crate::change_detection::TrackingDemand,
    /// 仅启用 changed_notice 时绑定；冻结共享目录，追加时才短借目标流，不持 RefMut。
    pub(crate) notifications: Option<&'w crate::notification::Notifications>,
    /// 仅默认查询的缺失分支绑定，普通/存在分支为 None；不在 fetch_at 查类型或创建默认值。
    pub(crate) default: Option<&'w C>,
    /// 保留查询结果形状，不持 Q 实例。
    pub(crate) marker: PhantomData<fn() -> Q>,
}
/// # 用途
/// 本次后端视图；可重复共享描述，不可重复兑现同一个值的写引用。
/// 生命周期来源于原授权保护的后端；self 短借或 Drop 不改变派生单槽结果寿命。
pub(crate) enum ComponentView<'w> {
    /// 当前范围无该组件；Option 返回 None，默认查询使用已绑定 default。
    Missing,
    /// 表列内部可变后端，仅按目标 table_row 形成引用。
    Column(
        /// 与原授权等长的后端共享描述，不重借整个列为 &mut。
        pi_ecs_storage::column::ColumnView<'w>,
    ),
    /// 稀疏后端，按完整 Entity 校验 owner 后定位单槽。
    Sparse(
        /// 域目录/owner 冻结，值和 tick 经单槽内部可变入口访问。
        pi_ecs_storage::sparse_set::SparseView<'w>,
    ),
}
/// # 用途
/// Has<C> 本次存在性取值器；prepare 固定结果，无需绑定组件值后端。
pub struct PresenceFetch<'w, C: Component> {
    /// 原授权，用于校验目标与范围，不含值访问器。
    pub(crate) access: crate::query_fetch::FetchAccess<'w>,
    /// Uniform 必须对所有变体有相同存在性，其他情况为单原型范围。
    pub(crate) scope: crate::query_fetch::FetchScope,
    /// 由已绑定长期来源判定，不按实体搜索稀疏存储。
    pub(crate) present: bool,
    /// 已登记真实组件类型；不持 C 值。
    pub(crate) marker: PhantomData<fn() -> C>,
}
/// # 用途
/// Entity/EntityLocation/空组合/PhantomData/原型声明等非组件值取值器。
/// 不绑定业务后端；原型声明查询准备时绑定只读元数据，不逐实体还原整个 World。
pub struct MetadataFetch<'w, Q> {
    /// 原授权，仅校验目标、范围和生命周期。
    pub(crate) access: crate::query_fetch::FetchAccess<'w>,
    /// 真实匹配范围，不以基础原型代表全表。
    pub(crate) scope: crate::query_fetch::FetchScope,
    /// 仅原型声明结果需要，按实际实体原型身份读取冻结声明；其他项为 None。
    pub(crate) archetypes: Option<&'w [crate::archetype::Archetype]>,
    /// 结果形状标记，不保存临时结果。
    pub(crate) marker: PhantomData<fn() -> Q>,
}
#[path = "query_data_inner.rs"]
mod query_data_inner;
impl<T> std::ops::Deref for ValueRef<'_, T> {
    type Target = T;
    /// 只读值，不推进进度或标记变化。
    fn deref(&self) -> &T {
        Self::value_inner(self)
    }
}
impl<T> ValueRef<'_, T> {
    /// 组件 added 是否在窗口内；默认分支 false。
    pub fn is_added(&self) -> bool {
        Self::added_inner(self)
    }
    /// 组件 changed 是否在窗口内；默认分支 false，不观察默认值替换为组件变化。
    pub fn is_changed(&self) -> bool {
        Self::changed_inner(self)
    }
}
impl crate::world::World {
    /// T 已登记为 PlainAccess 组件；独占维护时设置默认值，返回旧值；不插入实体或产生组件通知。
    /// 活查询/借用冲突按既有门禁拒绝；运行期间不能替换，不创建独立锁或权限表。
    /// 必须由宿主 &mut World 调用并检查 Idle；普通 HashMap 通过真实独占字段借用修改，
    /// 不以维护门禁或裸指针从共享 &World 制造可变引用。普通错误不替换旧值；展开标停。
    /// 默认值不隐式捕获，首版恢复功能暂缓；OrDefault 绑定缺失默认来源报错，不在 fetch 创建。
    pub fn set_query_default<T: Component<Access = PlainAccess>>(
        &mut self,
        value: T,
    ) -> Result<Option<T>, crate::error::EcsError> {
        Self::set_default_inner(self, value)
    }
}
// 不复用 ResourceId 作为默认来源身份，防止普通资源与默认值冲突域不一致。
// Ref/OrDefaultRef 的组件分支启用 added/changed tick；默认分支始终不算实体变化。
// 默认来源由独占维护隔离，无独立读写权限域；OrDefault 系列为可选组件读，不作为必需匹配条件。

/// # 用途
/// 单组件 fetch 字段的直接存储来源；非拥有地址不保活 World，不缓存值/tick 缓冲。
pub(crate) enum ComponentStorageBinding {
    /// 已绑定且本原型不含组件，可选取值 None；默认项另用已校验默认来源。
    Missing,
    /// 直接指向固定列容器；所属 Table 由获取计划的范围核验。
    Column(
        /// 从存储后端稳定根导出的 Column 地址，不从临时 &mut 派生。
        std::ptr::NonNull<pi_ecs_storage::column::Column>,
    ),
    /// 直接指向全 World 单组件稀疏容器，按完整 Entity 校验 owner。
    Sparse(
        /// 固定 SparseSetStorage 地址，不指向可增长缓冲。
        std::ptr::NonNull<pi_ecs_storage::sparse_set::SparseSetStorage>,
    ),
}

impl<'w, Q, C: Component> ComponentFetch<'w, Q, C> {
    /// # 用途
    /// entity 已通过完整身份/位置/范围与过滤，ticks 是同组件同实体从视图 write 取得的唯一槽。
    /// 核对 write_tick 存在、demand 与槽启用一致以及通知目录存在性，再构造 ChangeTarget::Component。
    /// 仅 changed_notice 可产生 NoticeWrite，锁定本组件和完整 entity；无 tick 需求仍允许只通知。
    /// 包装本身不标记、不取新 tick、不改 added；实际变化沿普通 Mut/内置对象既有标记规则发生。
    /// # Safety
    /// ticks 的唯一性覆盖 'w，不能在此前保留同槽 tick 读引用；调用方只对唯一活写结果消费一次。
    pub(crate) unsafe fn change_marker(
        &self,
        entity: &crate::query_fetch::FetchEntity,
        ticks: pi_ecs_types::change_detection::ComponentTicksMut<'w>,
    ) -> crate::change_detection::ChangeMarker<'w> {
        self.change_marker_inner(entity, ticks)
    }
}

// RC 冻结共享视图与单槽读写边界已声明，仍须实现验证，不凭接口签名认定安全链通过。
// @to_design 第二轮非 RC：分段/桶后端发布、长期根与共享视图的并发安全证明。
// RC 当前无增长授权内连续数组不搬移；非 RC 分段主区/桶须保持发布与内部可变性，不能构造覆盖活写结果的切片。
// 非 RC 的 Column/SparseSetStorage 容器长期根地址导出尚未落壳证明：目录增长、对象移动与独占维护均须不使缓存失效。
// 非 RC 并发 Added/Changed 观察经 AccessDeclaration/ResolvedAccess.tick_reads 与值 Write 声明排斥；
// 普通 Option<ChangeTick> 不是原子，容器 Sync 不提供同步；冲突及排序芯尚未兑现，不能宣称已安全并行。
// 本轮冻结视图契约统一排除受影响存储结构修改；若后续非 RC 允许共享增长，须另证目录与元数据并发读取，不从地址稳定直接放行。
