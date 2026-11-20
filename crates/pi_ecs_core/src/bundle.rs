//! 创建与严格结构修改的唯一类型化输入；值直接移入存储，不做拥有值擦除。
#[path = "bundle_inner.rs"]
mod bundle_inner;
use crate::{
    component::{Component, ComponentId, PlainAccess},
    error::EcsError,
};
use bundle_inner::BundleParts;
use std::any::TypeId;
/// # 用途
/// 静态展平类型与按值直接写入协议，允许携带作用域寿命，不要求 static/Clone/Send/Sync。
/// 安全手写实现可错报或 panic；writer 独立验证类型/布局/顺序/来源与完整性，不信任实现以保证内存安全。
/// 不提供 Component blanket；普通叶子用 ComponentValue，组合用派生或 1..8 Bundle 元组。
pub trait Bundle: Sized {
    /// # 用途
    /// 展平完整类型，保留写入顺序；重复与未知类型在首次修改前拒绝，不构造待删除值。
    fn component_types() -> Vec<TypeId>;
    /// # 用途
    /// context 为本次目标只读来源；首次修改前验证全部内置责任，不移走 self、不增计数。
    /// 错误保持输入拥有责任，由消费操作正常释放；writer 安装时仍独立复核实际输入，防止手写实现绕过。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError>;
    /// # 用途
    /// 消费 self，按描述顺序直接调用受限 writer；漏写/多写/错序在发布前拒绝并标停。
    /// 用户实现可能展开；调用栈进度清理已写目标，剩余字段正常析构，不回滚、不发布半初始化实体。
    fn write(self, writer: &mut BundleWriter<'_>);
}
/// # 用途
/// 系统参数的 Bundle 类型族；同一族的运行输入可借用短寿命来源。
/// 常用静态输入以自身为 Shape；借用输入可关联到静态形状。
/// Shape 是安全 trait 的关联类型，只约束调用签名，不证明名义身份、类型清单或来源。
/// 系统操作逐项核对实际输入的有序类型清单；来源和实际写入仍由 Bundle 校验及 writer 独立核验。
/// 宿主入口仍只要求 Bundle。
pub trait SystemBundle: Bundle {
    /// 用于参数声明的静态形状，只读取类型清单，不构造该类型的值。
    type Shape: 'static;
}

impl SystemBundle for () {
    type Shape = ();
}

impl<C: Component<Access = PlainAccess>> SystemBundle for ComponentValue<C> {
    type Shape = Self;
}

impl<B0: SystemBundle> SystemBundle for (B0,) {
    type Shape = (B0::Shape,);
}

impl<B0: SystemBundle, B1: SystemBundle> SystemBundle for (B0, B1) {
    type Shape = (B0::Shape, B1::Shape);
}

impl<B0: SystemBundle, B1: SystemBundle, B2: SystemBundle> SystemBundle for (B0, B1, B2) {
    type Shape = (B0::Shape, B1::Shape, B2::Shape);
}

impl<B0: SystemBundle, B1: SystemBundle, B2: SystemBundle, B3: SystemBundle> SystemBundle
    for (B0, B1, B2, B3)
{
    type Shape = (B0::Shape, B1::Shape, B2::Shape, B3::Shape);
}

impl<B0: SystemBundle, B1: SystemBundle, B2: SystemBundle, B3: SystemBundle, B4: SystemBundle>
    SystemBundle for (B0, B1, B2, B3, B4)
{
    type Shape = (B0::Shape, B1::Shape, B2::Shape, B3::Shape, B4::Shape);
}

impl<
        B0: SystemBundle,
        B1: SystemBundle,
        B2: SystemBundle,
        B3: SystemBundle,
        B4: SystemBundle,
        B5: SystemBundle,
    > SystemBundle for (B0, B1, B2, B3, B4, B5)
{
    type Shape = (
        B0::Shape,
        B1::Shape,
        B2::Shape,
        B3::Shape,
        B4::Shape,
        B5::Shape,
    );
}

impl<
        B0: SystemBundle,
        B1: SystemBundle,
        B2: SystemBundle,
        B3: SystemBundle,
        B4: SystemBundle,
        B5: SystemBundle,
        B6: SystemBundle,
    > SystemBundle for (B0, B1, B2, B3, B4, B5, B6)
{
    type Shape = (
        B0::Shape,
        B1::Shape,
        B2::Shape,
        B3::Shape,
        B4::Shape,
        B5::Shape,
        B6::Shape,
    );
}

impl<
        B0: SystemBundle,
        B1: SystemBundle,
        B2: SystemBundle,
        B3: SystemBundle,
        B4: SystemBundle,
        B5: SystemBundle,
        B6: SystemBundle,
        B7: SystemBundle,
    > SystemBundle for (B0, B1, B2, B3, B4, B5, B6, B7)
{
    type Shape = (
        B0::Shape,
        B1::Shape,
        B2::Shape,
        B3::Shape,
        B4::Shape,
        B5::Shape,
        B6::Shape,
        B7::Shape,
    );
}

/// # 用途
/// 普通单组件叶子，保留具体 C，不分配中间 Box。
pub struct ComponentValue<C: Component<Access = PlainAccess>>(
    /// 唯一拥有的普通值，失败析构、成功移动到槽。
    pub C,
);
/// # 用途
/// 安装前只读验证来源，不保存组件值或目标地址，不开放任意 World。
pub struct BundleContext<'a> {
    /// 不复用的目标 World 身份。
    pub(crate) world_identity: u64,
    /// 短借身份表，不跨回调/析构持 Ref。
    pub(crate) entities: &'a crate::entity_store::EntityIdentityStore,
    /// 同源归零登记。
    pub(crate) pending: &'a crate::destruction::PendingDestruction,
    /// 停止状态，禁止新增责任但允许已有责任释放。
    pub(crate) access: &'a crate::world::AccessState,
}
/// # 用途
/// 单次调用的直接写通道；构造/槽地址不公开，不拥有输入值，不是批次或回滚事务。
/// 全部目标空间在移交前准备；普通/内置写方法均独立复核真实类型、来源及剩余声明。
pub struct BundleWriter<'a> {
    /// 原安装作用域提供的同源完整底层句柄；不从 context 的字段引用反推 World 地址。
    /// 仅用于输入验证/最终安装，不授予其他组件读写，不得保存到输入或目标组件。
    pub(crate) world: crate::unsafe_world_cell::UnsafeWorldCell<'a>,
    /// 本次源身份验证能力，传给写入方法以复核内置输入。
    pub(crate) context: BundleContext<'a>,
    /// 与 types/layouts/targets 对齐的组件编号。
    pub(crate) components: &'a [ComponentId],
    /// 实际 TypeId，不能只核对尺寸。
    pub(crate) types: &'a [TypeId],
    /// 登记布局，包含 ZST 对齐。
    pub(crate) layouts: &'a [std::alloc::Layout],
    /// 当前调用独占的未初始化目标地址，不能逃逸。
    pub(crate) targets: &'a [std::ptr::NonNull<u8>],
    /// 已完整初始化前缀，异常只释放这些槽；内置解除责任到推进进度之间不可展开。
    pub(crate) written: &'a mut usize,
}
impl BundleWriter<'_> {
    /// # 用途
    /// input 为具体组件输入，按 I::Component 独立核对真实类型、布局及剩余声明。
    /// 不信任安全 Bundle 的自报校验，输入仍拥有责任时再次 validate；拒绝标停并展开，输入正常清偿。
    /// 全部准备完成后调用 install，紧接着无展开地推进 written；install 违约展开必须 abort。
    /// 不猜测半槽初始化状态，不将未知目标纳入普通前缀清理；句柄不越过安装作用域。
    /// 同步消费 input，不要求其来源寿命覆盖整个 writer；目标组件必须 static，不保存输入借用。
    /// 所有 World 责任转移仍须校验同源，不能由相同 Rust 寿命推定属于相同 World。
    pub fn write_input<I: crate::bundle_input::ComponentInput>(&mut self, input: I) {
        self.write_input_inner(input)
    }
    /// # 用途
    /// value 为普通 C，核验类型/布局/顺序后直接移入槽；失败标停，输入责任不丢失。
    pub fn write<C: Component<Access = PlainAccess>>(&mut self, value: C) {
        self.write_value_inner(value)
    }
}

