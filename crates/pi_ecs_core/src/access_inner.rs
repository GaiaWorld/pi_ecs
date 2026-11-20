//! 权限配对芯；静态声明判冲突与展开权限判可并发性分开实现。
#[cfg(all(test, feature = "rc"))]
mod rc_run_authority_tests {
    use crate::{access::*, unsafe_world_cell::UnsafeWorldCell, world::World};
    fn leaf() -> AccessTree {
        AccessTree::Leaf(AccessDeclaration {
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
        })
    }
    // Seam: executor 授权构造、按位置缩权和叶子 fetch 描述；只用真实根会话来源。
    #[test]
    fn rc_run_authority_preserves_source_and_tree_boundaries() {
        let mut world = World::new().unwrap();
        world.finish_registration().unwrap();
        let tree = AccessTree::Together(vec![leaf(), AccessTree::Exclusive(vec![leaf()])]);
        world
            .schedule_scope(|world| {
                let identity = UnsafeWorldCell::from_ref(world).id();
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                // SAFETY: 空声明无业务别名，根会话保活到全部描述释放。
                let access = unsafe { RunAccess::from_executor(world, &tree, version) };
                assert_eq!(access.world_cell().id(), identity);
                assert!(access.fetch_access().is_err());
                assert!(access.child(2).is_err());
                let first = access.child(0).unwrap();
                assert!(first.fetch_access().is_ok());
                assert!(first.child(0).is_err());
                assert!(unsafe { first.context_child(0) }.is_err());
                let exclusive = access.child(1).unwrap();
                assert!(exclusive.child(0).is_err());
                let selected = unsafe { exclusive.context_child(0) }.unwrap();
                assert_eq!(selected.world_cell().id(), identity);
                assert!(selected.fetch_access().is_ok());
                assert!(unsafe { access.context_child(2) }.is_err());
                Ok(())
            })
            .unwrap();
        world.close().unwrap();
    }
}

