//! 仅保存物理成员；逻辑原型声明和注册由 core 管理。
use pi_ecs_types::entity::ArchetypeId;
/// 全局原型中的实体成员；完整成员行与 Table 行分别维护。
pub(crate) struct ArchetypeMember {
    /// 完整身份，核对代际并修复位置。
    pub(crate) entity: pi_ecs_types::entity::Entity,
    /// 实体在原型所绑定全局 Table 内的物理行。
    pub(crate) table_row: crate::table::TableRow,
}
/// 全局原型成员存储，成员号对应 EntityLocation::archetype_row。
pub(crate) struct ArchetypeStorage {
    /// 共享的原型声明编号。
    pub(crate) id: ArchetypeId,
    /// 0..len 中 Some 为活成员，None 为洞；不假设 Table 行等于原型行。
    pub(crate) members: pi_arr::Arr<Option<ArchetypeMember>>,
    /// 已使用的原型行范围，含待整理洞，不能取数组容量代替。
    pub(crate) len: usize,
    /// 已移除成员行；阶段末填洞并修正被移动 Entity 的 archetype_row。
    pub(crate) pending_removals: Vec<pi_ecs_types::entity::ArchetypeRow>,
}

impl ArchetypeStorage {
    /// 创建空全局原型成员列表；variant/id 由调用方绑定。
    pub(crate) fn new(id: ArchetypeId) -> Self {
        Self {
            id,
            members: pi_arr::Arr::new(),
            len: 0,
            pending_removals: Vec::new(),
        }
    }
}
