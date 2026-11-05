//! Table 布局配对芯；设计期保持明确失败，不承担逻辑注册。
use crate::table::TableLayout;
use pi_ecs_types::{component::ComponentId, entity::ArchetypeId, error::EcsError};
pub(crate) fn new_inner(
    _layout: &TableLayout,
    _partition: pi_ecs_types::entity::PartitionId,
    _columns: Box<[crate::column::Column]>,
) -> Result<crate::table::Table, EcsError> {
    todo!("尚未实现空物理表构造")
}
pub(crate) fn layout_new_inner(
    _id: ArchetypeId,
    _columns: Box<[ComponentId]>,
) -> Result<TableLayout, EcsError> {
    todo!("尚未实现物理布局构造")
}
pub(crate) fn layout_id_inner(_layout: &TableLayout) -> ArchetypeId {
    todo!("尚未实现布局编号读取")
}
pub(crate) fn layout_columns_inner(_layout: &TableLayout) -> &[ComponentId] {
    todo!("尚未实现固定列顺序读取")
}
