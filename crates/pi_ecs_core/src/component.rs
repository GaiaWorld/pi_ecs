//! 显式组件登记所需的独立定义；不隐式要求 Clone 或 Default。
//! 组件值统一要求 Send + Sync（rc 也生效），供并行共享读取与跨线程写借用；
//! 仅系统独占的 Local/State 不受此约束，非 RC 的系统对象要求其 Send，RC 不要求 Send。
use std::{alloc::Layout, any::TypeId, ptr::NonNull};
#[path = "component_inner.rs"]
mod component_inner;
/// 用户组件显式选择框架访问策略；无所有 static 类型的 blanket 实现，宏后置。
/// 普通组件手写 type Access = PlainAccess；内置类型由框架实现，不得重选普通策略。
/// 组件值统一要求 Send + Sync（rc 也生效）：并行共享 &World 读取与移动写借用都需要；
/// 仅由系统独占持有的 Local/State 不在此列，非 RC 的系统对象要求其 Send，RC 不要求 Send。
pub trait Component: Sized + 'static + Send + Sync {
    /// 同时决定共享/写入结果及完整释放，注册元数据不能与此策略分离生成。
    type Access: ComponentPolicy<Self>;
}
/// # 用途
/// 组件策略释放的调用模式，由框架清理入口选择，策略不得自行切换模式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseMode {
    /// 普通移除或旧值回收；归零仅登记，不在释放调用栈递归物理销毁。
    Normal,
    /// 整体关闭；交还已有责任，但不登记新销毁、不发通知、不依赖其他业务值仍存活。
    WholeState,
}

