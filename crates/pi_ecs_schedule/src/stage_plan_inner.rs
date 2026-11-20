//! 阶段计划配对芯；冲突图与排序复用 pi_graph，不自行实现邻接或 Kahn。
impl StagePlan {
    pub(super) fn relation_inner(
        logical: &pi_ecs_core::access::OrderingArchetypes,
        _earlier: &[pi_ecs_core::access::OrderingAccess],
        _later: &[pi_ecs_core::access::OrderingAccess],
    ) -> super::OrderingRelation {
        use pi_ecs_core::access::{
            AccessMode, MultiResourceAccess, OrderingAccess, StructuralAccess,
        };
        fn overlaps(left: &[usize], right: &[usize]) -> bool {
            left.iter().any(|id| right.contains(id))
        }
        fn writes_structure(access: &OrderingAccess) -> bool {
            !matches!(access.declaration.structure, StructuralAccess::None)
        }
        // 查询范围还包含可选组件缺席的原型；读取成员不等于访问该组件。
        let shares_component =
            |left: &OrderingAccess,
             right: &OrderingAccess,
             id: &pi_ecs_core::component::ComponentId| {
                left.sources.iter().any(|&index| {
                    right.sources.contains(&index)
                        && logical.components[index].binary_search(id).is_ok()
                })
            };
        // 方向只来自业务写入；物理搬移和写写仅要求排斥。
        let precedes = |writer: &OrderingAccess, reader: &OrderingAccess| {
            let w = &writer.declaration;
            let r = &reader.declaration;
            if w.resources.iter().any(|(id, mode)| {
                *mode == AccessMode::Write
                    && r.resources
                        .iter()
                        .any(|(other, mode)| id == other && *mode == AccessMode::Read)
            }) {
                return true;
            }
            if w.multi_resources.iter().any(|(id, mode)| {
                matches!(mode, MultiResourceAccess::WriteSlot { .. })
                    && r.multi_resources.iter().any(|(other, mode)| {
                        id == other && matches!(mode, MultiResourceAccess::ReadAll)
                    })
            }) {
                return true;
            }
            if w.components.iter().any(|(id, mode)| {
                *mode == AccessMode::Write
                    && (r
                        .components
                        .iter()
                        .any(|(other, mode)| id == other && *mode == AccessMode::Read)
                        || r.tick_reads.contains(id))
                    && shares_component(writer, reader, id)
            }) {
                return true;
            }
            let mut notice = false;
            w.visit_notice_access(|a| {
                r.visit_notice_access(|b| {
                    notice |= a.component == b.component
                        && a.kind == b.kind
                        && a.mode == AccessMode::Write
                        && b.mode == AccessMode::Read;
                })
            });
            if notice {
                return true;
            }
            if matches!(w.structure, StructuralAccess::Spawn { .. }) {
                return StagePlan::spawn_target_precedes_inner(writer, reader);
            }
            if let StructuralAccess::Alter { added, removed } = &w.structure {
                let affected = |range: &[usize]| {
                    overlaps(&writer.sources, range) || overlaps(&writer.targets, range)
                };
                if affected(&reader.entities) {
                    return true;
                }
                if affected(&reader.sources)
                    && (r
                        .scope
                        .with
                        .iter()
                        .chain(r.scope.without.iter())
                        .any(|id| added.contains(id) || removed.contains(id))
                        || r.components.iter().any(|(id, mode)| {
                            *mode == AccessMode::Read
                                && (added.contains(id) || removed.contains(id))
                        })
                        || r.tick_reads
                            .iter()
                            .any(|id| added.contains(id) || removed.contains(id)))
                {
                    return true;
                }
            }
            false
        };
        let excludes = |left: &OrderingAccess, right: &OrderingAccess| {
            let a = &left.declaration;
            let b = &right.declaration;
            if writes_structure(left) || writes_structure(right) {
                return true;
            }
            if a.resources.iter().any(|(id, mode)| {
                b.resources.iter().any(|(other, other_mode)| {
                    id == other && (*mode == AccessMode::Write || *other_mode == AccessMode::Write)
                })
            }) {
                return true;
            }
            if a.multi_resources.iter().any(|(id, mode)| {
                b.multi_resources.iter().any(|(other, other_mode)| {
                    id == other
                        && match (mode, other_mode) {
                            (MultiResourceAccess::ReadAll, MultiResourceAccess::ReadAll) => false,
                            (
                                MultiResourceAccess::WriteSlot { producer: x },
                                MultiResourceAccess::WriteSlot { producer: y },
                            ) => x == y,
                            _ => true,
                        }
                })
            }) {
                return true;
            }
            if a.components.iter().any(|(id, mode)| {
                (b.components.iter().any(|(other, other_mode)| {
                    id == other && (*mode == AccessMode::Write || *other_mode == AccessMode::Write)
                }) || (*mode == AccessMode::Write && b.tick_reads.contains(id)))
                    && shares_component(left, right, id)
            }) || b.components.iter().any(|(id, mode)| {
                *mode == AccessMode::Write
                    && a.tick_reads.contains(id)
                    && shares_component(left, right, id)
            }) {
                return true;
            }
            if a.observations.iter().any(|(id, _)| {
                b.components
                    .iter()
                    .any(|(other, mode)| id == other && *mode == AccessMode::Write)
            }) || b.observations.iter().any(|(id, _)| {
                a.components
                    .iter()
                    .any(|(other, mode)| id == other && *mode == AccessMode::Write)
            }) {
                return true;
            }
            let mut conflict = false;
            a.visit_notice_access(|x| {
                b.visit_notice_access(|y| {
                    conflict |= x.component == y.component
                        && x.kind == y.kind
                        && (x.mode == AccessMode::Write || y.mode == AccessMode::Write);
                })
            });
            conflict
        };
        let both_structural =
            _earlier.iter().any(writes_structure) && _later.iter().any(writes_structure);
        if both_structural {
            return super::OrderingRelation::EarlierFirst;
        }
        let forward = _earlier
            .iter()
            .any(|a| _later.iter().any(|b| precedes(a, b)));
        let backward = _later
            .iter()
            .any(|b| _earlier.iter().any(|a| precedes(b, a)));
        if forward {
            super::OrderingRelation::EarlierFirst
        } else if backward {
            super::OrderingRelation::LaterFirst
        } else if _earlier.iter().any(writes_structure)
            || _later.iter().any(writes_structure)
            || _earlier
                .iter()
                .any(|a| _later.iter().any(|b| excludes(a, b)))
        {
            super::OrderingRelation::EarlierFirst
        } else {
            super::OrderingRelation::Independent
        }
    }
}
#[cfg(test)]
mod relation_tests {
    use super::super::{OrderingRelation, StagePlan};
    use pi_ecs_core::{
        access::*, component::ComponentId, notification::NoticeKind, resource::ResourceId,
    };

