//! 擦除环形队列配对芯；运行时 Layout、逻辑顺序和移出责任由调用方约束。
use crate::{
    ptr::{OwningPtr, Ptr, PtrMut},
    raw_vec_deque::RawVecDeque,
};
use pi_ecs_types::error::EcsError;
use std::{alloc::Layout, ptr::NonNull};

fn error(kind: pi_ecs_types::error::ErrorKind, message: &str) -> EcsError {
    pi_ecs_types::error::classified_error(kind, message)
}

fn stride(layout: Layout) -> usize {
    layout.pad_to_align().size()
}

fn allocation_layout(layout: Layout, capacity: usize) -> Option<Layout> {
    let size = layout.size().checked_mul(capacity)?;
    Layout::from_size_align(size, layout.align()).ok()
}

fn empty_allocation(layout: Layout) -> NonNull<u8> {
    let align = layout.align();
    NonNull::new(align as *mut u8).unwrap_or_else(NonNull::dangling)
}

impl RawVecDeque {
    pub(super) fn new_inner(layout: Layout) -> RawVecDeque {
        RawVecDeque {
            allocation: empty_allocation(layout),
            layout,
            capacity: 0,
            head: 0,
            len: 0,
        }
    }
}

impl RawVecDeque {
    pub(super) fn len_inner(queue: &RawVecDeque) -> usize {
        queue.len
    }
}

impl RawVecDeque {
    pub(super) fn is_empty_inner(queue: &RawVecDeque) -> bool {
        queue.len == 0
    }
}

impl RawVecDeque {
    pub(super) fn capacity_inner(queue: &RawVecDeque) -> usize {
        queue.capacity
    }
}

impl RawVecDeque {
    pub(super) fn layout_inner(queue: &RawVecDeque) -> Layout {
        queue.layout
    }
}

impl RawVecDeque {
    pub(super) fn reserve_inner(
        queue: &mut RawVecDeque,
        additional: usize,
    ) -> Result<(), EcsError> {
        let required = queue.len.checked_add(additional).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "RawVecDeque 容量溢出",
            )
        })?;
        if required <= queue.capacity {
            return Ok(());
        }
        let mut next = queue.capacity.max(1);
        while next < required {
            next = next.checked_mul(2).ok_or_else(|| {
                error(
                    pi_ecs_types::error::ErrorKind::ResourceExhausted,
                    "RawVecDeque 容量溢出",
                )
            })?;
        }
        if queue.layout.size() == 0 {
            queue.capacity = next;
            return Ok(());
        }
        let new_layout = allocation_layout(queue.layout, next).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "RawVecDeque 布局溢出",
            )
        })?;
        // SAFETY: new_layout 来自非零 Layout，分配失败返回空指针。
        let raw = unsafe { std::alloc::alloc(new_layout) };
        let Some(allocation) = NonNull::new(raw) else {
            return Err(error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "RawVecDeque 分配失败",
            ));
        };
        let element_stride = stride(queue.layout);
        for index in 0..queue.len {
            let source_index = (queue.head + index) % queue.capacity;
            let source = unsafe { queue.allocation.as_ptr().add(source_index * element_stride) };
            let target = unsafe { allocation.as_ptr().add(index * element_stride) };
            // SAFETY: 源/目标均为有效元素槽，新分配与旧分配不重叠。
            unsafe { std::ptr::copy_nonoverlapping(source, target, queue.layout.size()) };
        }
        if queue.capacity != 0 {
            let old_layout =
                allocation_layout(queue.layout, queue.capacity).expect("旧容量布局必须有效");
            // SAFETY: allocation 由同一布局分配，且活动值已复制。
            unsafe { std::alloc::dealloc(queue.allocation.as_ptr(), old_layout) };
        }
        queue.allocation = allocation;
        queue.capacity = next;
        queue.head = 0;
        Ok(())
    }
}

impl RawVecDeque {
    pub(super) fn push_back_inner(queue: &mut RawVecDeque, value: OwningPtr<'_>) {
        RawVecDeque::reserve_inner(queue, 1).expect("RawVecDeque 追加容量不足");
        let physical = (queue.head + queue.len) % queue.capacity;
        let element_stride = stride(queue.layout);
        let target = unsafe { queue.allocation.as_ptr().add(physical * element_stride) };
        if queue.layout.size() != 0 {
            // SAFETY: 目标槽未初始化，value 来源与队列不重叠且类型匹配。
            unsafe {
                std::ptr::copy_nonoverlapping(
                    value.as_non_null().as_ptr(),
                    target,
                    queue.layout.size(),
                )
            };
        }
        queue.len += 1;
    }
}

impl RawVecDeque {
    pub(super) fn push_front_inner(queue: &mut RawVecDeque, value: OwningPtr<'_>) {
        RawVecDeque::reserve_inner(queue, 1).expect("RawVecDeque 追加容量不足");
        queue.head = (queue.head + queue.capacity - 1) % queue.capacity;
        let element_stride = stride(queue.layout);
        let target = unsafe { queue.allocation.as_ptr().add(queue.head * element_stride) };
        if queue.layout.size() != 0 {
            // SAFETY: 目标槽未初始化，value 来源与队列不重叠且类型匹配。
            unsafe {
                std::ptr::copy_nonoverlapping(
                    value.as_non_null().as_ptr(),
                    target,
                    queue.layout.size(),
                )
            };
        }
        queue.len += 1;
    }
}

