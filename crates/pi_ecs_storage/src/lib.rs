//! 物理存储壳；不依赖 World、组件访问策略或逻辑注册表。
mod archetype;
pub mod column;
mod column_inner;
pub mod ptr;
pub mod settle;
pub mod sparse_set;
mod sparse_set_inner;
pub mod storage;
mod storage_inner;
pub mod table;
mod table_inner;
