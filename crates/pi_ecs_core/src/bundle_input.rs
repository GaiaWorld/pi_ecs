//! 内置组件的类型化输入，拥有责任不擦除，存储头不向用户开放。
#[path = "bundle_input_inner.rs"]
mod bundle_input_inner;
use crate::{
    bundle::{Bundle, BundleContext, BundleWriter},
    error::EcsError,
    owned_entity::OwnedEntity,
};
use bundle_input_inner::InputParts;
use std::{any::TypeId, marker::PhantomData};
/// # 用途
/// 可扩展的单组件输入，Self 拥有尚未安装的责任，允许携带来源寿命，不要求 Default/Clone。
/// 本协议只负责安装一个具体组件，访问与最终释放由组件策略负责。
/// 输入来源寿命可短于 writer 的存储描述寿命：调用同步消费输入，不保存输入或其引用。
/// 关联组件要求 static，输入可复制数据或移交由目标 World 接管的责任，但不能保存来源借用。
/// 跨 World 同源性不能由寿命证明，必须由 validate 检查；普通拒绝保持责任可清偿。
/// 短借用输入可以描述 static 目标，不要求输入本身 static。
///
/// 目标组件不能借用短寿命来源，即使输入实现了 unsafe 协议。
/// ```compile_fail
/// use pi_ecs_core::component::{Component, PlainAccess};
/// struct Borrowed<'a>(&'a u32);
/// impl<'a> Component for Borrowed<'a> { type Access = PlainAccess; }
/// ```
/// # Safety
/// validate 不移走责任或修改业务值；正常拒绝或展开后输入仍须能完整清偿。
/// install 整个调用（包括剩余输入字段 Drop）不得展开，必须恰好初始化一个组件并解除输入责任。
/// 不得保存 target/world、扩大来源寿命或重建整份 World 引用；框架无法验证这些 unsafe 实现义务。
pub unsafe trait ComponentInput: Sized {
    /// 实际存储组件，不是输入包装类型；登记、布局与释放策略均按此类型核对。
    type Component: crate::component::Component;
    /// # 用途
    /// world 为本次安装来源，检查输入同源及状态兼容；不安装、不移走责任，允许重复校验。
    /// Err 保持输入可正常析构；所有分配与可失败准备须在 install 之前完成。
    /// # Safety
    /// 调用方保证 world 属当前有效安装作用域且无冲突引用；实现不得访问未获准业务值。
    unsafe fn validate(
        &self,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError>;
    /// # 用途
    /// 消费 self，将责任直接移入唯一空槽 target；world 与校验同源，相关状态未改变。
    /// 返回前完整初始化，不分配、不回调、不展开；框架随后无展开地推进已写前缀。
    /// 已移交责任只能由目标释放，不能再由输入析构；剩余字段析构也须无展开。
    /// # Safety
    /// 调用方已成功 validate，target 类型/对齐正确且不与来源重叠，全部准备保持有效。
    /// 实现者保证移交期间无可展开检查或索引操作；违约展开由框架 abort，不重试未知半槽。
    unsafe fn install(
        self,
        target: &mut std::mem::MaybeUninit<Self::Component>,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    );
}
/// # 用途
/// 强输入构造拒绝时交回完整原数组，不留下部分移交。
pub struct StrongArrayInputError<'s, const N: usize> {
    /// 来源混合或状态拒绝原因。
    pub error: EcsError,
    /// 每位置原有 strong，保持顺序，重复目标逐份拥有。
    pub values: [OwnedEntity<'s>; N],
}
/// # 用途
/// 固定强数组输入，安装转移已有 strong，不先减再增；N=0 不要求来源绑定。
pub struct StrongArrayInput<'s, const N: usize, T: 'static> {
    /// 原责任按固定数组保留，不转 Vec；Option 在无展开移交窗口取走一次。
    pub(crate) values: Option<[OwnedEntity<'s>; N]>,
    /// 用途类型，不拥有 T。
    pub(crate) marker: PhantomData<fn() -> T>,
}
impl<'s, const N: usize, T: 'static> StrongArrayInput<'s, N, T> {
    /// # 用途
    /// values 全部同源且非停止，成功接管原 strong；拒绝返回完整数组，计数不变。
    pub fn new(values: [OwnedEntity<'s>; N]) -> Result<Self, StrongArrayInputError<'s, N>> {
        Self::strong_new_inner(values)
    }
}
/// # 用途
/// 固定弱数组输入，独占已取得的 weak；Drop 清偿未安装责任，不要求输入 strong 对象持续存在。
pub struct WeakArrayInput<'s, const N: usize, T: 'static> {
    /// 同源字段引用；N=0 可无绑定。
    pub(crate) binding: Option<WeakInputBinding<'s>>,
    /// 固定槽保存已取得的 weak 身份；未初始化后缀为 None，重复目标逐份记录。
    pub(crate) values: [Option<crate::entity::Entity>; N],
    /// 用途类型，不拥有 T。
    pub(crate) marker: PhantomData<fn() -> T>,
}
/// # 用途
/// 弱输入释放所需的单一来源绑定，不持整个 World。
pub(crate) struct WeakInputBinding<'s> {
    /// 非复用 World 身份。
    pub(crate) identity: u64,
    /// 每项短借身份表，不跨析构持 RefMut。
    pub(crate) entities: &'s crate::entity_store::EntityIdentityStore,
    /// 弱归零登记，不直接回收槽。
    pub(crate) pending: &'s crate::destruction::PendingDestruction,
    /// 停止态仍允许交还已有责任。
    pub(crate) access: &'s crate::world::AccessState,
}
impl<'s, const N: usize, T: 'static> WeakArrayInput<'s, N, T> {
    /// # 用途
    /// values 只证明同源活身份，不消费/增持 strong；逐项增 weak 并立即记录清理责任。
    /// 溢出/失效/停止拒绝，清偿已取得前缀；结果不保留数组内引用本身，仅绑定 's。
    pub fn new(values: [&OwnedEntity<'s>; N]) -> Result<Self, EcsError> {
        Self::weak_new_inner(values)
    }
}
impl<const N: usize, T: 'static> Drop for WeakArrayInput<'_, N, T> {
    /// # 用途
    /// 逐项取走未安装责任再交还，停止态也清偿；不变量损坏尽可能标停后 abort，不二次 panic。
    fn drop(&mut self) {
        self.drop_inner()
    }
}
/// # 用途
/// EntityList<T> 的空头输入，不持节点或引用责任，不暴露存储头。
pub struct EmptyList<T: 'static>(
    /// 用途类型，构造时不创建 T。
    pub(crate) PhantomData<fn() -> T>,
);
impl<T: 'static> EmptyList<T> {
    /// # 用途
    /// 构造空请求，不访问 World，不登记类型；实际入口校验已登记目标组件。
    pub fn new() -> Self {
        Self::new_list_inner()
    }
}
/// # 用途
/// EntityQueue<T> 的空头输入，不持节点或引用责任，不暴露存储头。
pub struct EmptyQueue<T: 'static>(
    /// 用途类型，构造时不创建 T。
    pub(crate) PhantomData<fn() -> T>,
);
impl<T: 'static> EmptyQueue<T> {
    /// # 用途
    /// 构造空请求，不访问 World，不登记类型；实际入口校验已登记目标组件。
    pub fn new() -> Self {
        Self::new_queue_inner()
    }
}
/// # 用途
/// EntityDeque<T> 的空头输入，不持节点或引用责任，不暴露存储头。
pub struct EmptyDeque<T: 'static>(
    /// 用途类型，构造时不创建 T。
    pub(crate) PhantomData<fn() -> T>,
);
impl<T: 'static> EmptyDeque<T> {
    /// # 用途
    /// 构造空请求，不访问 World，不登记类型；实际入口校验已登记目标组件。
    pub fn new() -> Self {
        Self::new_deque_inner()
    }
}
impl<const N: usize, T: 'static> crate::bundle::SystemBundle for StrongArrayInput<'_, N, T> {
    type Shape = StrongArrayInput<'static, N, T>;
}
impl<const N: usize, T: 'static> crate::bundle::SystemBundle for WeakArrayInput<'_, N, T> {
    type Shape = WeakArrayInput<'static, N, T>;
}
impl<T: 'static> crate::bundle::SystemBundle for EmptyList<T> {
    type Shape = Self;
}
impl<T: 'static> crate::bundle::SystemBundle for EmptyQueue<T> {
    type Shape = Self;
}
impl<T: 'static> crate::bundle::SystemBundle for EmptyDeque<T> {
    type Shape = Self;
}

