//! 列存储提供 OwningPtr 借用及移出接口，不提前绑定具体组件 T。
use crate::ptr::{OwningPtr, Ptr, PtrMut};
use pi_ecs_types::component::ComponentId;
use pi_ecs_types::error::EcsError;
/// 单一登记组件的物理列；活行由 Table 维护，移出不析构源值。
pub struct Column {
    /// 本列唯一组件编号。
    pub(crate) component: ComponentId,
    /// 运行时 Layout 的擦除槽存储，不绑定组件 T；分配槽不等于活组件。
    /// 空槽按 MaybeUninit<C> 的擦除语义初始化，不能构造 C::default 或将空槽读为 C。
    /// RawArr 不承担业务值析构；移出后由 core 登记的释放策略消费，留下合法空槽，防止重复析构。
    pub(crate) buffer: pi_arr::RawArr,
    // rc 为单线程模式，允许普通新增的容量增长搬移旧值；受影响引用/迭代器须先结束，旧地址缓存失效。
    // 非 rc 普通增删保持其他活值地址稳定；两种模式都不在增删中夹带填洞压缩。
    // @to_design 空槽回调、活行状态、OwningPtr 移出及增长后 fetch 地址重绑定的具体接线。
}
impl Column {
    /// component 为本 World 登记编号，layout 为已校验单值布局；只准备空槽存储，不持有析构/World 能力。
    /// 分配失败返回错误，不构造业务值；空槽和增长约束沿用本类型契约。
    pub fn new(component: ComponentId, layout: std::alloc::Layout) -> Result<Self, EcsError> {
        crate::column_inner::new_inner(component, layout)
    }
    /// row 为已准备的活 Table 行；返回只读 OwningPtr 借用，无值行不得调用。
    /// # Safety
    /// row 范围合法且拥有活值，访问无冲突，Table 在借用期间不可搬移或删除该行。
    pub unsafe fn get(&self, row: usize) -> Ptr<'_> {
        crate::column_inner::get_inner(self, row)
    }
    /// row 为合法活行，返回独占 OwningPtr 借用，不提前转为具体类型。
    /// # Safety
    /// 与 get 相同且必须独占访问该值，不能存在其他借用。
    pub unsafe fn get_mut(&mut self, row: usize) -> PtrMut<'_> {
        crate::column_inner::get_mut_inner(self, row)
    }
    /// 为 row 准备无值槽，不初始化组件；所有普通容量错误在移动前返回。
    pub fn prepare(&mut self, row: usize) -> Result<(), EcsError> {
        crate::column_inner::prepare_inner(self, row)
    }
    /// value 组件编号匹配且 row 已准备为空；消费拥有权，执行失败直接 panic。
    /// # Safety
    /// row 合法无值且类型已校验，不覆盖活实例，没有冲突借用。
    pub unsafe fn insert(&mut self, row: usize, value: OwningPtr<'_>) {
        crate::column_inner::insert_inner(self, row, value)
    }
    /// 移出 row 的拥有值，源不再拥有组件；不填洞、不析构移出值。
    /// # Safety
    /// row 合法且活、没有冲突借用；调用者同步更新 Table 行生命周期。
    pub unsafe fn remove(&mut self, row: usize) -> OwningPtr<'_> {
        crate::column_inner::remove_inner(self, row)
    }
    /// 独占同步点应用公共填洞计划再合并 pi_arr 桶，保留元数据，不重复析构。
    /// # Safety
    /// plan 源行活、目标洞无值，全部列共用同一计划且无访问者；执行失败直接 panic。
    pub(crate) unsafe fn settle(&mut self, plan: &crate::settle::SettlePlan) {
        crate::column_inner::settle_inner(self, plan)
    }
}