    #[test]
    fn resolved_plan_orders_tick_writes_only_in_overlapping_ranges() {
        use pi_ecs_core::entity::ArchetypeId;
        let make = || ResolvedAccess {
            resources: Box::new([]),
            multi_resources: Box::new([]),
            observations: Box::new([]),
            values: Box::new([]),
            tick_reads: Box::new([]),
            entities: Box::new([]),
            structure: None,
        };
        let a = ArchetypeId {
            table: 0,
            variant: 0,
        };
        let b = ArchetypeId {
            table: 0,
            variant: 1,
        };
        let mut reader = make();
        reader.tick_reads = vec![ResolvedValueAccess {
            component: ComponentId(3),
            mode: AccessMode::Read,
            archetypes: Box::new([a]),
        }]
        .into_boxed_slice();
        let mut other = make();
        other.tick_reads = vec![ResolvedValueAccess {
            component: ComponentId(3),
            mode: AccessMode::Read,
            archetypes: Box::new([b]),
        }]
        .into_boxed_slice();
        let mut writer = make();
        writer.values = vec![ResolvedValueAccess {
            component: ComponentId(3),
            mode: AccessMode::Write,
            archetypes: Box::new([a]),
        }]
        .into_boxed_slice();
        let plan = StagePlan::build(&[vec![reader], vec![other], vec![writer]]);
        assert_eq!(plan.waves.len(), 2);
        assert_eq!(&*plan.waves[0].systems, &[1, 2]);
        assert_eq!(&*plan.waves[1].systems, &[0]);
    }

