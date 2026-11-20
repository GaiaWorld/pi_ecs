//! 稀疏组件后端协议和枚举分派；SparseArray 自身提供映射 compact，不绑定具体组件 T。
#[path = "sparse_set_inner.rs"]
mod sparse_set_inner;
use crate::ptr::{OwningPtr, Ptr, PtrMut};
use pi_ecs_types::component::ComponentId;
use pi_ecs_types::entity::Entity;
use pi_ecs_types::error::EcsError;
#[cfg(not(feature = "rc"))]
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize};
/// 稀疏一级目录固定页容量（出生分区条目数）；Pages 要求非零二次幂。
/// 每页 64 个分区条目；DOMAIN_BITS=12 的 4096 个分区共 64 页。
pub const SPARSE_PAGE: usize = 64;
/// 单组件稀疏存储接口，调用方先校验身份并准备空间；执行失败直接 panic。
/// 同一稀疏位置的冲突操作由调用者保证不重叠；系统调度的冲突管理负责满足此前提。
/// 共享读取只依赖 Release/Acquire 发布和稳定槽地址，不持有额外 guard：
/// 非 rc 分段地址稳定，段增长不搬移旧槽；rc 增长需要独占，借用期内无法增长。
/// 写入必须持有互斥授权；移出必须证明无冲突借用。本后端不做逐槽冲突检测/仲裁。
pub trait SparseSet {
    /// 返回本存储组件编号，不改变状态。
    fn component(&self) -> ComponentId;
    /// entity 必须为完整键；缺失返回 None，借用期间不得移出/替换该值。
    fn get(&self, entity: Entity) -> Option<Ptr<'_>>;
    /// entity 必须为完整键；缺失返回 None，返回独占借用，不执行类型转换。
    fn get_mut(&mut self, entity: Entity) -> Option<PtrMut<'_>>;
    /// 为新增 entity 预留空间，不安装值；普通容量错误在移动前返回。
    fn prepare(&mut self, entity: Entity) -> Result<(), EcsError>;
    /// entity 为空且已准备，value 类型已校验；消费拥有权，不隐式替换现有值。
    /// 执行失败直接 panic，不返回普通错误或回滚。
    fn insert(&mut self, entity: Entity, value: OwningPtr<'_>);
    /// 移除完整 entity 并返回拥有值；缺失返回 None，不析构移出值、不移动其他实体。
    /// 返回值借用源存储/保护期，必须移交、读取或显式析构；普通 drop 不自动析构未知类型。
    fn remove(&mut self, entity: Entity) -> Option<OwningPtr<'_>>;
}
/// 两级稀疏数组；一级按出生分区寻址，二级 RawArr 直接按分区内本地槽索引寻址。
/// 不拼接域和槽，不引入固定 B 分块；代际不参与寻址，以槽元数据的完整 Entity 校验。
/// 多线程配置一级必须使用 Pages，不以固定域数为理由改用 Vec；rc 单线程一级才使用 Vec。
/// P 为 Pages 的物理页长，非零二次幂；后端固定取 SPARSE_PAGE，不由组件单独配置。
pub(crate) struct RawSparseArray<const P: usize> {
    /// 一级目录按出生分区编号索引；分区条目按需初始化，不默认构造业务组件。
    pub(crate) directory: SparseArrayDirectory<P>,
}
/// 类型化紧凑槽的实际表示；封闭于本模块，逻辑 Entity 不因原子存储改变公开身份类型。
pub(crate) trait SparseElement {
    /// 已初始化的 owner 键槽或普通未初始化值槽，不能混用普通/原子访问。
    type Members<const P: usize>;
}
impl SparseElement for Entity {
    type Members<const P: usize> = SparseOwners<P>;
}
impl SparseElement for Option<pi_ecs_types::change_detection::ChangeTick> {
    type Members<const P: usize> = ();
}
/// owner null 同时代表尚未发布/已退休；不另存发布状态数组，不据 null 判断洞可复用。
/// pi_key::KeyData::as_ffi/from_ffi 无损转换完整键，null 编码是 u64::MAX，不是零。
/// 非 rc 目标需要原生 64 位原子；不支持时的目标策略仍待定，不以锁模拟无锁保证。
pub(crate) struct SparseOwner {
    /// 两种模式统一保存完整键编码，显式初始化为 u64::MAX，不使用零。
    #[cfg(feature = "rc")]
    pub(crate) key: u64,
    /// 非 rc 为 AtomicU64；Release/Acquire 只在本模式生效。
    #[cfg(not(feature = "rc"))]
    pub(crate) key: AtomicU64,
}
/// 稀疏位置到紧凑 owner 下标的槽；合法值为 0..u32::MAX，u32::MAX 唯一表示不可用。
/// 不存运行时判别码：rc 为普通 u32，非 rc 为原子 u32。
/// 发布约定：先填组件值/tick/owner，再发布 owner，最后发布本下标；
/// 点查先读下标，再读 owner 并核对完整 Entity；删除先失效本下标。
pub(crate) struct SparseDenseIndex {
    /// rc 显式初始化为 u32::MAX；独占写入，无原子开销。
    #[cfg(feature = "rc")]
    pub(crate) index: u32,
    /// 非 rc 显式初始化为 u32::MAX；Acquire 读、Release 写，删除先失效。
    #[cfg(not(feature = "rc"))]
    pub(crate) index: AtomicU32,
}
/// 非 rc 待整理列表；当前使用本地 pi_append_vec，不自行实现链表。
#[cfg(not(feature = "rc"))]
pub(crate) struct PendingSparseRemovals {
    /// 每项为退休紧凑下标，不是 Entity；同一活成员删除只登记一次，旧洞 compact 前不复用。
    /// 当前依赖 push 使用短提交锁，不是无锁写入；只串行化记录追加，不保护本库组件值/映射。
    /// len 是已发布前缀，但 compact/清空仍必须等所有追加者收束，不能只观察长度替代等待。
    pub(crate) indices: pi_append_vec::AppendVec<usize>,
}
/// 组件成员集合的唯一拥有者；仅 SparseArray<Entity> 持有，值和 tick 数组不复制本结构。
pub(crate) struct SparseOwners<const P: usize> {
    /// 紧凑完整 Entity/发布标记；null 为未发布或退休，反向映射单一来源。
    pub(crate) dense: pi_arr::Arr<SparseOwner>,
    /// 域/槽到紧凑下标的两级数组；非 rc 一级必须 Pages，二级 RawArr 保存原子 u32 槽。
    /// 编码遵守 SparseDenseIndex；本数组仅存映射，不再包含自己的成员集合。
    pub(crate) indices: RawSparseArray<P>,
    /// 预留上界而非有效前缀，最多 u32::MAX 项。
    /// 退休洞 compact 前仍占范围，不回绕、不分配空码、不隐式整理。
    #[cfg(feature = "rc")]
    pub(crate) len: usize,
    /// 非 rc 使用原子长度，支持并发预留；rc 使用普通 usize。
    #[cfg(not(feature = "rc"))]
    pub(crate) len: AtomicUsize,
    /// 唯一待整理退休下标列表；rc 编译期消除，不保留无用列表字段。
    #[cfg(not(feature = "rc"))]
    pub(crate) pending_removals: PendingSparseRemovals,
}
impl<const P: usize> SparseOwners<P> {
    /// 在独占安全收束点整理本集合的非 rc 待删除洞，联合更新 dense/indices/len。
    /// 非 rc：全部追加线程退出后读取退休下标、跳过失效尾项，用存活尾 Entity 填洞并修复下标。
    /// 完成后才清理记录及截短范围；此前不复用任何旧洞，不搬移组件值、不承诺组件值连续化。
    /// rc：空操作，不分配、不修改映射；删除已立即用尾部 owner 填洞、修复下标并缩短范围。
    /// 全过程须结束受影响借用，保持业务 tick/通知不变；提交失败停止，不假成功。
    pub(crate) fn compact(&mut self) {
        Self::compact_inner(self)
    }
}
/// 类型化两级数组，复用 RawSparseArray 的布局、分配和域/槽寻址，不复制算法。
/// T 的有效值/空槽及析构规则须由类型化接口约束，不能只凭 Layout 将任意字节转换为 T。
pub(crate) struct SparseArray<T: SparseElement, const P: usize> {
    /// 擦除存储的真实拥有者，二级仍统一使用 pi_arr::RawArr。
    pub(crate) storage: RawSparseArray<P>,
    /// Entity 对应 SparseOwners，ChangeTick 对应零大小 ()；值/tick 不再复制长度、紧凑表或退休记录。
    pub(crate) members: T::Members<P>,
    /// 绑定真实元素类型及其拥有关系，不存放 T 实例。
    pub(crate) marker: std::marker::PhantomData<T>,
}
/// 单个组件的稀疏后端；四个数组均独立按出生分区/本地槽寻址，不将元数据嵌入值数组分区条目。
pub(crate) struct SparseArraySparseSet<const P: usize> {
    /// 本实例唯一组件编号。
    pub(crate) component: ComponentId,
    /// 组件值的单元素布局；域内 RawArr 按需用该布局创建，不持有业务值。
    pub(crate) value_layout: std::alloc::Layout,
    /// 擦除组件值，分配槽与业务活值分开管理。
    pub(crate) values: RawSparseArray<P>,
    /// 必存的完整身份数组，同时承担紧凑反向映射；Lookup-only 也不省略，不增加注册开关。
    /// owners.members.dense 在 0..len 的已发布活项保存完整 Entity；未发布/退休位置不可读取。
    /// 由已发布 Entity 解码出生分区/本地槽修复稀疏反向下标，compact 后恢复紧凑前缀。
    /// 非 rc 删除仅失效/登记无锁待整理链表，compact 才用尾 Entity 补洞并修复下标。
    /// rc 删除立即用尾部 owner 填洞并修复下标、缩短范围，compact 为空；不搬移组件值。
    pub(crate) owners: SparseArray<Entity, P>,
    /// 仅 Added 跟踪启用时存在；活槽为 Some(ChangeTick)，空槽不得读取。
    pub(crate) added: Option<SparseArray<Option<pi_ecs_types::change_detection::ChangeTick>, P>>,
    /// 仅 Changed 跟踪启用时存在；不是 Option<Arr>，与值数组使用相同出生分区/本地槽。
    pub(crate) changed: Option<SparseArray<Option<pi_ecs_types::change_detection::ChangeTick>, P>>,
}
impl<const P: usize> SparseArraySparseSet<P> {
    /// 同时返回独占值与已启用 tick 槽；未启用对应 tick 为 None。
    pub(crate) fn get_mut_with_ticks(
        &mut self,
        entity: Entity,
    ) -> Option<(
        PtrMut<'_>,
        pi_ecs_types::change_detection::ComponentTicksMut<'_>,
    )> {
        Self::get_mut_with_ticks_inner(self, entity)
    }
}
impl<const P: usize> SparseSet for SparseArraySparseSet<P> {
    /// # 用途
    /// 返回本后端组件编号，不分配或改变状态。
    fn component(&self) -> ComponentId {
        Self::component_inner(self)
    }
    /// # 用途
    /// 按完整 entity 点查，缺失 None；借用期间不得移出或替换值。
    fn get(&self, entity: Entity) -> Option<Ptr<'_>> {
        Self::get_inner(self, entity)
    }
    /// # 用途
    /// 按完整 entity 独占点查，缺失 None；不转换类型或标记变化。
    fn get_mut(&mut self, entity: Entity) -> Option<PtrMut<'_>> {
        Self::get_mut_inner(self, entity)
    }
    /// # 用途
    /// 为 entity 准备空槽，普通容量错误在移入前返回，不安装业务值。
    fn prepare(&mut self, entity: Entity) -> Result<(), EcsError> {
        Self::prepare_inner(self, entity)
    }
    /// # 用途
    /// 把 value 移入已为 entity 准备的空槽，不覆盖活值；执行失败 panic。
    fn insert(&mut self, entity: Entity, value: OwningPtr<'_>) {
        Self::insert_inner(self, entity, value)
    }
    /// # 用途
    /// 移出 entity 的唯一拥有责任，缺失 None；不析构值，不搬移其他组件。
    fn remove(&mut self, entity: Entity) -> Option<OwningPtr<'_>> {
        Self::remove_inner(self, entity)
    }
}
/// 单个出生分区的二级擦除数组；普通删除不收缩，分区内本地槽由 RawArr 自身分段增长。
/// 值与元数据分离；紧凑反向表仅交换地址项，不 swap-remove 组件值。共享稀疏迁区不改变寻址。
#[derive(Default)]
pub(crate) struct SparseArrayDomain {
    /// None 尚未创建分区内值数组；Some 的索引直接为实体本地槽索引，无固定块容量。
    /// rc 增长可搬移旧值，增长前结束受影响借用，增长后重绑 fetch 地址。
    /// 空槽为擦除 MaybeUninit 语义；活组件经 ECS 释放，RawArr 不重复析构。
    pub(crate) values: Option<pi_arr::RawArr>,
}
// 有效槽由稀疏下标/owners.members.dense 双向一致性及完整身份匹配定义，不以分配状态判断。
// 插入追加完整 Entity；非 rc 移除只失效并无锁登记，compact 再填洞/修复，无独立反向地址表。
// 不增加结构修改锁；并发删除不交换尾项。无锁登记、追加/发布与安全截点的原子协议按以下约定：
// - 预留计数只分配下标，不产生可读成员；owner 槽先初始化为 null，稀疏下标先置 u32::MAX；
// - 填好组件值、完整 owner、启用的 tick 及双向映射后 Release 发布 owner，再 Release 发布下标；
// - 点查 Acquire 读下标，再 Acquire 读 owner 并核对完整 Entity；不匹配或未发布则缺失；
// - 删除先失效下标，再置空 owner 并登记退休下标；旧洞 compact 前不复用；
// - 多写者可乱序完成，不以最后预留下标或裸长度假定此前全部完成；
// - 可恢复准备错误在认领前返回；认领后失败不得伪发布/读取未初始化项，不复用该位置；
// - compact/捕获等待全部写者及借用收束，不能用预留长度替代收束证明；
// - rc 顺序完成安装后更新有效范围，不新增并发发布成本。
// 相同身份重复插入拒绝；不同代际残留活值是内部一致性错误，不自动覆盖/析构。
// 移除撤销活状态并交接一次 OwningPtr；责任消费前不能覆盖原槽，Removed 独立保活旧值。
// 一级页面存在不代表二级域已初始化，二级槽已分配也不代表包含活组件。
/// 默认版使用 pi_buckets::Pages；rc 版由本库提供 Vec 一级目录，不借用并发 Pages。
/// 非 rc 多线程必须使用 Pages；既有页面不搬迁，初始化冲突由调用者/ECS 排除。
#[cfg(not(feature = "rc"))]
pub(crate) struct SparseArrayDirectory<const P: usize> {
    /// 按出生分区寻址的稳定页面，不新增分区初始化锁。
    pub(crate) pages: pi_buckets::Pages<SparseArrayDomain, P>,
}
/// rc 一级由本库使用 Vec；P 只为统一类型形状，不参与索引或 Vec 分配策略。
#[cfg(feature = "rc")]
pub(crate) struct SparseArrayDirectory<const P: usize> {
    /// 按出生分区寻址；增长前结束旧条目借用，增长允许搬移。
    pub(crate) entries: Vec<SparseArrayDomain>,
}
/// 按组件选择并冻结的稀疏后端枚举；通过匹配具体变体调用 trait，不使用 dyn SparseSet。
/// papaya 保底已移除：所有稀疏组件统一走两级 SparseArray，不再保留哈希分支。
pub enum SparseSetStorage {
    /// 两级 SparseArray 后端；页长固定为 SPARSE_PAGE，组件初始化时冻结选择。
    SparseArray(SparseArraySparseSet<SPARSE_PAGE>),
}
impl SparseSetStorage {
    /// # 用途
    /// RC 结构安装前准备空值槽、全部已启用 Added/Changed 槽及 owner 空标记；不发布活值。
    /// 普通失败发生在源值仍持责时，可保留空容量，不改变旧活值及已发布 owner；禁止复用旧视图。
    #[cfg(feature = "rc")]
    pub fn prepare_tracked(&mut self, entity: Entity) -> Result<(), EcsError> {
        self.prepare_tracked_inner(entity)
    }
    /// # 用途
    /// 为完整活身份复制当前 tick；缺失 None，未启用项为 None，启用而缺槽属于内部故障。
    /// 结构搬移须在移出值前取得此快照，不保留 tick 引用。
    #[cfg(feature = "rc")]
    pub fn ticks(&self, entity: Entity) -> Option<pi_ecs_types::change_detection::ComponentTicks> {
        self.ticks_inner(entity)
    }
    /// # 用途
    /// 将 value 与 ticks 安装到 prepare_tracked 的空槽；ticks 必须精确覆盖当前启用项。
    /// 新增的启用项均为结构操作 tick，共有值搬移逐项保留源快照，不重新赋当前 tick。
    /// 移值前验证身份、类型、准备状态和 tick 形状；开始移值后无分配地写值、写 tick，最后发布 owner。
    /// 安装期异常由 core 标停并依局部进度清偿，不返回普通错误或重复析构，源分配仍由调用者管理。
    #[cfg(feature = "rc")]
    pub fn insert_tracked(
        &mut self,
        entity: Entity,
        value: OwningPtr<'_>,
        ticks: pi_ecs_types::change_detection::ComponentTicks,
    ) {
        self.insert_tracked_inner(entity, value, ticks)
    }
    /// component 与 layout 由 core 登记校验；构造空两级稀疏实例，不持有 ComponentInfo 或 World。
    /// 分配失败不安装半实例，不创建业务值；本入口即默认后备，不再有哈希选择。
    pub fn sparse_array(
        component: ComponentId,
        layout: std::alloc::Layout,
    ) -> Result<Self, EcsError> {
        Self::sparse_array_inner(component, layout)
    }
    /// 返回本存储组件编号，不改变状态。
    pub fn component(&self) -> ComponentId {
        Self::storage_component_inner(self)
    }
    /// entity 必须为完整键；缺失返回 None，借用期间不得移出/替换该值。
    pub fn get(&self, entity: Entity) -> Option<Ptr<'_>> {
        Self::storage_get_inner(self, entity)
    }
    /// entity 必须为完整键；缺失返回 None，返回独占借用，不执行类型转换。
    pub fn get_mut(&mut self, entity: Entity) -> Option<PtrMut<'_>> {
        Self::storage_get_mut_inner(self, entity)
    }
    /// 为新增 entity 预留空间，不安装值；普通容量错误在移动前返回。
    pub fn prepare(&mut self, entity: Entity) -> Result<(), EcsError> {
        Self::storage_prepare_inner(self, entity)
    }
    /// entity 为空且已准备，value 类型已校验；消费拥有权，不隐式替换现有值。
    pub fn insert(&mut self, entity: Entity, value: OwningPtr<'_>) {
        Self::storage_insert_inner(self, entity, value)
    }
    /// 移除完整 entity 并返回拥有值；缺失返回 None，不析构移出值、不移动其他实体。
    pub fn remove(&mut self, entity: Entity) -> Option<OwningPtr<'_>> {
        Self::storage_remove_inner(self, entity)
    }
    /// 把 source 的字节值移入已准备槽；layout 必须等于本存储组件布局。
    /// 不析构 source、不释放其分配；成功后 source 不再包含活值，调用方负责释放原分配。
    /// # Safety
    /// source 必须指向一个已初始化且可按字节移动的值，调用后不得再读取或析构该值。
    pub unsafe fn insert_raw(
        &mut self,
        entity: Entity,
        source: *mut u8,
        layout: std::alloc::Layout,
    ) {
        Self::storage_insert_raw_inner(self, entity, source, layout)
    }
    /// 移出完整 entity 的拥有值并返回裸地址；缺失返回 None。
    /// 成功返回后该槽不再拥有活值，调用方必须移交、读取或按登记析构能力处理。
    /// # Safety
    /// 返回地址只在调用方消费前有效；不得在消费前重新准备/覆盖同一实体槽。
    pub unsafe fn remove_raw(&mut self, entity: Entity) -> Option<*mut u8> {
        Self::storage_remove_raw_inner(self, entity)
    }
}

