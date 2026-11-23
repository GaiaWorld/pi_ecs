//! 即时结构修改配对芯。
#[cfg(all(test, feature = "rc"))]
mod tests {
    use crate::{
        access::{AccessTree, StructuralAccess},
        alter::{AlterState, QueryAlterState},
        change_detection::ChangeWindow,
        component::{Component, PlainAccess, StoragePolicy},
        entity::{AllocationDomain, ArchetypeId, Entity, EntityLocation},
        error::EcsError,
        query::{FetchStateResult, NoFilter, QueryFilter, QueryFilterDeclaration, QueryState},
        query_data::Has,
        world::World,
    };

    struct Tag;
    impl Component for Tag {
        type Access = PlainAccess;
    }

    struct ExternalAll;
    // SAFETY: no component is read; every published row matches.
    unsafe impl QueryFilter for ExternalAll {
        const IS_ARCHETYPAL: bool = true;
        type State = <NoFilter as QueryFilter>::State;
        type FetchState = <NoFilter as QueryFilter>::FetchState;
        type Fetch<'w> = <NoFilter as QueryFilter>::Fetch<'w>;

        fn init_state(world: &World) -> Result<Self::State, EcsError> {
            NoFilter::init_state(world)
        }

        fn declare_access(state: &Self::State) -> Result<QueryFilterDeclaration, EcsError> {
            NoFilter::declare_access(state)
        }

        fn init_fetch_state(
            _state: &Self::State,
            _scope: crate::query_fetch::FetchScope,
            _binding: &crate::query_fetch::FetchBinding<'_>,
        ) -> Result<FetchStateResult<Self::FetchState>, EcsError> {
            Ok(FetchStateResult::Ready(()))
        }

