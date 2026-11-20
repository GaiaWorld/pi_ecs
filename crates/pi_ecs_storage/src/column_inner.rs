//! 列操作配对芯；rc 路径可读写擦除槽，非 rc 并发路径仍为失败桩。
//!
//! 冻结视图的派生结果不能跨越增长：
//! ```compile_fail
//! use pi_ecs_storage::column::Column;
//! use pi_ecs_types::component::ComponentId;
//! let mut column = Column::new(ComponentId(0), std::alloc::Layout::new::<u32>()).unwrap();
//! let view = unsafe { column.bind_view() };
//! column.prepare(1).unwrap();
//! let _ = unsafe { view.read(0) };
//! ```
use crate::{
    column::Column,
    ptr::{OwningPtr, Ptr, PtrMut},
    settle::SettlePlan,
};
use pi_ecs_types::{
    change_detection::{ChangeTick, ComponentTicksMut},
    error::EcsError,
};
use std::ptr::NonNull;

unsafe fn noop_init(_value: *mut u8) {}

#[cfg(all(test, feature = "rc"))]
mod view_tests {
    use super::*;
    use std::{alloc::Layout, num::NonZeroU64};
    fn tick(value: u64) -> ChangeTick {
        ChangeTick::new(NonZeroU64::new(value).unwrap())
    }

    #[test]
    fn distinct_slots_keep_value_and_tick_writes_independent() {
        let mut column = Column::new(
            pi_ecs_types::component::ComponentId(0),
            Layout::new::<u32>(),
        )
        .unwrap();
        column.added = Some(pi_arr::Arr::new());
        column.changed = Some(pi_arr::Arr::new());
        for row in 0..2 {
            column.prepare(row).unwrap();
            let mut value = 10u32 + row as u32;
            unsafe {
                column.insert_raw(row, (&mut value as *mut u32).cast(), Layout::new::<u32>());
            }
            column.set_ticks(row, Some(tick(2)), Some(tick(3)));
        }
        unsafe {
            let first = column.bind_view();
            let second = column.bind_view();
            let snapshot = first.ticks(0);
            assert_eq!(snapshot.added, Some(tick(2)));
            let (mut a, at) = first.write(0);
            let a = a.as_mut::<u32>();
            let (mut b, bt) = second.write(1);
            let b = b.as_mut::<u32>();
            *a = 41;
            *b = 53;
            *at.changed.unwrap() = Some(tick(7));
            *bt.changed.unwrap() = Some(tick(9));
            assert_eq!((*a, *b), (41, 53));
            assert_eq!(first.ticks(0).changed, Some(tick(7)));
            assert_eq!(second.ticks(1).changed, Some(tick(9)));
        }
    }

    #[test]
    fn untracked_view_has_no_tick_slots() {
        let mut column =
            Column::new(pi_ecs_types::component::ComponentId(0), Layout::new::<()>()).unwrap();
        column.prepare(0).unwrap();
        unsafe {
            column.insert_raw(
                0,
                NonNull::<()>::dangling().as_ptr().cast(),
                Layout::new::<()>(),
            );
            let view = column.bind_view();
            assert!(view.ticks(0).added.is_none());
            let (_, ticks) = view.write(0);
            assert!(ticks.added.is_none() && ticks.changed.is_none());
        }
    }
}

impl Column {
    pub(super) fn new_inner(
        component: pi_ecs_types::component::ComponentId,
        layout: std::alloc::Layout,
    ) -> Result<Column, EcsError> {
        // SAFETY: 空槽 init 不构造业务值；drop 为 None，列不自动析构未知类型。
        let buffer = unsafe { pi_arr::RawArr::new(layout, noop_init, None) };
        Ok(Column {
            component,
            buffer,
            added: None,
            changed: None,
        })
    }
}

#[cfg(feature = "rc")]
impl Column {
    pub(super) fn get_inner(column: &Column, row: usize) -> Ptr<'_> {
        let ptr = column.buffer.get(row).expect("Column::get 要求活行已准备");
        // SAFETY: 活行已写入且借用由调用方约束。
        unsafe { Ptr::new(NonNull::new_unchecked(ptr)) }
    }
}
#[cfg(not(feature = "rc"))]
impl Column {
    pub(super) fn get_inner(_column: &Column, _row: usize) -> Ptr<'_> {
        todo!("编号：T107，尚未实现， 非 rc 列读取")
    }
}

#[cfg(feature = "rc")]
impl Column {
    pub(super) fn get_mut_inner(column: &mut Column, row: usize) -> PtrMut<'_> {
        let ptr = column
            .buffer
            .get(row)
            .expect("Column::get_mut 要求活行已准备");
        // SAFETY: 独占借用由 &mut column 保证。
        unsafe { PtrMut::new(NonNull::new_unchecked(ptr)) }
    }
}
#[cfg(not(feature = "rc"))]
impl Column {
    pub(super) fn get_mut_inner(_column: &mut Column, _row: usize) -> PtrMut<'_> {
        todo!("编号：T108，尚未实现， 非 rc 列写读取")
    }
}

