//! 只点查已发布实体，不读取原型成员列表；支持只读及可变组件描述。
#[path = "lookup_inner.rs"]
mod lookup_inner;
use crate::{
    access::RunAccess,
    entity::Entity,
    error::EcsError,
    query::{NoFilter, QueryData, QueryFilter, QueryState, ReadOnlyQueryData},
};
/// 本次点查参数；Q 决定值权限，F 限定范围，不因点查豁免别名或过滤检查。
/// 普通添加/删除不搬移其他存活值；目标隐藏行不可查询，finish 前必须释放全部结果。
pub struct Lookup<'a, Q: QueryData, F: QueryFilter = NoFilter> {
    /// 已准备匹配计划；RC 复用 QueryState.cache.tables：
    /// 校验完整 Entity 与当前位置后按 Table 直取统一条目，Variants 才查 variant 映射。
    /// 全表路径不访问原型成员，不跳过动态过滤/权限/类型/发布校验；不取得遍历权限。
    pub(crate) state: &'a mut QueryState<Q, F>,
    /// 本次组件读写授权，保持到本系统 finish；不能用共享 World 伪造。
    pub(crate) access: RunAccess<'a>,
    /// 系统固定窗口及写 tick，变化过滤与值包装共用，不在点查时重新分配。
    pub(crate) window: crate::change_detection::ChangeWindow,
}
impl<Q: QueryData, F: QueryFilter> Lookup<'_, Q, F> {
    /// RC strong=0 且位置非 Null 仍可读写组件；不增持强引用，归零身份不能升级或结构修改。
    /// entity 完整有效且已发布，满足 Q/F 及授权范围；否则错误且不改值。
    /// 短借用 self 防止重复实体可变结果并存；返回 Mut 按既定变化规则工作。
    pub fn get_mut(&mut self, entity: Entity) -> Result<Q::Item<'_, '_>, EcsError> {
        Self::get_mut_inner(self, entity)
    }
}
impl<Q: ReadOnlyQueryData, F: QueryFilter> Lookup<'_, Q, F> {
    /// RC 待销毁但尚未隐藏的位置仍可查询，与遍历相同，不以 strong 计数额外过滤。
    /// entity 为已发布实体且通过全部过滤/权限；错误不修改数据，共享结果可同时存在。
    pub fn get(&self, entity: Entity) -> Result<Q::Item<'_, '_>, EcsError> {
        Self::get_inner(self, entity)
    }
}
// RC 参数注入使用 RunAccess::bind_lookup，配置为 ()；prepare 刷新同源匹配并展开实际值/tick 权限。
// 绑定与权限展开芯尚未实现；本次窗口按值保存在 Lookup，不借临时 SystemRunContext。
// 与隐藏行追加可兼容，不等于与源可变查询无冲突；重叠值读写仍拒绝或证明范围互斥。
