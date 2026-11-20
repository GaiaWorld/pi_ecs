//! 系统独立创建参数：登记完整目标，取参时绑定相同叶子授权。
#[path = "spawn_inner.rs"]
mod spawn_inner;

use crate::{
    access::{AccessDeclaration, RunAccess},
    bundle::SystemBundle,
    change_detection::ChangeTick,
    component::ComponentId,
    entity::PartitionId,
    error::EcsError,
    owned_entity::OwnedEntity,
};
use std::marker::PhantomData;

/// # 用途
/// 单个 Spawn 位置的冻结声明；不缓存物理地址，也不持跟踪、订阅或实体责任。
pub struct SpawnState<B: SystemBundle<Shape = B> + 'static> {
    /// 参数登记时固定的 World 身份；不可跨 World 重用。
    pub(crate) world_id: u64,
    /// 参数配置中的合法分区；初始位置使用此分区，出生域取相同编号。
    pub(crate) partition: PartitionId,
    /// B 展平后的冻结组件编号，保持 Bundle 写入顺序，已检查重复。
    pub(crate) ordered_components: Box<[ComponentId]>,
    /// 静态 Bundle 形状；仅作类型绑定，不拥有任何 B 值。
    pub(crate) shape: PhantomData<fn() -> B>,
}

impl<B: SystemBundle<Shape = B> + 'static> SpawnState<B> {
    /// # 用途
    /// 返回冻结声明所属 World 的身份，不提供运行授权。
    pub fn world_id(&self) -> u64 {
        self.world_id
    }

    /// # 用途
    /// 生成当前 Spawn 位置的结构叶子声明；目标是完整升序组件集合，包含固定分区。
    /// () 依然产生一个空组件原型的成员写。只返回声明，不发布原型或创建实体。
    pub fn declaration(&self) -> AccessDeclaration {
        self.declaration_inner()
    }
}

/// # 用途
/// 立即创建 B 声明的完整原型；不消费实体查询，不持改结构队列，也不重新获取宿主门禁。
/// 同一位置的全部调用使用固定分区和本系统 this_run；返回同源初始 strong。
pub struct Spawn<'run, B: SystemBundle<Shape = B> + 'static> {
    /// 登记完成的同源目标和分区，借用寿命由参数位置约束。
    pub(crate) state: &'run SpawnState<B>,
    /// 本系统本位置的结构写授权；不授予任意业务值或任意 World 访问。
    pub(crate) access: RunAccess<'run>,
    /// 同次系统运行的写入和通知 tick，不再分配宿主 tick。
    pub(crate) tick: ChangeTick,
}

impl<'run, B: SystemBundle<Shape = B> + 'static> Spawn<'run, B> {
    /// # 用途
    /// values 为 B 的同形 Bundle，短借输入可直接传入；须匹配有序类型清单且来源与本 World 一致。
    /// 普通前置拒绝不发布实体；成功在返回前发布位置、成员、活 tick 和 Added/Changed 通知，
    /// 交付一份绑定本次运行的 strong。不在 finish 延迟提交，不因展开新增恢复或回滚协议。
    pub fn spawn<I: SystemBundle<Shape = B>>(
        &mut self,
        values: I,
    ) -> Result<OwnedEntity<'run>, EcsError> {
        /* 设计已确认：前置核对声明和来源，解析或准备目标 variant 与空槽；
        保留 BundleWriter 直接写入及其既有责任保护；完成后发布全部元数据与流，交付 strong。
        使用登记分区映射出生域和初始位置，以 this_run 写 tick，不切换宿主门禁。 */
        self.spawn_inner(values)
    }
}