    #[test]
    fn logical_alter_closure_revisits_earlier_rule_without_publishing_variants() {
        use pi_ecs_core::{
            component::{Component, PlainAccess, StoragePolicy},
            world::World,
        };
        struct A;
        impl Component for A {
            type Access = PlainAccess;
        }
        struct B;
        impl Component for B {
            type Access = PlainAccess;
        }
        let mut world = World::new().unwrap();
        let a = world
            .register_component::<A>(StoragePolicy::SparseOnly)
            .unwrap();
        let b = world
            .register_component::<B>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let add = |required: Box<[ComponentId]>, component| {
            let mut declaration = access().declaration;
            declaration.scope.with = required;
            declaration.structure = StructuralAccess::Alter {
                added: Box::new([component]),
                removed: Box::new([]),
            };
            AccessTree::Leaf(declaration)
        };
        let add_b = add(Box::new([a]), b);
        let add_a = add(Box::new([]), a);
        #[cfg(feature = "rc")]
        let logical = StagePlan::ordering_archetypes(&mut world, &[&add_b, &add_a]).unwrap();
        #[cfg(not(feature = "rc"))]
        let logical = StagePlan::ordering_archetypes(&world, &[&add_b, &add_a]).unwrap();
        assert_eq!(
            logical
                .components
                .iter()
                .map(|ids| ids.to_vec())
                .collect::<Vec<_>>(),
            [vec![], vec![a], vec![a, b]]
        );
        assert_eq!(world.archetypes().unwrap().len(), 1);
    }

    fn logical() -> OrderingArchetypes {
        OrderingArchetypes {
            components: Box::new([
                Box::new([ComponentId(1)]),
                Box::new([ComponentId(1), ComponentId(2)]),
                Box::new([ComponentId(1), ComponentId(3)]),
            ]),
        }
    }

    fn access() -> OrderingAccess {
        OrderingAccess {
            declaration: AccessDeclaration {
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
            },
            sources: vec![0].into_boxed_slice(),
            entities: Box::new([]),
            targets: Box::new([]),
        }
    }
    // Seam: 阶段逻辑排序输出永久槽号，孤立系统保留，写者先于读者。
    #[test]
    fn ordering_plan_maps_slots_and_preserves_independent_systems() {
        let mut reader = access();
        reader.declaration.resources = vec![(ResourceId(0), AccessMode::Read)].into_boxed_slice();
        let mut writer = access();
        writer.declaration.resources = vec![(ResourceId(0), AccessMode::Write)].into_boxed_slice();
        let plan = StagePlan::build_ordering(
            &logical(),
            &[vec![reader], vec![writer], vec![]],
            &[2, 5, 9],
        );
        let waves: Vec<Vec<usize>> = plan
            .waves
            .iter()
            .map(|wave| wave.systems.to_vec())
            .collect();
        assert_eq!(waves, vec![vec![5, 9], vec![2]]);
        assert!(StagePlan::build_ordering(&logical(), &[], &[])
            .waves
            .is_empty());
    }

    // 三条不同资源依赖构成全局环；必须整阶段回退，包括无参活动槽。
    #[test]
    fn ordering_plan_cycle_falls_back_to_registration_order() {
        let specs = [
            vec![
                (ResourceId(0), AccessMode::Write),
                (ResourceId(2), AccessMode::Read),
            ],
            vec![
                (ResourceId(0), AccessMode::Read),
                (ResourceId(1), AccessMode::Write),
            ],
            vec![
                (ResourceId(1), AccessMode::Read),
                (ResourceId(2), AccessMode::Write),
            ],
        ];
        let mut accesses: Vec<_> = specs
            .into_iter()
            .map(|resources| {
                let mut item = access();
                item.declaration.resources = resources.into_boxed_slice();
                vec![item]
            })
            .collect();
        accesses.push(vec![]);
        let plan = StagePlan::build_ordering(&logical(), &accesses, &[1, 4, 8, 10]);
        let waves: Vec<Vec<usize>> = plan
            .waves
            .iter()
            .map(|wave| wave.systems.to_vec())
            .collect();
        assert_eq!(waves, vec![vec![1], vec![4], vec![8], vec![10]]);
    }

