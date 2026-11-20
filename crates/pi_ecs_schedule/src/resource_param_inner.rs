//! 资源参数值访问配对芯。
#[cfg(all(test, feature = "rc"))]
mod rc_resource_value_tests {
    use crate::resource_param::{MultiRes, MultiResMut, Res, ResMut};
    use pi_ecs_core::{
        access::*, change_detection::ChangeWindow, resource::ResourceId,
        unsafe_world_cell::UnsafeWorldCell, world::World,
    };

    fn leaf(
        resources: Vec<(ResourceId, AccessMode)>,
        multi_resources: Vec<(ResourceId, MultiResourceAccess)>,
    ) -> AccessTree {
        AccessTree::Leaf(AccessDeclaration {
            resources: resources.into_boxed_slice(),
            multi_resources: multi_resources.into_boxed_slice(),
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

    // Seam: 真实生产槽写入后通过 MultiRes 公开遍历，保留值/tick 配对和结构观察语义。
    #[test]
    fn rc_resource_value_multi_write_and_read_without_consuming() {
        let mut world = World::new().unwrap();
        let id = world.register_multi_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        let (first, second) = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|mut scope| {
                    Ok((
                        scope.register_producer(7u32)?,
                        scope.register_producer(11u32)?,
                    ))
                })
            })
            .unwrap();
        let binding = world.producer_binding(&first).unwrap();
        let writes = leaf(
            vec![],
            vec![(
                id,
                MultiResourceAccess::WriteSlot {
                    producer: first.producer_id(),
                },
            )],
        );
        let reads = leaf(vec![], vec![(id, MultiResourceAccess::ReadAll)]);
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                let last = world.read_change_tick();
                let tick = unsafe { world.next_system_tick()? };
                // 两种写入口分别使用新 Mut，显式标记不能被先前 DerefMut 掩盖。
                // SAFETY: 单槽只兑现一次；写会话结束后才开始全体只读会话。
                unsafe {
                    world.system_scope(&writes, version, |access| {
                        let mut resource = MultiResMut {
                            value: access.producer_mut::<u32>(&first, &binding, tick).unwrap(),
                            window: ChangeWindow {
                                last_run: Some(last),
                                this_run: tick,
                            },
                        };
                        assert_eq!(*resource, 7);
                        assert!(!resource.is_changed());
                        *resource = 19;
                        resource.set_changed();
                        assert!(resource.is_changed());
                        assert!(!resource.is_added());
                    })?;
                }
                let explicit_tick = unsafe { world.next_system_tick()? };
                unsafe {
                    world.system_scope(&writes, version, |access| {
                        let mut resource = MultiResMut {
                            value: access
                                .producer_mut::<u32>(&first, &binding, explicit_tick)
                                .unwrap(),
                            window: ChangeWindow {
                                last_run: Some(tick),
                                this_run: explicit_tick,
                            },
                        };
                        assert!(!resource.is_changed());
                        resource.set_changed();
                        assert!(resource.is_changed());
                        assert_eq!(*resource, 19);
                    })?;
                }
                let tick = explicit_tick;
                unsafe {
                    world.system_scope(&reads, version, |access| {
                        let view = access.multi_resource_parts::<u32>(id).unwrap();
                        let mut resources = MultiRes {
                            values: view.values,
                            ticks: view.ticks,
                            structure_version: view.structure_version,
                            previous_structure_version: None,
                            window: ChangeWindow {
                                last_run: Some(last),
                                this_run: tick,
                            },
                        };
                        assert_eq!(resources.len(), 2);
                        assert!(!resources.is_empty());
                        assert!(resources.is_structure_changed());
                        let mut iter = resources.iter();
                        let a = iter.next().unwrap();
                        let b = iter.next().unwrap();
                        assert_eq!((*a, *b), (19, 11));
                        assert!(a.is_changed());
                        assert!(!b.is_changed());
                        assert!(iter.next().is_none());
                        assert!(iter.next().is_none());
                        assert_eq!(
                            resources.iter().map(|value| *value).collect::<Vec<_>>(),
                            [19, 11]
                        );
                        resources.previous_structure_version = Some(view.structure_version);
                        assert!(!resources.is_structure_changed());
                        resources.previous_structure_version = Some(view.structure_version - 1);
                        assert!(resources.is_structure_changed());
                    })?;
                }
                Ok(())
            })
            .unwrap();
        world
            .parameter_unregistration_scope(|mut scope| {
                scope.unregister_producer::<u32>(first)?;
                scope.unregister_producer::<u32>(second)
            })
            .unwrap();
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                let tick = unsafe { world.next_system_tick()? };
                // SAFETY: 仅兑现空集合只读叶子。
                unsafe {
                    world.system_scope(&reads, version, |access| {
                        let view = access.multi_resource_parts::<u32>(id).unwrap();
                        let resources = MultiRes {
                            values: view.values,
                            ticks: view.ticks,
                            structure_version: view.structure_version,
                            previous_structure_version: Some(binding.structure_version),
                            window: ChangeWindow {
                                last_run: None,
                                this_run: tick,
                            },
                        };
                        assert_eq!(resources.len(), 0);
                        assert!(resources.is_empty());
                        assert!(resources.iter().next().is_none());
                        assert!(resources.is_structure_changed());
                    })?;
                }
                Ok(())
            })
            .unwrap();
        world.close().unwrap();
    }

    // Seam: ResMut 公开读写与显式标记，不把只读访问算成变化。
    #[test]
    fn rc_resource_value_unique_mutation_marks_only_on_write() {
        let mut world = World::new().unwrap();
        let id = world.register_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        world.insert_resource(17u32).unwrap();
        let last = world.read_change_tick();
        let declaration = leaf(vec![(id, AccessMode::Write)], vec![]);
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                let tick = unsafe { world.next_system_tick()? };
                // SAFETY: 单一独占叶子，仅兑现一次，包装不逃出会话。
                unsafe {
                    world.system_scope(&declaration, version, |access| {
                        let mut resource = ResMut {
                            value: access.resource_mut::<u32>(id, tick).unwrap().unwrap(),
                            window: ChangeWindow {
                                last_run: Some(last),
                                this_run: tick,
                            },
                        };
                        assert_eq!(*resource, 17);
                        assert!(!resource.is_changed());
                        assert!(!resource.is_added());
                        *resource = 23;
                        assert!(resource.is_changed());
                        resource.set_changed();
                        assert_eq!(*resource, 23);
                        assert!(!resource.is_added());
                    })?;
                }
                let explicit_tick = unsafe { world.next_system_tick()? };
                unsafe {
                    world.system_scope(&declaration, version, |access| {
                        let mut resource = ResMut {
                            value: access
                                .resource_mut::<u32>(id, explicit_tick)
                                .unwrap()
                                .unwrap(),
                            window: ChangeWindow {
                                last_run: Some(tick),
                                this_run: explicit_tick,
                            },
                        };
                        assert!(!resource.is_changed());
                        resource.set_changed();
                        assert!(resource.is_changed());
                        assert_eq!(*resource, 23);
                        assert!(!resource.is_added());
                    })?;
                }
                Ok(())
            })
            .unwrap();
        world.close().unwrap();
    }

    // Seam: Res 的公开只读与变化窗口接口；数据来自真实 World 运行授权。
    #[test]
    fn rc_resource_value_read_window_boundaries() {
        let mut world = World::new().unwrap();
        let id = world.register_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        world.insert_resource(17u32).unwrap();
        let declaration = leaf(vec![(id, AccessMode::Read)], vec![]);
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                let tick = unsafe { world.next_system_tick()? };
                // SAFETY: 单一读叶子；不重取可写引用，所有包装在本次会话内释放。
                unsafe {
                    world.system_scope(&declaration, version, |access| {
                        let (value, ticks) = access.resource_parts::<u32>(id).unwrap().unwrap();
                        let resource = Res {
                            value,
                            ticks,
                            window: ChangeWindow {
                                last_run: None,
                                this_run: tick,
                            },
                        };
                        assert_eq!(*resource, 17);
                        assert!(resource.is_added());
                        assert!(resource.is_changed());
                        let observed = Res {
                            value,
                            ticks,
                            window: ChangeWindow {
                                last_run: Some(ticks.changed),
                                this_run: tick,
                            },
                        };
                        assert!(!observed.is_added());
                        assert!(!observed.is_changed());
                        let inclusive = Res {
                            value,
                            ticks,
                            window: ChangeWindow {
                                last_run: None,
                                this_run: ticks.changed,
                            },
                        };
                        assert!(inclusive.is_changed());
                    })?;
                }
                Ok(())
            })
            .unwrap();
        world.close().unwrap();
    }
}

