//! 系统运行时：依赖数据内核，不依赖应用与分区扩展。
pub mod executor;
mod executor_inner;
pub mod local;
mod local_inner;
pub mod notification_reader;
mod notification_reader_inner;
pub mod param_composition;
mod param_composition_inner;
pub mod recovery;
mod recovery_inner;
pub mod resource_param;
mod resource_param_inner;
pub mod schedule;
mod schedule_inner;
pub mod snapshot;
pub mod stage;
mod stage_inner;
pub mod system;
pub mod system_builder;
mod system_builder_inner;
mod system_inner;
pub mod system_param;
// @to_design System/SystemParam、函数适配、Local、Commands、消息适配、Observer 与执行器契约待逐项设计。
// 查询执行归 core，参数注入和系统观察窗口归本包；跨包借用和 trait 实现需要实际编译验证。
