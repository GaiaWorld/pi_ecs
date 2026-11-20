//! 全 World 唯一资源与生产者多例；均始终维护变化状态。
#[path = "resource_inner.rs"]
mod resource_inner;
use crate::{
    change_detection::{ChangeTick, Mut},
    error::EcsError,
    world::World,
};
pub use pi_ecs_types::resource::ResourceId;
use std::any::{Any, TypeId};
/// 唯一资源及每个多例槽始终保存的时间，不受观察需求控制。
pub struct ResourceTicks {
    /// 首次插入时间；替换已有值保持。
    pub added: ChangeTick,
    /// 新增/替换/可写访问的时间。
    pub changed: ChangeTick,
}
/// 每种唯一资源一个可缺失实例；与同类型组件及多例资源分开登记。
pub(crate) struct ResourceStore {
    /// rc 按 ResourceId 直接索引；None 表示已登记但当前无实例。
    #[cfg(feature = "rc")]
    pub(crate) entries: Vec<Option<ResourceEntry>>,
    /// rc 类型到编号的登记表；重复登记返回原编号。
    #[cfg(feature = "rc")]
    pub(crate) ids: std::collections::HashMap<TypeId, ResourceId>,
    /// 非 rc 的资源存储表示随并发后端改造补齐。
    // @to_design 非 rc 资源存储/锁与线程能力登记；register_resource 的接线尚未实现
    #[cfg(not(feature = "rc"))]
    pub(crate) _pending: (),
}
/// rc 单个唯一资源槽：擦除实例、始终维护的 tick 与恢复分类。
pub(crate) struct ResourceEntry {
    /// RC 擦除 Box<ResourceValue<T>>；先 downcast 共享类型化描述，再经字段 UnsafeCell 取得根。
    /// T 仍要求 Send+Sync，但包含 UnsafeCell 的内部包装不要求 Sync，不伪造线程能力。
    #[cfg(feature = "rc")]
    pub(crate) value: Box<dyn Any>,
    /// 第二轮并发表示，不能把本字段直接用作共享转可变依据。
    #[cfg(not(feature = "rc"))]
    pub(crate) value: Option<Box<dyn Any + Send + Sync>>,
    /// 首次插入与最近变化时间；移除后随槽清空。
    #[cfg(not(feature = "rc"))]
    pub(crate) ticks: ResourceTicks,
    /// 游戏恢复策略；登记时固定。
    pub(crate) recovery: StateRecovery,
}
/// # 用途
/// RC 唯一资源的类型化内部可变边界；实例存在性归 ResourceStore.entries 的 Option 单源管理。
/// 存活期间不移动 Box 分配体，不克隆拥有者；替换/移除必须无活值和 tick 引用。
#[cfg(feature = "rc")]
pub(crate) struct ResourceValue<T> {
    /// 唯一拥有业务值，运行写根仅由本字段 get 得到，不从 &T 强转。
    pub(crate) value: std::cell::UnsafeCell<T>,
    /// 同一实例的变化时间；与 value 分离投影，仅在获准槽内形成独占引用。
    pub(crate) ticks: std::cell::UnsafeCell<ResourceTicks>,
}
/// 多例紧凑数组；生产者结构变化仅在阶段前静止点执行。
pub(crate) struct MultiResourceStore {
    /// RC 按多例 ResourceId 保存 Box<MultiResourceCore<T>>，不要求包装 Sync；只追加，不复用类型编号。
    #[cfg(feature = "rc")]
    pub(crate) entries: Vec<Box<dyn Any>>,
    /// RC 完整 Rust 类型到多例编号；与唯一资源编号空间独立，解析不得查错目录。
    #[cfg(feature = "rc")]
    pub(crate) ids: std::collections::HashMap<TypeId, ResourceId>,
    /// 非 rc 的多例并发表示随后端改造补齐。
    // @to_design 非 rc 多例紧凑数组与并发生产者登记
    #[cfg(not(feature = "rc"))]
    pub(crate) _pending: (),
}
/// rc 单个多例类型的紧凑值、tick 与稳定生产者身份；三元组按同一物理下标对应。
pub(crate) struct MultiResourceCore<T> {
    /// 下一稳定生产者编号，初始 0；checked 分配，到 u64::MAX 永久拒绝（该值保留不用）。
    /// 注销不复用，不借紧凑偏移分配身份；空集合结构版本初始 0。
    pub(crate) next_producer: u64,
    /// 本类型生产槽集合版本；增删推进，值写不推进；耗尽不得回绕。
    pub(crate) structure_version: u64,
    /// 稳定编号到当前紧凑偏移；与三个等长数组一致，搬移时仅在 core 更新。
    pub(crate) producer_offsets: std::collections::HashMap<ProducerId, usize>,
    /// RC 紧凑值唯一所有者；仅无活引用的维护期接触 Vec/搬移/扩容，运行期不重借整个 Vec。
    #[cfg(feature = "rc")]
    pub(crate) values: std::cell::UnsafeCell<Vec<T>>,
    /// RC 当前连续分配首地址；维护期经 values.get() 的独占来源生成，所有可能失效的操作前撤销准备。
    /// 空集合使用对齐 dangling；不得解引用，长度由 producers.len() 单源给出。
    #[cfg(feature = "rc")]
    pub(crate) values_root: std::ptr::NonNull<T>,
    /// 第二轮值所有者表示。
    #[cfg(not(feature = "rc"))]
    pub(crate) values: Vec<T>,
    /// RC tick 唯一所有者，维护约束同 values，运行期只按缓存根访问获准偏移。
    #[cfg(feature = "rc")]
    pub(crate) ticks: std::cell::UnsafeCell<Vec<ResourceTicks>>,
    /// RC tick 当前分配首地址；与 values_root/两个 Vec/producers 长度同时完成后才发布。
    /// 空集合使用对齐 dangling；不得从临时共享切片派生写根。
    #[cfg(feature = "rc")]
    pub(crate) ticks_root: std::ptr::NonNull<ResourceTicks>,
    /// 第二轮 tick 所有者表示。
    #[cfg(not(feature = "rc"))]
    pub(crate) ticks: Vec<ResourceTicks>,
    /// 与 values 同下标的稳定生产者编号，不随物理搬移改变。
    pub(crate) producers: Vec<ProducerId>,
}
/// 单个多例类型内稳定编号；由该类型 next_producer 单调分配，不随物理下标变化。
/// 与所属 World 身份和多例 ResourceId 一起使用才唯一，不能跨类型查目录。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ProducerId(
    /// 单调递增编号。
    pub(crate) u64,
);
/// # 用途
/// 生产者准备期位置缓存；不拥有值、不授予访问，不能跨结构版本盲目使用。
pub struct ProducerBinding {
    /// 同类型集合版本；不是读者观察点或 Stage 排序版本。
    pub structure_version: u64,
    /// 当前紧凑偏移，仅在上述版本下有效。
    pub offset: usize,
}
/// 值的恢复责任，资源和 Local 使用同一分类词汇，不假定复制能力已经登记。
pub enum StateRecovery {
    /// 独立保存；资源包含存在性，Local 按系统及参数位置匹配。
    Save,
    /// 资源清旧重建，Local 使用保存的初始化能力重置。
    Rebuild,
    /// 不参与游戏恢复，保留当前状态。
    Preserve,
}
impl World {
    /// # 用途
    /// 构建期在 Idle 且类型登记未冻结时显式登记多例 T，返回独立多例目录的 ResourceId。
    /// 同类型重复幂等（冻结后亦可查回原编号），新类型冻结后拒绝；不登记唯一资源或创建生产者。
    /// 成功发布空集合、版本和 next_producer；失败不发布半映射，不要求 T:Default。
    #[cfg(feature = "rc")]
    pub fn register_multi_resource<T: Send + Sync + 'static>(
        &mut self,
    ) -> Result<ResourceId, EcsError> {
        self.register_multi_resource_inner::<T>()
    }
    /// # 用途
    /// registration 为同源且仍登记的生产者责任；冻结后 RC prepare 静止点取得版本与偏移快照。
    /// 不读取业务值、不新增槽、不应用追踪或分配 tick；异源/未知责任 Err，不重建槽或改变责任。
    /// prepare 将结果更新到本参数缓存；失败使本次准备失效，不遍历其他 Stage。
    #[cfg(feature = "rc")]
    pub fn producer_binding(
        &self,
        registration: &crate::parameter_registration::ProducerRegistration,
    ) -> Result<ProducerBinding, EcsError> {
        self.producer_binding_inner(registration)
    }
    /// 初始化冻结前登记 T，不要求 Default；与组件/多例编号分离，重复相同登记幂等。
    pub fn register_resource<T: Send + Sync + 'static>(&mut self) -> Result<ResourceId, EcsError> {
        Self::register_resource_inner::<T>(self)
    }
    /// rc：&mut World 独占维护，直接改写资源槽。
    /// 消费 value，返回旧值所有权，不隐式产生 Removed 通知；新增设两 tick，替换仅更新 changed。
    #[cfg(feature = "rc")]
    pub fn insert_resource<T: Send + Sync + 'static>(
        &mut self,
        value: T,
    ) -> Result<Option<T>, EcsError> {
        Self::insert_resource_inner::<T>(self, value)
    }
    /// 非 rc：共享 World 上经内部维护同步插入资源。
    #[cfg(not(feature = "rc"))]
    pub fn insert_resource<T: Send + Sync + 'static>(
        &self,
        value: T,
    ) -> Result<Option<T>, EcsError> {
        Self::insert_resource_inner::<T>(self, value)
    }
    /// rc：&mut World 独占移出资源所有权；缺值返回 None。
    #[cfg(feature = "rc")]
    pub fn remove_resource<T: Send + Sync + 'static>(&mut self) -> Result<Option<T>, EcsError> {
        Self::remove_resource_inner::<T>(self)
    }
    /// 非 rc：共享 World 上经内部维护同步移出。
    #[cfg(not(feature = "rc"))]
    pub fn remove_resource<T: Send + Sync + 'static>(&self) -> Result<Option<T>, EcsError> {
        Self::remove_resource_inner::<T>(self)
    }
}
impl crate::world::WorldPin<'_> {
    /// 读取已登记的唯一资源，缺失为 None；借用 self，不越过读写授权，不标变化。
    pub fn resource<T: Send + Sync + 'static>(&self) -> Option<&T> {
        Self::pin_resource_inner::<T>(self)
    }
    /// 返回紧凑物理顺序的多例只读视图；未登记返回 None，已登记无生产者返回空视图。
    /// 借用 self，期间禁止生产者结构变更；不保证登记顺序，不提供稳定下标身份。
    pub fn multi_resource<T: Send + Sync + 'static>(&self) -> Option<MultiResourceRead<'_, T>> {
        Self::pin_multi_inner::<T>(self)
    }
}
impl crate::world::WorldWrite<'_> {
    /// 读取已登记的唯一资源，缺失为 None；借用 self，不越过读写授权，不标变化。
    pub fn resource<T: Send + Sync + 'static>(&self) -> Option<&T> {
        Self::write_resource_inner::<T>(self)
    }
    /// 返回紧凑物理顺序的多例只读视图；未登记返回 None，已登记无生产者返回空视图。
    /// 借用 self，期间禁止生产者结构变更；不保证登记顺序，不提供稳定下标身份。
    pub fn multi_resource<T: Send + Sync + 'static>(&self) -> Option<MultiResourceRead<'_, T>> {
        Self::write_multi_inner::<T>(self)
    }
}
impl crate::world::WorldWrite<'_> {
    /// 已有资源可写短借用；缺失 None，不插入默认值。可写解引用/显式标记更新 changed，tick 始终启用。
    /// 不开放资源通知流；结果与其他冲突读写不能并存，不得越过写授权。
    pub fn resource_mut<T: Send + Sync + 'static>(&mut self) -> Option<Mut<'_, T>> {
        Self::write_resource_mut_inner::<T>(self)
    }
}
/// 宿主只读多例视图，复用 pin/write 授权，不另申请 guard，不提供可变生产槽入口。
pub struct MultiResourceRead<'a, T> {
    /// 当前紧凑数组的共享切片，由所属授权保活，结构变化前必须结束借用。
    pub(crate) values: &'a [T],
}
impl<T> MultiResourceRead<'_, T> {
    /// 当前生产槽数量，不改变观察版本或 tick。
    pub fn len(&self) -> usize {
        Self::multi_len_inner(self)
    }
    /// 是否无生产槽，不消费或标记变化。
    pub fn is_empty(&self) -> bool {
        Self::multi_empty_inner(self)
    }
    /// 当前物理顺序的共享值遍历，不标变化，不产生跨调用稳定下标。
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        Self::multi_iter_inner(self)
    }
}

