//! 稀疏组件后端协议和枚举分派；SparseArray 自身提供映射 compact，不绑定具体组件 T。
use crate::ptr::{OwningPtr, Ptr, PtrMut};
use pi_ecs_types::component::ComponentId;
use pi_ecs_types::entity::Entity;
use pi_ecs_types::error::EcsError;
use std::cell::UnsafeCell;
/// 单组件稀疏存储接口，调用方先校验身份并准备空间；执行失败直接 panic。
/// 同一稀疏位置的冲突操作由调用者保证不重叠；系统调度的冲突管理负责满足此前提。
/// 本后端不做逐槽冲突检测/仲裁，不增加锁、CAS 认领或安装中状态；原子映射仅用于发布可见性。
/// 此约束不取消 ECS 已有访问冲突管理，也不豁免借用有效期与同实体整体结构发布的授权要求。
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
/// 两级稀疏数组；一级按 AllocationDomain 寻址，二级 RawArr 直接按域内实体槽索引寻址。
/// 不拼接域和槽，不引入固定 B 分块；代际不参与寻址，以槽元数据的完整 Entity 校验。
/// 多线程配置一级必须使用 Pages，不以固定域数为理由改用 Vec；rc 单线程一级才使用 Vec。
/// P 仅为 Pages 的物理页长，非零二次幂，不参与组件槽索引拆分。
/// @to_design P 的具体值、身份域解码及 rc feature/初始化同步接线仍待闭合。
pub(crate) struct RawSparseArray<const P: usize> {
    /// 一级目录按来源域编号索引；域条目按需初始化，不默认构造业务组件。
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
impl SparseElement for pi_ecs_types::change_detection::ChangeTick {
    type Members<const P: usize> = ();
}
/// owner null 同时代表尚未发布/已退休；不另存发布状态数组，不据 null 判断洞可复用。
/// pi_key::KeyData::as_ffi/from_ffi 无损转换完整键，null 编码是 u64::MAX，不是零。
/// 非 rc 目标需支持原生 64 位原子；不支持时的目标策略待定，不以锁模拟无锁保证。
pub(crate) struct SparseOwner {
    /// 两种模式统一保存完整键编码，显式初始化为 u64::MAX，不用 ShareU64::default 的零。
    /// 非 rc 为 AtomicU64，rc 为非原子 AtomicCell<u64>；Release/Acquire 在 rc 不产生线程同步。
    pub(crate) key: pi_share::ShareU64,
}
// @to_design 目录编译期 rc 分派及整个依赖图 feature 贯通仍待接线，不用运行时模式开关。
/// 稀疏位置到紧凑 owner 下标的槽；合法值为 0..u32::MAX，u32::MAX 唯一表示不可用。
/// 数组访问前检查空码再转 usize；两种模式统一使用 ShareU32，不存运行时判别码。
pub(crate) struct SparseDenseIndex {
    /// 显式初始化为 u32::MAX；非 rc AtomicU32，rc 非原子 AtomicCell<u32>。
    /// Acquire 读、安装末尾 Release 写，删除先失效；rc 相同接口不产生原子成本。
    pub(crate) index: pi_share::ShareU32,
}
/// 非 rc 待整理追加列表；选用 pi_append_vec，依赖接入前保留占位，不自行重写链表。
pub(crate) struct PendingSparseRemovals {
    /// 目标为 pi_append_vec::AppendVec<usize>，每项记录退休的紧凑下标，不是 Entity。
    /// 同一活成员删除只登记一次；整理前旧洞不复用，删除后重加须追加新位置。
    /// 记录载荷已确定，依赖兼容版本及实际接入仍待闭合。
    /// registry 0.6.3 的独立编译因旧 pi_share 失败，不以本地同版本源码代替发布事实。
    /// 只在全部追加线程退出后读取/清空；len 预留不证明记录已写完。
    _pending: (),
}
/// 组件成员集合的唯一拥有者；仅 SparseArray<Entity> 持有，值和 tick 数组不复制本结构。
pub(crate) struct SparseOwners<const P: usize> {
    /// 紧凑完整 Entity/发布标记；null 为未发布或退休，反向映射单一来源。
    pub(crate) dense: pi_arr::Arr<SparseOwner>,
    /// 域/槽到紧凑下标的两级数组；非 rc 一级必须 Pages，二级 RawArr 保存原子 u32 槽。
    /// 编码遵守 SparseDenseIndex；本数组仅存映射，不再包含自己的成员集合。
    pub(crate) indices: RawSparseArray<P>,
    /// 预留上界而非有效前缀，最多 u32::MAX 项；非 rc 有界原子认领的表示待接线。
    /// 退休洞 compact 前仍占范围，不回绕、不分配空码、不隐式整理。
    pub(crate) len: pi_share::ShareUsize,
    /// 唯一待整理退休下标列表；rc 不使用，具体编译期消除待 feature 接线。
    pub(crate) pending_removals: PendingSparseRemovals,
}
impl<const P: usize> SparseOwners<P> {
    /// 在独占安全收束点整理本集合的非 rc 待删除洞，联合更新 dense/indices/len。
    /// 非 rc：全部追加线程退出后读取退休下标、跳过失效尾项，用存活尾 Entity 填洞并修复下标。
    /// 完成后才清理记录及截短范围；此前不复用任何旧洞，不搬移组件值、不承诺组件值连续化。
    /// rc：空操作，不分配、不修改映射；删除已立即用尾部 owner 填洞、修复下标并缩短范围。
    /// 全过程须结束受影响借用，保持业务 tick/通知不变；提交失败停止，不假成功。
    pub(crate) fn compact(&mut self) {
        crate::sparse_set_inner::compact_inner(self)
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
/// 单个组件的稀疏后端；四个数组均独立按 domain/slot 寻址，不将元数据嵌入值数组域条目。
pub(crate) struct SparseArraySparseSet<const P: usize> {
    /// 本实例唯一组件编号。
    pub(crate) component: ComponentId,
    /// 擦除组件值，分配槽与业务活值分开管理。
    pub(crate) values: RawSparseArray<P>,
    /// 必存的完整身份数组，同时承担紧凑反向映射；Lookup-only 也不省略，不增加注册开关。
    /// owners.members.dense 在 0..len 的已发布活项保存完整 Entity；未发布/退休位置不可读取。
    /// 由已发布 Entity 解码域/槽修复稀疏反向下标，compact 后恢复紧凑前缀。
    /// 非 rc 删除仅失效/登记无锁待整理链表，compact 才用尾 Entity 补洞并修复下标。
    /// rc 删除立即用尾部 owner 填洞并修复下标、缩短范围，compact 为空；不搬移组件值。
    pub(crate) owners: SparseArray<Entity, P>,
    /// 仅 Added 跟踪启用时存在；槽内 ChangeTick 保持非零，未初始化空槽不得读取。
    pub(crate) added: Option<SparseArray<pi_ecs_types::change_detection::ChangeTick, P>>,
    /// 仅 Changed 跟踪启用时存在；不是 Option<Arr>，与值数组使用相同 domain/slot。
    pub(crate) changed: Option<SparseArray<pi_ecs_types::change_detection::ChangeTick, P>>,
}
/// 单个来源域的二级擦除数组；普通删除不收缩，域内槽由 RawArr 自身分段增长。
/// 值与元数据分离；紧凑反向表仅交换地址项，不 swap-remove 组件值。共享稀疏迁区不改变寻址。
#[derive(Default)]
pub(crate) struct SparseArrayDomain {
    /// None 尚未创建域内值数组；Some 的索引直接为实体槽索引，无固定块容量。
    /// rc 增长可搬移旧值，增长前结束受影响借用，增长后重绑 fetch 地址。
    /// 空槽为擦除 MaybeUninit 语义；活组件经 ECS 释放，RawArr 不重复析构。
    pub(crate) values: Option<pi_arr::RawArr>,
}
// 有效槽由稀疏下标/owners.members.dense 双向一致性及完整身份匹配定义，不以分配状态判断。
// 插入追加完整 Entity；非 rc 移除只失效并无锁登记，compact 再填洞/修复，无独立反向地址表。
// 不增加结构修改锁；并发删除不交换尾项。无锁登记、追加/发布与安全截点的原子协议待闭合。
// 设计已确认的新增主体：校验完整身份/目标空位并准备容量，独占认领一个新 owners 下标。
// 预留计数只分配下标，不产生可读成员；填写组件值、完整 owner、启用的 tick 及双向映射后才发布完成。
// owner 槽在预留可见前初始化为 null；稀疏下标先初始化为不可用。
// 填好组件值/tick/映射准备后 Release 发布 owner，最后 Release 发布稀疏下标。
// 点查 Acquire 取稀疏下标，不可用则缺失；再 Acquire 读 owner 并核对完整 Entity，匹配后才可取值。
// 遍历通过 owner 发布取得安装可见性；若需核对反向映射，则稀疏下标尚未发布时跳过，不读半成品。
// 非 rc 读取 Acquire 取 owner，null 跳过，否则解码完整 Entity；不得并发读写普通 Entity。
// 删除先原子失效稀疏下标，再原子置空 owner、登记退休下标；旧洞整理前不复用。
// null 不区分预留/退休，追加计数与退休列表决定处理方式；这些原子操作不保活已返回的组件引用。
// compact 在独占收束后填洞并修复原子下标，此时不引入并发交换协议。
// 多写者可乱序完成，不以最后预留下标或一个裸长度假定此前全部完成；不增加结构修改锁。
// 可恢复准备错误应在认领前返回；认领后失败不得伪发布/读取未初始化项，不复用该位置，致命失败停止。
// compact/捕获等待全部写者及借用收束，不能用预留长度或观察到某个发布项替代收束证明。
// rc 顺序完成安装后更新有效范围，不新增并发发布成本。
// @to_design Release/Acquire 的实际发布载体、与失效/映射的线性化接线及异常清理责任待闭合。
// 相同身份重复插入拒绝；不同代际残留活值是内部一致性错误，不自动覆盖/析构。
// 移除撤销活状态并交接一次 OwningPtr；责任消费前不能覆盖原槽，Removed 独立保活旧值。
// 一级页面存在不代表二级域已初始化，二级槽已分配也不代表包含活组件。
/// 默认版使用 pi_buckets::Pages；rc 版由本库提供 Vec 一级目录，不借用并发 Pages。
/// @to_design rc feature 贯通及目录共享初始化授权待接线；不能以运行时模式切换规避线程约束。
pub(crate) enum SparseArrayDirectory<const P: usize> {
    /// 多线程必须使用：既有页面不搬迁；二级域的安装/访问仍需 ECS 同步与权限。
    Pages(pi_buckets::Pages<SparseArrayDomain, P>),
    /// 单线程一级目录，增长可搬移目录槽；任何旧槽引用均须结束后才可增长。
    RcVec(Vec<SparseArrayDomain>),
}
/// 哈希保底后端，可用于共享或分区实例；不替代已选的两级 SparseArray 方向。
pub struct HashMapSparseSet {
    /// 本实例唯一组件编号。
    pub(crate) component: ComponentId,
    /// 擦除拥有值，无 dense owners、局部索引映射或泛型值包装。
    pub(crate) values: papaya::HashMap<Entity, SparseEntry>,
}
/// 按组件选择并冻结的枚举；通过匹配具体变体调用 trait，不使用 dyn SparseSet。
pub enum SparseSetStorage {
    /// papaya 哈希保底；SparseArray 变体待两级索引/页长及 rc 配置闭合后接入，不表示已运行。
    HashMap(HashMapSparseSet),
}
impl SparseSetStorage {
    /// component 与 layout 由 core 登记校验；构造空哈希保底实例，不持有 ComponentInfo 或 World。
    /// 本入口只承接现有保底分支，不决定默认后备；分配失败不安装半实例，不创建业务值。
    pub fn hash_map(component: ComponentId, layout: std::alloc::Layout) -> Result<Self, EcsError> {
        crate::sparse_set_inner::hash_map_inner(component, layout)
    }
}
/// papaya 管理条目回收，ECS 权限管理组件读写；共享节点引用不授予可写权限。
pub(crate) struct SparseEntry {
    // @to_design 条目自己拥有分配和析构责任，不长期保存临时 OwningPtr 或伪造 static。
    // 移出须保活源分配/guard 到消费结束，节点回收与值析构责任分开。
    pub(crate) value: UnsafeCell<PendingSparseAllocation>,
}
/// 稀疏条目分配的未设计占位，不是可用的拥有指针实现。
pub(crate) struct PendingSparseAllocation {
    /// 分配/活值及析构表示待闭合，不能用临时指针代替分配拥有者。
    _pending: (),
}
// @to_design 当前 SparseSet 签名仅为宿主独占适配面，并发 fetch 须接收权限令牌及 guard。
// papaya 0.2.5 remove 返回 guard 绑定的 &V，不是 V；删除节点不代表旧读者已退出。
// get 返回值不得超出 guard，移出 OwningPtr 必须证明没有冲突读写且同键只取一次。
// reserve 是容量估计，不能保证后续完全不分配；执行期异常依约直接失败。
// @to_design guard 持有层级、并发方法签名和移出证明待闭合，不手工 unsafe Send/Sync 掩盖。
// 非 rc 的串行执行仍使用并发后端；rc 是独立单线程存储配置，一级 Vec、二级连续 RawArr。
