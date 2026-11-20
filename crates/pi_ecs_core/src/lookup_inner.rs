//! 点查参数配对芯，不调用其他芯。
use crate::{
    entity::Entity,
    error::EcsError,
    lookup::Lookup,
    query::{QueryData, QueryFilter, ReadOnlyQueryData},
};
impl<Q: QueryData, F: QueryFilter> Lookup<'_, Q, F> {
    pub(crate) fn get_mut_inner<'a>(
        _lookup: &'a mut Lookup<'_, Q, F>,
        _entity: Entity,
    ) -> Result<Q::Item<'a, 'a>, EcsError> {
        #[cfg(feature = "rc")]
        {
            let mut access = _lookup.access.fetch_access()?;
            access.world = _lookup.access.world.reborrow();
            access.window = Some(_lookup.window);
            access.write_tick = Some(_lookup.window.this_run);
            // SAFETY: the exclusive short borrow excludes a conflicting lookup result.
            unsafe { crate::query::QueryState::<Q, F>::fetch_one(_lookup.state, &access, _entity) }
        }
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC 点查延期",
        ))
    }
}
impl<Q: ReadOnlyQueryData, F: QueryFilter> Lookup<'_, Q, F> {
    pub(crate) fn get_inner<'a>(
        _lookup: &'a Lookup<'_, Q, F>,
        _entity: Entity,
    ) -> Result<Q::Item<'a, 'a>, EcsError> {
        #[cfg(feature = "rc")]
        {
            let mut access = _lookup.access.fetch_access()?;
            access.window = Some(_lookup.window);
            // SAFETY: the read-only descriptor and original leaf authorize shared results.
            unsafe { crate::query::QueryState::<Q, F>::fetch_one(_lookup.state, &access, _entity) }
        }
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC 点查延期",
        ))
    }
}