impl<'s, const N: usize, T: 'static> Bundle for StrongArrayInput<'s, N, T> {
    /// # 用途
    /// 描述具体内置组件 N/T，不描述输入包装自身。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// 首次修改前验证 context 同源、责任完整和状态可用；不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// 由封闭内置写入口复核来源后直接安装，责任解除到进度推进间不可展开。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}
impl<'s, const N: usize, T: 'static> Bundle for WeakArrayInput<'s, N, T> {
    /// # 用途
    /// 描述具体内置组件 N/T，不描述输入包装自身。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// 首次修改前验证 context 同源、责任完整和状态可用；不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// 由封闭内置写入口复核来源后直接安装，责任解除到进度推进间不可展开。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}
impl<T: 'static> Bundle for EmptyList<T> {
    /// # 用途
    /// 描述具体内置组件 N/T，不描述输入包装自身。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// 首次修改前验证 context 同源、责任完整和状态可用；不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// 由封闭内置写入口复核来源后直接安装，责任解除到进度推进间不可展开。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}
impl<T: 'static> Bundle for EmptyQueue<T> {
    /// # 用途
    /// 描述具体内置组件 N/T，不描述输入包装自身。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// 首次修改前验证 context 同源、责任完整和状态可用；不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// 由封闭内置写入口复核来源后直接安装，责任解除到进度推进间不可展开。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}
