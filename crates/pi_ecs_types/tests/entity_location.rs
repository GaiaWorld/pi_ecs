use pi_ecs_types::entity::{EntityLocation, TableId, VariantId, DOMAIN_MASK, LOCAL_SLOT_MASK};
use pi_null::Null;

#[test]
fn typed_ids_reserve_only_the_maximum_value() {
    assert_eq!(TableId::null(), TableId(u16::MAX));
    assert_eq!(VariantId::null(), VariantId(u16::MAX));
    assert!(TableId(u16::MAX).is_null());
    assert!(VariantId(u16::MAX).is_null());
    assert!(!TableId(0).is_null());
    assert!(!VariantId(0).is_null());
    assert_eq!(std::mem::size_of::<TableId>(), 2);
    assert_eq!(std::mem::size_of::<VariantId>(), 2);
}

#[test]
fn packed_location_preserves_live_ids_and_its_canonical_null() {
    assert_eq!(std::mem::size_of::<EntityLocation>(), 12);
    assert!(EntityLocation::NULL.is_null());
    let location = EntityLocation::from_parts(
        DOMAIN_MASK,
        LOCAL_SLOT_MASK,
        u32::MAX,
        TableId(u16::MAX - 1),
        VariantId(u16::MAX - 1),
    );
    assert!(!location.is_null());
    assert_eq!(location.archetype().table, TableId(u16::MAX - 1));
    assert_eq!(location.archetype().variant, VariantId(u16::MAX - 1));
    assert_eq!(location.table_row(), u32::MAX);
    assert!(!EntityLocation::from_parts(0, 0, 0, TableId(0), VariantId(0)).is_null());
}

#[test]
fn a_null_id_cannot_be_packed_as_a_live_location() {
    assert!(std::panic::catch_unwind(|| {
        EntityLocation::from_parts(0, 0, 0, TableId::null(), VariantId(0))
    })
    .is_err());
    assert!(std::panic::catch_unwind(|| {
        EntityLocation::from_parts(0, 0, 0, TableId(0), VariantId::null())
    })
    .is_err());
}
