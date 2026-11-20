//! 变化追踪配对芯。
#[cfg(all(test, feature = "rc"))]
mod host_registry_resource_batch_mut {
    use super::*;
    use crate::change_detection::{ChangeMarker, ChangeTarget, ChangeTick};
    use crate::resource::ResourceTicks;

    fn tick(value: u64) -> ChangeTick {
        ChangeTick::new(std::num::NonZeroU64::new(value).unwrap())
    }

    // Seam: 显式标记支持内部可变性，资源及多例保持 added。
    #[test]
    fn host_registry_resource_batch_mut_explicit_resource_and_multi() {
        for multi in [false, true] {
            let mut value = std::cell::Cell::new(7);
            let mut ticks = ResourceTicks {
                added: tick(1),
                changed: tick(2),
            };
            {
                let target = if multi {
                    ChangeTarget::Multi(&mut ticks)
                } else {
                    ChangeTarget::Resource(&mut ticks)
                };
                let mut access = Mut {
                    value: &mut value,
                    marker: ChangeMarker {
                        tick: tick(3),
                        target,
                    },
                    marked: false,
                };
                access.set(9);
                access.set_changed();
                access.set_changed();
            }
            assert_eq!(value.get(), 9);
            assert_eq!(ticks.added, tick(1));
            assert_eq!(ticks.changed, tick(3));
        }
    }

    // Seam: Mut 写访问及显式标记，经真实 Notifications 读取验证通知与去重。
    #[test]
    fn host_registry_resource_batch_mut_component_notifications() {
        use crate::change_detection::{NoticeWrite, TrackingDemand};
        use crate::component::ComponentId;
        use crate::notification::{NoticeKind, Notifications};
        use pi_ecs_types::change_detection::ComponentTicksMut;
        for tracked in [false, true] {
            for explicit_first in [false, true] {
                let mut notifications = Notifications {
                    subscriptions: pi_slotmap::SlotMap::with_key(),
                    components: Vec::new(),
                    layouts: vec![Some(std::alloc::Layout::new::<u32>())],
                    type_ids: vec![Some(std::any::TypeId::of::<u32>())],
                };
                let state = crate::world::AccessState {
                    mode: std::cell::Cell::new(crate::world::AccessMode::Maintaining),
                };
                let component = ComponentId(0);
                let entity = Default::default();
                let mut reader = notifications
                    .subscribe(component, NoticeKind::Changed)
                    .unwrap();
                let demand = TrackingDemand {
                    changed_tick: tracked,
                    changed_notice: true,
                    ..Default::default()
                };
                let mut value = 7;
                let mut added = Some(tick(1));
                let mut changed = Some(tick(2));
                for expected in [9, 11] {
                    {
                        let mut access = Mut {
                            value: &mut value,
                            marker: ChangeMarker {
                                tick: tick(3),
                                target: ChangeTarget::Component {
                                    ticks: ComponentTicksMut {
                                        added: Some(&mut added),
                                        changed: if tracked { Some(&mut changed) } else { None },
                                    },
                                    demand: &demand,
                                    notice: Some(NoticeWrite {
                                        notifications: &notifications,
                                        access: &state,
                                        component,
                                        entity,
                                        borrow: std::marker::PhantomData,
                                    }),
                                },
                            },
                            marked: false,
                        };
                        let _read = *access;
                        assert!(notifications.is_empty(&reader).unwrap());
                        if explicit_first {
                            access.set_changed();
                        }
                        // 取得可写引用时通知已发布，无需等到实际赋值或包装析构。
                        let writable = &mut *access;
                        assert!(!notifications.is_empty(&reader).unwrap());
                        *writable = expected;
                        *access += 1;
                        access.set_changed();
                        assert_eq!(*access, expected + 1);
                    }
                    assert_eq!(value, expected + 1);
                    assert_eq!(added, Some(tick(1)));
                    assert_eq!(changed, Some(tick(if tracked { 3 } else { 2 })));
                    let mut batch = notifications.read(&mut reader).unwrap();
                    let notice = batch.next().expect("每个 Mut 发布一次 Changed");
                    assert_eq!(notice.entity, entity);
                    assert_eq!(notice.tick, tick(3));
                    assert!(batch.next().is_none(), "同一 Mut 的混合标记不得重复发布");
                }
            }
        }
    }

