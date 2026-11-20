//! 列存储提供 OwningPtr 借用及移出接口，不提前绑定具体组件 T。
#[path = "column_inner.rs"]
mod column_inner;
use crate::ptr::{OwningPtr, Ptr, PtrMut};
use pi_ecs_types::change_detection::{ChangeTick, ComponentTicksMut};
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
    /// Added tick 列，与 buffer 行号对齐；未启用追踪时不分配。
    /// 启用后活行槽为 Some；插入/搬行时复制，settle 与值一起搬移。
    pub(crate) added: Option<pi_arr::Arr<Option<ChangeTick>>>,
    /// Changed tick 列，与 buffer 行号对齐；未启用追踪时不分配。
    pub(crate) changed: Option<pi_arr::Arr<Option<ChangeTick>>>,
    // rc 为单线程模式，允许普通新增的容量增长搬移旧值；受影响引用/迭代器须先结束，旧地址缓存失效。
    // 非 rc 普通增删保持其他活值地址稳定；两种模式都不在增删中夹带填洞压缩。
    // 空槽以 noop init 回调分配：RawArr 只提供分段地址和容量，不构造 C、不读取未写入字节；
    // drop_element 为 None，本列不自动析构。insert 覆盖写入真实值，活行状态由 Table 维护，
    // 移出值交 OwningPtr，最终由 core 登记的释放能力消费。任何空槽都不得按 C 读取或转换。
    // 增长后旧地址失效；fetch 必须在借用结束后重绑地址，不缓存跨增长指针。
}
impl Column {
    /// component 为本 World 登记编号，layout 为已校验单值布局；只准备空槽存储，不持有析构/World 能力。
    /// 分配失败返回错误，不构造业务值；空槽和增长约束沿用本类型契约。
    pub fn new(component: ComponentId, layout: std::alloc::Layout) -> Result<Self, EcsError> {
        Self::new_inner(component, layout)
    }
    /// row 为已准备的活 Table 行；返回只读 OwningPtr 借用，无值行不得调用。
    /// # Safety
    /// row 范围合法且拥有活值，访问无冲突，Table 在借用期间不可搬移或删除该行。
    pub unsafe fn get(&self, row: usize) -> Ptr<'_> {
        Self::get_inner(self, row)
    }
    /// row 为合法活行，返回独占 OwningPtr 借用，不提前转为具体类型。
    /// # Safety
    /// 与 get 相同且必须独占访问该值，不能存在其他借用。
    pub unsafe fn get_mut(&mut self, row: usize) -> PtrMut<'_> {
        Self::get_mut_inner(self, row)
    }
    /// 为 row 准备无值槽，不初始化组件；所有普通容量错误在移动前返回。
    /// RawArr::alloc 的 Result 必须传播并保留原错误类型附件，不能丢弃后返回 Ok。
    /// 部分空容量增长可保留，但不发布活值或设置追踪；失败后的缓冲地址可能已变，旧视图必须失效。
    pub fn prepare(&mut self, row: usize) -> Result<(), EcsError> {
        Self::prepare_inner(self, row)
    }
    /// value 组件编号匹配且 row 已准备为空；消费拥有权，执行失败直接 panic。
    /// # Safety
    /// row 合法无值且类型已校验，不覆盖活实例，没有冲突借用。
    pub unsafe fn insert(&mut self, row: usize, value: OwningPtr<'_>) {
        Self::insert_inner(self, row, value)
    }
    /// 移出 row 的拥有值，源不再拥有组件；不填洞、不析构移出值。
    /// # Safety
    /// row 合法且活、没有冲突借用；调用者同步更新 Table 行生命周期。
    pub unsafe fn remove(&mut self, row: usize) -> OwningPtr<'_> {
        Self::remove_inner(self, row)
    }
    /// 同时返回独占值与已启用 tick 槽；未启用对应 tick 为 None。
    /// # Safety
    /// row 合法且活、没有冲突借用；tick 槽与值同属本次独占访问。
    pub(crate) unsafe fn get_mut_with_ticks(
        &mut self,
        row: usize,
    ) -> (PtrMut<'_>, ComponentTicksMut<'_>) {
        Self::get_mut_with_ticks_inner(self, row)
    }
    /// 把外部已初始化值按布局移入已准备行；调用方随后负责释放源分配。
    /// # Safety
    /// source 与布局匹配且由调用方唯一拥有；row 已 prepare。
    pub unsafe fn insert_raw(&mut self, row: usize, source: *mut u8, layout: std::alloc::Layout) {
        Self::insert_raw_inner(self, row, source, layout)
    }
    /// 设置该行已启用的 Added/Changed tick 槽；未启用分支忽略。
    pub fn set_ticks(
        &mut self,
        row: usize,
        added: Option<ChangeTick>,
        changed: Option<ChangeTick>,
    ) {
        Self::set_ticks_inner(self, row, added, changed)
    }
    /// 移出值的裸地址但不析构；调用方按 ComponentInfo 释放。
    /// # Safety
    /// row 为活行且独占访问。
    pub unsafe fn remove_raw(&mut self, row: usize) -> *mut u8 {
        Self::remove_raw_inner(self, row)
    }
    /// 移出值的裸地址并复制已启用的 Added/Changed tick；不析构值。
    /// # Safety
    /// row 为活行且独占访问；返回值地址在消费前有效。
    pub unsafe fn remove_raw_with_ticks(
        &mut self,
        row: usize,
    ) -> (*mut u8, Option<ChangeTick>, Option<ChangeTick>) {
        Self::remove_raw_with_ticks_inner(self, row)
    }
    /// 独占同步点应用公共填洞计划再合并 pi_arr 桶，保留元数据，不重复析构。
    /// # Safety
    /// plan 源行活、目标洞无值，全部列共用同一计划且无访问者；执行失败直接 panic。
    pub(crate) unsafe fn settle(&mut self, plan: &crate::settle::SettlePlan) {
        Self::settle_inner(self, plan)
    }
}