        unsafe fn prepare<'w>(
            access: &crate::query_fetch::FetchAccess<'w>,
            scope: crate::query_fetch::FetchScope,
            state: &Self::State,
            plan: &Self::FetchState,
        ) -> Result<Self::Fetch<'w>, EcsError> {
            unsafe { NoFilter::prepare(access, scope, state, plan) }
        }
    }

    fn publish_empty(world: &mut World) -> Entity {
        let storage = world.storage.as_mut().unwrap();
        let id = ArchetypeId {
            table: crate::entity::TableId(0),
            variant: crate::entity::VariantId(0),
        };
        let row = storage
            .table_mut(crate::entity::TableId(0))
            .unwrap()
            .claim_row()
            .unwrap();
        let location = EntityLocation::from_parts(
            0,
            storage.member_len(id).unwrap(),
            row.index(),
            crate::entity::TableId(0),
            crate::entity::VariantId(0),
        );
        let entity = world
            .entities
            .get_mut()
            .allocate(AllocationDomain(0), location)
            .unwrap();
        storage.append_member(id, entity, row).unwrap();
        storage
            .table_mut(crate::entity::TableId(0))
            .unwrap()
            .publish_row(row, entity)
            .unwrap();
        entity
    }

    #[test]
    fn rc_query_alter_reuses_leaf_and_skips_retired_rows() {
        let mut world = World::new().unwrap();
        let tag = world
            .register_component::<Tag>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let first = publish_empty(&mut world);
        let retired = publish_empty(&mut world);
        let last = publish_empty(&mut world);
        // SAFETY: the fixture gave this test one strong responsibility per published key.
        assert!(unsafe { world.despawn(retired) }.unwrap());
        world.flush_destruction().unwrap();
        let query = QueryState::<Has<Tag>, ExternalAll>::new(&mut world).unwrap();
        let mut declaration = query.declaration().unwrap().to_access().unwrap();
        let wrong = AccessTree::Leaf(query.declaration().unwrap().to_access().unwrap());
        declaration.structure = StructuralAccess::Alter {
            added: Box::new([tag]),
            removed: Box::new([]),
        };
        let tree = AccessTree::Leaf(declaration);
        let mut state = QueryAlterState {
            query,
            alteration: AlterState::new(&world, &[tag], &[]).unwrap(),
            added_order: Box::new([tag]),
            iteration_order: Box::new([]),
            table_order: None,
        };
        let window = ChangeWindow {
            last_run: None,
            this_run: world.read_change_tick(),
        };
        world
            .schedule_scope(|world| {
                let version = crate::unsafe_world_cell::UnsafeWorldCell::from_ref(world)
                    .archetype_revision()
                    .0;
                // SAFETY: no sibling access, and every Item is consumed within this run.
                unsafe {
                    world.system_scope(&wrong, version, |access| {
                        assert!(access.refresh_alter(&mut state).is_err());
                    })?;
                    world.system_scope(&tree, version, |access| -> Result<(), EcsError> {
                        let access = access.refresh_alter(&mut state)?;
                        let mut query = access
                            .bind_alter::<_, _, crate::bundle::ComponentValue<Tag>, ()>(
                                &mut state, window,
                            )?;
                        assert!(!query.get_mut(last)?);
                        assert!(query.get_mut(retired).is_err());
                        let mut cursor = query.iter_mut();
                        assert_eq!(cursor.fetch_next(), Some(false));
                        assert_eq!(cursor.fetch_next(), Some(false));
                        assert_eq!(cursor.fetch_next(), None);
                        assert!(cursor.alter(crate::bundle::ComponentValue(Tag)).is_err());
                        drop(cursor);
                        query.alter(first, crate::bundle::ComponentValue(Tag))?;
                        assert!(query
                            .alter(first, crate::bundle::ComponentValue(Tag))
                            .is_err());
                        Ok(())
                    })??;
                }
                Ok(())
            })
            .unwrap();
        state.query.unregister_tracking(&mut world).unwrap();
        for entity in [first, last] {
            assert!(unsafe { world.despawn(entity) }.unwrap());
        }
        world.flush_destruction().unwrap();
    }
}
use super::{Alter, AlterIter, AlterState, QueryAlter};
use crate::{
    bundle::{Bundle, SystemBundle},
    component::ComponentId,
    entity::Entity,
    error::EcsError,
    world::{World, WorldRef},
};
fn empty_cache(
    identity: u64,
    epoch: u64,
    registry: &crate::registry::TypeRegistry,
    added: &[ComponentId],
) -> super::AlterCache {
    super::AlterCache {
        tables: registry
            .tables
            .iter()
            .filter(|layout| added.iter().all(|id| !layout.columns().contains(id)))
            .map(|layout| crate::archetype_scan::TableScan {
                table: layout.id().table,
                scanned_variants: 0,
            })
            .collect(),
        world_identity: identity,
        layout_epoch: epoch,
        map: crate::plan_map::ArchetypePlanMap::new(registry.tables.len()),
        plans: Vec::new(),
    }
}

fn fill_cache(
    cache: &mut super::AlterCache,
    registry: &crate::registry::TypeRegistry,
    added: &[ComponentId],
    removed: &[ComponentId],
) -> Result<(), EcsError> {
    use crate::{
        archetype::ComponentSource,
        entity::ArchetypeId,
        error::{classified_error, ErrorKind},
        structural_change::TransferComponent,
        table::ColumnId,
    };
    for plan in &mut cache.plans {
        plan.resolved_target = registry
            .archetype_ids
            .get(plan.target_components.as_ref())
            .copied();
    }
    for scan in &mut cache.tables {
        for source in registry
            .archetypes
            .iter()
            .filter(|source| source.id.table == scan.table)
            .skip(scan.scanned_variants)
        {
            let has = |id: &ComponentId| {
                source
                    .components
                    .binary_search_by_key(id, |entry| entry.component)
                    .is_ok()
            };
            let plan_id = if added.iter().any(has) || !removed.iter().all(has) {
                None
            } else {
                let retained = source
                    .components
                    .iter()
                    .filter(|entry| !removed.contains(&entry.component));
                let mut target_components: Vec<_> = retained
                    .clone()
                    .map(|entry| entry.component)
                    .chain(added.iter().copied())
                    .collect();
                target_components.sort_unstable();
                let target = registry.plan_archetype(&target_components)?;
                let target_table = ArchetypeId {
                    table: target.id.table,
                    variant: crate::entity::VariantId(0),
                };
                let layout = registry.table(target_table).expect("Alter 目标 Table 缺失");
                let retained = retained
                    .map(|entry| TransferComponent {
                        component: entry.component,
                        source: entry.source,
                        target: layout
                            .columns()
                            .binary_search(&entry.component)
                            .ok()
                            .map_or(ComponentSource::Sparse, |column| {
                                ComponentSource::Table(ColumnId(column as u32))
                            }),
                    })
                    .collect();
                let index = u16::try_from(cache.plans.len())
                    .ok()
                    .filter(|index| *index != u16::MAX)
                    .ok_or_else(|| {
                        classified_error(ErrorKind::ResourceExhausted, "Alter 计划超限")
                    })?;
                cache.plans.push(super::AlterPlan {
                    source: source.id,
                    target_components: target_components.into(),
                    target_table,
                    resolved_target: (!target.is_new).then_some(target.id),
                    retained,
                    added: added.into(),
                    removed: removed.into(),
                });
                Some(index)
            };
            cache.map.set(scan.table, source.id.variant, plan_id)?;
            scan.scanned_variants += 1;
        }
    }
    Ok(())
}

