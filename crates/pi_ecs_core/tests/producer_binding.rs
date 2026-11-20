#![cfg(feature = "rc")]

use pi_ecs_core::world::World;

// Seam: World::producer_binding returns a snapshot without consuming registration.
#[test]
fn producer_binding_tracks_compaction_and_keeps_world_and_type_identity() {
    let mut world = World::new().unwrap();
    let mut other = World::new().unwrap();
    world.register_multi_resource::<u32>().unwrap();
    world.register_multi_resource::<String>().unwrap();
    other.register_multi_resource::<u32>().unwrap();
    world.finish_registration().unwrap();
    other.finish_registration().unwrap();
    let (first, last, text) = world
        .schedule_scope(|world| {
            world.parameter_registration_scope(|mut scope| {
                Ok((
                    scope.register_producer(7u32)?,
                    scope.register_producer(11u32)?,
                    scope.register_producer(String::from("text"))?,
                ))
            })
        })
        .unwrap();
    let tick = world.read_change_tick();
    let before = world.producer_binding(&last).unwrap();
    assert_eq!(before.offset, 1);
    assert_eq!(before.structure_version, 2);
    assert_eq!(world.producer_binding(&text).unwrap().offset, 0);
    assert_eq!(world.producer_binding(&text).unwrap().structure_version, 1);
    assert!(other.producer_binding(&last).is_err());
    assert_eq!(world.read_change_tick(), tick);
    world
        .parameter_unregistration_scope(|mut scope| scope.unregister_producer::<u32>(first))
        .unwrap();
    world
        .schedule_scope(|world| {
            let after = world.producer_binding(&last)?;
            assert_eq!(after.offset, 0);
            assert_eq!(after.structure_version, 3);
            Ok(())
        })
        .unwrap();
    assert_eq!(world.producer_binding(&text).unwrap().structure_version, 1);
    world
        .parameter_unregistration_scope(|mut scope| {
            scope.unregister_producer::<u32>(last)?;
            scope.unregister_producer::<String>(text)
        })
        .unwrap();
    assert!(world
        .read_scope(|pin| pin.multi_resource::<u32>().unwrap().is_empty())
        .unwrap());
    world.close().unwrap();
    other.close().unwrap();
}