/// # 用途
/// 外部可实现的类型化访问与释放策略；不独立接受擦除释放函数或自报内置分类。
/// Component::Access 选择本策略；框架为真实 C 生成登记释放入口，不自报分类。
/// # Safety
/// 实现者保证所有包装与释放均对应真实 C，访问只涉及当前单槽及原授权允许的字段。
/// 不扩大引用寿命，不从 world 恢复整份 World 引用，不绕过变化标记，不保存释放槽引用。
/// Read 派生行为不得修改组件值；Write 不得开放破坏托管责任的任意替换/移出能力。
/// 对无已验证责任的任意 C，不得通过安全构造器伪造可释放状态；安装输入须满足 ComponentInput 协议。
pub unsafe trait ComponentPolicy<C: Component> {
    /// 单槽只读结果；'a 为值借用，'w 为来源身份等能力寿命，不借临时策略对象。
    type Read<'a, 'w>;
    /// 单槽写结果，寿命含义同 Read；原变化通道必须随实际修改使用。
    type Write<'a, 'w>;
    /// # 用途
    /// value 为已定位的真实活值，world 为同源原授权；不重做存储解析或取得新权限。
    /// # Safety
    /// 调用者保证 'a 内共享访问合法，world 覆盖 'w，相关字段能力覆盖返回结果。
    unsafe fn read<'a, 'w>(value: &'a C, world: UnsafeWorldCell<'w>) -> Self::Read<'a, 'w>;
    /// # 用途
    /// value 为唯一单槽，marker 与该组件同源同目标；包装本身不标记、不分配 tick。
    /// # Safety
    /// 调用者保证 'a 内无重叠读写及结构修改，world/marker 与 value 同源且覆盖结果。
    unsafe fn write<'a, 'w>(
        value: &'a mut C,
        world: UnsafeWorldCell<'w>,
        marker: ChangeMarker<'a>,
    ) -> Self::Write<'a, 'w>;
    /// # 用途
    /// value 是 Removed 中保留的旧值，不按原实体读取当前活槽；仅返回旧值的共享包装。
    /// # Safety
    /// 调用者保证旧值及节点在 'a 内存活且不可变，world 属于原来源并覆盖结果所需字段。
    unsafe fn read_removed<'a>(value: &'a C, world: UnsafeWorldCell<'a>) -> Self::Read<'a, 'a>;
    /// # 用途
    /// value 为已从活槽或通知目录撤销的唯一释放责任，world/mode 来自实际清理入口。
    /// 完整释放 C 及其管理责任，不释放背后分配；框架随后不再普通析构或重试此值。
    /// panic 沿清理入口的停止/终止边界处理，WholeState 不读取其他业务值或启动工作。
    /// # Safety
    /// 调用者保证 C 类型/对齐/初始化正确，所有冲突引用已结束，来源分配覆盖调用。
    /// 实现者不得遗留待再次释放的 C、重复析构、保存槽引用或把普通模式用于整体关闭。
    unsafe fn release(value: &mut ManuallyDrop<C>, world: UnsafeWorldCell<'_>, mode: ReleaseMode);
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
/// 统一共享访问结果，Query/Lookup/直接点查都使用相同策略。
pub type ComponentRead<'a, 'w, C> = <<C as Component>::Access as ComponentPolicy<C>>::Read<'a, 'w>;
/// 统一写访问结果，Option/元组/批量仅组合此映射，不能恢复裸内置头。
pub type ComponentWrite<'a, 'w, C> =
    <<C as Component>::Access as ComponentPolicy<C>>::Write<'a, 'w>;
pub use pi_ecs_types::component::ComponentId;
/// 组件的存储约束；实际获取位置由具体原型确定。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoragePolicy {
    /// 可以进入预设 Table；未覆盖时使用全 World 单组件的全局稀疏后备。
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
    /// 为具体 C/Access 单态生成的唯一完整释放入口，含普通 Rust 析构；不接受用户拼装擦除函数。
    pub(crate) release: ComponentRelease,
    /// 类型在本注册生命周期内固定的存储约束。
    pub(crate) storage: StoragePolicy,
    /// 查询/通知登记需求的汇总；未启用项不写 tick 或追加对应通知。
    pub(crate) tracking: crate::change_detection::TrackingDemand,
}

/// # 用途
/// 注册表按具体 C 及其策略生成的擦除入口；只由框架存入 ComponentInfo。
pub(crate) type ComponentRelease =
    for<'a, 'w> unsafe fn(&crate::destruction::ReleaseContext<'w>, crate::ptr::OwningPtr<'a>);
// 普通与关系组件统一交错登记，访问与释放均由 ComponentPolicy<C> 决定，不保留分类常量。
// 字段嵌套不能靠 Rust 自动反射检查；只封构造器也不等价于完整 QueryData 安全证明。

// 每组件在初始化冻结时选择 SparseSetStorage；当前唯一后端为两级 SparseArray（SPARSE_PAGE）。
// papaya 哈希后端已移除；默认后备与显式稀疏共用全 World 单组件后端，工厂校验一致性后构造。
// RC 访问与完整释放由 ComponentPolicy 声明，变化标记沿 ChangeMarker；不另提供任意生命周期回调。
// 非 RC 的线程安全证明延期，Component 的 Send+Sync 约束不能替代存储/授权证明。

/// # 用途
/// 已定位活值的统一共享包装入口；value 为单槽，不重做 Table/Sparse 定位，不取得额外授权。
/// 返回值短借 value，内置结果仅将身份能力保留到原 world 寿命。
/// # Safety
/// 调用方已证明同源、真实类型、已初始化、声明读权限及整个结果期无冲突修改/回收。
pub(crate) unsafe fn read_component<'a, 'w: 'a, C: Component>(
    value: &'a C,
    world: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
) -> ComponentRead<'a, 'w, C> {
    <C::Access as ComponentPolicy<C>>::read(value, world)
}
/// # 用途
/// 已定位活值的统一独占包装入口；marker 携带既定写 tick 及本目标变化通道，不在包装时标记。
/// 返回普通 Mut 或内置专用写对象，不能恢复内置头任意替换能力。
/// # Safety
/// 调用方已证明唯一单槽引用、同源同目标 marker、写权限及全结果期无冲突别名/结构修改。
pub(crate) unsafe fn write_component<'a, 'w: 'a, C: Component>(
    value: &'a mut C,
    world: crate::unsafe_world_cell::UnsafeWorldCell<'w>,
    marker: crate::change_detection::ChangeMarker<'a>,
) -> ComponentWrite<'a, 'w, C> {
    <C::Access as ComponentPolicy<C>>::write(value, world, marker)
}

use crate::{change_detection::ChangeMarker, unsafe_world_cell::UnsafeWorldCell};
use std::mem::ManuallyDrop;