impl<'w> super::RunAccess<'w> {
    pub(super) fn refresh_query_inner<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        self,
        _state: &mut crate::query::QueryState<Q, F>,
    ) -> Result<Self, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            self.validate_query_leaf(_state)?;
            let root = self.world.pointer.as_ptr();
            // SAFETY: refresh uses only same-source registry and stable storage metadata.
            unsafe {
                _state.refresh_cache(
                    &*std::ptr::addr_of!((*root).registry),
                    (&*std::ptr::addr_of!((*root).storage)).as_ref(),
                    self.world.id(),
                )?;
            }
            Ok(Self {
                archetype_version: self.world.archetype_revision().0,
                ..self
            })
        }
        #[cfg(not(feature = "rc"))]
        Err(authority_tree_error())
    }
    pub(super) fn refresh_alter_inner<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        self,
        _state: &mut crate::alter::QueryAlterState<Q, F>,
    ) -> Result<Self, crate::error::EcsError> {
        #[cfg(feature = "rc")]
        {
            self.validate_alter_leaf(_state)?;
            let root = self.world.pointer.as_ptr();
            // SAFETY: the original exclusive structure leaf excludes live views during refresh.
            unsafe {
                _state.refresh_alter_cache(
                    &*std::ptr::addr_of!((*root).registry),
                    (&*std::ptr::addr_of!((*root).storage)).as_ref(),
                    self.world.id(),
                    self.world.archetype_revision().0,
                )?;
            }
            Ok(Self {
                archetype_version: self.world.archetype_revision().0,
                ..self
            })
        }
        #[cfg(not(feature = "rc"))]
        Err(authority_tree_error())
    }
}
use crate::access::{AccessDeclaration, ResolvedAccess, RunAccess};
use crate::error::EcsError;
fn authority_tree_error() -> EcsError {
    pi_ecs_types::error::classified_error(
        pi_ecs_types::error::ErrorKind::InvalidState,
        "授权树节点种类或位置无效",
    )
}
impl crate::access::NoticeAccessSource<'_> {
    pub(super) fn visit_inner(&self, mut visit: impl FnMut(crate::access::NoticeAccess)) {
        use crate::{
            access::{AccessMode, NoticeAccess, NoticeAccessSource, StructuralAccess},
            notification::NoticeKind,
        };
        let (observations, added, removed) = match self {
            NoticeAccessSource::Declaration(access) => {
                for &(component, mode) in &access.components {
                    if mode == AccessMode::Write {
                        visit(NoticeAccess {
                            component,
                            kind: NoticeKind::Changed,
                            mode,
                        });
                    }
                }
                let (added, removed): (&[_], &[_]) = match &access.structure {
                    StructuralAccess::None => (&[], &[]),
                    StructuralAccess::Spawn { components, .. } => (components, &[]),
                    StructuralAccess::Alter { added, removed } => (added, removed),
                };
                (&access.observations, added, removed)
            }
            NoticeAccessSource::Resolved(access) => {
                for value in &access.values {
                    if value.mode == AccessMode::Write {
                        visit(NoticeAccess {
                            component: value.component,
                            kind: NoticeKind::Changed,
                            mode: AccessMode::Write,
                        });
                    }
                }
                let (added, removed): (&[_], &[_]) = match &access.structure {
                    None => (&[], &[]),
                    Some(structure) => (&structure.added, &structure.removed),
                };
                (&access.observations, added, removed)
            }
        };
        for &(component, kind) in observations.iter() {
            visit(NoticeAccess {
                component,
                kind,
                mode: AccessMode::Read,
            });
        }
        for &component in added {
            for kind in [NoticeKind::Added, NoticeKind::Changed] {
                visit(NoticeAccess {
                    component,
                    kind,
                    mode: AccessMode::Write,
                });
            }
        }
        for &component in removed {
            visit(NoticeAccess {
                component,
                kind: NoticeKind::Removed,
                mode: AccessMode::Write,
            });
        }
    }
}
impl AccessDeclaration {
    pub(super) fn visit_notice_access_inner(&self, visit: impl FnMut(crate::access::NoticeAccess)) {
        crate::access::NoticeAccessSource::Declaration(self).visit(visit)
    }
}
impl ResolvedAccess {
    pub(super) fn visit_notice_access_inner(&self, visit: impl FnMut(crate::access::NoticeAccess)) {
        crate::access::NoticeAccessSource::Resolved(self).visit(visit)
    }
}

fn resource_conflicts(
    left: &[(crate::resource::ResourceId, crate::access::AccessMode)],
    right: &[(crate::resource::ResourceId, crate::access::AccessMode)],
) -> bool {
    use crate::access::AccessMode;
    left.iter().any(|(id, mode)| {
        right.iter().any(|(other, other_mode)| {
            id == other && (*mode == AccessMode::Write || *other_mode == AccessMode::Write)
        })
    })
}

fn multi_resource_conflicts(
    left: &[(
        crate::resource::ResourceId,
        crate::access::MultiResourceAccess,
    )],
    right: &[(
        crate::resource::ResourceId,
        crate::access::MultiResourceAccess,
    )],
) -> bool {
    use crate::access::MultiResourceAccess;
    left.iter().any(|(id, mode)| {
        right.iter().any(|(other, other_mode)| {
            id == other
                && match (mode, other_mode) {
                    (MultiResourceAccess::ReadAll, MultiResourceAccess::ReadAll) => false,
                    (
                        MultiResourceAccess::WriteSlot { producer: a },
                        MultiResourceAccess::WriteSlot { producer: b },
                    ) => a == b,
                    _ => true,
                }
        })
    })
}