#[cfg(feature = "rc")]
impl<Q: crate::query::QueryData, F: crate::query::QueryFilter> super::QueryAlterState<Q, F> {
    pub(super) fn declaration_inner(
        &self,
    ) -> Result<
        (
            crate::query::QueryAccessDeclaration,
            crate::access::AccessDeclaration,
        ),
        EcsError,
    > {
        let query = self.query.declaration()?;
        let mut access = query.to_access()?;
        access.structure = crate::access::StructuralAccess::Alter {
            added: self.alteration.added.clone(),
            removed: self.alteration.removed.clone(),
        };
        Ok((query, access))
    }

    pub(crate) fn refresh_alter_cache(
        &mut self,
        registry: &crate::registry::TypeRegistry,
        storage: Option<&crate::storage::WorldStorage>,
        identity: u64,
        epoch: u64,
    ) -> Result<(), EcsError> {
        use crate::query::QueryTable;
        let alteration = &mut self.alteration;
        let cache = alteration.updated_cache(registry, identity, epoch)?;
        self.query.refresh_cache(registry, storage, identity)?;
        let mut order = Vec::new();
        for (offset, table) in self.query.cache.tables.iter().enumerate() {
            let table_id = usize::from(self.query.cache.first_table) + offset;
            match table {
                QueryTable::Excluded => {}
                QueryTable::Uniform { .. } => order.extend(
                    registry
                        .archetypes
                        .iter()
                        .filter(|entry| usize::from(entry.id.table) == table_id)
                        .map(|entry| entry.id),
                ),
                QueryTable::Variants { entries, .. } => order.extend(entries.iter().map(
                    |(variant, _)| crate::entity::ArchetypeId {
                        table: crate::entity::TableId(table_id as u16),
                        variant: *variant,
                    },
                )),
            }
        }
        let no_change = alteration.added.is_empty() && alteration.removed.is_empty();
        if !no_change {
            order.sort_by_key(|id| cache.map.get(id.table, id.variant).is_some());
        }
        // Only collapse tables when every matched variant is covered by a uniform plan
        // and the target-first ordering does not revisit a table.
        let mut tables = Vec::new();
        let mut table_positions = vec![None; registry.tables.len()];
        let merges = order.iter().all(|id| {
            let offset = usize::from(id.table) - usize::from(self.query.cache.first_table);
            if !matches!(self.query.cache.tables[offset], QueryTable::Uniform { .. }) {
                return false;
            }
            if tables.last() == Some(&id.table) {
                return true;
            }
            let position = &mut table_positions[usize::from(id.table)];
            if position.is_some() {
                return false;
            }
            *position = Some(tables.len());
            tables.push(id.table);
            true
        });
        // Same-table conversions preserve the physical row. Cross-table conversions
        // must not append to any later scanned table, including a future variant of it.
        let no_revisit = no_change
            || cache.plans.iter().all(|plan| {
                let source = table_positions[usize::from(plan.source.table)];
                let target = table_positions[usize::from(plan.target_table.table)];
                match (source, target) {
                    (Some(source), Some(target)) => target <= source,
                    _ => true,
                }
            });
        self.table_order = (merges && no_revisit).then(|| tables.into_boxed_slice());
        self.iteration_order = order.into_boxed_slice();
        alteration.cache = cache;
        Ok(())
    }
}