// SAFETY: 具体类型、单槽寿命和唯一清偿由本策略共同保持。
unsafe impl<C: Component<Access = PlainAccess>> ComponentPolicy<C> for PlainAccess {
    /// 单槽共享结果，不暴露托管头替换能力。
    type Read<'a, 'w> = &'a C;
    /// 单槽写结果，沿原变化标记操作。
    type Write<'a, 'w> = crate::change_detection::Mut<'a, C>;
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read<'a, 'w>(value: &'a C, world: UnsafeWorldCell<'w>) -> Self::Read<'a, 'w> {
        Self::read_inner::<C>(value, world)
    }
    /// # 用途
    /// value/world/marker 遵守 ComponentPolicy::write 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::write 的全部前提；结果不越过授权，释放不重复。
    unsafe fn write<'a, 'w>(
        value: &'a mut C,
        world: UnsafeWorldCell<'w>,
        marker: crate::change_detection::ChangeMarker<'a>,
    ) -> Self::Write<'a, 'w> {
        Self::write_inner::<C>(value, world, marker)
    }
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read_removed 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read_removed 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read_removed<'a>(value: &'a C, world: UnsafeWorldCell<'a>) -> Self::Read<'a, 'a> {
        Self::read_removed_inner::<C>(value, world)
    }
    /// # 用途
    /// value/world/mode 遵守 ComponentPolicy::release 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::release 的全部前提；结果不越过授权，释放不重复。
    unsafe fn release(value: &mut ManuallyDrop<C>, world: UnsafeWorldCell<'_>, mode: ReleaseMode) {
        Self::release_inner::<C>(value, world, mode)
    }
}

impl<T: 'static> Component for crate::entity_container::EntityList<T> {
    /// 托管访问与完整释放的唯一策略，不能退回普通头替换。
    type Access = ListAccess;
}

// SAFETY: 具体类型、单槽寿命和唯一清偿由本策略共同保持。
unsafe impl<T: 'static> ComponentPolicy<crate::entity_container::EntityList<T>> for ListAccess {
    /// 单槽共享结果，不暴露托管头替换能力。
    type Read<'a, 'w> = crate::entity_container::EntityListRef<'a, 'w, T>;
    /// 单槽写结果，沿原变化标记操作。
    type Write<'a, 'w> = crate::entity_container::EntityListMut<'a, 'w, T>;
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read<'a, 'w>(
        value: &'a crate::entity_container::EntityList<T>,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Read<'a, 'w> {
        Self::read_inner::<T>(value, world)
    }
    /// # 用途
    /// value/world/marker 遵守 ComponentPolicy::write 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::write 的全部前提；结果不越过授权，释放不重复。
    unsafe fn write<'a, 'w>(
        value: &'a mut crate::entity_container::EntityList<T>,
        world: UnsafeWorldCell<'w>,
        marker: crate::change_detection::ChangeMarker<'a>,
    ) -> Self::Write<'a, 'w> {
        Self::write_inner::<T>(value, world, marker)
    }
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read_removed 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read_removed 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read_removed<'a>(
        value: &'a crate::entity_container::EntityList<T>,
        world: UnsafeWorldCell<'a>,
    ) -> Self::Read<'a, 'a> {
        Self::read_removed_inner::<T>(value, world)
    }
    /// # 用途
    /// value/world/mode 遵守 ComponentPolicy::release 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::release 的全部前提；结果不越过授权，释放不重复。
    unsafe fn release(
        value: &mut ManuallyDrop<crate::entity_container::EntityList<T>>,
        world: UnsafeWorldCell<'_>,
        mode: ReleaseMode,
    ) {
        Self::release_inner::<T>(value, world, mode)
    }
}

impl<T: 'static> Component for crate::entity_container::EntityQueue<T> {
    /// 托管访问与完整释放的唯一策略，不能退回普通头替换。
    type Access = QueueAccess;
}