impl ResolvedAccess {
    pub(crate) fn resolved_conflicts_inner(left: &ResolvedAccess, right: &ResolvedAccess) -> bool {
        use crate::access::AccessMode;
        if left.structure.is_some() || right.structure.is_some() {
            return true;
        }
        if resource_conflicts(&left.resources, &right.resources)
            || multi_resource_conflicts(&left.multi_resources, &right.multi_resources)
        {
            return true;
        }
        let overlap = |a: &crate::access::ResolvedValueAccess,
                       b: &crate::access::ResolvedValueAccess| {
            a.component == b.component && a.archetypes.iter().any(|id| b.archetypes.contains(id))
        };
        if left.values.iter().any(|a| {
            right.values.iter().any(|b| {
                (a.mode == AccessMode::Write || b.mode == AccessMode::Write) && overlap(a, b)
            })
        }) || left
            .values
            .iter()
            .any(|a| a.mode == AccessMode::Write && right.tick_reads.iter().any(|b| overlap(a, b)))
            || right.values.iter().any(|a| {
                a.mode == AccessMode::Write && left.tick_reads.iter().any(|b| overlap(a, b))
            })
        {
            return true;
        }
        // 观察还与同组件活值写存在业务排斥，不局限于观察的具体通知种类。
        if left.observations.iter().any(|(id, _)| {
            right
                .values
                .iter()
                .any(|v| v.component == *id && v.mode == AccessMode::Write)
        }) || right.observations.iter().any(|(id, _)| {
            left.values
                .iter()
                .any(|v| v.component == *id && v.mode == AccessMode::Write)
        }) {
            return true;
        }
        let mut conflict = false;
        left.visit_notice_access(|a| {
            right.visit_notice_access(|b| {
                conflict |= a.component == b.component
                    && a.kind == b.kind
                    && (a.mode == AccessMode::Write || b.mode == AccessMode::Write);
            })
        });
        conflict
    }
}

impl AccessDeclaration {
    pub(crate) fn conflicts_inner(left: &AccessDeclaration, right: &AccessDeclaration) -> bool {
        use crate::access::{AccessMode, StructuralAccess};
        if resource_conflicts(&left.resources, &right.resources)
            || multi_resource_conflicts(&left.multi_resources, &right.multi_resources)
        {
            return true;
        }
        let entity_access = |access: &AccessDeclaration| {
            access.reads_entities
                || !access.components.is_empty()
                || !access.tick_reads.is_empty()
                || !access.observations.is_empty()
                || !matches!(access.structure, StructuralAccess::None)
        };
        if (!matches!(left.structure, StructuralAccess::None) && entity_access(right))
            || (!matches!(right.structure, StructuralAccess::None) && entity_access(left))
        {
            return true;
        }
        let mut notice_conflict = false;
        left.visit_notice_access(|a| {
            right.visit_notice_access(|b| {
                // 同系统发布者不持长期流独占借用；Reader 与发布者不能共存。
                notice_conflict |=
                    a.component == b.component && a.kind == b.kind && a.mode != b.mode;
            })
        });
        if notice_conflict {
            return true;
        }
        // 观察与任意同组件活值写业务排斥，不受实体过滤影响。
        if left.observations.iter().any(|(id, _)| {
            right
                .components
                .iter()
                .any(|(other, mode)| id == other && *mode == AccessMode::Write)
        }) || right.observations.iter().any(|(id, _)| {
            left.components
                .iter()
                .any(|(other, mode)| id == other && *mode == AccessMode::Write)
        }) {
            return true;
        }
        let empty = |scope: &crate::access::EntityScope| {
            scope.with.iter().any(|id| scope.without.contains(id))
        };
        let disjoint = empty(&left.scope)
            || empty(&right.scope)
            || left
                .scope
                .with
                .iter()
                .any(|id| right.scope.without.contains(id))
            || right
                .scope
                .with
                .iter()
                .any(|id| left.scope.without.contains(id));
        if disjoint {
            return false;
        }
        left.components.iter().any(|(id, mode)| {
            right.components.iter().any(|(other, other_mode)| {
                id == other && (*mode == AccessMode::Write || *other_mode == AccessMode::Write)
            }) || (*mode == AccessMode::Write && right.tick_reads.contains(id))
        }) || right
            .components
            .iter()
            .any(|(id, mode)| *mode == AccessMode::Write && left.tick_reads.contains(id))
    }
}

impl<'w> RunAccess<'w> {
    pub(crate) fn from_executor_inner(
        world: crate::world::WorldRef<'w>,
        declaration: &'w crate::access::AccessTree,
        archetype_version: u64,
    ) -> RunAccess<'w> {
        #[cfg(feature = "rc")]
        let world = crate::unsafe_world_cell::UnsafeWorldCell::from_mut(world);
        #[cfg(not(feature = "rc"))]
        let world = crate::unsafe_world_cell::UnsafeWorldCell::from_ref(world);
        RunAccess {
            world,
            declaration,
            archetype_version,
        }
    }
}

