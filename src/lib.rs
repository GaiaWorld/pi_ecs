//! 统一 ECS 使用入口。
//!
//! 系统结构参数使用静态形状；运行输入可以携带调用方的短借用。
//!
//! 以下例子只检查输入形状、短借用和参数配置，不执行系统。
//! ```no_run
//! use pi_ecs::{core::bundle_input::StrongArrayInput, prelude::*, schedule::system_builder::SystemBuilder};
//! #[derive(Bundle)]
//! struct Related<'a, const N: usize>(#[bundle(nested)] StrongArrayInput<'a, N, ()>);
//! type Shape = Related<'static, 1>;
//! fn short_spawn<'v>(mut create: Spawn<'_, Shape>, input: Related<'v, 1>) {
//!     let _ = create.spawn(input).unwrap();
//! }
//! fn short_alter<'v>(mut edit: QueryAlter<'_, Entity, (), Shape>, id: Entity, input: Related<'v, 1>) {
//!     edit.alter(id, input).unwrap();
//! }
//! fn configured(_create: Spawn<'_, Shape>) {}
//! let _ = SystemBuilder::new(configured);
//! let _ = SystemBuilder::with_config(configured, (SpawnConfig { partition: PartitionId(1) },));
//! ```
//!
//! 两个独立派生的静态 Bundle 各自生成 Shape；即使组件清单相同，下面的调用也不满足 Shape 约束。
//! ```compile_fail
//! use pi_ecs::prelude::*;
//! #[derive(Bundle)] struct Declared;
//! #[derive(Bundle)] struct Other;
//! fn wrong(mut spawn: Spawn<'_, Declared>) { let _ = spawn.spawn(Other); }
//! ```
//!
//! Use a named query description to give its results field names:
//! ```no_run
//! use pi_ecs::prelude::*;
//! struct Position(i32);
//! impl Component for Position { type Access = PlainAccess; }
//! #[derive(QueryData)]
//! struct Motion { entity: Entity, position: &'static Position }
//! fn print_motion(query: Query<'_, Motion>) {
//!     for item in query.iter().unwrap() {
//!         println!("{:?}: {}", item.entity, item.position.0);
//!     }
//! }
//! ```
//!
//! A query with a writable field does not implement the read-only protocol:
//! ```compile_fail
//! use pi_ecs::{core::query::ReadOnlyQueryData, prelude::*};
//! struct Position;
//! impl Component for Position { type Access = PlainAccess; }
//! type WritePosition = &'static mut Position;
//! #[derive(QueryData)]
//! struct Motion { position: WritePosition }
//! fn require_read_only<Q: ReadOnlyQueryData>() {}
//! require_read_only::<Motion>();
//! ```
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
        alter::QueryAlter,
        bundle::{Bundle, ComponentValue, SystemBundle},
        entity::{Entity, PartitionId},
        entity_scope::EntityScope,
        error::EcsError,
        owned_entity::OwnedEntity,
        query::{Query, QueryData, QueryMut},
        world::{World, WorldPin, WorldWrite},
    };
    pub use pi_ecs_macros::{Bundle, QueryData};
    pub use pi_ecs_schedule::{
        schedule::Schedule,
        spawn::{Spawn, SpawnConfig},
    };
}