// 资源插入/移除内部申请 Maintaining；pin/write/运行/维护冲突前置拒绝，不等待，不共享转可变。
// 多例生产槽增删仍归系统参数阶段前登记/注销，不向宿主视图开放。
// 首版不提供资源通知流、分区资源或非 Send 资源；变更存在性仅维护点允许。

/// # 用途
/// RC 系统多例读取的按值描述；只借本次原授权，不拥有实例或记录观察进度。
#[cfg(feature = "rc")]
pub struct MultiResourceView<'a, T> {
    /// 冻结紧凑数组，实例增删前须结束全部结果。
    pub values: &'a [T],
    /// 与 values 等长同偏移的 tick，只读权限覆盖两者。
    pub ticks: &'a [ResourceTicks],
    /// 本次读到的结构版本，schedule 记录进 Pending，成功 finish 才提交。
    pub structure_version: u64,
}
#[cfg(feature = "rc")]
impl<'w> crate::access::RunAccess<'w> {
    /// # 用途
    /// resource 为已解析同源编号，必须与 T 类型相符，当前 self 是已校验的参数叶子且有资源 Read/Write。
    /// 缺实例为 Ok(None)，未知编号/类型/权限失配 Err；不将错误伪装可选缺失，不新增实例或门禁。
    /// 值及 tick 引用由原授权保活，不借临时 self；不允许随后兑现重叠资源写权限。
    /// RC 只借目录并 downcast ResourceValue<T> 描述，从两个 UnsafeCell 根构造共享结果。
    /// # Safety
    /// 原会话覆盖 'w，调用方保证全部返回引用期无冲突写/替换/移除，不重复兑现父子参数权限。
    pub unsafe fn resource_parts<T: Send + Sync + 'static>(
        &self,
        resource: ResourceId,
    ) -> Result<Option<(&'w T, &'w ResourceTicks)>, EcsError> {
        self.resource_parts_inner(resource)
    }
    /// # 用途
    /// resource 同源同类型，叶子必须有唯一资源 Write；缺实例 None，其余错误 Err，不创建默认值。
    /// Mut 使用 tick 构造 ChangeTarget::Resource，marked=false，不在包装时标记，不含组件通知能力。
    /// tick 为系统 window.this_run，不分配新 tick，已有 added 保持不变。
    /// RC 从 ResourceValue<T> 的 value.get()/ticks.get() 分别投影，不独占借目录或整个擦除 Box。
    /// # Safety
    /// 原授权覆盖 'w 且本资源值/tick 唯一，调用方不得再次兑现本资源的读写引用。
    pub unsafe fn resource_mut<T: Send + Sync + 'static>(
        &self,
        resource: ResourceId,
        tick: ChangeTick,
    ) -> Result<Option<Mut<'w, T>>, EcsError> {
        self.resource_mut_inner(resource, tick)
    }
    /// # 用途
    /// resource 已登记为 T 的多例类型，叶子有 ReadAll；未知类型/权限错误 Err，零生产槽返回空切片。
    /// 只读值/tick 及结构版本来自同一次冻结集合；返回后不重绑或移动实例，不推进观察点。
    /// RC 从缓存根及 producers.len() 构造两个连续共享切片，运行期不访问 Vec 所有者。
    /// # Safety
    /// 原授权覆盖 'w，排除本集合全部生产者写及结构变化，不通过临时共享引用取得整集合可变引用。
    pub unsafe fn multi_resource_parts<T: Send + Sync + 'static>(
        &self,
        resource: ResourceId,
    ) -> Result<MultiResourceView<'w, T>, EcsError> {
        self.multi_resource_parts_inner(resource)
    }
    /// # 用途
    /// registration 属同源已登记 T，binding 为本次 prepare 缓存且版本/偏移仍有效，叶子 WriteSlot 对应该生产者。
    /// 错误不重建槽、不查找其他生产者替代；成功仅对该偏移的值/tick 构造 Mut，不能独占借整个 Vec/集合。
    /// 使用 tick 构造 ChangeTarget::Multi，marked=false，无组件通知；同类型不同生产者各自拥有单槽。
    /// RC 仅用 values_root/ticks_root 加已校验偏移构造引用，不借整集合/整 Vec/整切片为可变。
    /// # Safety
    /// 原授权覆盖 'w，槽唯一且集合结构固定；tick 为本系统 this_run，不重复兑现同生产者访问。
    pub unsafe fn producer_mut<T: Send + Sync + 'static>(
        &self,
        registration: &crate::parameter_registration::ProducerRegistration,
        binding: &ProducerBinding,
        tick: ChangeTick,
    ) -> Result<Mut<'w, T>, EcsError> {
        self.producer_mut_inner(registration, binding, tick)
    }
}