impl<'w> RunAccess<'w> {
    pub(crate) fn fetch_access_inner(
        _access: &RunAccess<'w>,
    ) -> Result<crate::query_fetch::FetchAccess<'w>, crate::error::EcsError> {
        let crate::access::AccessTree::Leaf(declaration) = _access.declaration else {
            return Err(authority_tree_error());
        };
        Ok(crate::query_fetch::FetchAccess {
            world: _access.world,
            declaration,
            window: None,
            write_tick: None,
        })
    }
}

impl<'w> RunAccess<'w> {
    pub(super) fn world_cell_inner(&self) -> crate::unsafe_world_cell::UnsafeWorldCell<'w> {
        self.world
    }
}

impl<'w> RunAccess<'w> {
    pub(super) fn child_inner(
        &self,
        _position: usize,
    ) -> Result<RunAccess<'_>, crate::error::EcsError> {
        let crate::access::AccessTree::Together(children) = self.declaration else {
            return Err(authority_tree_error());
        };
        let declaration = children.get(_position).ok_or_else(authority_tree_error)?;
        Ok(RunAccess {
            world: self.world.reborrow(),
            declaration,
            archetype_version: self.archetype_version,
        })
    }
    pub(super) fn context_child_inner(
        &self,
        _position: usize,
    ) -> Result<RunAccess<'w>, crate::error::EcsError> {
        let children = match self.declaration {
            crate::access::AccessTree::Together(children)
            | crate::access::AccessTree::Exclusive(children) => children,
            crate::access::AccessTree::Leaf(_) => return Err(authority_tree_error()),
        };
        let declaration = children.get(_position).ok_or_else(authority_tree_error)?;
        Ok(RunAccess {
            world: self.world,
            declaration,
            archetype_version: self.archetype_version,
        })
    }
}

