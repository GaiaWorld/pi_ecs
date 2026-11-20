//! 身份出生分区位段的运行期验收；不调用未实现的存储。
use pi_ecs_core::entity::{
    domain_bits, slot_index, with_domain_bits, Entity, Key, KeyData, DOMAIN_MASK,
};

#[test]
fn domain_bits_round_trip_preserves_slot_and_version() {
    let base = Entity::from(KeyData::new(0x0012_3456, 0x00A0));
    assert_eq!(domain_bits(&base), 1);
    assert_eq!(slot_index(&base), 0x2_3456);

    let tagged = with_domain_bits(base, 5);
    assert_eq!(domain_bits(&tagged), 5);
    assert_eq!(slot_index(&tagged), 0x2_3456);
    assert_eq!(tagged.data().version(), 0x00A0);

    let retagged = with_domain_bits(tagged, 9);
    assert_eq!(domain_bits(&retagged), 9);
    assert_eq!(slot_index(&retagged), 0x2_3456);
}

#[test]
#[should_panic(expected = "出生分区超出")]
fn domain_bits_reject_out_of_range() {
    let base = Entity::from(KeyData::new(1, 0));
    let _ = with_domain_bits(base, DOMAIN_MASK + 1);
}