#[cfg(feature = "rc")]
impl Column {
    pub(super) fn get_mut_with_ticks_inner<'a>(
        column: &'a mut Column,
        row: usize,
    ) -> (PtrMut<'a>, ComponentTicksMut<'a>) {
        let ptr = column
            .buffer
            .get(row)
            .expect("Column::get_mut_with_ticks 要求活行已准备");
        let added = column.added.as_mut().and_then(|ticks| ticks.get_mut(row));
        let changed = column.changed.as_mut().and_then(|ticks| ticks.get_mut(row));
        // SAFETY: buffer/added/changed 是不同字段，row 由同一独占列借用约束。
        let value = unsafe { PtrMut::new(NonNull::new_unchecked(ptr)) };
        (value, ComponentTicksMut { added, changed })
    }
}
#[cfg(not(feature = "rc"))]
impl Column {
    pub(super) fn get_mut_with_ticks_inner<'a>(
        _column: &'a mut Column,
        _row: usize,
    ) -> (PtrMut<'a>, ComponentTicksMut<'a>) {
        todo!("编号：T109，尚未实现， 非 rc 值与 tick 同步借用")
    }
}

#[cfg(feature = "rc")]
impl Column {
    pub(super) fn prepare_inner(column: &mut Column, row: usize) -> Result<(), EcsError> {
        column.buffer.alloc(row).map_err(|error| {
            use pi_ecs_types::error::{ErrorKind, Report};
            let kind = match error {
                pi_arr::RawArrError::InvalidRange => ErrorKind::InvalidInput,
                _ => ErrorKind::ResourceExhausted,
            };
            Report::new(kind)
                .attach_opaque(error)
                .attach("pi_arr 列准备失败")
        })?;
        if let Some(ticks) = column.added.as_mut() {
            ticks.alloc(row);
        }
        if let Some(ticks) = column.changed.as_mut() {
            ticks.alloc(row);
        }
        Ok(())
    }
}
#[cfg(not(feature = "rc"))]
impl Column {
    pub(super) fn prepare_inner(_column: &mut Column, _row: usize) -> Result<(), EcsError> {
        todo!("编号：T110，尚未实现， 非 rc 列槽准备")
    }
}

#[cfg(feature = "rc")]
impl Column {
    pub(super) fn insert_inner(column: &mut Column, row: usize, value: OwningPtr<'_>) {
        let target = column
            .buffer
            .get(row)
            .expect("Column::insert 要求目标槽已准备");
        let layout = column.buffer.element_layout();
        let target = NonNull::new(target).expect("已准备的列槽地址为空");
        // SAFETY: target 是同一布局的空槽，value 是唯一拥有值，源与目标不重叠。
        unsafe { OwningPtr::move_into_layout(value, target, layout) };
    }
}
#[cfg(not(feature = "rc"))]
impl Column {
    pub(super) fn insert_inner(_column: &mut Column, _row: usize, _value: OwningPtr<'_>) {
        todo!("编号：T111，尚未实现， 非 rc 列写入")
    }
}

#[cfg(feature = "rc")]
impl Column {
    pub(super) fn remove_inner(column: &mut Column, row: usize) -> OwningPtr<'_> {
        let source = column.buffer.get(row).expect("Column::remove 要求活行");
        // SAFETY: 值仍初始化，调用方取得唯一移出责任。
        unsafe { OwningPtr::new(NonNull::new_unchecked(source)) }
    }
}
#[cfg(not(feature = "rc"))]
impl Column {
    pub(super) fn remove_inner(_column: &mut Column, _row: usize) -> OwningPtr<'_> {
        todo!("编号：T112，尚未实现， 非 rc 列移出")
    }
}

#[cfg(not(feature = "rc"))]
impl Column {
    pub(super) fn settle_inner(_column: &mut Column, _plan: &SettlePlan) {
        todo!("编号：T113，尚未实现， 非 rc 按计划整理擦除缓冲（延期分支）")
    }
}

