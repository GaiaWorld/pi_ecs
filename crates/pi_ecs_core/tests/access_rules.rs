//! 已确认 access.rs 公共接缝：潜在通知流推导与静态/展开权限冲突。
use pi_ecs_core::{
    access::*, component::ComponentId, entity::ArchetypeId, notification::NoticeKind,
};

fn declaration() -> AccessDeclaration {
    AccessDeclaration {
        resources: Box::new([]),
        multi_resources: Box::new([]),
        components: Box::new([]),
        tick_reads: Box::new([]),
        reads_entities: false,
        scope: EntityScope {
            with: Box::new([]),
            without: Box::new([]),
        },
        structure: StructuralAccess::None,
        observations: Box::new([]),
    }
}

fn resolved() -> ResolvedAccess {
    ResolvedAccess {
        resources: Box::new([]),
        multi_resources: Box::new([]),
        observations: Box::new([]),
        values: Box::new([]),
        tick_reads: Box::new([]),
        entities: Box::new([]),
        structure: None,
    }
}

fn value(component: u32, mode: AccessMode, variants: &[u16]) -> ResolvedValueAccess {
    ResolvedValueAccess {
        component: ComponentId(component),
        mode,
        archetypes: variants
            .iter()
            .map(|&variant| ArchetypeId { table: 0, variant })
            .collect(),
    }
}

#[test]
fn readers_coexist_and_overlapping_value_aliases_are_rejected() {
    let mut left = declaration();
    let mut right = declaration();
    left.components = Box::new([(ComponentId(7), AccessMode::Read)]);
    right.components = Box::new([(ComponentId(7), AccessMode::Read)]);
    left.tick_reads = Box::new([ComponentId(7)]);
    right.tick_reads = Box::new([ComponentId(7)]);
    left.observations = Box::new([(ComponentId(7), NoticeKind::Removed)]);
    right.observations = Box::new([(ComponentId(7), NoticeKind::Removed)]);
    assert!(!left.conflicts(&right));
    assert!(!right.conflicts(&left));
    left.observations = Box::new([]);
    right.observations = Box::new([]);
    left.components = Box::new([(ComponentId(7), AccessMode::Write)]);
    assert!(left.conflicts(&right));
    assert!(right.conflicts(&left));
    let mut a = resolved();
    let mut b = resolved();
    a.values = Box::new([value(7, AccessMode::Read, &[0])]);
    b.values = Box::new([value(7, AccessMode::Read, &[0])]);
    a.tick_reads = Box::new([value(7, AccessMode::Read, &[0])]);
    b.tick_reads = Box::new([value(7, AccessMode::Read, &[0])]);
    a.observations = Box::new([(ComponentId(7), NoticeKind::Removed)]);
    b.observations = Box::new([(ComponentId(7), NoticeKind::Removed)]);
    assert!(!a.conflicts(&b));
    assert!(!b.conflicts(&a));
    a.observations = Box::new([]);
    b.observations = Box::new([]);
    a.values = Box::new([value(7, AccessMode::Write, &[0])]);
    assert!(a.conflicts(&b));
    assert!(b.conflicts(&a));
    b.values = Box::new([value(8, AccessMode::Write, &[0])]);
    b.tick_reads = Box::new([]);
    assert!(!a.conflicts(&b));
    assert!(!b.conflicts(&a));
}

#[test]
fn resource_and_producer_slot_conflicts_are_independent_of_entity_scopes() {
    use pi_ecs_core::resource::ResourceId;
    for (left_mode, right_mode, expected) in [
        (AccessMode::Read, AccessMode::Read, false),
        (AccessMode::Read, AccessMode::Write, true),
        (AccessMode::Write, AccessMode::Read, true),
        (AccessMode::Write, AccessMode::Write, true),
    ] {
        let mut left = declaration();
        let mut right = declaration();
        left.scope.with = Box::new([ComponentId(9)]);
        right.scope.without = Box::new([ComponentId(9)]);
        left.resources = Box::new([(ResourceId(0), left_mode)]);
        right.resources = Box::new([(ResourceId(0), right_mode)]);
        assert_eq!(left.conflicts(&right), expected);
        let mut a = resolved();
        let mut b = resolved();
        a.resources = left.resources;
        b.resources = right.resources;
        assert_eq!(a.conflicts(&b), expected);
    }
    for (left_mode, right_mode, expected) in [
        (
            MultiResourceAccess::ReadAll,
            MultiResourceAccess::ReadAll,
            false,
        ),
        (
            MultiResourceAccess::ReadAll,
            MultiResourceAccess::WriteSlot { producer: 1 },
            true,
        ),
        (
            MultiResourceAccess::WriteSlot { producer: 1 },
            MultiResourceAccess::ReadAll,
            true,
        ),
        (
            MultiResourceAccess::WriteSlot { producer: 1 },
            MultiResourceAccess::WriteSlot { producer: 1 },
            true,
        ),
        (
            MultiResourceAccess::WriteSlot { producer: 1 },
            MultiResourceAccess::WriteSlot { producer: 2 },
            false,
        ),
    ] {
        let mut left = declaration();
        let mut right = declaration();
        left.multi_resources = Box::new([(ResourceId(0), left_mode)]);
        right.multi_resources = Box::new([(ResourceId(0), right_mode)]);
        assert_eq!(left.conflicts(&right), expected);
        assert_eq!(right.conflicts(&left), expected);
        let mut a = resolved();
        let mut b = resolved();
        a.multi_resources = left.multi_resources;
        b.multi_resources = right.multi_resources;
        assert_eq!(a.conflicts(&b), expected);
        assert_eq!(b.conflicts(&a), expected);
    }
}

