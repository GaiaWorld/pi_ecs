//! 擦除指针的可信操作芯；类型校验与活行状态由调用方登记提供。
use crate::ptr::{OwningPtr, Ptr, PtrMut};
use std::ptr::NonNull;
/// 构造只读借用；地址有效性与无冲突由调用方保证。
impl<'a> Ptr<'a> {
    pub(super) fn ptr_new(value: NonNull<u8>) -> Ptr<'a> {
        Ptr {
            value,
            borrow: std::marker::PhantomData,
        }
    }
}
/// 返回只读借用地址，不做类型或生命周期转换。
impl<'a> Ptr<'a> {
    pub(super) fn ptr_address(value: &Ptr<'a>) -> NonNull<u8> {
        value.value
    }
}
/// 按登记类型重借用；调用方保证 T 匹配且无并发写。
impl<'a> Ptr<'a> {
    pub(super) fn ptr_as_ref<T>(value: &Ptr<'a>) -> &'a T {
        // SAFETY: 调用方保证地址指向真实 T，借用期无写/移动/移出。
        unsafe { &*value.value.as_ptr().cast::<T>() }
    }
}
/// 在独占授权下转为可变借用。
impl<'a> Ptr<'a> {
    pub(super) fn ptr_into_mut(value: Ptr<'a>) -> PtrMut<'a> {
        PtrMut {
            value: value.value,
            borrow: std::marker::PhantomData,
        }
    }
}
/// 构造独占借用；地址有效且无别名由调用方保证。
impl<'a> PtrMut<'a> {
    pub(super) fn ptr_mut_new(value: NonNull<u8>) -> PtrMut<'a> {
        PtrMut {
            value,
            borrow: std::marker::PhantomData,
        }
    }
}
/// 返回独占借用地址，不降级写权限。
impl<'a> PtrMut<'a> {
    pub(super) fn ptr_mut_address(value: &PtrMut<'a>) -> NonNull<u8> {
        value.value
    }
}
/// 按登记类型重借用为可变引用；调用方保证无别名。
impl<'a> PtrMut<'a> {
    pub(super) fn ptr_mut_as_mut<T>(value: &mut PtrMut<'a>) -> &'a mut T {
        // SAFETY: 调用方保证地址指向真实 T，且本次独占借用无别名。
        unsafe { &mut *value.value.as_ptr().cast::<T>() }
    }
}
/// 降级为只读借用，结束独占写权限。
impl<'a> PtrMut<'a> {
    pub(super) fn ptr_mut_into_ptr(value: PtrMut<'a>) -> Ptr<'a> {
        Ptr {
            value: value.value,
            borrow: std::marker::PhantomData,
        }
    }
}
/// 构造临时拥有权；来源分配保活由调用方负责。
impl<'a> OwningPtr<'a> {
    pub(super) fn owning_new(value: NonNull<u8>) -> OwningPtr<'a> {
        OwningPtr {
            value,
            borrow: std::marker::PhantomData,
        }
    }
}
/// 返回拥有值地址，不移交责任。
impl<'a> OwningPtr<'a> {
    pub(super) fn owning_address(value: &OwningPtr<'a>) -> NonNull<u8> {
        value.value
    }
}
#[cfg(test)]
mod owning_move_tests {
    use super::*;
    use std::mem::{ManuallyDrop, MaybeUninit};

    #[test]
    fn erased_move_transfers_the_whole_value_and_its_drop_responsibility() {
        let mut source = ManuallyDrop::new((String::from("whole value"), 317u64));
        let mut target = MaybeUninit::<(String, u64)>::uninit();
        // SAFETY: the source is live and uniquely owned; the target is distinct,
        // correctly aligned, and uninitialized. Only the destination is dropped.
        unsafe {
            let value = OwningPtr::from_raw(NonNull::from(&mut *source).cast());
            OwningPtr::move_into_layout(
                value,
                NonNull::from(&mut target).cast(),
                std::alloc::Layout::new::<(String, u64)>(),
            );
            assert_eq!(target.assume_init(), (String::from("whole value"), 317));
        }
    }
}

/// 按字节移入无活值目标，结束拥有权。
impl<'a> OwningPtr<'a> {
    /// Moves a live erased value using its registered layout.
    ///
    /// # Safety
    /// The slots must match the layout, be disjoint for nonzero sizes, and the
    /// target must be uninitialized. The source must not be read or dropped again.
    pub(crate) unsafe fn move_into_layout(
        value: OwningPtr<'a>,
        target: NonNull<u8>,
        layout: std::alloc::Layout,
    ) {
        if layout.size() != 0 {
            // SAFETY: the caller proved source/target validity, common layout and disjointness.
            unsafe {
                std::ptr::copy_nonoverlapping(value.value.as_ptr(), target.as_ptr(), layout.size())
            };
        }
    }
}
/// 按真实类型读出值，结束拥有权。
impl<'a> OwningPtr<'a> {
    pub(super) fn owning_read<T>(value: OwningPtr<'a>) -> T {
        // SAFETY: T 与来源登记一致，读出后调用方不再访问来源槽。
        unsafe { value.value.as_ptr().cast::<T>().read() }
    }
}
/// 使用登记析构能力释放值，结束拥有权。
impl<'a> OwningPtr<'a> {
    pub(super) fn owning_drop_as<T>(value: OwningPtr<'a>) {
        // SAFETY: T 与来源登记一致，调用后调用方不再访问来源槽。
        unsafe { std::ptr::drop_in_place(value.value.as_ptr().cast::<T>()) }
    }
}
/// 降级为只读借用，拥有权仍由调用方承担。
impl<'a> OwningPtr<'a> {
    pub(super) fn owning_into_ptr(value: OwningPtr<'a>) -> Ptr<'a> {
        Ptr {
            value: value.value,
            borrow: std::marker::PhantomData,
        }
    }
}
/// 交出裸地址并结束拥有权；不释放底层分配。
impl<'a> OwningPtr<'a> {
    pub(super) fn owning_into_raw(value: OwningPtr<'_>) -> *mut u8 {
        value.value.as_ptr()
    }
}