impl AlterState {
    pub(super) fn new_inner(
        _world: &World,
        _added: &[ComponentId],
        _removed: &[ComponentId],
    ) -> Result<AlterState, EcsError> {
        use crate::error::{classified_error, ErrorKind};
        if !_world.registry.frozen {
            return Err(classified_error(
                ErrorKind::InvalidState,
                "Alter 要求冻结登记",
            ));
        }
        let mut added = _added.to_vec();
        let mut removed = _removed.to_vec();
        added.sort_unstable();
        removed.sort_unstable();
        if added.windows(2).any(|pair| pair[0] == pair[1])
            || removed.windows(2).any(|pair| pair[0] == pair[1])
            || added.iter().any(|id| removed.binary_search(id).is_ok())
            || added
                .iter()
                .chain(&removed)
                .any(|id| _world.registry.component(*id).is_none())
        {
            return Err(classified_error(
                ErrorKind::InvalidInput,
                "Alter 增删重复、相交或未登记",
            ));
        }
        let cache = empty_cache(
            _world.identity,
            _world.layout_epoch,
            &_world.registry,
            &added,
        );
        Ok(Self {
            added: added.into(),
            removed: removed.into(),
            cache,
        })
    }
}
impl AlterState {
    pub(super) fn prepare_inner(&mut self, _world: WorldRef<'_>) -> Result<(), EcsError> {
        use crate::error::{classified_error, ErrorKind};
        if _world.identity != self.cache.world_identity || !_world.registry.frozen {
            return Err(classified_error(
                ErrorKind::InvalidState,
                "Alter 与冻结 World 不同源",
            ));
        }
        self.cache = self.updated_cache(&_world.registry, _world.identity, _world.layout_epoch)?;
        Ok(())
    }