#[cfg(feature = "rc")]
impl<'w> RunAccess<'w> {
    fn validate_query_leaf<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        &self,
        state: &crate::query::QueryState<Q, F>,
    ) -> Result<(), EcsError> {
        let declaration = self.fetch_access()?.declaration;
        let expected = state.declaration()?.to_access()?;
        if state.bound_world != Some(self.world.id())
            || state.tracking_unregistered
            || declaration.components != expected.components
            || declaration.tick_reads != expected.tick_reads
            || declaration.reads_entities != expected.reads_entities
            || declaration.scope.with != expected.scope.with
            || declaration.scope.without != expected.scope.without
            || !declaration.resources.is_empty()
            || !declaration.multi_resources.is_empty()
            || !declaration.observations.is_empty()
            || !matches!(declaration.structure, crate::access::StructuralAccess::None)
        {
            return Err(authority_tree_error());
        }
        Ok(())
    }

    fn validate_query_binding<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        &self,
        state: &crate::query::QueryState<Q, F>,
        window: crate::change_detection::ChangeWindow,
    ) -> Result<(), EcsError> {
        self.validate_query_leaf(state)?;
        if self.archetype_version != self.world.archetype_revision().0
            || window.last_run.is_some_and(|last| last > window.this_run)
        {
            return Err(authority_tree_error());
        }
        // A frozen RC world always has at least the base table. Empty cache is valid only
        // when every frozen table was excluded, which does not grant any entity access.
        Ok(())
    }

    pub(super) fn bind_query_inner<
        Q: crate::query::ReadOnlyQueryData,
        F: crate::query::QueryFilter,
    >(
        self,
        _state: &'w mut crate::query::QueryState<Q, F>,
        _window: crate::change_detection::ChangeWindow,
    ) -> Result<crate::query::Query<'w, Q, F>, EcsError> {
        self.validate_query_binding(_state, _window)?;
        Ok(crate::query::Query {
            window: Some(_window),
            access: self,
            state: _state,
        })
    }
    pub(super) fn bind_query_mut_inner<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        self,
        _state: &'w mut crate::query::QueryState<Q, F>,
        _window: crate::change_detection::ChangeWindow,
    ) -> Result<crate::query::QueryMut<'w, Q, F>, EcsError> {
        self.validate_query_binding(_state, _window)?;
        Ok(crate::query::QueryMut {
            window: Some(_window),
            write_tick: _window.this_run,
            access: self,
            state: _state,
        })
    }
    pub(super) fn bind_lookup_inner<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        self,
        _state: &'w mut crate::query::QueryState<Q, F>,
        _window: crate::change_detection::ChangeWindow,
    ) -> Result<crate::lookup::Lookup<'w, Q, F>, EcsError> {
        self.validate_query_binding(_state, _window)?;
        Ok(crate::lookup::Lookup {
            window: _window,
            access: self,
            state: _state,
        })
    }
    pub(super) fn bind_alter_inner<
        Q: crate::query::QueryData,
        F: crate::query::QueryFilter,
        Add: crate::bundle::SystemBundle<Shape = Add> + 'static,
        Remove: crate::bundle::Bundle,
    >(
        self,
        _state: &'w mut crate::alter::QueryAlterState<Q, F>,
        _window: crate::change_detection::ChangeWindow,
    ) -> Result<crate::alter::QueryAlter<'w, Q, F, Add, Remove>, EcsError> {
        self.validate_alter_leaf(_state)?;
        let epoch = self.world.archetype_revision().0;
        if self.archetype_version != epoch
            || _state.alteration.cache.layout_epoch != epoch
            || _window.last_run.is_some_and(|last| last > _window.this_run)
        {
            return Err(authority_tree_error());
        }
        Ok(crate::alter::QueryAlter {
            shape: std::marker::PhantomData,
            state: _state,
            access: self,
            window: _window,
        })
    }

    pub(super) fn bind_spawn_inner<B: crate::bundle::SystemBundle<Shape = B> + 'static>(
        self,
        state: &'w crate::spawn::SpawnState<B>,
        tick: crate::change_detection::ChangeTick,
    ) -> Result<crate::spawn::Spawn<'w, B>, EcsError> {
        use crate::access::StructuralAccess;
        let declaration = self.fetch_access()?.declaration;
        let mut expected = state.ordered_components.clone();
        expected.sort_unstable();
        if !self.world.mutable
            || self.world.id() != state.world_id
            || self.archetype_version != self.world.archetype_revision().0
            || declaration.reads_entities
            || !declaration.components.is_empty()
            || !declaration.tick_reads.is_empty()
            || !declaration.scope.with.is_empty()
            || !declaration.scope.without.is_empty()
            || !declaration.resources.is_empty()
            || !declaration.multi_resources.is_empty()
            || !declaration.observations.is_empty()
            || !matches!(&declaration.structure, StructuralAccess::Spawn { components, partition }
                if components == &expected && *partition == state.partition)
        {
            return Err(authority_tree_error());
        }
        Ok(crate::spawn::Spawn {
            state,
            access: self,
            tick,
        })
    }

    fn validate_alter_leaf<Q: crate::query::QueryData, F: crate::query::QueryFilter>(
        &self,
        state: &crate::alter::QueryAlterState<Q, F>,
    ) -> Result<(), EcsError> {
        use crate::access::StructuralAccess;
        let declaration = self.fetch_access()?.declaration;
        let expected = state.query.declaration()?.to_access()?;
        if !self.world.mutable
            || state.query.bound_world != Some(self.world.id())
            || state.alteration.cache.world_identity != self.world.id()
            || state.query.tracking_unregistered
            || declaration.components != expected.components
            || declaration.tick_reads != expected.tick_reads
            || declaration.reads_entities != expected.reads_entities
            || declaration.scope.with != expected.scope.with
            || declaration.scope.without != expected.scope.without
            || !declaration.resources.is_empty()
            || !declaration.multi_resources.is_empty()
            || !declaration.observations.is_empty()
            || !matches!(&declaration.structure, StructuralAccess::Alter { added, removed }
                if added == &state.alteration.added && removed == &state.alteration.removed)
        {
            return Err(authority_tree_error());
        }
        Ok(())
    }
}