impl<const P: usize> RawSparseArray<P> {
    /// 创建无分区的空两级数组；不申请二级槽，不构造业务值。
    pub(crate) fn new() -> Self {
        Self::new_inner()
    }
}
impl SparseOwner {
    /// null 编码统一为 u64::MAX；不表示活值。
    pub(crate) fn empty() -> Self {
        Self::empty_inner()
    }
}
impl Default for SparseOwner {
    /// # 用途
    /// 创建本类型规范空状态，不安装业务活值或分配实体身份。
    fn default() -> Self {
        Self::empty()
    }
}
impl SparseDenseIndex {
    /// 不可用下标统一为 u32::MAX。
    pub(crate) fn empty() -> Self {
        Self::empty_inner()
    }
}
impl Default for SparseDenseIndex {
    /// # 用途
    /// 创建本类型规范空状态，不安装业务活值或分配实体身份。
    fn default() -> Self {
        Self::empty()
    }
}
impl<const P: usize> SparseOwners<P> {
    /// 创建空 owner 集合；dense/indices 为空，长度为零。
    pub(crate) fn new() -> Self {
        Self::new_inner()
    }
}
#[cfg(not(feature = "rc"))]
impl Default for PendingSparseRemovals {
    /// # 用途
    /// 创建本类型规范空状态，不安装业务活值或分配实体身份。
    fn default() -> Self {
        Self::default_inner()
    }
}
#[cfg(not(feature = "rc"))]
impl<const P: usize> SparseArrayDirectory<P> {
    /// 创建空分区目录；页面按需初始化。
    pub(crate) fn new() -> Self {
        Self::new_inner()
    }
}
#[cfg(feature = "rc")]
impl<const P: usize> SparseArrayDirectory<P> {
    /// 创建空分区目录；分区条目按需增长。
    pub(crate) fn new() -> Self {
        Self::new_inner()
    }
}