    fn updated_cache(
        &self,
        registry: &crate::registry::TypeRegistry,
        identity: u64,
        epoch: u64,
    ) -> Result<super::AlterCache, EcsError> {
        let mut next = if epoch == self.cache.layout_epoch {
            super::AlterCache {
                tables: self
                    .cache
                    .tables
                    .iter()
                    .map(|scan| crate::archetype_scan::TableScan {
                        table: scan.table,
                        scanned_variants: scan.scanned_variants,
                    })
                    .collect(),
                world_identity: self.cache.world_identity,
                layout_epoch: self.cache.layout_epoch,
                map: crate::plan_map::ArchetypePlanMap {
                    entries: self.cache.map.entries.clone(),
                    table_count: self.cache.map.table_count,
                },
                plans: self.cache.plans.clone(),
            }
        } else {
            empty_cache(identity, epoch, registry, &self.added)
        };
        fill_cache(&mut next, registry, &self.added, &self.removed)?;
        Ok(next)
    }
    pub(super) fn scope_inner<R>(
        &mut self,
        _world: WorldRef<'_>,
        _run: impl for<'a> FnOnce(&mut Alter<'a>) -> R,
    ) -> Result<R, EcsError> {
        #[cfg(feature = "rc")]
        {
            self.prepare(_world)?;
            let mut authorization = _world.write()?;
            let tick = authorization.tick;
            let world =
                crate::unsafe_world_cell::UnsafeWorldCell::from_mut(&mut *authorization.world);
            Ok(_run(&mut Alter {
                state: self,
                world,
                tick,
            }))
        }
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC Alter 延期",
        ))
    }
}
#[cfg(feature = "rc")]
fn apply_fixed_alter<I: Bundle>(
    cell: crate::unsafe_world_cell::UnsafeWorldCell<'_>,
    state: &AlterState,
    tick: crate::change_detection::ChangeTick,
    entity: Entity,
    values: I,
    declared_order: Option<&[ComponentId]>,
) -> Result<(), EcsError> {
    use crate::{
        bundle::{BundleContext, BundleWriter},
        error::{classified_error, ErrorKind},
        world::AccessMode,
    };
    let root = cell.pointer.as_ptr();
    // SAFETY: each caller supplies the exclusive short structural borrow for the
    // original World. Only detached identity and registry projections are read.
    unsafe {
        let access = &*std::ptr::addr_of!((*root).access);
        let mode = access.mode.replace(AccessMode::Stopped);
        let active = matches!(mode, AccessMode::Maintaining | AccessMode::Running);
        access.mode.set(mode);
        if !active || state.cache.world_identity != cell.id() {
            return Err(classified_error(
                ErrorKind::InvalidState,
                "Alter 缺少同源结构授权",
            ));
        }
        let entities = &*std::ptr::addr_of!((*root).entities);
        if !entities
            .borrow()
            .counts(entity)
            .is_some_and(|(strong, _)| strong > 0)
        {
            return Err(classified_error(ErrorKind::NotFound, "Alter 要求存活实体"));
        }
        let registry = &*std::ptr::addr_of!((*root).registry);
        let ids =
            I::component_types()
                .into_iter()
                .map(|ty| {
                    registry.component_ids.get(&ty).copied().ok_or_else(|| {
                        classified_error(ErrorKind::NotFound, "Alter 新增类型未登记")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
        if declared_order.is_some_and(|order| ids != order) {
            return Err(classified_error(
                ErrorKind::InvalidInput,
                "Bundle 与登记的 Add 有序组件声明不一致",
            ));
        }
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        if sorted.as_slice() != state.added.as_ref() {
            return Err(classified_error(
                ErrorKind::InvalidInput,
                "Bundle 未精确覆盖固定新增组件",
            ));
        }
        let context = BundleContext {
            world_identity: cell.id(),
            entities,
            pending: &*std::ptr::addr_of!((*root).pending_destruction),
            access,
        };
        values.validate(&context)?;
        if ids.is_empty() && state.removed.is_empty() {
            let mut written = 0;
            values.write(&mut BundleWriter {
                world: cell,
                context,
                components: &[],
                types: &[],
                layouts: &[],
                targets: &[],
                written: &mut written,
            });
            let mode = access.mode.replace(AccessMode::Stopped);
            assert!(
                matches!(mode, AccessMode::Maintaining | AccessMode::Running) && written == 0,
                "新增 Bundle 未按声明完整写入"
            );
            access.mode.set(mode);
            return Ok(());
        }
        let authorized = declared_order.unwrap_or(&ids);
        World::change_components_rc(
            cell,
            entity,
            authorized,
            values,
            &state.removed,
            tick,
            false,
        )
    }
}

impl Alter<'_> {
    pub(super) fn alter_inner<I: Bundle>(
        &mut self,
        _entity: Entity,
        _values: I,
    ) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        {
            apply_fixed_alter(self.world, self.state, self.tick, _entity, _values, None)
        }
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC Alter 延期",
        ))
    }
}
impl<'run, Q, F, Add, Remove> QueryAlter<'run, Q, F, Add, Remove>
where
    Q: crate::query::QueryData,
    F: crate::query::QueryFilter,
    Add: SystemBundle<Shape = Add> + 'static,
    Remove: Bundle,
{
    pub(super) fn query_get_inner(&mut self, _entity: Entity) -> Result<Q::Item<'_, '_>, EcsError> {
        #[cfg(feature = "rc")]
        {
            let mut fetch = self.access.fetch_access()?;
            fetch.world = self.access.world.reborrow();
            fetch.window = Some(self.window);
            fetch.write_tick = Some(self.window.this_run);
            // SAFETY: the short exclusive borrow excludes every other operation on this leaf.
            unsafe { crate::query::QueryState::fetch_one(&self.state.query, &fetch, _entity) }
        }
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC Alter 延期",
        ))
    }
    pub(super) fn iter_inner(&mut self) -> AlterIter<'_, 'run, Q, F, Add, Remove> {
        AlterIter {
            source: self,
            current: None,
            archetype_cursor: 0,
            member_cursor: 0,
        }
    }
    pub(super) fn query_alter_inner<I: SystemBundle<Shape = Add>>(
        &mut self,
        _entity: Entity,
        _values: I,
    ) -> Result<(), EcsError> {
        #[cfg(feature = "rc")]
        {
            {
                let mut access = self.access.fetch_access()?;
                access.world = self.access.world.reborrow();
                access.window = Some(self.window);
                access.write_tick = Some(self.window.this_run);
                // SAFETY: the exclusive short borrow protects the original leaf;
                // the temporary fetch is dropped before any structure mutation.
                unsafe {
                    crate::query::QueryState::prepare_cached_target(
                        &self.state.query,
                        &access,
                        _entity,
                    )?
                };
            }
            apply_fixed_alter(
                self.access.world,
                &self.state.alteration,
                self.window.this_run,
                _entity,
                _values,
                Some(&self.state.added_order),
            )
        }
        #[cfg(not(feature = "rc"))]
        Err(crate::error::classified_error(
            crate::error::ErrorKind::Unsupported,
            "非 RC QueryAlter 延期",
        ))
    }
}
impl<Q, F, Add, Remove> AlterIter<'_, '_, Q, F, Add, Remove>
where
    Q: crate::query::QueryData,
    F: crate::query::QueryFilter,
    Add: SystemBundle<Shape = Add> + 'static,
    Remove: Bundle,
{
    pub(super) fn iter_next_inner(&mut self) -> Option<Q::Item<'_, '_>> {
        self.current = None;
        #[cfg(feature = "rc")]
        {
            use crate::{query::QueryState, query_fetch::QueryFetch, table::TableRow};
            let source = &mut *self.source;
            let mut access = source.access.fetch_access().expect("Alter requires a leaf");
            access.world = source.access.world.reborrow();
            access.window = Some(source.window);
            access.write_tick = Some(source.window.this_run);
            // SAFETY: the iterator owns the short leaf borrow. Only copied member data
            // is retained before fetching a component result.
            let storage = unsafe { &*std::ptr::addr_of!((*access.world.pointer.as_ptr()).storage) }
                .as_ref()
                .expect("Alter requires frozen storage");
            loop {
                let row = u32::try_from(self.member_cursor).expect("Alter row overflow");
                let (end, entity) = if let Some(tables) = &source.state.table_order {
                    let table = *tables.get(self.archetype_cursor)?;
                    let end = storage.published_rows(table).expect("Alter table missing");
                    (
                        end,
                        (row < end)
                            .then(|| storage.row_entity(table, TableRow::new(row)))
                            .flatten(),
                    )
                } else {
                    let id = *source.state.iteration_order.get(self.archetype_cursor)?;
                    let end = storage.member_len(id).expect("Alter archetype missing");
                    (
                        end,
                        (row < end)
                            .then(|| storage.member(id, row))
                            .flatten()
                            .map(|member| member.entity),
                    )
                };
                if row >= end {
                    self.archetype_cursor += 1;
                    self.member_cursor = 0;
                    continue;
                }
                self.member_cursor += 1;
                let Some(entity) = entity else { continue };
                let target = unsafe {
                    QueryState::prepare_optional_cached_target(&source.state.query, &access, entity)
                }
                .expect("Alter could not prepare a published member");
                if let Some((target, mut fetch)) = target {
                    self.current = Some(entity);
                    return Some(unsafe { fetch.fetch_at(&target) });
                }
            }
        }
        #[cfg(not(feature = "rc"))]
        None
    }
    pub(super) fn iter_alter_inner<I: SystemBundle<Shape = Add>>(
        &mut self,
        _values: I,
    ) -> Result<(), EcsError> {
        let entity = self.current.ok_or_else(|| {
            crate::error::classified_error(
                crate::error::ErrorKind::InvalidState,
                "Alter 游标无当前项",
            )
        })?;
        self.source.query_alter_inner(entity, _values)
    }
}