// SAFETY: 具体类型、单槽寿命和唯一清偿由本策略共同保持。
unsafe impl<T: 'static> ComponentPolicy<crate::entity_container::EntityQueue<T>> for QueueAccess {
    /// 单槽共享结果，不暴露托管头替换能力。
    type Read<'a, 'w> = crate::entity_container::EntityQueueRef<'a, 'w, T>;
    /// 单槽写结果，沿原变化标记操作。
    type Write<'a, 'w> = crate::entity_container::EntityQueueMut<'a, 'w, T>;
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read<'a, 'w>(
        value: &'a crate::entity_container::EntityQueue<T>,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Read<'a, 'w> {
        Self::read_inner::<T>(value, world)
    }
    /// # 用途
    /// value/world/marker 遵守 ComponentPolicy::write 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::write 的全部前提；结果不越过授权，释放不重复。
    unsafe fn write<'a, 'w>(
        value: &'a mut crate::entity_container::EntityQueue<T>,
        world: UnsafeWorldCell<'w>,
        marker: crate::change_detection::ChangeMarker<'a>,
    ) -> Self::Write<'a, 'w> {
        Self::write_inner::<T>(value, world, marker)
    }
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read_removed 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read_removed 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read_removed<'a>(
        value: &'a crate::entity_container::EntityQueue<T>,
        world: UnsafeWorldCell<'a>,
    ) -> Self::Read<'a, 'a> {
        Self::read_removed_inner::<T>(value, world)
    }
    /// # 用途
    /// value/world/mode 遵守 ComponentPolicy::release 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::release 的全部前提；结果不越过授权，释放不重复。
    unsafe fn release(
        value: &mut ManuallyDrop<crate::entity_container::EntityQueue<T>>,
        world: UnsafeWorldCell<'_>,
        mode: ReleaseMode,
    ) {
        Self::release_inner::<T>(value, world, mode)
    }
}

impl<T: 'static> Component for crate::entity_container::EntityDeque<T> {
    /// 托管访问与完整释放的唯一策略，不能退回普通头替换。
    type Access = DequeAccess;
}

// SAFETY: 具体类型、单槽寿命和唯一清偿由本策略共同保持。
unsafe impl<T: 'static> ComponentPolicy<crate::entity_container::EntityDeque<T>> for DequeAccess {
    /// 单槽共享结果，不暴露托管头替换能力。
    type Read<'a, 'w> = crate::entity_container::EntityDequeRef<'a, 'w, T>;
    /// 单槽写结果，沿原变化标记操作。
    type Write<'a, 'w> = crate::entity_container::EntityDequeMut<'a, 'w, T>;
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read<'a, 'w>(
        value: &'a crate::entity_container::EntityDeque<T>,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Read<'a, 'w> {
        Self::read_inner::<T>(value, world)
    }
    /// # 用途
    /// value/world/marker 遵守 ComponentPolicy::write 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::write 的全部前提；结果不越过授权，释放不重复。
    unsafe fn write<'a, 'w>(
        value: &'a mut crate::entity_container::EntityDeque<T>,
        world: UnsafeWorldCell<'w>,
        marker: crate::change_detection::ChangeMarker<'a>,
    ) -> Self::Write<'a, 'w> {
        Self::write_inner::<T>(value, world, marker)
    }
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read_removed 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read_removed 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read_removed<'a>(
        value: &'a crate::entity_container::EntityDeque<T>,
        world: UnsafeWorldCell<'a>,
    ) -> Self::Read<'a, 'a> {
        Self::read_removed_inner::<T>(value, world)
    }
    /// # 用途
    /// value/world/mode 遵守 ComponentPolicy::release 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::release 的全部前提；结果不越过授权，释放不重复。
    unsafe fn release(
        value: &mut ManuallyDrop<crate::entity_container::EntityDeque<T>>,
        world: UnsafeWorldCell<'_>,
        mode: ReleaseMode,
    ) {
        Self::release_inner::<T>(value, world, mode)
    }
}

impl<const N: usize, T: 'static> Component for crate::owned_entity::StrongEntities<N, T> {
    /// 托管访问与完整释放的唯一策略，不能退回普通头替换。
    type Access = StrongAccess;
}

