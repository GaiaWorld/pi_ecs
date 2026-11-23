#[cfg(all(test, feature = "rc"))]
mod rc_tracking_tests {
    use crate::{
        component::{Component, PlainAccess, StoragePolicy},
        notification::NoticeKind,
        tracking::ComponentTickDemand,
        world::World,
    };
    struct First;
    impl Component for First {
        type Access = PlainAccess;
    }
    struct Second;
    impl Component for Second {
        type Access = PlainAccess;
    }

    #[test]
    fn rc_tracking_exhaustion_stops_a_running_world_and_keeps_the_diagnostic() {
        let mut world = World::new().unwrap();
        let component = world
            .register_component::<First>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let _ = world.tracking.queries.insert(
            vec![ComponentTickDemand {
                component,
                added: true,
                changed: false,
            }]
            .into_boxed_slice(),
        );
        world.change_tick = u64::MAX;
        let result = world.schedule_scope(World::apply_tracking);
        assert!(world.is_stopped());
        assert_eq!(world.read_change_tick().get(), u64::MAX);
        assert!(
            !world
                .registry
                .component(component)
                .unwrap()
                .tracking
                .added_tick
        );
        assert!(format!("{:?}", result.unwrap_err()).contains("追踪维护时钟耗尽"));
        world.close().unwrap();
    }

    // Seam: World::apply_tracking；耗尽及物理准备错误不能发布需求或推进时间。
    #[test]
    fn rc_tracking_failure_atomicity_and_lifecycle_gates() {
        let mut world = World::new().unwrap();
        assert!(world.apply_tracking().is_err());
        let component = world
            .register_component::<First>(StoragePolicy::SparseOnly)
            .unwrap();
        world.finish_registration().unwrap();
        let registration = world.tracking.queries.insert(
            vec![ComponentTickDemand {
                component,
                added: true,
                changed: false,
            }]
            .into_boxed_slice(),
        );
        world.change_tick = u64::MAX;
        assert!(world.apply_tracking().is_err());
        assert_eq!(world.change_tick, u64::MAX);
        assert!(
            !world
                .registry
                .component(component)
                .unwrap()
                .tracking
                .added_tick
        );
        world.change_tick = 1;
        let storage = world.storage.take().unwrap();
        world.storage =
            Some(crate::storage::WorldStorage::new(Box::new([]), Box::new([])).unwrap());
        assert!(world.apply_tracking().is_err());
        assert_eq!(world.change_tick, 1);
        assert!(
            !world
                .registry
                .component(component)
                .unwrap()
                .tracking
                .added_tick
        );
        world.storage = Some(storage);
        world.apply_tracking().unwrap();
        let enabled = world.read_change_tick();
        world.change_tick = u64::MAX;
        world.apply_tracking().unwrap();
        world.tracking.queries.remove(registration).unwrap();
        world.apply_tracking().unwrap();
        assert_eq!(world.change_tick, u64::MAX);
        assert!(
            !world
                .registry
                .component(component)
                .unwrap()
                .tracking
                .added_tick
        );
        world.access.mode.set(crate::world::AccessMode::Maintaining);
        assert!(world.apply_tracking().is_err());
        assert!(matches!(
            world.access.mode.replace(crate::world::AccessMode::Stopped),
            crate::world::AccessMode::Maintaining
        ));
        assert!(world.apply_tracking().is_err());
        assert!(world.pin().is_err());
        world.close().unwrap();
        assert!(world.apply_tracking().is_err());
        assert!(enabled.get() > 1);
    }

