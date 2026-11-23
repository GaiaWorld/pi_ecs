//! 系统运行时：依赖数据内核，不依赖应用与分区扩展。
mod empty_param;
pub mod executor;
pub mod function_adapter;
#[cfg(feature = "rc")]
pub mod inter_frame;
pub mod local;
pub mod notification_reader;
pub mod param_composition;
pub mod param_state;
pub mod recovery;
pub mod resource_param;
pub mod schedule;
pub mod snapshot;
pub mod spawn;
pub mod stage;
pub mod stage_plan;
pub mod system;
pub mod system_builder;
pub mod system_param;
mod system_param_impls;
// RC SystemParam/函数适配/Local/命令与串行执行按各壳声明；通用消息、Observer 和运行期系统控制延期。
// 查询执行归 core，参数注入和系统观察窗口归本包；跨包借用和 trait 实现需要实际编译验证。
