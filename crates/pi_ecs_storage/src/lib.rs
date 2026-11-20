//! 物理存储壳；不依赖 World、组件访问策略或逻辑注册表。
mod archetype;
pub mod column;
pub mod ptr;
pub mod raw_vec_deque;
pub mod settle;
pub mod sparse_set;
pub mod storage;
pub mod table;