    #[test]
    fn ordering_plan_uses_longest_dependency_path() {
        let mut first = access();
        first.declaration.resources = vec![(ResourceId(0), AccessMode::Write)].into_boxed_slice();
        let mut second = access();
        second.declaration.resources = vec![
            (ResourceId(0), AccessMode::Read),
            (ResourceId(1), AccessMode::Write),
        ]
        .into_boxed_slice();
        let mut third = access();
        third.declaration.resources = vec![
            (ResourceId(0), AccessMode::Read),
            (ResourceId(1), AccessMode::Read),
        ]
        .into_boxed_slice();
        let plan = StagePlan::build_ordering(
            &logical(),
            &[vec![first], vec![second], vec![third]],
            &[0, 3, 7],
        );
        let waves: Vec<Vec<usize>> = plan
            .waves
            .iter()
            .map(|wave| wave.systems.to_vec())
            .collect();
        assert_eq!(waves, vec![vec![0], vec![3], vec![7]]);
    }

    #[test]
    fn ordering_relation_disjoint_ticks_slots_and_notice_streams() {
        let mut reader = access();
        let mut writer = access();
        reader.sources = vec![1].into_boxed_slice();
        reader.declaration.tick_reads = vec![ComponentId(1)].into_boxed_slice();
        writer.declaration.components =
            vec![(ComponentId(1), AccessMode::Write)].into_boxed_slice();
        assert!(matches!(
            StagePlan::relation(&logical(), &[reader], &[writer]),
            OrderingRelation::Independent
        ));
        let mut first = access();
        let mut second = access();
        first.declaration.multi_resources = vec![(
            ResourceId(0),
            MultiResourceAccess::WriteSlot { producer: 7 },
        )]
        .into_boxed_slice();
        second.declaration.multi_resources = vec![(
            ResourceId(0),
            MultiResourceAccess::WriteSlot { producer: 9 },
        )]
        .into_boxed_slice();
        assert!(matches!(
            StagePlan::relation(&logical(), &[first], &[second]),
            OrderingRelation::Independent
        ));
        let mut first = access();
        let mut second = access();
        first.declaration.components = vec![(ComponentId(1), AccessMode::Write)].into_boxed_slice();
        second.sources = vec![1].into_boxed_slice();
        second.declaration.components =
            vec![(ComponentId(1), AccessMode::Write)].into_boxed_slice();
        assert!(matches!(
            StagePlan::relation(&logical(), &[first], &[second]),
            OrderingRelation::EarlierFirst
        ));
        let mut reader = access();
        let mut writer = access();
        reader.declaration.observations =
            vec![(ComponentId(1), NoticeKind::Removed)].into_boxed_slice();
        writer.declaration.structure = StructuralAccess::Alter {
            added: Box::new([]),
            removed: vec![ComponentId(1)].into_boxed_slice(),
        };
        assert!(matches!(
            StagePlan::relation(&logical(), &[reader], &[writer]),
            OrderingRelation::LaterFirst
        ));
    }

