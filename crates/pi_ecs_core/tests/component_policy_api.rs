//! 仅编译外部扩展边界，不调用 World 构造或设计失败桩。
use pi_ecs_core::{
    bundle_input::ComponentInput,
    change_detection::ChangeMarker,
    component::Component,
    component_policy::{ComponentPolicy, ReleaseMode},
    error::EcsError,
    unsafe_world_cell::UnsafeWorldCell,
};
use std::mem::{ManuallyDrop, MaybeUninit};

struct ExternalValue(u32);
impl Component for ExternalValue {
    type Access = ExternalPolicy;
}
struct ExternalPolicy;
// SAFETY: 所有结果仅借当前槽，写结果只读，释放只销毁确切 ExternalValue，不访问 World。
unsafe impl ComponentPolicy<ExternalValue> for ExternalPolicy {
    type Read<'a, 'w> = &'a ExternalValue;
    type Write<'a, 'w> = &'a ExternalValue;
    unsafe fn read<'a, 'w>(
        value: &'a ExternalValue,
        _world: UnsafeWorldCell<'w>,
    ) -> Self::Read<'a, 'w> {
        value
    }
    unsafe fn write<'a, 'w>(
        value: &'a mut ExternalValue,
        _world: UnsafeWorldCell<'w>,
        _marker: ChangeMarker<'a>,
    ) -> Self::Write<'a, 'w> {
        value
    }
    unsafe fn read_removed<'a>(
        value: &'a ExternalValue,
        _world: UnsafeWorldCell<'a>,
    ) -> Self::Read<'a, 'a> {
        value
    }
    unsafe fn release(
        value: &mut ManuallyDrop<ExternalValue>,
        _world: UnsafeWorldCell<'_>,
        _mode: ReleaseMode,
    ) {
        // SAFETY: 框架交付当前具体值的唯一析构责任，随后不会再次析构。
        unsafe { ManuallyDrop::drop(value) };
    }
}
struct ExternalInput(u32);
struct BorrowedInput<'a>(&'a u32);
// SAFETY: 同步复制来源数值到 static 组件，不保存借用，不访问 World 或转移外部责任。
unsafe impl ComponentInput for BorrowedInput<'_> {
    type Component = ExternalValue;
    unsafe fn validate(&self, _world: UnsafeWorldCell<'_>) -> Result<(), EcsError> {
        Ok(())
    }
    unsafe fn install(self, target: &mut MaybeUninit<ExternalValue>, _world: UnsafeWorldCell<'_>) {
        target.write(ExternalValue(*self.0));
    }
}
fn short_source(writer: &mut pi_ecs_core::bundle::BundleWriter<'_>) {
    let source = 9u32;
    writer.write_input(BorrowedInput(&source));
}
// SAFETY: 输入没有 World 责任；校验不改状态，安装只构造并写入一个值，无分配、回调或析构。
unsafe impl ComponentInput for ExternalInput {
    type Component = ExternalValue;
    unsafe fn validate(&self, _world: UnsafeWorldCell<'_>) -> Result<(), EcsError> {
        Ok(())
    }
    unsafe fn install(self, target: &mut MaybeUninit<ExternalValue>, _world: UnsafeWorldCell<'_>) {
        target.write(ExternalValue(self.0));
    }
}

// 真正经过公开 writer 签名；不运行，因为 writer 芯仍是失败桩。
fn write_external(writer: &mut pi_ecs_core::bundle::BundleWriter<'_>) {
    writer.write_input(ExternalInput(7));
}

// 仅编译：现有输入按具体目标类型适配，不要求用途类型 Default，也不擦除来源寿命。
fn existing_inputs<'s>(
    writer: &mut pi_ecs_core::bundle::BundleWriter<'_>,
    strong: pi_ecs_core::bundle_input::StrongArrayInput<'s, 2, ExternalValue>,
    weak: pi_ecs_core::bundle_input::WeakArrayInput<'s, 2, ExternalValue>,
    list: pi_ecs_core::bundle_input::EmptyList<ExternalValue>,
    queue: pi_ecs_core::bundle_input::EmptyQueue<ExternalValue>,
    deque: pi_ecs_core::bundle_input::EmptyDeque<ExternalValue>,
) {
    writer.write_input(strong);
    writer.write_input(weak);
    writer.write_input(list);
    writer.write_input(queue);
    writer.write_input(deque);
}
