//! 按订阅启用的变化时钟及可变访问；不是模拟帧号。
#[path = "change_detection_inner.rs"]
mod change_detection_inner;
use crate::{component::ComponentId, entity::Entity};
pub use pi_ecs_types::change_detection::{ChangeTick, ChangeWindow, TrackingDemand};
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};
/// 类型化独占组件/资源访问，读取不标记，DerefMut 标记变化但不比较值相等。
pub struct Mut<'a, T> {
    /// 已证明无别名的值引用，不能公开绕过标记的写入口。
    pub(crate) value: &'a mut T,
    /// 本次写入上下文及对应组件元数据的受限通道。
    pub(crate) marker: ChangeMarker<'a>,
    /// 同一 Mut 首次标记后置位，后续 DerefMut/set_changed 不重复登记 Changed。
    pub(crate) marked: bool,
}
/// 普通 Mut 与内置专用写对象共用的标记通道，由 fetch/宿主授权入口构造。
/// 组件按 TrackingDemand 更新 changed 及登记 Changed 通知；资源/多例始终更新 changed，不发组件通知。
/// 每个写对象独立保存 marked，不能按整个系统去重；已有值标记绝不改 added。
/// 普通 Mut 在暴露可写引用前标记；内置对象在成功修改后返回前标记，拒绝/空操作不标。
pub struct ChangeMarker<'a> {
    /// 宿主取 WorldWrite 的固定 tick，系统取 this_run；包装不另分配 tick。
    pub(crate) tick: ChangeTick,
    /// 本次写目标的 tick 槽；Table/Sparse/Resource/Multi 由 fetch 选择，未追踪为 None。
    pub(crate) target: ChangeTarget<'a>,
}
impl ChangeMarker<'_> {
    /// # 用途
    /// marked 为当前写对象独占保存的去重位，首次为 false；不得跨对象共享或在标记后复位。
    /// 首次按原需求更新 changed 并追加通知，成功后置 true；已为 true 无动作，不改 added 或分配 tick。
    /// 自定义策略在暴露普通可写引用前调用，受控修改在成功后返回前调用；拒绝/空操作不调用。
    /// RC 的 Changed 追加失败在展开中局部标停，不回滚已发生修改；不能标记其他组件。
    pub fn set_changed(&mut self, marked: &mut bool) {
        self.marker_set_changed_inner(marked)
    }
}
/// 本次可写的 tick 槽类别；Component 覆盖 Table 列与稀疏槽，Resource/Multi 使用资源 tick。
pub(crate) enum ChangeTarget<'a> {
    /// 组件活值及组件专用需求/通知；无 tick 追踪仍保留本分支，可只登记 Changed 通知。
    Component {
        /// 已启用槽的唯一访问；未追踪槽为 None，added 只供变化检查，已有值标记不写 added。
        /// 整个 tick 槽借用受本标记保护，过滤必须在构造前完成短读，不能保留重叠共享引用。
        ticks: pi_ecs_types::change_detection::ComponentTicksMut<'a>,
        /// 屏障后固定需求；与 ticks 启用情况一致，changed_notice 与通知令牌存在性一致。
        demand: &'a TrackingDemand,
        /// 仅 changed_notice 启用时存在；覆盖本次组件/实体及完整写授权，不携带 Added/Removed 发布责任。
        /// changed_notice 不要求 changed_tick；无 tick 槽也不能降级为无变化标记。
        notice: Option<NoticeWrite<'a>>,
    },
    /// 唯一资源：两 tick 始终存在。
    Resource(&'a mut crate::resource::ResourceTicks),
    /// 多例生产槽：两 tick 始终存在。
    Multi(&'a mut crate::resource::ResourceTicks),
}
/// 通知追加令牌；由已取得写授权且声明覆盖通知流的 fetch/宿主入口构造。
/// 追加沿用通知流自身的发布/回收协议；不延长 World，不用于其他组件或实体。
pub(crate) struct NoticeWrite<'a> {
    /// 共享冻结目录；追加只短借本组件 Changed 流，不恢复整份 &mut Notifications。
    pub(crate) notifications: &'a crate::notification::Notifications,
    /// 与通知目录同源；RC 追加展开时局部标停，不等待外层会话退出。
    #[cfg(feature = "rc")]
    pub(crate) access: &'a crate::world::AccessState,
    /// 目标组件；必须是本次写声明覆盖的组件。
    pub(crate) component: ComponentId,
    /// 目标实体；Insert/Change/Remove 记录使用该完整身份。
    pub(crate) entity: Entity,
    /// 约束令牌不超过本次写授权。
    pub(crate) borrow: PhantomData<&'a mut ()>,
}
impl<T> Mut<'_, T> {
    /// # 用途
    /// window 为调用方提供的有效观察窗口，满足 last_run <= this_run；首次无下界。
    /// 短借 self，经原 marker.target 读取当前 added 槽并按窗口比较，不另建持久 tick 引用。
    /// 组件未启用 added 时 false；资源/多例始终有槽。只读，不标变化、不推进观察点。
    pub fn is_added(&self, window: ChangeWindow) -> bool {
        self.is_added_inner(window)
    }
    /// # 用途
    /// window 为有效观察窗口，规则与 is_added 一致；读取原通道当前 changed，而非构造时快照。
    /// set_changed/可写解引用返回后立即反映本次 tick；仅 marked=true 不代替窗口比较。
    /// 未启用 changed 时 false；不追加通知、不重新分配 tick、不改变 marked 或窗口。
    pub fn is_changed(&self, window: ChangeWindow) -> bool {
        self.is_changed_inner(window)
    }
    /// 显式标记本次变化，内部可变性场景使用；不要求 T:PartialEq。
    /// 已有值只写 changed，保持 added；组件按需求立即登记 Changed，不等待系统结束。
    /// 资源/多例始终更新 changed 且不发组件通知；消费者须声明对应观察权限。
    pub fn set_changed(&mut self) {
        Self::set_changed_inner(self)
    }
}
impl<T> Deref for Mut<'_, T> {
    type Target = T;
    /// 只读访问，不标 Changed。
    fn deref(&self) -> &T {
        Self::deref_inner(self)
    }
}
impl<T> DerefMut for Mut<'_, T> {
    /// 暴露可写引用前按订阅标记 tick/Changed 通知；执行器排除冲突观察者。
    /// 不增加通知专用的作用域末发布屏障，不承诺本次写入的历史值快照。
    fn deref_mut(&mut self) -> &mut T {
        Self::deref_mut_inner(self)
    }
}
// 新增组件按需求同时设置 added/changed 并登记相应通知；despawn 对实际组件记录 Removed。
// 同一 Mut 的可写解引用和显式标记共用一次登记；重新取得 Mut 可产生新记录。
// 动态启用/重新启用某项 tick：现存值对应槽初始化为维护时刻，保守视为变化。
// 不补发历史通知，新 Reader 仍从流尾开始。
// tick 由 WorldWrite/query 在交出可写结果前一次性分配；耗尽在写入前报错，不在此重分配。
// 已有值标记只写已分配 tick：Component 按 demand 写 changed Option 槽并追加 Changed，
// Resource/Multi 只写始终存在的 changed，全部保持 added；插入/追踪启用另行初始化 added。
// 恢复安装沿历史分配进度继续，不在此回绕，也不经已有值标记制造变化。
// 动态启用 added/changed tick 时，在阶段前静止点把现存活值槽填为当时 tick，保守视为变化。