impl<'a, T: Send + Sync + 'static> crate::resource_param::MultiResIter<'a, T> {
    pub(crate) fn multi_next(
        _iter: &mut crate::resource_param::MultiResIter<'a, T>,
    ) -> Option<crate::resource_param::Res<'a, T>> {
        let value = _iter.source.values.get(_iter.offset)?;
        let ticks = &_iter.source.ticks[_iter.offset];
        _iter.offset += 1;
        Some(crate::resource_param::Res {
            value,
            ticks,
            window: _iter.source.window,
        })
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::MultiRes<'_, T> {
    pub(crate) fn multi_len(_value: &crate::resource_param::MultiRes<'_, T>) -> usize {
        _value.values.len()
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::MultiRes<'_, T> {
    pub(crate) fn multi_empty(_value: &crate::resource_param::MultiRes<'_, T>) -> bool {
        _value.values.is_empty()
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::MultiRes<'_, T> {
    pub(crate) fn multi_iter<'a>(
        _value: &'a crate::resource_param::MultiRes<'_, T>,
    ) -> crate::resource_param::MultiResIter<'a, T> {
        crate::resource_param::MultiResIter {
            source: _value,
            offset: 0,
        }
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::MultiRes<'_, T> {
    pub(crate) fn multi_structure(_value: &crate::resource_param::MultiRes<'_, T>) -> bool {
        _value.previous_structure_version != Some(_value.structure_version)
    }
}