impl<T: 'static> Bundle for EmptyDeque<T> {
    /// # 用途
    /// 描述具体内置组件 N/T，不描述输入包装自身。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// 首次修改前验证 context 同源、责任完整和状态可用；不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// 由封闭内置写入口复核来源后直接安装，责任解除到进度推进间不可展开。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}

// SAFETY: 输入保持原责任至最终移交，具体芯须保证 install 全调用无展开且恰好初始化一个目标。
unsafe impl<C: crate::component::Component<Access = crate::component::PlainAccess>> ComponentInput
    for crate::bundle::ComponentValue<C>
{
    type Component = C;
    /// # 用途
    /// world 为本次安装来源，复核来源/停止态及输入责任完整性；不移动责任，错误保持输入可清偿。
    /// # Safety
    /// 调用方保证同源安装作用域及无冲突引用；N=0 无来源绑定不免除目标状态检查。
    unsafe fn validate(
        &self,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        self.plain_validate_input_inner(world)
    }
    /// # 用途
    /// 将已验证输入责任移入唯一空槽，保留固定数组/空头表示，不增减后再重建计数。
    /// 返回后输入不再释放已移交责任；整个调用无分配/回调/展开，writer 随后推进 written。
    /// # Safety
    /// 调用方已完成 validate、类型/布局和目标准备，状态未改变；target 不与输入重叠。
    unsafe fn install(
        self,
        target: &mut std::mem::MaybeUninit<Self::Component>,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        self.plain_install_input_inner(target, world)
    }
}

// SAFETY: 输入保持原责任至最终移交，具体芯须保证 install 全调用无展开且恰好初始化一个目标。
unsafe impl<'s, const N: usize, T: 'static> ComponentInput for StrongArrayInput<'s, N, T> {
    type Component = crate::owned_entity::StrongEntities<N, T>;
    /// # 用途
    /// world 为本次安装来源，复核来源/停止态及输入责任完整性；不移动责任，错误保持输入可清偿。
    /// # Safety
    /// 调用方保证同源安装作用域及无冲突引用；N=0 无来源绑定不免除目标状态检查。
    unsafe fn validate(
        &self,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        self.strong_validate_input_inner(world)
    }
    /// # 用途
    /// 将已验证输入责任移入唯一空槽，保留固定数组/空头表示，不增减后再重建计数。
    /// 返回后输入不再释放已移交责任；整个调用无分配/回调/展开，writer 随后推进 written。
    /// # Safety
    /// 调用方已完成 validate、类型/布局和目标准备，状态未改变；target 不与输入重叠。
    unsafe fn install(
        self,
        target: &mut std::mem::MaybeUninit<Self::Component>,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        self.strong_install_input_inner(target, world)
    }
}