    // Seam: Mut 的公共 Deref；只读访问不能产生变化。
    #[test]
    fn host_registry_resource_batch_mut_read_does_not_mark() {
        let mut value = 7;
        let mut ticks = ResourceTicks {
            added: tick(1),
            changed: tick(2),
        };
        {
            let access = Mut {
                value: &mut value,
                marker: ChangeMarker {
                    tick: tick(3),
                    target: ChangeTarget::Resource(&mut ticks),
                },
                marked: false,
            };
            assert_eq!(*access, 7);
        }
        assert_eq!(ticks.added, tick(1));
        assert_eq!(ticks.changed, tick(2));
        assert_eq!(value, 7);
    }
}
use crate::change_detection::Mut;
impl crate::change_detection::NoticeWrite<'_> {
    fn append_changed(&self, tick: crate::change_detection::ChangeTick) {
        #[cfg(feature = "rc")]
        let guard = StopOnNoticeFailure(self.access);
        self.notifications.append_change(
            self.component,
            crate::notification::NoticeKind::Changed,
            self.entity,
            tick,
        );
        #[cfg(feature = "rc")]
        std::mem::forget(guard);
    }
}

#[cfg(feature = "rc")]
struct StopOnNoticeFailure<'a>(&'a crate::world::AccessState);

#[cfg(feature = "rc")]
impl Drop for StopOnNoticeFailure<'_> {
    fn drop(&mut self) {
        self.0.mode.set(crate::world::AccessMode::Stopped);
    }
}

impl crate::change_detection::ChangeMarker<'_> {
    pub(super) fn marker_set_changed_inner(&mut self, _marked: &mut bool) {
        if *_marked {
            return;
        }
        match &mut self.target {
            crate::change_detection::ChangeTarget::Component {
                ticks,
                demand,
                notice,
            } => {
                if demand.changed_tick {
                    *ticks.changed.as_deref_mut().expect("已启用 changed 槽") = Some(self.tick);
                }
                if demand.changed_notice {
                    let notice = notice.as_ref().expect("已启用 Changed 通知令牌");
                    notice.append_changed(self.tick);
                }
            }
            crate::change_detection::ChangeTarget::Resource(ticks)
            | crate::change_detection::ChangeTarget::Multi(ticks) => ticks.changed = self.tick,
        }
        *_marked = true;
    }
}
impl<T> Mut<'_, T> {
    pub(super) fn is_added_inner(&self, _window: crate::change_detection::ChangeWindow) -> bool {
        let tick = match &self.marker.target {
            crate::change_detection::ChangeTarget::Component { ticks, .. } => {
                ticks.added.as_deref().copied().flatten()
            }
            crate::change_detection::ChangeTarget::Resource(ticks)
            | crate::change_detection::ChangeTarget::Multi(ticks) => Some(ticks.added),
        };
        tick.is_some_and(|tick| {
            tick <= _window.this_run && _window.last_run.is_none_or(|last| tick > last)
        })
    }
    pub(super) fn is_changed_inner(&self, _window: crate::change_detection::ChangeWindow) -> bool {
        let tick = match &self.marker.target {
            crate::change_detection::ChangeTarget::Component { ticks, .. } => {
                ticks.changed.as_deref().copied().flatten()
            }
            crate::change_detection::ChangeTarget::Resource(ticks)
            | crate::change_detection::ChangeTarget::Multi(ticks) => Some(ticks.changed),
        };
        tick.is_some_and(|tick| {
            tick <= _window.this_run && _window.last_run.is_none_or(|last| tick > last)
        })
    }
}
impl<T> Mut<'_, T> {
    pub(crate) fn set_changed_inner(_value: &mut Mut<'_, T>) {
        _value.marker.set_changed(&mut _value.marked);
    }
}
impl<T> Mut<'_, T> {
    pub(crate) fn deref_inner<'a>(_value: &'a Mut<'_, T>) -> &'a T {
        _value.value
    }
}
impl<T> Mut<'_, T> {
    pub(crate) fn deref_mut_inner<'a>(_value: &'a mut Mut<'_, T>) -> &'a mut T {
        Self::set_changed_inner(_value);
        _value.value
    }
}