#[cfg(feature = "rc")]
impl Column {
    pub(super) fn settle_inner(column: &mut Column, plan: &SettlePlan) {
        // RC RawArr/Arr 本来就是连续区；无需调用会重新分配的 settle。
        // 普通容量保留，逻辑范围由 Table 的提交长度唯一维护。
        for action in &plan.moves {
            let source = action.source.index() as usize;
            let target = action.target.index() as usize;
            let bytes = column.buffer.element_layout().size();
            if bytes != 0 {
                // SAFETY: 公共计划保证源活、目标为空且不同；仅转移责任，不析构。
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        column.buffer.get(source).expect("源槽缺失"),
                        column.buffer.get(target).expect("目标槽缺失"),
                        bytes,
                    )
                };
            }
            for ticks in [&mut column.added, &mut column.changed]
                .into_iter()
                .flatten()
            {
                let value = ticks.get_mut(source).expect("源 tick 缺失").take();
                *ticks.get_mut(target).expect("目标 tick 缺失") = value;
            }
        }
        for ticks in [&mut column.added, &mut column.changed]
            .into_iter()
            .flatten()
        {
            for (_, slot) in ticks.slice_mut(plan.len..ticks.contiguous_len()) {
                *slot = None;
            }
        }
    }
}

#[cfg(feature = "rc")]
impl Column {
    pub(super) unsafe fn insert_raw_inner(
        column: &mut Column,
        row: usize,
        source: *mut u8,
        layout: std::alloc::Layout,
    ) {
        let target = column
            .buffer
            .get(row)
            .expect("Column::insert_raw 要求目标槽已准备");
        if layout.size() != 0 {
            unsafe { std::ptr::copy_nonoverlapping(source, target, layout.size()) };
        }
    }
}
#[cfg(not(feature = "rc"))]
impl Column {
    pub(super) unsafe fn insert_raw_inner(
        _column: &mut Column,
        _row: usize,
        _source: *mut u8,
        _layout: std::alloc::Layout,
    ) {
        todo!("编号：T114，尚未实现， 非 rc 原始值写入")
    }
}

#[cfg(feature = "rc")]
impl Column {
    pub(super) fn set_ticks_inner(
        column: &mut Column,
        row: usize,
        added: Option<pi_ecs_types::change_detection::ChangeTick>,
        changed: Option<pi_ecs_types::change_detection::ChangeTick>,
    ) {
        if let (Some(ticks), Some(value)) = (column.added.as_mut(), added) {
            if let Some(slot) = ticks.get_mut(row) {
                *slot = Some(value);
            }
        }
        if let (Some(ticks), Some(value)) = (column.changed.as_mut(), changed) {
            if let Some(slot) = ticks.get_mut(row) {
                *slot = Some(value);
            }
        }
    }
}
#[cfg(not(feature = "rc"))]
impl Column {
    pub(super) fn set_ticks_inner(
        _column: &mut Column,
        _row: usize,
        _added: Option<pi_ecs_types::change_detection::ChangeTick>,
        _changed: Option<pi_ecs_types::change_detection::ChangeTick>,
    ) {
        todo!("编号：T115，尚未实现， 非 rc tick 写入")
    }
}

#[cfg(feature = "rc")]
impl Column {
    pub(super) unsafe fn remove_raw_inner(column: &mut Column, row: usize) -> *mut u8 {
        column.buffer.get(row).expect("Column::remove_raw 要求活行")
    }
}
#[cfg(not(feature = "rc"))]
impl Column {
    pub(super) unsafe fn remove_raw_inner(_column: &mut Column, _row: usize) -> *mut u8 {
        todo!("编号：T116，尚未实现， 非 rc 原始值移出")
    }
}

#[cfg(feature = "rc")]
impl Column {
    pub(super) unsafe fn remove_raw_with_ticks_inner(
        column: &mut Column,
        row: usize,
    ) -> (
        *mut u8,
        Option<pi_ecs_types::change_detection::ChangeTick>,
        Option<pi_ecs_types::change_detection::ChangeTick>,
    ) {
        let pointer = column.buffer.get(row).expect("Column::remove_raw 要求活行");
        let added = column
            .added
            .as_ref()
            .and_then(|ticks| ticks.get(row).copied())
            .flatten();
        let changed = column
            .changed
            .as_ref()
            .and_then(|ticks| ticks.get(row).copied())
            .flatten();
        (pointer, added, changed)
    }
}
#[cfg(not(feature = "rc"))]
impl Column {
    pub(super) unsafe fn remove_raw_with_ticks_inner(
        _column: &mut Column,
        _row: usize,
    ) -> (
        *mut u8,
        Option<pi_ecs_types::change_detection::ChangeTick>,
        Option<pi_ecs_types::change_detection::ChangeTick>,
    ) {
        todo!("编号：T117，尚未实现， 非 rc tick 原始值移出")
    }
}

