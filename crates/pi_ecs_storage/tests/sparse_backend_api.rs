//! 稀疏后端的外部签名验收；构造已可创建空后端。
use pi_ecs_storage::sparse_set::{SparseSetStorage, SPARSE_PAGE};
use pi_ecs_types::component::ComponentId;
use pi_ecs_types::error::EcsError;
use std::alloc::Layout;

fn assert_constructor(_: fn(ComponentId, Layout) -> Result<SparseSetStorage, EcsError>) {}

#[test]
fn sparse_backend_surface_is_frozen() {
    assert_constructor(SparseSetStorage::sparse_array);
    assert!(SPARSE_PAGE.is_power_of_two());
    assert_eq!(SPARSE_PAGE, 64);
}