    #[test]
    fn ordering_relation_bidirectional_and_alter_physical_exclusion() {
        let mut first = access();
        let mut second = access();
        first.declaration.resources = vec![
            (ResourceId(0), AccessMode::Read),
            (ResourceId(1), AccessMode::Write),
        ]
        .into_boxed_slice();
        second.declaration.resources = vec![
            (ResourceId(0), AccessMode::Write),
            (ResourceId(1), AccessMode::Read),
        ]
        .into_boxed_slice();
        assert!(matches!(
            StagePlan::relation(&logical(), &[first], &[second]),
            OrderingRelation::EarlierFirst
        ));
        let mut reader = access();
        let mut alter = access();
        reader.entities = vec![2].into_boxed_slice();
        alter.targets = vec![2].into_boxed_slice();
        alter.declaration.structure = StructuralAccess::Alter {
            added: vec![ComponentId(1)].into_boxed_slice(),
            removed: Box::new([]),
        };
        assert!(matches!(
            StagePlan::relation(&logical(), &[reader], &[alter]),
            OrderingRelation::LaterFirst
        ));
        let mut first = access();
        let mut second = access();
        first.declaration.structure = StructuralAccess::Alter {
            added: Box::new([]),
            removed: Box::new([]),
        };
        second.declaration.structure = StructuralAccess::Alter {
            added: Box::new([]),
            removed: Box::new([]),
        };
        assert!(matches!(
            StagePlan::relation(&logical(), &[first], &[second]),
            OrderingRelation::EarlierFirst
        ));
        let reader = access();
        let mut alter = access();
        alter.declaration.structure = StructuralAccess::Alter {
            added: Box::new([]),
            removed: Box::new([]),
        };
        assert!(matches!(
            StagePlan::relation(&logical(), &[reader], &[alter]),
            OrderingRelation::EarlierFirst
        ));
    }
    // Seam: 阶段排序关系入口；方向来自业务写先读，物理排斥单独保留。
    #[test]
    fn ordering_relation_resource_and_tick_writer_precedes_reader() {
        let mut reader = access();
        let mut writer = access();
        reader.declaration.resources = vec![(ResourceId(0), AccessMode::Read)].into_boxed_slice();
        writer.declaration.resources = vec![(ResourceId(0), AccessMode::Write)].into_boxed_slice();
        assert!(matches!(
            StagePlan::relation(&logical(), &[reader], &[writer]),
            OrderingRelation::LaterFirst
        ));
        let mut reader = access();
        let mut writer = access();
        reader.declaration.tick_reads = vec![ComponentId(1)].into_boxed_slice();
        writer.declaration.components =
            vec![(ComponentId(1), AccessMode::Write)].into_boxed_slice();
        assert!(matches!(
            StagePlan::relation(&logical(), &[reader], &[writer]),
            OrderingRelation::LaterFirst
        ));
    }
}
use super::StagePlan;
use pi_ecs_core::access::ResolvedAccess;
impl StagePlan {
    pub(crate) fn build_inner(access: &[Vec<ResolvedAccess>]) -> StagePlan {
        use super::OrderingRelation;
        use pi_ecs_core::access::{AccessMode, MultiResourceAccess};
        fn writes_before(writer: &ResolvedAccess, reader: &ResolvedAccess) -> bool {
            if writer.resources.iter().any(|(id, mode)| {
                *mode == AccessMode::Write
                    && reader
                        .resources
                        .iter()
                        .any(|(other, mode)| id == other && *mode == AccessMode::Read)
            }) || writer.multi_resources.iter().any(|(id, mode)| {
                matches!(mode, MultiResourceAccess::WriteSlot { .. })
                    && reader.multi_resources.iter().any(|(other, mode)| {
                        id == other && matches!(mode, MultiResourceAccess::ReadAll)
                    })
            }) || writer.values.iter().any(|value| {
                value.mode == AccessMode::Write
                    && reader.values.iter().chain(&reader.tick_reads).any(|read| {
                        read.mode == AccessMode::Read
                            && value.component == read.component
                            && value
                                .archetypes
                                .iter()
                                .any(|id| read.archetypes.contains(id))
                    })
            }) {
                return true;
            }
            if let Some(structure) = &writer.structure {
                if structure
                    .sources
                    .iter()
                    .chain(&structure.targets)
                    .any(|id| reader.entities.contains(id))
                {
                    return true;
                }
            }
            let mut notice = false;
            writer.visit_notice_access(|a| {
                reader.visit_notice_access(|b| {
                    notice |= a.component == b.component
                        && a.kind == b.kind
                        && a.mode == AccessMode::Write
                        && b.mode == AccessMode::Read;
                })
            });
            notice
        }
        let slots: Vec<_> = (0..access.len()).collect();
        Self::graph_ordering(&slots, |earlier, later| {
            let a = &access[earlier];
            let b = &access[later];
            let structural_a = a.iter().any(|leaf| leaf.structure.is_some());
            let structural_b = b.iter().any(|leaf| leaf.structure.is_some());
            if (structural_a && structural_b)
                || a.iter().any(|w| b.iter().any(|r| writes_before(w, r)))
            {
                OrderingRelation::EarlierFirst
            } else if b.iter().any(|w| a.iter().any(|r| writes_before(w, r))) {
                OrderingRelation::LaterFirst
            } else if structural_a
                || structural_b
                || a.iter()
                    .any(|left| b.iter().any(|right| left.conflicts(right)))
            {
                OrderingRelation::EarlierFirst
            } else {
                OrderingRelation::Independent
            }
        })
    }
}

