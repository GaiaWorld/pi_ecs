//! 双模式宿主访问签名验收：rc 独占 &mut，非 rc 共享 &。
use pi_ecs_core::error::EcsError;
use pi_ecs_core::world::{World, WorldPin, WorldWrite};

#[cfg(feature = "rc")]
#[test]
fn rc_pin_and_write_require_mut_world() {
    fn assert_pin(_: for<'a> fn(&'a mut World) -> Result<WorldPin<'a>, EcsError>) {}
    fn assert_write(_: for<'a> fn(&'a mut World) -> Result<WorldWrite<'a>, EcsError>) {}
    assert_pin(World::pin);
    assert_write(World::write);
}

#[cfg(not(feature = "rc"))]
#[test]
fn shared_pin_and_write_accept_shared_world() {
    fn assert_pin(_: for<'a> fn(&'a World) -> Result<WorldPin<'a>, EcsError>) {}
    fn assert_write(_: for<'a> fn(&'a World) -> Result<WorldWrite<'a>, EcsError>) {}
    assert_pin(World::pin);
    assert_write(World::write);
}
