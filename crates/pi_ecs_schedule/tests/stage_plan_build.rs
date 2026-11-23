//! 阶段计划风险验收保留；旧算法暂停后仅编译，不运行失败桩。
//! 实现时须恢复这些场景执行，并为正式逻辑入口补资源/多例/tick/潜在目标与范围方向场景。
use pi_ecs_core::access::{
    AccessMode, ResolvedAccess, ResolvedStructureAccess, ResolvedValueAccess,
};
use pi_ecs_core::component::ComponentId;
use pi_ecs_core::entity::ArchetypeId;
use pi_ecs_schedule::stage_plan::StagePlan;

fn arch(variant: u16) -> ArchetypeId {
    ArchetypeId {
        table: pi_ecs_core::entity::TableId(0),
        variant: pi_ecs_core::entity::VariantId(variant),
    }
}

fn access(
    values: Vec<(u32, AccessMode, u16)>,
    structure: Option<ResolvedStructureAccess>,
) -> ResolvedAccess {
    ResolvedAccess {
        resources: Box::new([]),
        multi_resources: Box::new([]),
        observations: Box::new([]),
        tick_reads: Box::new([]),
        values: values
            .into_iter()
            .map(|(component, mode, variant)| ResolvedValueAccess {
                component: ComponentId(component),
                mode,
                archetypes: vec![arch(variant)].into_boxed_slice(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        entities: Box::new([]),
        structure,
    }
}

fn structure(sources: &[u16], targets: &[u16], added: &[u32]) -> ResolvedStructureAccess {
    ResolvedStructureAccess {
        sources: sources
            .iter()
            .map(|variant| arch(*variant))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        targets: targets
            .iter()
            .map(|variant| arch(*variant))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        added: added
            .iter()
            .map(|component| ComponentId(*component))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        removed: Box::new([]),
    }
}

fn waves(plan: &StagePlan) -> Vec<Vec<usize>> {
    plan.waves
        .iter()
        .map(|wave| wave.systems.to_vec())
        .collect()
}

#[allow(dead_code)]
fn empty_stage_has_no_waves() {
    assert!(waves(&StagePlan::build(&[])).is_empty());
}

#[allow(dead_code)]
fn disjoint_reads_run_together() {
    let plan = StagePlan::build(&[
        vec![access(vec![(0, AccessMode::Read, 0)], None)],
        vec![access(vec![(1, AccessMode::Read, 0)], None)],
        vec![access(vec![(2, AccessMode::Read, 0)], None)],
    ]);
    assert_eq!(waves(&plan), vec![vec![0, 1, 2]]);
}

#[allow(dead_code)]
fn writer_runs_before_reader() {
    // 0 只读，1 写同组件；即便 0 先登记，也先跑写者。
    let plan = StagePlan::build(&[
        vec![access(vec![(0, AccessMode::Read, 0)], None)],
        vec![access(vec![(0, AccessMode::Write, 0)], None)],
    ]);
    assert_eq!(waves(&plan), vec![vec![1], vec![0]]);
}

#[allow(dead_code)]
fn multiple_writers_follow_registration_order() {
    let plan = StagePlan::build(&[
        vec![access(vec![(0, AccessMode::Write, 0)], None)],
        vec![access(vec![(0, AccessMode::Write, 0)], None)],
    ]);
    assert_eq!(waves(&plan), vec![vec![0], vec![1]]);
}

#[allow(dead_code)]
fn same_component_on_disjoint_archetypes_runs_together() {
    let plan = StagePlan::build(&[
        vec![access(vec![(0, AccessMode::Write, 0)], None)],
        vec![access(vec![(0, AccessMode::Write, 1)], None)],
    ]);
    assert_eq!(waves(&plan), vec![vec![0, 1]]);
}

#[allow(dead_code)]
fn structure_change_conflicts_with_value_on_same_archetype() {
    let plan = StagePlan::build(&[
        vec![access(vec![(0, AccessMode::Read, 0)], None)],
        vec![access(Vec::new(), Some(structure(&[0], &[1], &[5])))],
    ]);
    assert_eq!(waves(&plan), vec![vec![0], vec![1]]);
}

#[allow(dead_code)]
fn cyclic_precedence_falls_back_to_serial_registration_order() {
    // 0 写 X 读 Y；1 写 Y 读 Z；2 写 Z 读 X，加边会成环。
    let plan = StagePlan::build(&[
        vec![access(
            vec![(0, AccessMode::Write, 0), (1, AccessMode::Read, 0)],
            None,
        )],
        vec![access(
            vec![(1, AccessMode::Write, 0), (2, AccessMode::Read, 0)],
            None,
        )],
        vec![access(
            vec![(2, AccessMode::Write, 0), (0, AccessMode::Read, 0)],
            None,
        )],
    ]);
    assert_eq!(waves(&plan), vec![vec![0], vec![1], vec![2]]);
}
