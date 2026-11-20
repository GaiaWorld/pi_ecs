//! 错误类型与跨模块报告兼容性验收，不构造 World。
use pi_ecs_core::error::prelude::ErrorInspectExt;

#[test]
fn explicit_classification_keeps_diagnostic_without_parsing_it() {
    let error = pi_ecs_core::error::classified_error(ErrorKind::ResourceExhausted, "容量边界");
    assert_eq!(error.current_kind(), &ErrorKind::ResourceExhausted);
    assert_eq!(
        error.downcast_ref::<String>().map(String::as_str),
        Some("容量边界")
    );
}
use pi_ecs_core::{
    error::{EcsError, EcsResult, ErrorKind, Report},
    query_batch::{BatchQueryContext, QueryEntityContext},
};

#[test]
fn ecs_error_is_the_shared_report_and_preserves_typed_context() {
    let entity = pi_ecs_core::entity::Entity::default();
    let report: EcsError =
        Report::new(QueryEntityContext::NotMatched(entity)).change_context(ErrorKind::InvalidInput);
    let result: EcsResult<()> = Err(report);
    let error = result.unwrap_err();
    assert_eq!(error.current_kind(), &ErrorKind::InvalidInput);
    assert!(matches!(
        error.downcast_ref::<QueryEntityContext>(),
        Some(QueryEntityContext::NotMatched(_))
    ));
}

#[test]
fn batch_report_keeps_index_and_reason() {
    let entity = pi_ecs_core::entity::Entity::default();
    let error: EcsError = Report::new(BatchQueryContext {
        index: 3,
        cause: QueryEntityContext::AliasedEntity(entity),
    })
    .change_context(ErrorKind::Conflict);
    assert_eq!(error.current_kind(), &ErrorKind::Conflict);
    let context = error.downcast_ref::<BatchQueryContext>().unwrap();
    assert_eq!(context.index, 3);
    assert!(matches!(
        context.cause,
        QueryEntityContext::AliasedEntity(_)
    ));
}