    // Seam: World::apply_tracking；真实 World/订阅，需求登记作为查询入口的前置数据。
    #[test]
    fn rc_tracking_unions_independent_demands_and_restarts_without_notices_enabling_ticks() {
        let mut world = World::new().unwrap();
        let first = world
            .register_component::<First>(StoragePolicy::SparseOnly)
            .unwrap();
        let second = world
            .register_component::<Second>(StoragePolicy::TableEligible)
            .unwrap();
        world.finish_registration().unwrap();
        let reader = world
            .notifications
            .subscribe(second, NoticeKind::Changed)
            .unwrap();
        let added = world.tracking.queries.insert(
            vec![ComponentTickDemand {
                component: first,
                added: true,
                changed: false,
            }]
            .into_boxed_slice(),
        );
        let changed = world.tracking.queries.insert(
            vec![ComponentTickDemand {
                component: first,
                added: false,
                changed: true,
            }]
            .into_boxed_slice(),
        );
        let before = world.read_change_tick();
        world.apply_tracking().unwrap();
        assert!(world.read_change_tick() > before);
        let enabled = world.read_change_tick();
        let demand = world.registry.component(first).unwrap().tracking;
        assert!(demand.added_tick && demand.changed_tick);
        let other = world.registry.component(second).unwrap().tracking;
        assert!(other.changed_notice);
        assert!(!other.added_tick && !other.changed_tick);
        world.apply_tracking().unwrap();
        assert_eq!(world.read_change_tick(), enabled);
        world.tracking.queries.remove(added).unwrap();
        world.apply_tracking().unwrap();
        let demand = world.registry.component(first).unwrap().tracking;
        assert!(!demand.added_tick && demand.changed_tick);
        assert_eq!(world.read_change_tick(), enabled);
        world.tracking.queries.remove(changed).unwrap();
        world.apply_tracking().unwrap();
        assert!(
            !world
                .registry
                .component(first)
                .unwrap()
                .tracking
                .changed_tick
        );
        world.notifications.unsubscribe(reader).unwrap();
        world.apply_tracking().unwrap();
        assert!(
            !world
                .registry
                .component(second)
                .unwrap()
                .tracking
                .changed_notice
        );
        assert_eq!(world.read_change_tick(), enabled);
        let restarted = world.tracking.queries.insert(
            vec![ComponentTickDemand {
                component: first,
                added: false,
                changed: true,
            }]
            .into_boxed_slice(),
        );
        world
            .schedule_scope(|world| world.apply_tracking())
            .unwrap();
        assert!(world.read_change_tick() > enabled);
        world.tracking.queries.remove(restarted).unwrap();
        world.close().unwrap();
    }
}

#[cfg(feature = "rc")]
impl crate::world::World {
    pub(super) fn apply_tracking_inner(&mut self) -> Result<(), crate::error::EcsError> {
        use crate::{change_detection::TrackingDemand, world::AccessMode};
        let invalid = || {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidState,
                "追踪维护要求冻结、未关闭的静止点",
            )
        };
        let previous = self.access.mode.replace(AccessMode::Maintaining);
        if !matches!(previous, AccessMode::Idle | AccessMode::Running)
            || self.shutdown != crate::shutdown::ShutdownState::Open
            || !self.registry.frozen
            || self.storage.is_none()
        {
            self.access.mode.set(previous);
            return Err(invalid());
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut demands = vec![TrackingDemand::default(); self.registry.components.len()];
            for (_, query) in self.tracking.queries.iter() {
                for demand in query.iter() {
                    let target = demands
                        .get_mut(demand.component.0 as usize)
                        .ok_or_else(invalid)?;
                    target.added_tick |= demand.added;
                    target.changed_tick |= demand.changed;
                }
            }
            for (_, subscription) in self.notifications.subscriptions.iter() {
                let target = demands
                    .get_mut(subscription.stream.0 .0 as usize)
                    .ok_or_else(invalid)?;
                match subscription.stream.1 {
                    crate::notification::NoticeKind::Added => target.added_notice = true,
                    crate::notification::NoticeKind::Changed => target.changed_notice = true,
                    crate::notification::NoticeKind::Removed => target.removed_notice = true,
                }
            }
            let mut targets = Vec::new();
            let mut enabling = false;
            for (info, demand) in self.registry.components.iter().zip(&demands) {
                let old = info.tracking;
                enabling |= (!old.added_tick && demand.added_tick)
                    || (!old.changed_tick && demand.changed_tick);
                if old.added_tick != demand.added_tick || old.changed_tick != demand.changed_tick {
                    targets.push(crate::storage::TrackingTarget {
                        component: info.id,
                        added: demand.added_tick,
                        changed: demand.changed_tick,
                    });
                }
            }
            let next = if enabling {
                self.change_tick.checked_add(1).ok_or_else(|| {
                    if matches!(previous, AccessMode::Running) {
                        self.access.mode.set(AccessMode::Stopped);
                    }
                    pi_ecs_types::error::classified_error(
                        pi_ecs_types::error::ErrorKind::ResourceExhausted,
                        "追踪维护时钟耗尽",
                    )
                })?
            } else {
                self.change_tick
            };
            let tick = crate::change_detection::ChangeTick::new(
                std::num::NonZeroU64::new(next).expect("World 时钟非零"),
            );
            if !targets.is_empty() {
                self.storage
                    .as_mut()
                    .ok_or_else(invalid)?
                    .apply_tracking(&targets, tick)?;
            }
            // 物理准备与提交成功后才公开有效需求及维护时间。
            for (info, demand) in self.registry.components.iter_mut().zip(demands) {
                info.tracking = demand;
            }
            self.change_tick = next;
            Ok(())
        }));
        match result {
            Ok(result) => {
                if !self.is_stopped() {
                    self.access.mode.set(previous);
                }
                result
            }
            Err(payload) => {
                self.access.mode.set(AccessMode::Stopped);
                std::panic::resume_unwind(payload)
            }
        }
    }
}
