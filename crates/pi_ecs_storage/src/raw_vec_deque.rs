//! 运行时 Layout 的擦除环形队列；服务 rc Removed 旧值，不依赖 World。
#[path = "raw_vec_deque_inner.rs"]
mod raw_vec_deque_inner;
use crate::ptr::{OwningPtr, Ptr, PtrMut};
use pi_ecs_types::error::EcsError;
use std::{alloc::Layout, ptr::NonNull};
/// 独占拥有一段环形分配，元素可字节搬移且非 Pin；不构造 Default，不逐元素堆分配。
/// 活值由调用者经登记释放能力处理；容器只回收分配，不自动执行 ECS 普通/特殊析构。
/// ZST 不分配元素空间，但每次移入仍是一份独立逻辑值及释放责任，包括带 Drop 的零大小类型。
/// ZST 可共用满足 layout 对齐的非空标记地址；不同逻辑值不要求地址不同，不按地址去重。
pub struct RawVecDeque {
    /// 唯一分配首址；空/ZST 使用满足 layout 对齐的标记，不释放零字节标记。
    pub(crate) allocation: NonNull<u8>,
    /// 固定元素大小/对齐；stride 从 pad_to_align 推导，不用 u8 默认对齐。
    pub(crate) layout: Layout,
    /// 非 ZST 为已分配槽数，不是字节数；ZST 为有限逻辑容量，不对应元素空间分配。
    pub(crate) capacity: usize,
    /// 队首物理槽；空队列为零，有容量时小于 capacity。
    pub(crate) head: usize,
    /// 活值数；逻辑 0..len 按环形寻址，其他槽不含活值。
    pub(crate) len: usize,
}
impl RawVecDeque {
    /// 创建空队列，layout 为真实元素布局，不初始化元素、不分配值空间。
    /// # Safety
    /// 调用者保证类型/Layout 匹配、值可字节搬移，并在队列销毁前移出或正确析构全部活值。
    /// 不得依赖普通 Drop 释放业务资源；异常路径同样须履行活值清理责任。
    pub unsafe fn new(layout: Layout) -> Self {
        Self::new_inner(layout)
    }
    /// 返回活值数，不分配、不修改队列。
    pub fn len(&self) -> usize {
        Self::len_inner(self)
    }
    /// 返回是否无活值，不分配、不改变容量。
    pub fn is_empty(&self) -> bool {
        Self::is_empty_inner(self)
    }
    /// 返回无需增长即可容纳的总元素数，不是剩余槽数或字节数。
    pub fn capacity(&self) -> usize {
        Self::capacity_inner(self)
    }
    /// 返回所有值共同的运行时布局，不改变队列。
    pub fn layout(&self) -> Layout {
        Self::layout_inner(self)
    }
    /// 为 additional 个新增值准备容量，前后两端共用容量；失败保持逻辑内容，成功增长可使旧地址失效。
    /// 容量足够不搬移；增长先分配，再按逻辑顺序搬入新分配，head 归零，len/值顺序不变。
    /// 搬移不调用业务代码或析构，旧分配只释放空间；分配失败原队列不变，增长比例不构成契约。
    /// 所有借用/移出指针须结束；容量/布局溢出或分配失败返回错误，不调用业务析构。
    /// ZST 仍 checked 计算 len + additional 和逻辑容量增长，不分配元素空间；溢出拒绝且状态不变。
    pub fn reserve(&mut self, additional: usize) -> Result<(), EcsError> {
        Self::reserve_inner(self, additional)
    }
    /// 尽力缩小空闲容量且不低于 len；可搬移值，不保证容量恰好等于 len，不连续化作为独立副作用。
    /// 失败保持队列不变；空队列释放分配并复位 head，保留 layout；ZST 只调整逻辑容量。
    /// 调用时不得保留元素借用或移出指针，不执行任何业务析构。
    pub fn shrink_to_fit(&mut self) -> Result<(), EcsError> {
        Self::shrink_to_fit_inner(self)
    }
    /// 将 value 移入队首，原有元素逻辑下标加一；容量不足自动扩容，可搬移现有值。
    /// 不要求预先 reserve；容量溢出 panic，分配失败采用标准集合分配失败路径，不返回可恢复错误。
    /// # Safety
    /// value 类型/Layout 匹配，来源在扩容和移入期间有效且不与本队列存储别名，无冲突借用；ZST 同样移交一份责任。
    pub unsafe fn push_front(&mut self, value: OwningPtr<'_>) {
        Self::push_front_inner(self, value)
    }
    /// 将 value 移入队尾，消费值责任但不接管来源分配；容量不足自动扩容，不要求预先 reserve。
    /// 容量溢出 panic，分配失败采用标准集合分配失败路径；增长可搬移现有值。
    /// ZST 虽无字节可复制，仍消费一份来源责任并增加一份队列活值，不省略逻辑提交。
    /// # Safety
    /// value 与 layout 匹配，来源在扩容和移入期间有效且不与本队列存储别名，无冲突借用。
    pub unsafe fn push_back(&mut self, value: OwningPtr<'_>) {
        Self::push_back_inner(self, value)
    }
    /// 按逻辑 index 读取，越界 None，不分配、不改变活值；返回借用阻止队列增长/移出。
    pub fn get(&self, index: usize) -> Option<Ptr<'_>> {
        Self::get_inner(self, index)
    }
    /// 按逻辑 index 独占借用活值；越界 None，不分配、不改队列顺序，不携带 ECS 变化标记能力。
    pub fn get_mut(&mut self, index: usize) -> Option<PtrMut<'_>> {
        Self::get_mut_inner(self, index)
    }
    /// 借用队首活值；空队列 None，不移交责任或改变队列。
    pub fn front(&self) -> Option<Ptr<'_>> {
        Self::front_inner(self)
    }
    /// 借用队尾活值；空队列 None，不移交责任或改变队列。
    pub fn back(&self) -> Option<Ptr<'_>> {
        Self::back_inner(self)
    }
    /// 独占借用队首活值；空队列 None，借用期间禁止队列结构修改。
    pub fn front_mut(&mut self) -> Option<PtrMut<'_>> {
        Self::front_mut_inner(self)
    }
    /// 独占借用队尾活值；空队列 None，借用期间禁止队列结构修改。
    pub fn back_mut(&mut self) -> Option<PtrMut<'_>> {
        Self::back_mut_inner(self)
    }
    /// 移出队尾责任；空队列 None，不分配、不搬移其他值、不析构、不收缩容量。
    /// 返回指针借住 self，消费完成前禁止修改/销毁队列；ZST 仍交还一份值责任。
    pub fn pop_back(&mut self) -> Option<OwningPtr<'_>> {
        Self::pop_back_inner(self)
    }
    /// 移出队首责任；空队列 None，不移动其他值，不析构旧值，不收缩分配。
    /// 只推进环形队首及减少 len，不分配、不连续化；逻辑下标随队首前移重新起于零。
    /// 返回指针借住 self，消费完成前不得复用该槽、增长或销毁队列。
    /// ZST 每次成功也移出一份责任，调用方须逐值释放或移交，不能因地址相同或 size == 0 跳过。
    pub fn pop_front(&mut self) -> Option<OwningPtr<'_>> {
        Self::pop_front_inner(self)
    }
}
impl Drop for RawVecDeque {
    /// 仅释放环形分配；全部活值已正确清理是 unsafe new 的持续调用者责任。
    fn drop(&mut self) {
        Self::drop_inner(self)
    }
}
// 标准双端队列顺序/边界/容量语义直接采用，不逐项作为新设计取舍。
// push_* 与标准 VecDeque 一样自动扩容；显式 reserve 返回可恢复错误，仅为可选优化，不是插入前置步骤。
// 不提供自动析构的 clear/truncate/drain 或隐式 Clone；调用者逐项 pop 后经登记能力释放。
// 不为当前消费者添加连续切片、排序、任意插入删除或专用迭代器；按 get 的逻辑下标遍历即可。
// 容器不持有业务析构能力，不引入 ECS 异常清理 guard：调用者须在 drop 前逐值 pop 并经登记能力释放；
// 增长/缩容只搬移字节，不调用业务代码。类型不手写 Send/Sync。
