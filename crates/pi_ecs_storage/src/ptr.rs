//! 沿用 Bevy 指针命名和职责；本地设计声明，不是 bevy_ptr 的重导出或兼容承诺。
#[path = "ptr_inner.rs"]
mod ptr_inner;
use std::{marker::PhantomData, ptr::NonNull};
/// 擦除只读借用，不析构值或释放分配，不提前转换为具体 T。
pub struct Ptr<'a> {
    /// 指向活值的非空对齐地址，包括零大小组件的正确对齐。
    pub(crate) value: NonNull<u8>,
    /// 约束实际存储及 guard 的保活期。
    pub(crate) borrow: PhantomData<&'a u8>,
}
/// 擦除独占借用，不能 Clone/Copy，不得与冲突借用共存。
pub struct PtrMut<'a> {
    /// 已授权且唯一可写的活值地址。
    pub(crate) value: NonNull<u8>,
    /// 约束本次独占借用，不扩大到整个 World。
    pub(crate) borrow: PhantomData<&'a mut u8>,
}
/// 活值的临时唯一拥有权，不拥有背后的分配，不能 Clone/Copy。
/// 来源容器/栈仍负责保活内存；消费后移入目标、read 或显式 drop_as，不能重复析构。
/// 不保存动态 Layout/析构函数，不承诺普通 drop 自动析构未知类型；调用方承担消费责任。
#[must_use = "移入目标、读取或使用已登记的析构能力处理此值"]
pub struct OwningPtr<'a> {
    /// 允许读取/移动的活值地址，类型和对齐由登记与存储准备保证。
    pub(crate) value: NonNull<u8>,
    /// 来源分配必须在本指针消费前保持有效且不能重用该槽。
    pub(crate) borrow: PhantomData<&'a mut u8>,
}
impl<'a> Ptr<'a> {
    /// 从可信地址构造只读借用；不校验类型、对齐或存活，由调用方保证。
    /// # Safety
    /// value 必须在 'a 内指向一个已初始化、未被独占借用的活值。
    pub(crate) unsafe fn new(value: NonNull<u8>) -> Self {
        Self::ptr_new(value)
    }
    /// 返回底层非空地址，不延长借用、不授予写权限。
    pub fn as_non_null(&self) -> NonNull<u8> {
        Self::ptr_address(self)
    }
    /// 按调用方登记的真实类型重借用为共享引用。
    /// # Safety
    /// T 必须与来源存储类型一致，且借用期内不得发生写、移动或移出。
    pub(crate) unsafe fn as_ref<T: 'a>(&self) -> &'a T {
        Self::ptr_as_ref(self)
    }
    /// 在已授予独占授权的前提下转为独占借用。
    /// # Safety
    /// 必须证明来源当前没有其他借用，且调用方持有该位置的互斥写授权。
    pub(crate) unsafe fn into_mut(self) -> PtrMut<'a> {
        Self::ptr_into_mut(self)
    }
}
impl<'a> PtrMut<'a> {
    /// 从可信地址构造独占借用；不校验类型、对齐或存活，由调用方保证。
    /// # Safety
    /// value 必须在 'a 内指向一个已初始化活值，且不存在其他共享或独占借用。
    pub(crate) unsafe fn new(value: NonNull<u8>) -> Self {
        Self::ptr_mut_new(value)
    }
    /// 返回底层非空地址，不延长借用；写权限仍由本值独占。
    pub(crate) fn as_non_null(&self) -> NonNull<u8> {
        Self::ptr_mut_address(self)
    }
    /// 按调用方登记的真实类型重借用为可变引用。
    /// # Safety
    /// T 必须与来源存储类型一致；借用期内不得有别名访问。
    pub(crate) unsafe fn as_mut<T: 'a>(&mut self) -> &'a mut T {
        Self::ptr_mut_as_mut(self)
    }
    /// 降级为只读借用，结束独占写权限。
    pub(crate) fn into_ptr(self) -> Ptr<'a> {
        Self::ptr_mut_into_ptr(self)
    }
}
impl<'a> OwningPtr<'a> {
    /// 从可信地址构造临时拥有权；不接管背后分配，不校验类型或析构能力。
    /// # Safety
    /// value 必须在消费前保持有效，来源槽不得复用；类型与对齐由登记保证。
    pub(crate) unsafe fn new(value: NonNull<u8>) -> Self {
        Self::owning_new(value)
    }
    /// 从已脱离来源容器的裸地址重建拥有责任；不释放底层分配。
    /// # Safety
    /// value 必须指向已初始化、类型匹配且只剩这一份消费责任的值。
    pub unsafe fn from_raw(value: NonNull<u8>) -> Self {
        Self::owning_new(value)
    }
    /// 返回底层非空地址，不移交拥有权或析构责任。
    pub(crate) fn as_non_null(&self) -> NonNull<u8> {
        Self::owning_address(self)
    }
    /// 读出真实类型值并结束拥有权；来源槽不再包含活值。
    /// # Safety
    /// T 必须与登记类型一致且可按值移动，来源槽此后不得再次读取或析构。
    pub(crate) unsafe fn read<T>(self) -> T {
        Self::owning_read(self)
    }
    /// 使用登记析构能力释放值并结束拥有权；不释放背后分配。
    /// # Safety
    /// T 必须与登记类型及析构策略一致，调用后不得再次访问来源槽。
    pub(crate) unsafe fn drop_as<T>(self) {
        Self::owning_drop_as::<T>(self)
    }
    /// 降级为只读借用；拥有权与析构责任仍在调用方，借用结束前不得消费或覆盖值。
    pub(crate) fn into_ptr(self) -> Ptr<'a> {
        Self::owning_into_ptr(self)
    }
    /// 交出裸地址并结束拥有权；调用方按登记能力消费或析构，仍不拥有底层分配。
    /// # Safety
    /// 调用方必须保证地址在消费前有效；使用后不得再次访问 OwningPtr 来源槽。
    pub unsafe fn into_raw(self) -> *mut u8 {
        Self::owning_into_raw(self)
    }
}
// 构造/读取都由登记类型与活行状态提供前提，不提供运行时类型检查。
// 普通 drop 不自动析构未知类型；不提供 Send/Sync 或生命周期强转。