impl StagePlan {
    pub(super) fn ordering_archetypes_inner(
        world: pi_ecs_core::world::WorldRef<'_>,
        trees: &[&pi_ecs_core::access::AccessTree],
    ) -> Result<pi_ecs_core::access::OrderingArchetypes, pi_ecs_core::error::EcsError> {
        use pi_ecs_core::{
            access::{AccessDeclaration, AccessTree, OrderingArchetypes, StructuralAccess},
            component::ComponentId,
            error::{classified_error, ErrorKind},
        };
        use std::collections::BTreeSet;

        fn collect<'a>(
            tree: &'a AccessTree,
            alters: &mut Vec<&'a AccessDeclaration>,
            spawns: &mut Vec<&'a [ComponentId]>,
        ) {
            match tree {
                AccessTree::Leaf(declaration)
                    if matches!(declaration.structure, StructuralAccess::Alter { .. }) =>
                {
                    alters.push(declaration)
                }
                AccessTree::Leaf(declaration) => {
                    if let StructuralAccess::Spawn { components, .. } = &declaration.structure {
                        spawns.push(components);
                    }
                }
                AccessTree::Together(children) | AccessTree::Exclusive(children) => {
                    for child in children {
                        collect(child, alters, spawns);
                    }
                }
            }
        }
        let tables = world.table_layouts()?;
        let mut counts = vec![0usize; tables.len()];
        let mut known = BTreeSet::new();
        let mut combinations: Vec<Box<[ComponentId]>> = Vec::new();
        for archetype in world.archetypes()? {
            counts[archetype.table().table as usize] += 1;
            let components: Box<[_]> = archetype
                .components()
                .iter()
                .map(|entry| entry.component())
                .collect();
            known.insert(components.clone());
            combinations.push(components);
        }
        let mut alters = Vec::new();
        let mut spawns = Vec::new();
        for tree in trees {
            collect(tree, &mut alters, &mut spawns);
        }
        if !spawns.is_empty() {
            Self::seed_spawn_targets_inner(
                tables,
                &spawns,
                &mut counts,
                &mut known,
                &mut combinations,
            )?;
        }
        for declaration in &alters {
            let StructuralAccess::Alter { added, removed } = &declaration.structure else {
                unreachable!()
            };
            let mut distinct = BTreeSet::new();
            for &component in added.iter().chain(removed.iter()) {
                if world.component_info(component).is_none() || !distinct.insert(component) {
                    return Err(classified_error(
                        ErrorKind::InvalidInput,
                        "Alter 增删组件未登记、重复或相交",
                    ));
                }
            }
        }
        let mut cursor = 0;
        while cursor < combinations.len() {
            for declaration in &alters {
                let source = &combinations[cursor];
                let has = |component: &ComponentId| source.binary_search(component).is_ok();
                let StructuralAccess::Alter { added, removed } = &declaration.structure else {
                    unreachable!()
                };
                if !declaration.scope.with.iter().all(has)
                    || declaration.scope.without.iter().any(has)
                    || added.iter().any(has)
                    || !removed.iter().all(has)
                {
                    continue;
                }
                let mut target: Vec<_> = source
                    .iter()
                    .copied()
                    .filter(|id| !removed.contains(id))
                    .collect();
                target.extend_from_slice(added);
                target.sort_unstable();
                Self::insert_ordering_target(
                    tables,
                    &target,
                    &mut counts,
                    &mut known,
                    &mut combinations,
                )?;
            }
            cursor += 1;
        }
        Ok(OrderingArchetypes {
            components: combinations.into_boxed_slice(),
        })
    }
}
impl StagePlan {
    pub(super) fn spawn_target_precedes_inner(
        _writer: &pi_ecs_core::access::OrderingAccess,
        _reader: &pi_ecs_core::access::OrderingAccess,
    ) -> bool {
        use pi_ecs_core::access::{AccessMode, StructuralAccess};
        let StructuralAccess::Spawn { components, .. } = &_writer.declaration.structure else {
            return false;
        };
        let overlaps = |range: &[usize]| _writer.targets.iter().any(|id| range.contains(id));
        overlaps(&_reader.entities)
            || (overlaps(&_reader.sources)
                && (_reader.declaration.components.iter().any(|(id, mode)| {
                    *mode == AccessMode::Read && components.binary_search(id).is_ok()
                }) || _reader
                    .declaration
                    .tick_reads
                    .iter()
                    .any(|id| components.binary_search(id).is_ok())))
    }

