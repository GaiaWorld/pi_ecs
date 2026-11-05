//! 统一 ECS 使用入口；当前为可编译设计骨架，行为尚未实现。
pub use pi_ecs_app as app;
pub use pi_ecs_app::plugin;
pub use pi_ecs_core as core;
pub use pi_ecs_core::{archetype, component, table};
pub use pi_ecs_core::{entity, entity_store, error, partition, snapshot, snapshot_rules, world};
pub use pi_ecs_partition as partition_systems;
pub use pi_ecs_schedule as schedule;
pub use pi_ecs_schedule::{recovery, snapshot::GameSnapshot};
/// 常用声明；不产生额外运行状态。
pub mod prelude {
    pub use pi_ecs_app::{
        app::{App, ScheduleLabel},
        plugin::Plugin,
    };
    pub use pi_ecs_core::component::{Component, ComponentId, PlainAccess, StoragePolicy};
    pub use pi_ecs_core::{
        entity::{Entity, PartitionId},
        error::EcsError,
        owned_entity::OwnedEntity,
        world::{World, WorldPin, WorldWrite},
    };
    pub use pi_ecs_schedule::schedule::Schedule;
}
