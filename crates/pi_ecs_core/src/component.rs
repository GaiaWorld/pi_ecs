//! 显式组件登记所需的独立定义；不隐式要求 Clone、Default 或 Send/Sync。
use std::{alloc::Layout, any::TypeId};
/// 用户组件显式选择框架访问策略；无所有 static 类型的 blanket 实现，宏后置。
/// 普通组件手写 type Access = PlainAccess；内置类型由框架实现，不得重选普通策略。
pub trait Component: Sized + 'static {
    /// 同时决定共享/写入结果及登记分类，注册元数据不能与此策略分离生成。
    type Access: ComponentAccess<Self>;
}
/// 框架封闭的访问策略，用户不能实现或构造不受授权的 fetch。
/// 返回类型映射在编译期决定，ComponentInfo 的运行时分类仅服务存储与释放。
pub trait ComponentAccess<C: Component>: access_private::Sealed<C> {
    /// 'a 为组件/头与节点读取借用，'w 仅为转出拥有责任的 World/运行授权寿命。
    /// 宿主 'w 可越过 pin/write，系统 'w 不得超过运行授权；不借用临时 fetch 缓冲。
    type Read<'a, 'w>;
    /// 写结果不必是裸引用；内置类型禁止向头的 DerefMut/AsMut/into_inner。
    type Write<'a, 'w>;
}
/// 普通值组件策略；外部用户可为自己的组件选择，不适用于框架已实现的内置类型。
pub struct PlainAccess;
/// 框架内置列表策略。
pub struct ListAccess;
/// 框架内置队列策略。
pub struct QueueAccess;
/// 框架内置双端链策略。
pub struct DequeAccess;
/// 框架内置定长强引用策略。
pub struct StrongAccess;
/// 框架内置定长弱引用策略。
pub struct WeakAccess;
mod access_private {
    pub trait Sealed<C> {}
}
impl<C: Component<Access = PlainAccess>> access_private::Sealed<C> for PlainAccess {}
impl<C: Component<Access = PlainAccess>> ComponentAccess<C> for PlainAccess {
    type Read<'a, 'w> = &'a C;
    type Write<'a, 'w> = crate::change_detection::Mut<'a, C>;
}
macro_rules! container_access {
    ($container:ident, $read:ident, $write:ident, $access:ident) => {
        impl<T: 'static> Component for crate::entity_container::$container<T> {
            type Access = $access;
        }
        impl<T: 'static> access_private::Sealed<crate::entity_container::$container<T>>
            for $access
        {
        }
        impl<T: 'static> ComponentAccess<crate::entity_container::$container<T>> for $access {
            type Read<'a, 'w> = crate::entity_container::$read<'a, 'w, T>;
            type Write<'a, 'w> = crate::entity_container::$write<'a, 'w, T>;
        }
    };
}
container_access!(EntityList, EntityListRef, EntityListMut, ListAccess);
container_access!(EntityQueue, EntityQueueRef, EntityQueueMut, QueueAccess);
container_access!(EntityDeque, EntityDequeRef, EntityDequeMut, DequeAccess);
macro_rules! array_access {
    ($array:ident, $read:ident, $write:ident, $access:ident) => {
        impl<const N: usize, T: 'static> Component for crate::owned_entity::$array<N, T> {
            type Access = $access;
        }
        impl<const N: usize, T: 'static> access_private::Sealed<crate::owned_entity::$array<N, T>>
            for $access
        {
        }
        impl<const N: usize, T: 'static> ComponentAccess<crate::owned_entity::$array<N, T>>
            for $access
        {
            type Read<'a, 'w> = crate::owned_entity::$read<'a, 'w, N, T>;
            type Write<'a, 'w> = crate::owned_entity::$write<'a, 'w, N, T>;
        }
    };
}
array_access!(
    StrongEntities,
    StrongEntitiesRef,
    StrongEntitiesMut,
    StrongAccess
);
array_access!(WeakEntities, WeakEntitiesRef, WeakEntitiesMut, WeakAccess);
/// 统一共享访问结果，Query/Lookup/直接点查都使用相同策略。
pub type ComponentRead<'a, 'w, C> = <<C as Component>::Access as ComponentAccess<C>>::Read<'a, 'w>;
/// 统一写访问结果，Option/元组/批量仅组合此映射，不能恢复裸内置头。
pub type ComponentWrite<'a, 'w, C> =
    <<C as Component>::Access as ComponentAccess<C>>::Write<'a, 'w>;
