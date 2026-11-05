//! 全 World 唯一资源与生产者多例；均始终维护变化状态。
use crate::{
    change_detection::{ChangeTick, Mut},
    error::EcsError,
    world::World,
};
pub use pi_ecs_types::resource::ResourceId;
/// 唯一资源及每个多例槽始终保存的时间，不受观察需求控制。
pub struct ResourceTicks {
    /// 首次插入时间；替换已有值保持。
    pub added: ChangeTick,
    /// 新增/替换/可写访问的时间。
    pub changed: ChangeTick,
}
/// 每种唯一资源一个可缺失实例；与同类型组件及多例资源分开登记。
pub(crate) struct ResourceStore {
    // @to_design 类型登记表、擦除实例、恢复分类和线程能力登记待闭合。
    pub(crate) _pending: (),
}
/// 多例紧凑数组；生产者结构变化仅在阶段前静止点执行。
pub(crate) struct MultiResourceStore {
    // @to_design 各 T 的紧凑值/tick 数组及稳定生产者身份，类型擦除表示待闭合。
    // 增删后全遍历生产者更新偏移缓存，不使用可变位宽映射；执行中禁止搬移/扩容失效引用。
    pub(crate) _pending: (),
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
    /// 初始化冻结前登记 T，不要求 Default；与组件/多例编号分离，重复相同登记幂等。
    pub fn register_resource<T: Send + Sync + 'static>(&mut self) -> Result<ResourceId, EcsError> {
        crate::resource_inner::register_resource_inner::<T>(self)
    }
    /// T 已登记且处于独占维护点；消费 value，返回旧值所有权，不隐式产生 Removed 通知。新增设置两 tick，替换仅更新 changed。
    pub fn insert_resource<T: Send + Sync + 'static>(
        &self,
        value: T,
    ) -> Result<Option<T>, EcsError> {
        crate::resource_inner::insert_resource_inner::<T>(self, value)
    }
    /// 独占维护点移出资源所有权；缺值返回 None，错误不改变实例。
    pub fn remove_resource<T: Send + Sync + 'static>(&self) -> Result<Option<T>, EcsError> {
        crate::resource_inner::remove_resource_inner::<T>(self)
    }
}
impl crate::world::WorldPin<'_> {
    /// 读取已登记的唯一资源，缺失为 None；借用 self，不越过读写授权，不标变化。
    pub fn resource<T: Send + Sync + 'static>(&self) -> Option<&T> {
        crate::resource_inner::pin_resource_inner::<T>(self)
    }
    /// 返回紧凑物理顺序的多例只读视图；未登记返回 None，已登记无生产者返回空视图。
    /// 借用 self，期间禁止生产者结构变更；不保证登记顺序，不提供稳定下标身份。
    pub fn multi_resource<T: Send + Sync + 'static>(&self) -> Option<MultiResourceRead<'_, T>> {
        crate::resource_inner::pin_multi_inner::<T>(self)
    }
}
impl crate::world::WorldWrite<'_> {
    /// 读取已登记的唯一资源，缺失为 None；借用 self，不越过读写授权，不标变化。
    pub fn resource<T: Send + Sync + 'static>(&self) -> Option<&T> {
        crate::resource_inner::write_resource_inner::<T>(self)
    }
    /// 返回紧凑物理顺序的多例只读视图；未登记返回 None，已登记无生产者返回空视图。
    /// 借用 self，期间禁止生产者结构变更；不保证登记顺序，不提供稳定下标身份。
    pub fn multi_resource<T: Send + Sync + 'static>(&self) -> Option<MultiResourceRead<'_, T>> {
        crate::resource_inner::write_multi_inner::<T>(self)
    }
}
impl crate::world::WorldWrite<'_> {
    /// 已有资源可写短借用；缺失 None，不插入默认值。可写解引用/显式标记更新 changed，tick 始终启用。
    /// 不开放资源通知流；结果与其他冲突读写不能并存，不得越过写授权。
    pub fn resource_mut<T: Send + Sync + 'static>(&mut self) -> Option<Mut<'_, T>> {
        crate::resource_inner::write_resource_mut_inner::<T>(self)
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
        crate::resource_inner::multi_len_inner(self)
    }
    /// 是否无生产槽，不消费或标记变化。
    pub fn is_empty(&self) -> bool {
        crate::resource_inner::multi_empty_inner(self)
    }
    /// 当前物理顺序的共享值遍历，不标变化，不产生跨调用稳定下标。
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        crate::resource_inner::multi_iter_inner(self)
    }
}

// 资源插入/移除内部申请 Maintaining；pin/write/运行/维护冲突前置拒绝，不等待，不共享转可变。
// 多例生产槽增删仍归系统参数阶段前登记/注销，不向宿主视图开放。
// 首版不提供资源通知流、分区资源或非 Send 资源；变更存在性仅维护点允许。
