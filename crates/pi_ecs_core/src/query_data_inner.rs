//! 查询数据配对芯。
#[cfg(test)]
mod value_ref_tests {
    use crate::{
        change_detection::{ChangeTick, ChangeWindow},
        query_data::ValueRef,
    };
    fn tick(value: u64) -> ChangeTick {
        ChangeTick::new(std::num::NonZeroU64::new(value).unwrap())
    }
    // Seam: is_added 的首次窗口、左开右闭边界和默认来源。
    #[test]
    fn value_ref_added_window_and_default_source() {
        let value = 7;
        for (last, added, expected) in [
            (None, 1, true),
            (None, 9, true),
            (None, 10, false),
            (Some(4), 3, false),
            (Some(4), 4, false),
            (Some(4), 5, true),
            (Some(4), 9, true),
            (Some(4), 10, false),
            (Some(9), 9, false),
        ] {
            let view = ValueRef {
                value: &value,
                ticks: Some((tick(added), tick(2))),
                window: ChangeWindow {
                    last_run: last.map(tick),
                    this_run: tick(9),
                },
            };
            assert_eq!(view.is_added(), expected);
            assert_eq!(view.is_added(), expected);
        }
        let default = ValueRef {
            value: &value,
            ticks: None,
            window: ChangeWindow {
                last_run: None,
                this_run: tick(9),
            },
        };
        assert!(!default.is_added());
    }
    // Seam: is_changed 独立读取 changed 快照，不使用 added，不推进窗口。
    #[test]
    fn value_ref_changed_window_and_default_source() {
        let value = 11;
        for (last, changed, expected) in [
            (None, 1, true),
            (None, 9, true),
            (None, 10, false),
            (Some(4), 3, false),
            (Some(4), 4, false),
            (Some(4), 5, true),
            (Some(4), 9, true),
            (Some(4), 10, false),
            (Some(9), 9, false),
        ] {
            let view = ValueRef {
                value: &value,
                ticks: Some((tick(10), tick(changed))),
                window: ChangeWindow {
                    last_run: last.map(tick),
                    this_run: tick(9),
                },
            };
            assert_eq!(view.is_changed(), expected);
            assert_eq!(view.is_changed(), expected);
            assert!(!view.is_added());
        }
        let default = ValueRef {
            value: &value,
            ticks: None,
            window: ChangeWindow {
                last_run: None,
                this_run: tick(9),
            },
        };
        assert!(!default.is_changed());
    }
    // Seam: ValueRef 的公开 Deref 只读重借用，不要求值可复制。
    #[test]
    fn value_ref_deref_preserves_original_value() {
        let value = String::from("query value");
        let view = ValueRef {
            value: &value,
            ticks: None,
            window: ChangeWindow {
                last_run: None,
                this_run: tick(9),
            },
        };
        assert_eq!(view.as_str(), "query value");
        assert!(std::ptr::eq::<String>(&*view, &value));
    }
}
#[cfg(all(test, feature = "rc"))]
mod default_tests {
    use crate::{
        component::{Component, PlainAccess, StoragePolicy},
        world::World,
    };
    #[derive(Debug, PartialEq)]
    struct DefaultValue(u32);
    impl Component for DefaultValue {
        type Access = PlainAccess;
    }
    // Seam: 宿主默认值登记和替换，不隐式登记组件，不分配变化 tick。
    #[test]
    fn query_default_replacement_and_rejected_run_preserve_value() {
        let mut world = World::new().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert!(world.set_query_default(DefaultValue(3)).is_err());
            world
                .register_component::<DefaultValue>(StoragePolicy::SparseOnly)
                .unwrap();
            assert_eq!(world.set_query_default(DefaultValue(7)).unwrap(), None);
            world.finish_registration().unwrap();
            let before = world.read_change_tick();
            world
                .schedule_scope(|world| {
                    assert!(world.set_query_default(DefaultValue(11)).is_err());
                    Ok(())
                })
                .unwrap();
            assert_eq!(
                world.set_query_default(DefaultValue(13)).unwrap(),
                Some(DefaultValue(7))
            );
            assert_eq!(
                world.set_query_default(DefaultValue(17)).unwrap(),
                Some(DefaultValue(13))
            );
            assert_eq!(world.read_change_tick(), before);
        }));
        world.close().unwrap();
        assert!(world.set_query_default(DefaultValue(19)).is_err());
        if let Err(error) = result {
            std::panic::resume_unwind(error);
        }
    }
}
use super::ValueRef;
impl<T> ValueRef<'_, T> {
    pub(super) fn value_inner<'a>(_v: &'a ValueRef<'_, T>) -> &'a T {
        _v.value
    }
}
impl<T> ValueRef<'_, T> {
    pub(super) fn added_inner(_v: &ValueRef<'_, T>) -> bool {
        _v.ticks.is_some_and(|(added, _)| {
            added <= _v.window.this_run && _v.window.last_run.is_none_or(|last| added > last)
        })
    }
}
impl<T> ValueRef<'_, T> {
    pub(super) fn changed_inner(_v: &ValueRef<'_, T>) -> bool {
        _v.ticks.is_some_and(|(_, changed)| {
            changed <= _v.window.this_run && _v.window.last_run.is_none_or(|last| changed > last)
        })
    }
}
impl crate::world::World {
    pub(super) fn set_default_inner<
        T: crate::component::Component<Access = crate::component::PlainAccess>,
    >(
        _world: &mut crate::world::World,
        _value: T,
    ) -> Result<Option<T>, crate::error::EcsError> {
        #[cfg(not(feature = "rc"))]
        {
            let _ = (_world, _value);
            Err(pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "非 RC 默认值维护授权尚未接通",
            ))
        }
        #[cfg(feature = "rc")]
        {
            let mode = _world
                .access
                .mode
                .replace(crate::world::AccessMode::Stopped);
            let idle = matches!(mode, crate::world::AccessMode::Idle);
            _world.access.mode.set(mode);
            if !idle || _world.shutdown != crate::shutdown::ShutdownState::Open {
                return Err(pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::InvalidState,
                    "默认值维护要求开放的 Idle World",
                ));
            }
            let ty = std::any::TypeId::of::<T>();
            if !_world.registry.component_ids.contains_key(&ty) {
                return Err(pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::NotFound,
                    "默认值组件类型尚未登记",
                ));
            }
            if _world
                .query_defaults
                .values
                .get(&ty)
                .is_some_and(|old| !old.is::<T>())
            {
                return Err(pi_ecs_types::error::classified_error(
                    pi_ecs_types::error::ErrorKind::InvalidState,
                    "默认值目录类型不一致",
                ));
            }
            // 独占字段借用；所有普通拒绝先完成，展开时保持停止态。
            _world.access.mode.set(crate::world::AccessMode::Stopped);
            let old = _world.query_defaults.values.insert(ty, Box::new(_value));
            let old = old.map(|old| {
                *old.downcast::<T>()
                    .unwrap_or_else(|_| unreachable!("已核对默认值类型"))
            });
            _world.access.mode.set(crate::world::AccessMode::Idle);
            Ok(old)
        }
    }
}