/// # 用途
/// 单组件的冻结稀疏视图；域目录与完整身份映射只读，值及 tick 经内部可变后端逐槽访问。
/// 不借整个 SparseSetStorage，不拥有值，不提供 Clone/重置；不同 fetch 可保存同源共享后端描述。
pub struct SparseView<'a> {
    /// 单源组件编号，所有域数组具有同一登记布局。
    pub(crate) component: ComponentId,
    /// 冻结值目录；只经 RawArr::get 定位，不增长或复制逐实体地址表。
    pub(crate) values: &'a RawSparseArray<SPARSE_PAGE>,
    /// 完整身份及反向下标只读证明，缺失/代际不匹配拒绝取值，不按分配槽推断存活。
    pub(crate) owners: &'a SparseOwners<SPARSE_PAGE>,
    /// Added 二级数组目录，未启用为 None；仅活值对应槽可读写。
    pub(crate) added: Option<&'a RawSparseArray<SPARSE_PAGE>>,
    /// Changed 二级数组目录；不将整个值数组或 tick 数组转换为共享切片。
    pub(crate) changed: Option<&'a RawSparseArray<SPARSE_PAGE>>,
}
impl SparseSetStorage {
    /// # 用途
    /// 首次交出本存储结果前绑定视图；不新增域、不预留值、不产生组件或 tick 引用。
    /// # Safety
    /// 调用方持有效原始授权；整个 'a 及结果期内目录、映射、追踪启用情况固定，无增长/删除/整理/释放。
    /// 只共享支持内部可变访问的后端描述和只读身份映射；不能通过其他路径重借整个值后端。
    pub unsafe fn bind_view<'a>(&'a self) -> SparseView<'a> {
        self.bind_view_inner()
    }
}
impl<'a> SparseView<'a> {
    /// # 用途
    /// entity 按出生域、本地槽定位并核对完整 owner；合法缺失返回 None，不访问未发布值。
    /// # Safety
    /// 结果期允许该槽共享读取且无冲突写入；原授权保活到 'a，不由 self 的短借用延长。
    pub unsafe fn read(&self, entity: Entity) -> Option<Ptr<'a>> {
        self.read_inner(entity)
    }
    /// # 用途
    /// 按完整 entity 返回单值及对应已启用 tick 槽，缺失 None；不设置 tick，不转移拥有责任。
    /// # Safety
    /// 本实体值及 tick 在 'a 内唯一；同物理槽不可重复兑现，代际校验不代替别名证明。
    /// 身份与目录先验证，再形成引用；已发布活值却缺值/tick 槽属于不变量损坏，不伪装 None。
    pub unsafe fn write(
        &self,
        entity: Entity,
    ) -> Option<(
        PtrMut<'a>,
        pi_ecs_types::change_detection::ComponentTicksMut<'a>,
    )> {
        self.write_inner(entity)
    }
}

impl SparseView<'_> {
    /// # 用途
    /// entity 为已验证目标，短读并复制该单槽 added/changed；未启用项为 None，不返回 tick 引用。
    /// 追踪已启用而活值槽未初始化为内部错误；稀疏完整身份缺失才返回外层 None。
    /// 不调用 load 取得独占 tick，不扫描整数组、不启停追踪或修改状态。
    /// # Safety
    /// 目标已发布且来源/类型正确，调用期间该 tick 槽无活独占引用或并发写入。
    /// 同实体写查询先复制过滤 tick 并结束短读，再调用 write；不同实体的写引用可存活。
    pub unsafe fn ticks(
        &self,
        entity: Entity,
    ) -> Option<pi_ecs_types::change_detection::ComponentTicks> {
        self.ticks_inner(entity)
    }
}