impl Bundle for () {
    /// # 用途
    /// 按元组位置描述嵌套类型，空元组为空集合；入口核验重复。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// context 同源验证全部成员，错误不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// writer 顺序直写成员，不构造中间值容器。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}

impl<B0: Bundle> Bundle for (B0,) {
    /// # 用途
    /// 按元组位置描述嵌套类型，空元组为空集合；入口核验重复。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// context 同源验证全部成员，错误不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// writer 顺序直写成员，不构造中间值容器。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}

impl<B0: Bundle, B1: Bundle> Bundle for (B0, B1) {
    /// # 用途
    /// 按元组位置描述嵌套类型，空元组为空集合；入口核验重复。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// context 同源验证全部成员，错误不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// writer 顺序直写成员，不构造中间值容器。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}

impl<B0: Bundle, B1: Bundle, B2: Bundle> Bundle for (B0, B1, B2) {
    /// # 用途
    /// 按元组位置描述嵌套类型，空元组为空集合；入口核验重复。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// context 同源验证全部成员，错误不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// writer 顺序直写成员，不构造中间值容器。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}

impl<B0: Bundle, B1: Bundle, B2: Bundle, B3: Bundle> Bundle for (B0, B1, B2, B3) {
    /// # 用途
    /// 按元组位置描述嵌套类型，空元组为空集合；入口核验重复。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// context 同源验证全部成员，错误不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// writer 顺序直写成员，不构造中间值容器。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}

impl<B0: Bundle, B1: Bundle, B2: Bundle, B3: Bundle, B4: Bundle> Bundle for (B0, B1, B2, B3, B4) {
    /// # 用途
    /// 按元组位置描述嵌套类型，空元组为空集合；入口核验重复。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// context 同源验证全部成员，错误不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// writer 顺序直写成员，不构造中间值容器。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}

impl<B0: Bundle, B1: Bundle, B2: Bundle, B3: Bundle, B4: Bundle, B5: Bundle> Bundle
    for (B0, B1, B2, B3, B4, B5)
{
    /// # 用途
    /// 按元组位置描述嵌套类型，空元组为空集合；入口核验重复。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// context 同源验证全部成员，错误不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// writer 顺序直写成员，不构造中间值容器。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}

impl<B0: Bundle, B1: Bundle, B2: Bundle, B3: Bundle, B4: Bundle, B5: Bundle, B6: Bundle> Bundle
    for (B0, B1, B2, B3, B4, B5, B6)
{
    /// # 用途
    /// 按元组位置描述嵌套类型，空元组为空集合；入口核验重复。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// context 同源验证全部成员，错误不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// writer 顺序直写成员，不构造中间值容器。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}

impl<
        B0: Bundle,
        B1: Bundle,
        B2: Bundle,
        B3: Bundle,
        B4: Bundle,
        B5: Bundle,
        B6: Bundle,
        B7: Bundle,
    > Bundle for (B0, B1, B2, B3, B4, B5, B6, B7)
{
    /// # 用途
    /// 按元组位置描述嵌套类型，空元组为空集合；入口核验重复。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// context 同源验证全部成员，错误不移动责任。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// writer 顺序直写成员，不构造中间值容器。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}
impl<C: Component<Access = PlainAccess>> Bundle for ComponentValue<C> {
    /// # 用途
    /// 描述唯一 C，不登记类型。
    fn component_types() -> Vec<TypeId> {
        Self::types_parts()
    }
    /// # 用途
    /// 普通值无内置责任，校验本次 context 可用。
    fn validate(&self, context: &BundleContext<'_>) -> Result<(), EcsError> {
        self.validate_parts(context)
    }
    /// # 用途
    /// 将 C 交给 writer，按具体类型移入存储。
    fn write(self, writer: &mut BundleWriter<'_>) {
        self.write_parts(writer)
    }
}
impl BundleWriter<'_> {
    /// # 用途
    /// value 为已拥有的内置输入；独立复核来源/具体组件与槽布局，直接移交，无分配或可展开责任空窗。
    pub(crate) fn write_strong<'s, const N: usize, T: 'static>(
        &mut self,
        value: crate::bundle_input::StrongArrayInput<'s, N, T>,
    ) {
        self.write_strong_inner(value)
    }
}
impl BundleWriter<'_> {
    /// # 用途
    /// value 为已拥有的内置输入；独立复核来源/具体组件与槽布局，直接移交，无分配或可展开责任空窗。
    pub(crate) fn write_weak<'s, const N: usize, T: 'static>(
        &mut self,
        value: crate::bundle_input::WeakArrayInput<'s, N, T>,
    ) {
        self.write_weak_inner(value)
    }
}
impl BundleWriter<'_> {
    /// # 用途
    /// value 为已拥有的内置输入；独立复核来源/具体组件与槽布局，直接移交，无分配或可展开责任空窗。
    pub(crate) fn write_list<T: 'static>(&mut self, value: crate::bundle_input::EmptyList<T>) {
        self.write_list_inner(value)
    }
}
impl BundleWriter<'_> {
    /// # 用途
    /// value 为已拥有的内置输入；独立复核来源/具体组件与槽布局，直接移交，无分配或可展开责任空窗。
    pub(crate) fn write_queue<T: 'static>(&mut self, value: crate::bundle_input::EmptyQueue<T>) {
        self.write_queue_inner(value)
    }
}
impl BundleWriter<'_> {
    /// # 用途
    /// value 为已拥有的内置输入；独立复核来源/具体组件与槽布局，直接移交，无分配或可展开责任空窗。
    pub(crate) fn write_deque<T: 'static>(&mut self, value: crate::bundle_input::EmptyDeque<T>) {
        self.write_deque_inner(value)
    }
}