impl<'w, Q, C: crate::component::Component> super::ComponentFetch<'w, Q, C> {
    pub(super) fn change_marker_inner(
        &self,
        _entity: &crate::query_fetch::FetchEntity,
        _ticks: pi_ecs_types::change_detection::ComponentTicksMut<'w>,
    ) -> crate::change_detection::ChangeMarker<'w> {
        use crate::change_detection::{ChangeMarker, ChangeTarget, NoticeWrite};
        let tick = self.access.write_tick.expect("写访问缺少时钟");
        assert_eq!(_ticks.added.is_some(), self.demand.added_tick);
        assert_eq!(_ticks.changed.is_some(), self.demand.changed_tick);
        assert_eq!(self.notifications.is_some(), self.demand.changed_notice);
        let notice = self.notifications.map(|notifications| {
            let component = unsafe {
                let registry = &*std::ptr::addr_of!((*self.access.world.pointer.as_ptr()).registry);
                *registry
                    .component_ids
                    .get(&std::any::TypeId::of::<C>())
                    .expect("写标记缺少组件登记")
            };
            NoticeWrite {
                notifications,
                // SAFETY: the fetch's original authorization keeps this disjoint field
                // alive for the same lifetime as the notification directory.
                #[cfg(feature = "rc")]
                access: unsafe {
                    &*std::ptr::addr_of!((*self.access.world.pointer.as_ptr()).access)
                },
                component,
                entity: _entity.entity,
                borrow: std::marker::PhantomData,
            }
        });
        ChangeMarker {
            tick,
            target: ChangeTarget::Component {
                ticks: _ticks,
                demand: self.demand,
                notice,
            },
        }
    }
}