/// # 用途
/// 本次授权中的冻结列视图；只借后端描述，不借整个 Column 或组件切片，不拥有业务值。
/// 非 Clone、无重置；各字段可重复持有共享后端描述，不创建组件引用。丢弃视图不释放值，派生结果由原授权保活。
pub struct ColumnView<'a> {
    /// 已登记组件身份；与值及 tick 数组同源。
    pub(crate) component: ComponentId,
    /// 内部可变擦除后端，只经 get 定位单槽；分配状态不代表业务活行。
    pub(crate) values: &'a pi_arr::RawArr,
    /// 启用的 Added 数组，仅允许访问已初始化活行槽，不共享整个 tick 切片。
    pub(crate) added: Option<&'a pi_arr::Arr<Option<ChangeTick>>>,
    /// 启用的 Changed 数组；写入只经后端单槽内部可变入口。
    pub(crate) changed: Option<&'a pi_arr::Arr<Option<ChangeTick>>>,
}
impl Column {
    /// # 用途
    /// 在本存储首次交出结果前绑定冻结视图；不分配、不启停追踪、不建立组件引用。
    /// 各字段 prepare 可重复建立共享描述，不独占重借列；已有 get_mut 仅服务独占点查。
    /// # Safety
    /// 调用方持原始读/写授权并保证整个 'a 及派生结果期无增长、结构修改、追踪切换、搬移或释放。
    /// 列元数据与后端描述在 'a 内固定；后续写引用仅指向获准单槽，不能通过其他路径重借整列。
    pub unsafe fn bind_view<'a>(&'a self) -> ColumnView<'a> {
        self.bind_view_inner()
    }
}
impl<'a> ColumnView<'a> {
    /// # 用途
    /// row 为已校验活行，返回只借该值的擦除引用；不按容量推断存活、不标记变化。
    /// # Safety
    /// 调用方证明 row 存活、类型匹配且结果期无冲突写入，原授权保活到 'a。
    pub unsafe fn read(&self, row: usize) -> Ptr<'a> {
        self.read_inner(row)
    }
    /// # 用途
    /// row 为获准独占的活行，返回单值及对应已启用 tick 槽；不自行写 tick 或发布通知。
    /// # Safety
    /// 值与全部返回 tick 槽在 'a 内唯一，无既有冲突结果；不同 row 可并存，同 row 不可重复兑现。
    /// 检查权限/活行在形成引用前完成，内部布局失配 panic，不能伪造空 tick 掩盖错误。
    pub unsafe fn write(&self, row: usize) -> (PtrMut<'a>, ComponentTicksMut<'a>) {
        self.write_inner(row)
    }
}

/// # 用途
/// RC 固定容器的受限独占维护能力；无 Deref/DerefMut、解包、Clone 或整体替换入口。
/// 只在全部运行视图/结果结束后构造；缓存地址由原 UnsafeCell 导出，不能从临时维护引用派生。
#[cfg(feature = "rc")]
pub struct ColumnMaintenance<'a> {
    /// 唯一固定槽借用，槽自身不可替换；方法内部才临时访问容器内容，不向调用方交出对象引用。
    pub(crate) slot: &'a std::cell::UnsafeCell<Column>,
}