// SAFETY: 具体类型、单槽寿命和唯一清偿由本策略共同保持。
unsafe impl<const N: usize, T: 'static> ComponentPolicy<crate::owned_entity::StrongEntities<N, T>>
    for StrongAccess
{
    /// 单槽共享结果，不暴露托管头替换能力。
    type Read<'a, 'w> = crate::owned_entity::StrongEntitiesRef<'a, 'w, N, T>;
    /// 单槽写结果，沿原变化标记操作。
    type Write<'a, 'w> = crate::owned_entity::StrongEntitiesMut<'a, 'w, N, T>;
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read<'a, 'w>(
        value: &'a crate::owned_entity::StrongEntities<N, T>,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Read<'a, 'w> {
        Self::read_inner::<N, T>(value, world)
    }
    /// # 用途
    /// value/world/marker 遵守 ComponentPolicy::write 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::write 的全部前提；结果不越过授权，释放不重复。
    unsafe fn write<'a, 'w>(
        value: &'a mut crate::owned_entity::StrongEntities<N, T>,
        world: UnsafeWorldCell<'w>,
        marker: crate::change_detection::ChangeMarker<'a>,
    ) -> Self::Write<'a, 'w> {
        Self::write_inner::<N, T>(value, world, marker)
    }
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read_removed 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read_removed 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read_removed<'a>(
        value: &'a crate::owned_entity::StrongEntities<N, T>,
        world: UnsafeWorldCell<'a>,
    ) -> Self::Read<'a, 'a> {
        Self::read_removed_inner::<N, T>(value, world)
    }
    /// # 用途
    /// value/world/mode 遵守 ComponentPolicy::release 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::release 的全部前提；结果不越过授权，释放不重复。
    unsafe fn release(
        value: &mut ManuallyDrop<crate::owned_entity::StrongEntities<N, T>>,
        world: UnsafeWorldCell<'_>,
        mode: ReleaseMode,
    ) {
        Self::release_inner::<N, T>(value, world, mode)
    }
}

impl<const N: usize, T: 'static> Component for crate::owned_entity::WeakEntities<N, T> {
    /// 托管访问与完整释放的唯一策略，不能退回普通头替换。
    type Access = WeakAccess;
}

// SAFETY: 具体类型、单槽寿命和唯一清偿由本策略共同保持。
unsafe impl<const N: usize, T: 'static> ComponentPolicy<crate::owned_entity::WeakEntities<N, T>>
    for WeakAccess
{
    /// 单槽共享结果，不暴露托管头替换能力。
    type Read<'a, 'w> = crate::owned_entity::WeakEntitiesRef<'a, 'w, N, T>;
    /// 单槽写结果，沿原变化标记操作。
    type Write<'a, 'w> = crate::owned_entity::WeakEntitiesMut<'a, 'w, N, T>;
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read<'a, 'w>(
        value: &'a crate::owned_entity::WeakEntities<N, T>,
        world: UnsafeWorldCell<'w>,
    ) -> Self::Read<'a, 'w> {
        Self::read_inner::<N, T>(value, world)
    }
    /// # 用途
    /// value/world/marker 遵守 ComponentPolicy::write 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::write 的全部前提；结果不越过授权，释放不重复。
    unsafe fn write<'a, 'w>(
        value: &'a mut crate::owned_entity::WeakEntities<N, T>,
        world: UnsafeWorldCell<'w>,
        marker: crate::change_detection::ChangeMarker<'a>,
    ) -> Self::Write<'a, 'w> {
        Self::write_inner::<N, T>(value, world, marker)
    }
    /// # 用途
    /// value/world 遵守 ComponentPolicy::read_removed 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::read_removed 的全部前提；结果不越过授权，释放不重复。
    unsafe fn read_removed<'a>(
        value: &'a crate::owned_entity::WeakEntities<N, T>,
        world: UnsafeWorldCell<'a>,
    ) -> Self::Read<'a, 'a> {
        Self::read_removed_inner::<N, T>(value, world)
    }
    /// # 用途
    /// value/world/mode 遵守 ComponentPolicy::release 的同源单槽及唯一责任契约，不新增权限。
    /// # Safety
    /// 调用方须满足 ComponentPolicy::release 的全部前提；结果不越过授权，释放不重复。
    unsafe fn release(
        value: &mut ManuallyDrop<crate::owned_entity::WeakEntities<N, T>>,
        world: UnsafeWorldCell<'_>,
        mode: ReleaseMode,
    ) {
        Self::release_inner::<N, T>(value, world, mode)
    }
}