impl RawVecDeque {
    pub(super) fn get_inner(queue: &RawVecDeque, index: usize) -> Option<Ptr<'_>> {
        if index >= queue.len || queue.capacity == 0 {
            return None;
        }
        let physical = (queue.head + index) % queue.capacity;
        let pointer = unsafe {
            queue
                .allocation
                .as_ptr()
                .add(physical * stride(queue.layout))
        };
        // SAFETY: 逻辑活槽地址非空，借用由 queue 约束。
        Some(unsafe { Ptr::new(NonNull::new_unchecked(pointer)) })
    }
}

impl RawVecDeque {
    pub(super) fn get_mut_inner(queue: &mut RawVecDeque, index: usize) -> Option<PtrMut<'_>> {
        if index >= queue.len || queue.capacity == 0 {
            return None;
        }
        let physical = (queue.head + index) % queue.capacity;
        let pointer = unsafe {
            queue
                .allocation
                .as_ptr()
                .add(physical * stride(queue.layout))
        };
        // SAFETY: &mut queue 保证独占活槽，地址非空。
        Some(unsafe { PtrMut::new(NonNull::new_unchecked(pointer)) })
    }
}

impl RawVecDeque {
    pub(super) fn front_inner(queue: &RawVecDeque) -> Option<Ptr<'_>> {
        RawVecDeque::get_inner(queue, 0)
    }
}

impl RawVecDeque {
    pub(super) fn back_inner(queue: &RawVecDeque) -> Option<Ptr<'_>> {
        queue
            .len
            .checked_sub(1)
            .and_then(|index| RawVecDeque::get_inner(queue, index))
    }
}

impl RawVecDeque {
    pub(super) fn front_mut_inner(queue: &mut RawVecDeque) -> Option<PtrMut<'_>> {
        RawVecDeque::get_mut_inner(queue, 0)
    }
}

impl RawVecDeque {
    pub(super) fn back_mut_inner(queue: &mut RawVecDeque) -> Option<PtrMut<'_>> {
        let index = queue.len.checked_sub(1)?;
        RawVecDeque::get_mut_inner(queue, index)
    }
}

impl RawVecDeque {
    pub(super) fn pop_front_inner(queue: &mut RawVecDeque) -> Option<OwningPtr<'_>> {
        if queue.len == 0 || queue.capacity == 0 {
            return None;
        }
        let physical = queue.head;
        let pointer = unsafe {
            queue
                .allocation
                .as_ptr()
                .add(physical * stride(queue.layout))
        };
        queue.head = (queue.head + 1) % queue.capacity;
        queue.len -= 1;
        // SAFETY: 指针指向本队首活值，消费前不再复用该槽。
        Some(unsafe { OwningPtr::new(NonNull::new_unchecked(pointer)) })
    }
}

impl RawVecDeque {
    pub(super) fn pop_back_inner(queue: &mut RawVecDeque) -> Option<OwningPtr<'_>> {
        let index = queue.len.checked_sub(1)?;
        if queue.capacity == 0 {
            return None;
        }
        let physical = (queue.head + index) % queue.capacity;
        let pointer = unsafe {
            queue
                .allocation
                .as_ptr()
                .add(physical * stride(queue.layout))
        };
        queue.len -= 1;
        // SAFETY: 指针指向本队尾活值，消费前不再复用该槽。
        Some(unsafe { OwningPtr::new(NonNull::new_unchecked(pointer)) })
    }
}

impl RawVecDeque {
    pub(super) fn shrink_to_fit_inner(queue: &mut RawVecDeque) -> Result<(), EcsError> {
        if queue.capacity == queue.len {
            return Ok(());
        }
        if queue.layout.size() == 0 {
            queue.capacity = queue.len;
            queue.head = 0;
            return Ok(());
        }
        let target = queue.len.max(0);
        if target == 0 {
            if queue.capacity != 0 {
                let old_layout =
                    allocation_layout(queue.layout, queue.capacity).expect("旧容量布局必须有效");
                // SAFETY: 无活值时释放旧分配。
                unsafe { std::alloc::dealloc(queue.allocation.as_ptr(), old_layout) };
            }
            queue.allocation = empty_allocation(queue.layout);
            queue.capacity = 0;
            queue.head = 0;
            return Ok(());
        }
        let new_layout = allocation_layout(queue.layout, target).ok_or_else(|| {
            error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "RawVecDeque 布局溢出",
            )
        })?;
        let raw = unsafe { std::alloc::alloc(new_layout) };
        let Some(allocation) = NonNull::new(raw) else {
            return Err(error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "RawVecDeque 分配失败",
            ));
        };
        let element_stride = stride(queue.layout);
        for index in 0..queue.len {
            let source_index = (queue.head + index) % queue.capacity;
            let source = unsafe { queue.allocation.as_ptr().add(source_index * element_stride) };
            let target_ptr = unsafe { allocation.as_ptr().add(index * element_stride) };
            // SAFETY: 逻辑活值复制到新分配，两者不重叠。
            unsafe { std::ptr::copy_nonoverlapping(source, target_ptr, queue.layout.size()) };
        }
        let old_layout =
            allocation_layout(queue.layout, queue.capacity).expect("旧容量布局必须有效");
        // SAFETY: 活动值已复制，旧分配释放。
        unsafe { std::alloc::dealloc(queue.allocation.as_ptr(), old_layout) };
        queue.allocation = allocation;
        queue.capacity = target;
        queue.head = 0;
        Ok(())
    }
}

impl RawVecDeque {
    pub(super) fn drop_inner(queue: &mut RawVecDeque) {
        if queue.layout.size() == 0 || queue.capacity == 0 {
            return;
        }
        let layout = allocation_layout(queue.layout, queue.capacity).expect("容量布局必须有效");
        // SAFETY: allocation 由同一布局分配；活值析构由调用方负责。
        unsafe { std::alloc::dealloc(queue.allocation.as_ptr(), layout) };
    }
}