// SAFETY: 输入保持原责任至最终移交，具体芯须保证 install 全调用无展开且恰好初始化一个目标。
unsafe impl<'s, const N: usize, T: 'static> ComponentInput for WeakArrayInput<'s, N, T> {
    type Component = crate::owned_entity::WeakEntities<N, T>;
    /// # 用途
    /// world 为本次安装来源，复核来源/停止态及输入责任完整性；不移动责任，错误保持输入可清偿。
    /// # Safety
    /// 调用方保证同源安装作用域及无冲突引用；N=0 无来源绑定不免除目标状态检查。
    unsafe fn validate(
        &self,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        self.weak_validate_input_inner(world)
    }
    /// # 用途
    /// 将已验证输入责任移入唯一空槽，保留固定数组/空头表示，不增减后再重建计数。
    /// 返回后输入不再释放已移交责任；整个调用无分配/回调/展开，writer 随后推进 written。
    /// # Safety
    /// 调用方已完成 validate、类型/布局和目标准备，状态未改变；target 不与输入重叠。
    unsafe fn install(
        self,
        target: &mut std::mem::MaybeUninit<Self::Component>,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        self.weak_install_input_inner(target, world)
    }
}

// SAFETY: 输入保持原责任至最终移交，具体芯须保证 install 全调用无展开且恰好初始化一个目标。
unsafe impl<T: 'static> ComponentInput for EmptyList<T> {
    type Component = crate::entity_container::EntityList<T>;
    /// # 用途
    /// world 为本次安装来源，复核来源/停止态及输入责任完整性；不移动责任，错误保持输入可清偿。
    /// # Safety
    /// 调用方保证同源安装作用域及无冲突引用；N=0 无来源绑定不免除目标状态检查。
    unsafe fn validate(
        &self,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        self.list_validate_input_inner(world)
    }
    /// # 用途
    /// 将已验证输入责任移入唯一空槽，保留固定数组/空头表示，不增减后再重建计数。
    /// 返回后输入不再释放已移交责任；整个调用无分配/回调/展开，writer 随后推进 written。
    /// # Safety
    /// 调用方已完成 validate、类型/布局和目标准备，状态未改变；target 不与输入重叠。
    unsafe fn install(
        self,
        target: &mut std::mem::MaybeUninit<Self::Component>,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        self.list_install_input_inner(target, world)
    }
}

// SAFETY: 输入保持原责任至最终移交，具体芯须保证 install 全调用无展开且恰好初始化一个目标。
unsafe impl<T: 'static> ComponentInput for EmptyQueue<T> {
    type Component = crate::entity_container::EntityQueue<T>;
    /// # 用途
    /// world 为本次安装来源，复核来源/停止态及输入责任完整性；不移动责任，错误保持输入可清偿。
    /// # Safety
    /// 调用方保证同源安装作用域及无冲突引用；N=0 无来源绑定不免除目标状态检查。
    unsafe fn validate(
        &self,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        self.queue_validate_input_inner(world)
    }
    /// # 用途
    /// 将已验证输入责任移入唯一空槽，保留固定数组/空头表示，不增减后再重建计数。
    /// 返回后输入不再释放已移交责任；整个调用无分配/回调/展开，writer 随后推进 written。
    /// # Safety
    /// 调用方已完成 validate、类型/布局和目标准备，状态未改变；target 不与输入重叠。
    unsafe fn install(
        self,
        target: &mut std::mem::MaybeUninit<Self::Component>,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        self.queue_install_input_inner(target, world)
    }
}

// SAFETY: 输入保持原责任至最终移交，具体芯须保证 install 全调用无展开且恰好初始化一个目标。
unsafe impl<T: 'static> ComponentInput for EmptyDeque<T> {
    type Component = crate::entity_container::EntityDeque<T>;
    /// # 用途
    /// world 为本次安装来源，复核来源/停止态及输入责任完整性；不移动责任，错误保持输入可清偿。
    /// # Safety
    /// 调用方保证同源安装作用域及无冲突引用；N=0 无来源绑定不免除目标状态检查。
    unsafe fn validate(
        &self,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) -> Result<(), EcsError> {
        self.deque_validate_input_inner(world)
    }
    /// # 用途
    /// 将已验证输入责任移入唯一空槽，保留固定数组/空头表示，不增减后再重建计数。
    /// 返回后输入不再释放已移交责任；整个调用无分配/回调/展开，writer 随后推进 written。
    /// # Safety
    /// 调用方已完成 validate、类型/布局和目标准备，状态未改变；target 不与输入重叠。
    unsafe fn install(
        self,
        target: &mut std::mem::MaybeUninit<Self::Component>,
        world: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    ) {
        self.deque_install_input_inner(target, world)
    }
}