#[test]
fn static_conflicts_respect_scopes_but_keep_global_notice_and_alter_constraints() {
    let mut writer = declaration();
    let mut reader = declaration();
    writer.components = Box::new([(ComponentId(1), AccessMode::Write)]);
    reader.tick_reads = Box::new([ComponentId(1)]);
    assert!(writer.conflicts(&reader));
    assert!(reader.conflicts(&writer));
    writer.scope.with = Box::new([ComponentId(2)]);
    reader.scope.without = Box::new([ComponentId(2)]);
    assert!(!writer.conflicts(&reader));
    reader.components = Box::new([(ComponentId(1), AccessMode::Write)]);
    assert!(
        !writer.conflicts(&reader),
        "same-system short stream appends may coexist for disjoint values"
    );
    reader.observations = Box::new([(ComponentId(1), NoticeKind::Changed)]);
    assert!(
        writer.conflicts(&reader),
        "streams are global despite disjoint scopes"
    );
    reader.observations = Box::new([]);
    writer.structure = StructuralAccess::Alter {
        added: Box::new([ComponentId(3)]),
        removed: Box::new([]),
    };
    assert!(writer.conflicts(&reader));
    assert!(reader.conflicts(&writer));
    let resource_only = declaration();
    assert!(!writer.conflicts(&resource_only));
}

#[test]
fn resolved_conflicts_cover_ticks_global_streams_and_structure() {
    let mut writer = resolved();
    let mut reader = resolved();
    writer.values = Box::new([value(1, AccessMode::Write, &[0])]);
    reader.tick_reads = Box::new([value(1, AccessMode::Read, &[0])]);
    assert!(writer.conflicts(&reader));
    assert!(reader.conflicts(&writer));
    reader.tick_reads = Box::new([value(1, AccessMode::Read, &[1])]);
    assert!(!writer.conflicts(&reader));
    reader.values = Box::new([value(1, AccessMode::Write, &[1])]);
    assert!(
        writer.conflicts(&reader),
        "potential Changed writes share one stream despite disjoint values"
    );
    reader.values = Box::new([]);
    reader.observations = Box::new([(ComponentId(1), NoticeKind::Added)]);
    assert!(
        writer.conflicts(&reader),
        "observation conflicts with the same live component writer"
    );
    reader.observations = Box::new([(ComponentId(2), NoticeKind::Removed)]);
    assert!(!writer.conflicts(&reader));
    reader.structure = Some(ResolvedStructureAccess {
        sources: Box::new([]),
        targets: Box::new([]),
        added: Box::new([]),
        removed: Box::new([]),
    });
    assert!(writer.conflicts(&reader));
    assert!(reader.conflicts(&writer));
}

#[test]
fn potential_notice_access_is_preserved_without_matches_or_subscriptions() {
    let mut access = declaration();
    access.components = Box::new([
        (ComponentId(1), AccessMode::Write),
        (ComponentId(2), AccessMode::Read),
    ]);
    access.tick_reads = Box::new([ComponentId(3)]);
    access.observations = Box::new([(ComponentId(4), NoticeKind::Removed)]);
    access.structure = StructuralAccess::Alter {
        added: Box::new([ComponentId(5)]),
        removed: Box::new([ComponentId(6)]),
    };
    let mut notices = Vec::new();
    access.visit_notice_access(|n| notices.push((n.component, n.kind, n.mode)));
    assert_eq!(notices.len(), 5);
    for expected in [
        (ComponentId(1), NoticeKind::Changed, AccessMode::Write),
        (ComponentId(4), NoticeKind::Removed, AccessMode::Read),
        (ComponentId(5), NoticeKind::Added, AccessMode::Write),
        (ComponentId(5), NoticeKind::Changed, AccessMode::Write),
        (ComponentId(6), NoticeKind::Removed, AccessMode::Write),
    ] {
        assert!(notices.contains(&expected));
    }
    let mut expanded = resolved();
    expanded.values = Box::new([ResolvedValueAccess {
        component: ComponentId(1),
        mode: AccessMode::Write,
        archetypes: Box::new([]),
    }]);
    expanded.tick_reads = Box::new([ResolvedValueAccess {
        component: ComponentId(3),
        mode: AccessMode::Read,
        archetypes: Box::new([]),
    }]);
    let mut notices = Vec::new();
    expanded.visit_notice_access(|n| notices.push((n.component, n.kind, n.mode)));
    assert_eq!(
        notices,
        vec![(ComponentId(1), NoticeKind::Changed, AccessMode::Write)]
    );
}
