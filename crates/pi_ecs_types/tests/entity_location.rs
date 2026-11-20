use pi_ecs_types::entity::{EntityLocation, DOMAIN_MASK, LOCAL_SLOT_MASK};

#[test]
fn only_the_complete_reserved_location_is_null() {
    assert!(EntityLocation::NULL.is_null());
    for location in [
        EntityLocation::from_parts(0, 0, 0, 0, 0),
        EntityLocation::from_parts(DOMAIN_MASK, LOCAL_SLOT_MASK, 0, u16::MAX, u16::MAX),
        EntityLocation::from_parts(DOMAIN_MASK, LOCAL_SLOT_MASK, u32::MAX, 0, 0),
        EntityLocation::from_parts(0, 0, u32::MAX, u16::MAX, u16::MAX),
    ] {
        assert!(!location.is_null(), "{location:?} is a valid location");
    }
}
