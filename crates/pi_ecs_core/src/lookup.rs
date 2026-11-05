//! 只点查已发布实体，不读取原型成员列表；支持只读及可变组件描述。
use crate::{
    access::RunAccess,
    entity::Entity,
    error::EcsError,
    query::{NoFilter, QueryData, QueryFilter, QueryState, ReadOnlyQueryData},
};
/// 本次点查参数；Q 决定值权限，F 限定范围，不因点查豁免别名或过滤检查。
/// 普通添加/删除不搬移其他存活值；目标隐藏行不可查询，finish 前必须释放全部结果。
pub struct Lookup<'a, Q: QueryData, F: QueryFilter = NoFilter> {
    /// 已准备匹配计划；不取得成员遍历权限，不公开迭代。
    pub(crate) state: &'a mut QueryState<Q, F>,
    /// 本次组件读写授权，保持到本系统 finish；不能用共享 World 伪造。
    pub(crate) access: &'a RunAccess<'a>,
}
impl<Q: QueryData, F: QueryFilter> Lookup<'_, Q, F> {
    /// entity 完整有效且已发布，满足 Q/F 及授权范围；否则错误且不改值。
    /// 短借用 self 防止重复实体可变结果并存；返回 Mut 按既定变化规则工作。
    pub fn get_mut(&mut self, entity: Entity) -> Result<Q::Item<'_, '_>, EcsError> {
        crate::lookup_inner::get_mut_inner(self, entity)
    }
}
impl<Q: ReadOnlyQueryData, F: QueryFilter> Lookup<'_, Q, F> {
    /// entity 为已发布实体且通过全部过滤/权限；错误不修改数据，共享结果可同时存在。
    pub fn get(&self, entity: Entity) -> Result<Q::Item<'_, '_>, EcsError> {
        crate::lookup_inner::get_inner(self, entity)
    }
}
// @to_design SystemParam 配置为 ()，准备/注入及精确权限展开待接通；无公开伪构造器。
// 与隐藏行追加可兼容，不等于与源可变查询无冲突；重叠值读写仍拒绝或证明范围互斥。