    pub(super) fn seed_spawn_targets_inner(
        _tables: &[pi_ecs_core::table::TableLayout],
        _spawns: &[&[pi_ecs_core::component::ComponentId]],
        _counts: &mut [usize],
        _known: &mut std::collections::BTreeSet<Box<[pi_ecs_core::component::ComponentId]>>,
        _combinations: &mut Vec<Box<[pi_ecs_core::component::ComponentId]>>,
    ) -> Result<(), pi_ecs_core::error::EcsError> {
        for target in _spawns {
            Self::insert_ordering_target(_tables, target, _counts, _known, _combinations)?;
        }
        Ok(())
    }

    fn insert_ordering_target(
        tables: &[pi_ecs_core::table::TableLayout],
        target: &[pi_ecs_core::component::ComponentId],
        counts: &mut [usize],
        known: &mut std::collections::BTreeSet<Box<[pi_ecs_core::component::ComponentId]>>,
        combinations: &mut Vec<Box<[pi_ecs_core::component::ComponentId]>>,
    ) -> Result<(), pi_ecs_core::error::EcsError> {
        use pi_ecs_core::error::{classified_error, ErrorKind};
        if known.contains(target) {
            return Ok(());
        }
        let mut best = None;
        for table in tables {
            let columns = table.columns();
            if columns.iter().all(|id| target.binary_search(id).is_ok())
                && best.is_none_or(|(_, size)| columns.len() > size)
            {
                best = Some((table.id().table as usize, columns.len()));
            }
        }
        let (table, _) = best
            .ok_or_else(|| classified_error(ErrorKind::NotFound, "结构写潜在目标缺少基础 Table"))?;
        if counts[table] > u16::MAX as usize {
            return Err(classified_error(
                ErrorKind::ResourceExhausted,
                "结构写潜在目标超过 Table 变体容量",
            ));
        }
        counts[table] += 1;
        let target: Box<[_]> = target.into();
        known.insert(target.clone());
        combinations.push(target);
        Ok(())
    }

    pub(super) fn build_ordering_inner(
        logical: &pi_ecs_core::access::OrderingArchetypes,
        _access: &[Vec<pi_ecs_core::access::OrderingAccess>],
        _slots: &[usize],
    ) -> StagePlan {
        Self::graph_ordering(_slots, |earlier, later| {
            Self::relation(logical, &_access[earlier], &_access[later])
        })
    }

    fn graph_ordering(
        slots: &[usize],
        relation: impl Fn(usize, usize) -> super::OrderingRelation,
    ) -> StagePlan {
        use graph_core::{GraphInsert, NodeAccess, NodeRef, OutNeighbors};
        let mut graph = graph_storage::sparse::SparseGraph::<usize, ()>::new();
        let nodes: Vec<_> = (0..slots.len())
            .map(|index| graph.insert_node(index).expect("新建排序图节点"))
            .collect();
        for earlier in 0..slots.len() {
            for later in earlier + 1..slots.len() {
                let endpoints = match relation(earlier, later) {
                    super::OrderingRelation::Independent => continue,
                    super::OrderingRelation::EarlierFirst => (earlier, later),
                    super::OrderingRelation::LaterFirst => (later, earlier),
                };
                graph
                    .insert_edge(nodes[endpoints.0], nodes[endpoints.1], ())
                    .expect("新建排序图边");
            }
        }
        let Ok(order) = graph_algo::topo::toposort(&graph) else {
            return StagePlan {
                waves: slots
                    .iter()
                    .map(|&slot| super::StageWave {
                        systems: vec![slot].into_boxed_slice(),
                    })
                    .collect(),
            };
        };
        let mut levels = vec![0usize; slots.len()];
        for node in order {
            let index = *graph.node(node).expect("排序节点存在").weight();
            for neighbor in graph.out_neighbors(node) {
                let target = *graph.node(neighbor).expect("排序目标存在").weight();
                levels[target] = levels[target].max(levels[index] + 1);
            }
        }
        let mut waves: Vec<super::StageWave> =
            (0..levels.iter().max().map_or(0, |level| level + 1))
                .map(|_| super::StageWave {
                    systems: Box::new([]),
                })
                .collect();
        let mut members = vec![Vec::new(); waves.len()];
        for (index, &level) in levels.iter().enumerate() {
            members[level].push(slots[index]);
        }
        for (wave, systems) in waves.iter_mut().zip(members) {
            wave.systems = systems.into_boxed_slice();
        }
        StagePlan {
            waves: waves.into_boxed_slice(),
        }
    }
}
