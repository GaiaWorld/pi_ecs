//! 参数集借用配对芯。
#[cfg(all(test, feature = "rc"))]
mod tests {
    use super::*;
    use crate::{
        system::SystemRunContext,
        system_param::{ParamRegistration, SystemParam},
    };
    use pi_ecs_core::{
        access::*, change_detection::ChangeWindow, error::EcsError, resource::ResourceId,
        unsafe_world_cell::UnsafeWorldCell, world::World,
    };

    struct Counter;
    struct State {
        id: ResourceId,
        rebinds: usize,
    }
    // SAFETY: 声明覆盖唯一资源写，fetch 只兑现一次短借槽，不缓存业务引用。
    unsafe impl SystemParam for Counter {
        type Config = ResourceId;
        type State = State;
        type Pending = usize;
        type Item<'r> = pi_ecs_core::change_detection::Mut<'r, u32>;
        fn initialize(id: ResourceId, _: &mut ParamRegistration<'_>) -> Result<State, EcsError> {
            Ok(State { id, rebinds: 0 })
        }
        fn prepare(
            _: &mut State,
            _: &mut World,
        ) -> Result<crate::param_composition::ParamAccess, EcsError> {
            unreachable!("本切片仅调用 ParamSet 运行短借 seam")
        }
        fn declaration(_: &State, _: &mut World) -> Result<AccessTree, EcsError> {
            unreachable!()
        }
        fn ordering_access(
            _: &State,
            _: &OrderingArchetypes,
        ) -> Result<Vec<OrderingAccess>, EcsError> {
            unreachable!()
        }
        fn begin(_: &mut State) -> usize {
            0
        }
        unsafe fn fetch<'r>(
            state: &'r mut State,
            pending: &'r mut usize,
            context: SystemRunContext<'r>,
        ) -> Self::Item<'r> {
            *pending += 1;
            let (access, window) = context.into_parts();
            unsafe {
                access
                    .resource_mut::<u32>(state.id, window.this_run)
                    .unwrap()
                    .unwrap()
            }
        }
        fn finish(_: &mut State, _: usize, _: &SystemRunContext<'_>) {}
        fn unregister(_: State, _: &mut ParamRegistration<'_>) -> Result<(), EcsError> {
            Ok(())
        }
    }
    // SAFETY: 每次切换只复核该叶子资源，不持旧结果或扩大权限。
    unsafe impl SequentialParam for Counter {
        unsafe fn rebind<'r>(
            state: &mut State,
            context: SystemRunContext<'r>,
        ) -> Result<SystemRunContext<'r>, EcsError> {
            state.rebinds += 1;
            Ok(context)
        }
    }
    // Seam: ParamSetItem::p0 反复短借同一真实资源，必须每次重绑。
    #[test]
    fn rc_paramset_first_rebinds_on_every_borrow() {
        exercise(false);
    }
    // Seam: p0→p1→p0 的冲突资源写按短借顺序立即可见。
    #[test]
    fn rc_paramset_second_observes_first_and_rebinds() {
        exercise(true);
    }
    fn exercise(second: bool) {
        let mut world = World::new().unwrap();
        let id = world.register_resource::<u32>().unwrap();
        world.finish_registration().unwrap();
        world.insert_resource(7u32).unwrap();
        let leaf = || {
            AccessTree::Leaf(AccessDeclaration {
                resources: vec![(id, AccessMode::Write)].into_boxed_slice(),
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
        };
        let tree = AccessTree::Exclusive(vec![leaf(), leaf()]);
        let mut first = State { id, rebinds: 0 };
        let mut last = State { id, rebinds: 0 };
        let mut a = 0;
        let mut b = 0;
        world
            .schedule_scope(|world| {
                let version = UnsafeWorldCell::from_ref(world).archetype_revision().0;
                let tick = unsafe { world.next_system_tick()? };
                // SAFETY: Exclusive 各成员仅经 set 的独占短借兑现；结果不跨切换。
                unsafe {
                    world.system_scope(&tree, version, |access| {
                        let context = SystemRunContext::new(
                            access,
                            ChangeWindow {
                                last_run: None,
                                this_run: tick,
                            },
                        )
                        .unwrap();
                        let mut set = ParamSetItem::<Counter, Counter> {
                            first: &mut first,
                            second: &mut last,
                            first_pending: &mut a,
                            second_pending: &mut b,
                            context,
                        };
                        {
                            let mut value = set.p0();
                            assert_eq!(*value, 7);
                            *value = 19;
                        }
                        if second {
                            {
                                let mut value = set.p1();
                                assert_eq!(*value, 19);
                                *value = 23;
                            }
                            assert_eq!(*set.p0(), 23);
                        } else {
                            assert_eq!(*set.p0(), 19);
                        }
                    })?;
                }
                Ok(())
            })
            .unwrap();
        assert_eq!((first.rebinds, a), (2, 2));
        assert_eq!((last.rebinds, b), if second { (1, 1) } else { (0, 0) });
        assert_eq!(
            world
                .read_scope(|pin| pin.resource::<u32>().copied())
                .unwrap(),
            Some(if second { 23 } else { 19 })
        );
        world.close().unwrap();
    }
}
use crate::param_composition::{ParamSetItem, SequentialParam};
impl<A: SequentialParam, B: SequentialParam> ParamSetItem<'_, '_, A, B> {
    pub(crate) fn p0_inner<'a>(_set: &'a mut ParamSetItem<'_, '_, A, B>) -> A::Item<'a> {
        // SAFETY: 独占短借 set 同时缩短状态、工作区与子描述的寿命；
        // Rust 借用阻止旧结果存活时切换成员，不保留任何物理视图。
        unsafe {
            let context = _set
                .context
                .exclusive_child(0)
                .expect("ParamSet 第一成员授权无效");
            let context = A::rebind(&mut *_set.first, context).expect("ParamSet 第一成员重绑失败");
            A::fetch(&mut *_set.first, &mut *_set.first_pending, context)
        }
    }
}
impl<A: SequentialParam, B: SequentialParam> ParamSetItem<'_, '_, A, B> {
    pub(crate) fn p1_inner<'a>(_set: &'a mut ParamSetItem<'_, '_, A, B>) -> B::Item<'a> {
        // SAFETY: 与 p0 相同，独占短借覆盖该成员的全部结果；不缓存旧视图。
        unsafe {
            let context = _set
                .context
                .exclusive_child(1)
                .expect("ParamSet 第二成员授权无效");
            let context = B::rebind(&mut *_set.second, context).expect("ParamSet 第二成员重绑失败");
            B::fetch(&mut *_set.second, &mut *_set.second_pending, context)
        }
    }
}