pub use pi_ecs_types::component::ComponentId;
/// 组件的存储约束；实际获取位置由具体原型确定。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoragePolicy {
    /// 可以进入预设 Table；未覆盖时使用当前分区独立的默认稀疏后备。
    TableEligible,
    /// 显式稀疏，禁止进入 Table，使用跨分区共享的组件后端。
    SparseOnly,
}
/// 组件类型元数据；由 register_component 从 Rust 类型安全生成，不接受外部拼装描述。
/// 与保存/派生规则分离，登记本身不承诺值能够复制。
pub struct ComponentInfo {
    /// 本 World 的组件编号。
    pub(crate) id: ComponentId,
    /// Rust 类型身份，仅在本进程类型系统内使用。
    pub(crate) type_id: TypeId,
    /// 用于诊断的 Rust 类型名称，不作为持久兼容协议。
    pub(crate) name: &'static str,
    /// 单值大小和对齐，包括零大小类型。
    pub(crate) layout: Layout,
    /// 类型安全登记时生成的析构函数；没有析构需求时为 None。
    /// 调用者必须保证指针正确对齐、指向一个活的本类型值并拥有唯一析构权。
    pub(crate) drop_value: Option<unsafe fn(*mut u8)>,
    /// 内置引用/容器为特殊创建和释放种类；普通值为 None，具体前段编号分配待细化。
    pub(crate) builtin: Option<BuiltinEntityComponent>,
    /// 登记具体内置类型时生成的擦除释放入口，捕获 N/T 的类型布局，不在释放时查 TypeId。
    /// builtin 为 Some 时必须存在且 drop_value 为 None；普通值相反，不重复执行两种析构。
    pub(crate) release_builtin: Option<BuiltinRelease>,
    /// 类型在本注册生命周期内固定的存储约束。
    pub(crate) storage: StoragePolicy,
    /// 查询/通知登记需求的汇总；未启用项不写 tick 或追加对应通知。
    pub(crate) tracking: crate::change_detection::TrackingDemand,
}

/// 框架内置关系组件种类；不能由普通构造/可变解引用/移出绕过特殊管理。
#[derive(Clone, Copy, Debug)]
pub enum BuiltinEntityComponent {
    /// 定长强身份数组，N/T 具体注册由类型元数据区分。
    Strong,
    /// 定长弱身份数组。
    Weak,
    /// 单链头插头取。
    List,
    /// 单链尾插头取。
    Queue,
    /// 双端链。
    Deque,
}
/// 内置释放入口；指针必须对齐、初始化、与 ComponentInfo 类型一致且拥有唯一释放责任。
/// 上下文须属于该值/节点池的 World，授权覆盖节点回收、计数操作及记录保活；失败停止不重试。
pub(crate) type BuiltinRelease =
    for<'a, 'w> unsafe fn(&crate::destruction::ReleaseContext<'w>, crate::ptr::OwningPtr<'a>);
// 内置种类使用组件注册前段，N/T 特化类型需要独立编号；前段容量/冻结规则仍待设计。
// 字段嵌套不能靠 Rust 自动反射检查；只封构造器也不等价于完整 QueryData 安全证明。

// @to_design 每组件选择 SparseSet 后端并在初始化冻结；通过 SparseSetStorage 枚举选择；papaya 仅保底，默认分区后备算法、工厂和登记待闭合。
// @to_design 线程能力、变化追踪和生命周期 hooks 随访问与系统契约补充。
