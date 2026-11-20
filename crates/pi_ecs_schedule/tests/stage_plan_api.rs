//! 外部 crate 视角的阶段声明验收；不执行设计失败桩。

// 仅编译：即时 Alter 与查询/Reader 均可作为真正的 ParamSet 成员。
#[allow(dead_code)]
fn sequential_alter_query<'a, 'w>(
    set: &mut pi_ecs_schedule::param_composition::ParamSetItem<
        'a,
        'w,
        pi_ecs_core::alter::QueryAlter<'static, pi_ecs_core::entity::Entity>,
        pi_ecs_core::query::Query<'static, pi_ecs_core::entity::Entity>,
    >,
) {
    {
        let _alter = set.p0();
    }
    {
        let _query = set.p1();
    }
    {
        let _alter_again = set.p0();
    }
}
#[cfg(feature = "rc")]
#[allow(dead_code)]
fn sequential_alter_reader<'a, 'w, C: pi_ecs_core::component::Component>(
    set: &mut pi_ecs_schedule::param_composition::ParamSetItem<
        'a,
        'w,
        pi_ecs_core::alter::QueryAlter<'static, pi_ecs_core::entity::Entity>,
        pi_ecs_schedule::notification_reader::RemovedReader<'static, C>,
    >,
) {
    {
        let _alter = set.p0();
    }
    {
        let _reader = set.p1();
    }
}
#[allow(dead_code)]
fn assert_sequential_system_params() {
    fn accepts<P: pi_ecs_schedule::system_param::SystemParam>() {}
    accepts::<
        pi_ecs_schedule::param_composition::ParamSet<(
            pi_ecs_core::alter::QueryAlter<'static, pi_ecs_core::entity::Entity>,
            pi_ecs_core::query::Query<'static, pi_ecs_core::entity::Entity>,
        )>,
    >();
}
use pi_ecs_core::access::ResolvedAccess;
use pi_ecs_schedule::stage::{Stage, StageSystemBound};
use pi_ecs_schedule::stage_plan::StagePlan;

// 外部参数能使用受限登记与原窗口授权，不访问包内字段；仅编译。
#[allow(dead_code)]
fn external_registration<'a, 'b>(
    registration: &'a mut pi_ecs_schedule::system_param::ParamRegistration<'b>,
) -> &'a mut pi_ecs_core::parameter_registration::ParameterRegistration<'b> {
    registration.access()
}
#[allow(dead_code)]
fn external_run_parts<'a>(
    context: pi_ecs_schedule::system::SystemRunContext<'a>,
) -> (
    pi_ecs_core::access::RunAccess<'a>,
    pi_ecs_core::change_detection::ChangeWindow,
) {
    context.into_parts()
}

// 编译接缝：外部 crate 只通过公开声明取得逻辑闭包输入，不运行失败桩。
#[allow(dead_code)]
fn inspect_metadata(world: &pi_ecs_core::world::World) -> Result<(), pi_ecs_core::error::EcsError> {
    for table in world.table_layouts()? {
        let _ = (table.id(), table.columns());
    }
    for archetype in world.archetypes()? {
        let _ = (archetype.id(), archetype.table());
        for component in archetype.components() {
            let _ = (component.component(), component.source());
        }
    }
    Ok(())
}

#[cfg(feature = "rc")]
#[allow(dead_code)]
fn register_multi_before_freeze(
    world: &mut pi_ecs_core::world::World,
) -> Result<(), pi_ecs_core::error::EcsError> {
    let _id = world.register_multi_resource::<u32>()?;
    world.finish_registration()?;
    Ok(())
}

#[allow(dead_code)]
fn register<S: StageSystemBound + 'static>(stage: &mut Stage, system: S) {
    let _ = stage.add_system(system);
}

fn build_plan(access: &[Vec<ResolvedAccess>]) -> StagePlan {
    StagePlan::build(access)
}

#[test]
fn stage_and_plan_protocol_is_public() {
    let _ = build_plan as fn(&[Vec<ResolvedAccess>]) -> StagePlan;
}