#[cfg(feature = "rc")]
impl ColumnMaintenance<'_> {
    /// # 用途
    /// 在受限维护期转发 Column::get，原参数、责任与失败语义不变；不交出容器引用。
    /// # Safety
    /// 满足原同名操作的完整槽有效性、类型、别名及拥有责任前提。
    pub unsafe fn get(&mut self, row: usize) -> Ptr<'_> {
        self.maintenance_get(row)
    }
    /// # 用途
    /// 在受限维护期转发 Column::get_mut，原参数、责任与失败语义不变；不交出容器引用。
    /// # Safety
    /// 满足原同名操作的完整槽有效性、类型、别名及拥有责任前提。
    pub unsafe fn get_mut(&mut self, row: usize) -> PtrMut<'_> {
        self.maintenance_get_mut(row)
    }
    /// # 用途
    /// 在受限维护期转发 Column::prepare，原参数、责任与失败语义不变；不交出容器引用。
    pub fn prepare(&mut self, row: usize) -> Result<(), EcsError> {
        self.maintenance_prepare(row)
    }
    /// # 用途
    /// 在受限维护期转发 Column::insert，原参数、责任与失败语义不变；不交出容器引用。
    /// # Safety
    /// 满足原同名操作的完整槽有效性、类型、别名及拥有责任前提。
    pub unsafe fn insert(&mut self, row: usize, value: OwningPtr<'_>) {
        self.maintenance_insert(row, value)
    }
    /// # 用途
    /// 在受限维护期转发 Column::remove，原参数、责任与失败语义不变；不交出容器引用。
    /// # Safety
    /// 满足原同名操作的完整槽有效性、类型、别名及拥有责任前提。
    pub unsafe fn remove(&mut self, row: usize) -> OwningPtr<'_> {
        self.maintenance_remove(row)
    }
    /// # 用途
    /// 在受限维护期转发 Column::insert_raw，原参数、责任与失败语义不变；不交出容器引用。
    /// # Safety
    /// 满足原同名操作的完整槽有效性、类型、别名及拥有责任前提。
    pub unsafe fn insert_raw(&mut self, row: usize, source: *mut u8, layout: std::alloc::Layout) {
        self.maintenance_insert_raw(row, source, layout)
    }
    /// # 用途
    /// 在受限维护期转发 Column::set_ticks，原参数、责任与失败语义不变；不交出容器引用。
    pub fn set_ticks(
        &mut self,
        row: usize,
        added: Option<ChangeTick>,
        changed: Option<ChangeTick>,
    ) {
        self.maintenance_set_ticks(row, added, changed)
    }
    /// # 用途
    /// 在受限维护期转发 Column::remove_raw，原参数、责任与失败语义不变；不交出容器引用。
    /// # Safety
    /// 满足原同名操作的完整槽有效性、类型、别名及拥有责任前提。
    pub unsafe fn remove_raw(&mut self, row: usize) -> *mut u8 {
        self.maintenance_remove_raw(row)
    }
    /// # 用途
    /// 在受限维护期转发 Column::remove_raw_with_ticks，原参数、责任与失败语义不变；不交出容器引用。
    /// # Safety
    /// 满足原同名操作的完整槽有效性、类型、别名及拥有责任前提。
    pub unsafe fn remove_raw_with_ticks(
        &mut self,
        row: usize,
    ) -> (*mut u8, Option<ChangeTick>, Option<ChangeTick>) {
        self.maintenance_remove_raw_with_ticks(row)
    }
}

impl ColumnView<'_> {
    /// # 用途
    /// row 为已验证目标，短读并复制该单槽 added/changed；未启用项为 None，不返回 tick 引用。
    /// 追踪已启用而活值槽未初始化为内部错误；稀疏完整身份缺失才返回外层 None。
    /// 不调用 load 取得独占 tick，不扫描整数组、不启停追踪或修改状态。
    /// # Safety
    /// 目标已发布且来源/类型正确，调用期间该 tick 槽无活独占引用或并发写入。
    /// 同实体写查询先复制过滤 tick 并结束短读，再调用 write；不同实体的写引用可存活。
    pub unsafe fn ticks(&self, row: usize) -> pi_ecs_types::change_detection::ComponentTicks {
        self.ticks_inner(row)
    }
}
