//! 按订阅启用的变化时钟及可变访问；不是模拟帧号。
pub use pi_ecs_types::change_detection::{ChangeTick, ChangeWindow, TrackingDemand};
use std::ops::{Deref, DerefMut};
/// 类型化独占组件访问，读取不标记，DerefMut 标记变化但不比较值相等。
pub struct Mut<'a, T> {
    /// 已证明无别名的组件引用，不能公开绕过标记的写入口。
    pub(crate) value: &'a mut T,
    /// 本次写入上下文及对应组件元数据的受限通道。
    pub(crate) marker: ChangeMarker<'a>,
    /// 同一 Mut 首次标记后置位，后续 DerefMut/set_changed 不重复登记 Changed。
    pub(crate) marked: bool,
}
/// 普通 Mut 与内置专用写对象共用的标记通道，由 fetch/宿主授权入口构造。
/// 按 TrackingDemand 更新时间及登记通知；每个写对象独立保存 marked，不能按整个系统去重。
/// 普通 Mut 在暴露可写引用前标记；内置对象在成功修改后返回前标记，拒绝/空操作不标。
pub(crate) struct ChangeMarker<'a> {
    /// 宿主取 WorldWrite 的固定 tick，系统取 this_run；每包装重新取得可追加通知但不另分配 tick。
    pub(crate) tick: ChangeTick,
    /// 跟踪需求，未启用的分支不得写对应状态。
    pub(crate) demand: &'a TrackingDemand,
    // @to_design 目标 tick 槽和并发通知追加通道的安全借用表示待 fetch 接线。
}
impl<T> Mut<'_, T> {
    /// 显式标记本次变化，内部可变性场景使用；不要求 T:PartialEq。
    /// 按需求立即登记通知，不等待系统结束；消费者必须声明对应变化观察权限。
    pub fn set_changed(&mut self) {
        crate::change_detection_inner::set_changed_inner(self)
    }
}
impl<T> Deref for Mut<'_, T> {
    type Target = T;
    /// 只读访问，不标 Changed。
    fn deref(&self) -> &T {
        crate::change_detection_inner::deref_inner(self)
    }
}
impl<T> DerefMut for Mut<'_, T> {
    /// 暴露可写引用前按订阅标记 tick/Changed 通知；执行器排除冲突观察者。
    /// 不增加通知专用的作用域末发布屏障，不承诺本次写入的历史值快照。
    fn deref_mut(&mut self) -> &mut T {
        crate::change_detection_inner::deref_mut_inner(self)
    }
}
// 新增组件按需求同时设置 added/changed 并登记相应通知；despawn 对实际组件记录 Removed。
// 同一 Mut 的可写解引用和显式标记共用一次登记；重新取得 Mut 可产生新记录。
// 动态启用/重新启用某项 tick：现存值对应槽初始化为维护时刻，保守视为变化。
// 不补发历史通知，新 Reader 仍从流尾开始。
// @to_design tick 耗尽错误与执行/维护入口贯通、实际标记通道和恢复接线待完成。
