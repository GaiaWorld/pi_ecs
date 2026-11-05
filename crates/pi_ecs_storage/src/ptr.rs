//! 沿用 Bevy 指针命名和职责；本地设计声明，不是 bevy_ptr 的重导出或兼容承诺。
use std::{marker::PhantomData, ptr::NonNull};
/// 擦除只读借用，不析构值或释放分配，不提前转换为具体 T。
pub struct Ptr<'a> {
    /// 指向活值的非空对齐地址，包括零大小组件的正确对齐。
    value: NonNull<u8>,
    /// 约束实际存储及 guard 的保活期。
    borrow: PhantomData<&'a u8>,
}
/// 擦除独占借用，不能 Clone/Copy，不得与冲突借用共存。
pub struct PtrMut<'a> {
    /// 已授权且唯一可写的活值地址。
    value: NonNull<u8>,
    /// 约束本次独占借用，不扩大到整个 World。
    borrow: PhantomData<&'a mut u8>,
}
/// 活值的临时唯一拥有权，不拥有背后的分配，不能 Clone/Copy。
/// 来源容器/栈仍负责保活内存；消费后移入目标、read 或显式 drop_as，不能重复析构。
/// 不保存动态 Layout/析构函数，不承诺普通 drop 自动析构未知类型；调用方承担消费责任。
#[must_use = "移入目标、读取或使用已登记的析构能力处理此值"]
pub struct OwningPtr<'a> {
    /// 允许读取/移动的活值地址，类型和对齐由登记与存储准备保证。
    value: NonNull<u8>,
    /// 来源分配必须在本指针消费前保持有效且不能重用该槽。
    borrow: PhantomData<&'a mut u8>,
}
// @to_design 构造、重借用、类型转换、read/drop_as 的可信接口随后闭合。
// 不以类型声明或 must_use 代替已实现的析构协议；不提供 Send/Sync 或生命周期强转。