impl Column {
    pub(super) fn bind_view_inner(&self) -> super::ColumnView<'_> {
        super::ColumnView {
            component: self.component,
            values: &self.buffer,
            added: self.added.as_ref(),
            changed: self.changed.as_ref(),
        }
    }
}
impl<'a> super::ColumnView<'a> {
    pub(super) fn read_inner(&self, row: usize) -> Ptr<'a> {
        let pointer = self.values.get(row).expect("活列值槽必须已准备");
        // SAFETY: 冻结视图及调用方授权保证该单槽的存活、类型与共享访问。
        unsafe { Ptr::new(NonNull::new(pointer).expect("RawArr 非空槽地址")) }
    }
    pub(super) fn write_inner(&self, row: usize) -> (PtrMut<'a>, ComponentTicksMut<'a>) {
        let pointer = self.values.get(row).expect("活列值槽必须已准备");
        // 先完成全部布局验证，再兑现独占 tick；不借整列或整个数组。
        for array in [self.added, self.changed].into_iter().flatten() {
            assert!(
                array.get(row).expect("活列 tick 槽缺失").is_some(),
                "活列 tick 未初始化"
            );
        }
        // SAFETY: 调用方保证本行值及 tick 唯一，后端冻结，load 只创建单槽引用。
        unsafe {
            let added = self
                .added
                .map(|array| array.load(row).expect("已验证 Added 槽"));
            let changed = self
                .changed
                .map(|array| array.load(row).expect("已验证 Changed 槽"));
            (
                PtrMut::new(NonNull::new(pointer).expect("RawArr 非空槽地址")),
                ComponentTicksMut { added, changed },
            )
        }
    }
}

#[cfg(feature = "rc")]
impl super::ColumnMaintenance<'_> {
    pub(super) unsafe fn maintenance_get(&mut self, row: usize) -> Ptr<'_> {
        // SAFETY: 受限维护借用保证独占操作，原槽协议前提沿用；不替换容器。
        unsafe { (&mut *self.slot.get()).get(row) }
    }
    pub(super) unsafe fn maintenance_get_mut(&mut self, row: usize) -> PtrMut<'_> {
        // SAFETY: 受限维护借用保证独占操作，原槽协议前提沿用；不替换容器。
        unsafe { (&mut *self.slot.get()).get_mut(row) }
    }
    pub(super) fn maintenance_prepare(&mut self, row: usize) -> Result<(), EcsError> {
        // SAFETY: 受限维护借用保证独占操作，原槽协议前提沿用；不替换容器。
        unsafe { (&mut *self.slot.get()).prepare(row) }
    }
    pub(super) unsafe fn maintenance_insert(&mut self, row: usize, value: OwningPtr<'_>) {
        // SAFETY: 受限维护借用保证独占操作，原槽协议前提沿用；不替换容器。
        unsafe { (&mut *self.slot.get()).insert(row, value) }
    }
    pub(super) unsafe fn maintenance_remove(&mut self, row: usize) -> OwningPtr<'_> {
        // SAFETY: 受限维护借用保证独占操作，原槽协议前提沿用；不替换容器。
        unsafe { (&mut *self.slot.get()).remove(row) }
    }
    pub(super) unsafe fn maintenance_insert_raw(
        &mut self,
        row: usize,
        source: *mut u8,
        layout: std::alloc::Layout,
    ) {
        // SAFETY: 受限维护借用保证独占操作，原槽协议前提沿用；不替换容器。
        unsafe { (&mut *self.slot.get()).insert_raw(row, source, layout) }
    }
    pub(super) fn maintenance_set_ticks(
        &mut self,
        row: usize,
        added: Option<ChangeTick>,
        changed: Option<ChangeTick>,
    ) {
        // SAFETY: 受限维护借用保证独占操作，原槽协议前提沿用；不替换容器。
        unsafe { (&mut *self.slot.get()).set_ticks(row, added, changed) }
    }
    pub(super) unsafe fn maintenance_remove_raw(&mut self, row: usize) -> *mut u8 {
        // SAFETY: 受限维护借用保证独占操作，原槽协议前提沿用；不替换容器。
        unsafe { (&mut *self.slot.get()).remove_raw(row) }
    }
    pub(super) unsafe fn maintenance_remove_raw_with_ticks(
        &mut self,
        row: usize,
    ) -> (*mut u8, Option<ChangeTick>, Option<ChangeTick>) {
        // SAFETY: 受限维护借用保证独占操作，原槽协议前提沿用；不替换容器。
        unsafe { (&mut *self.slot.get()).remove_raw_with_ticks(row) }
    }
}

impl super::ColumnView<'_> {
    pub(super) fn ticks_inner(&self, row: usize) -> pi_ecs_types::change_detection::ComponentTicks {
        let read = |array: &pi_arr::Arr<Option<ChangeTick>>| {
            Some(
                array
                    .get(row)
                    .expect("活列 tick 槽缺失")
                    .expect("活列 tick 未初始化"),
            )
        };
        pi_ecs_types::change_detection::ComponentTicks {
            added: self.added.and_then(read),
            changed: self.changed.and_then(read),
        }
    }
}