impl<T: Send + Sync + 'static> crate::resource_param::Res<'_, T> {
    pub(crate) fn res_deref<'a>(_value: &'a crate::resource_param::Res<'_, T>) -> &'a T {
        _value.value
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::Res<'_, T> {
    pub(crate) fn res_is_added(_value: &crate::resource_param::Res<'_, T>) -> bool {
        _value.ticks.added <= _value.window.this_run
            && _value
                .window
                .last_run
                .is_none_or(|last| _value.ticks.added > last)
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::Res<'_, T> {
    pub(crate) fn res_is_changed(_value: &crate::resource_param::Res<'_, T>) -> bool {
        _value.ticks.changed <= _value.window.this_run
            && _value
                .window
                .last_run
                .is_none_or(|last| _value.ticks.changed > last)
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::ResMut<'_, T> {
    pub(crate) fn resmut_deref<'a>(_value: &'a crate::resource_param::ResMut<'_, T>) -> &'a T {
        &_value.value
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::ResMut<'_, T> {
    pub(crate) fn resmut_deref_mut<'a>(
        _value: &'a mut crate::resource_param::ResMut<'_, T>,
    ) -> &'a mut T {
        &mut _value.value
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::ResMut<'_, T> {
    pub(crate) fn resmut_set_changed(_value: &mut crate::resource_param::ResMut<'_, T>) {
        _value.value.set_changed()
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::MultiResMut<'_, T> {
    pub(crate) fn multiresmut_deref<'a>(
        _value: &'a crate::resource_param::MultiResMut<'_, T>,
    ) -> &'a T {
        &_value.value
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::MultiResMut<'_, T> {
    pub(crate) fn multiresmut_deref_mut<'a>(
        _value: &'a mut crate::resource_param::MultiResMut<'_, T>,
    ) -> &'a mut T {
        &mut _value.value
    }
}
impl<T: Send + Sync + 'static> crate::resource_param::MultiResMut<'_, T> {
    pub(crate) fn multiresmut_set_changed(_value: &mut crate::resource_param::MultiResMut<'_, T>) {
        _value.value.set_changed()
    }
}
