//! 独立通知参数，不是 QueryFilter；每参数实例有独立订阅进度。
#[path = "notification_reader_inner.rs"]
pub(crate) mod notification_reader_inner;
use pi_ecs_core::{
    error::EcsError,
    notification::{NoticeBatch, NoticeSubscription, Notifications},
};
use std::marker::PhantomData;
/// Added 通知读取参数；只启用对应通知，不强制开启活组件 tick。
/// 只提供发生通知的完整 Entity/时间，点查得到当前值而非历史快照。
pub struct AddedReader<'a, T: 'static> {
    /// 本次通知系统共享访问；调度须声明对应组件观察权限，排除冲突写入。
    pub(crate) notifications: &'a Notifications,
    /// 参数长期状态中的独立订阅游标，短借用阻止重复并发消费。
    pub(crate) subscription: &'a mut NoticeSubscription,
    /// 类型与订阅组件必须一致，由 SystemParam 初始化校验。
    pub(crate) marker: PhantomData<fn() -> T>,
}
impl<T: 'static> AddedReader<'_, T> {
    /// 查询当前有无未读项，不消费，不保证之后仍为空；失效订阅报错不改游标。
    pub fn is_empty(&self) -> Result<bool, EcsError> {
        Self::added_empty_inner(self)
    }
    /// 放弃调用时流尾以前的本读者未读项，不清共享流、不影响其他读者或此后追加项。
    /// 活批次/旧值借用阻止本可变借用；失效订阅错误不推进。无 ack/事务或业务成功含义。
    /// 全部读者通过且无活借用后才可回收记录。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        Self::added_clear_inner(self)
    }

    /// 固定本次读取上界，不自动确认全批；早停须保留未消费记录。
    /// 返回批次借用 self，引用结束前不得回收记录；错误不推进游标。
    pub fn read(&mut self) -> Result<NoticeBatch<'_>, EcsError> {
        Self::added_read_inner(self)
    }
}
/// Changed 通知读取参数；只启用对应通知，不强制开启活组件 tick。
/// 只提供发生通知的完整 Entity/时间，点查得到当前值而非历史快照。
pub struct ChangedReader<'a, T: 'static> {
    /// 本次通知系统共享访问；调度须声明对应组件观察权限，排除冲突写入。
    pub(crate) notifications: &'a Notifications,
    /// 参数长期状态中的独立订阅游标，短借用阻止重复并发消费。
    pub(crate) subscription: &'a mut NoticeSubscription,
    /// 类型与订阅组件必须一致，由 SystemParam 初始化校验。
    pub(crate) marker: PhantomData<fn() -> T>,
}
impl<T: 'static> ChangedReader<'_, T> {
    /// 查询当前有无未读项，不消费，不保证之后仍为空；失效订阅报错不改游标。
    pub fn is_empty(&self) -> Result<bool, EcsError> {
        Self::changed_empty_inner(self)
    }
    /// 放弃调用时流尾以前的本读者未读项，不清共享流、不影响其他读者或此后追加项。
    /// 活批次/旧值借用阻止本可变借用；失效订阅错误不推进。无 ack/事务或业务成功含义。
    /// 全部读者通过且无活借用后才可回收记录。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        Self::changed_clear_inner(self)
    }

    /// 固定本次读取上界，不自动确认全批；早停须保留未消费记录。
    /// 返回批次借用 self，引用结束前不得回收记录；错误不推进游标。
    pub fn read(&mut self) -> Result<NoticeBatch<'_>, EcsError> {
        Self::changed_read_inner(self)
    }
}
/// Removed 通知读取参数；只启用对应通知，不强制开启活组件 tick。
/// Removed 包含旧值共享借用，不允许取走其他读者的历史值。
pub struct RemovedReader<'a, T: pi_ecs_core::component::Component> {
    /// 本次通知系统共享访问；调度须声明对应组件观察权限，排除冲突写入。
    pub(crate) notifications: &'a Notifications,
    /// 参数长期状态中的独立订阅游标，短借用阻止重复并发消费。
    pub(crate) subscription: &'a mut NoticeSubscription,
    /// 内置旧值读取需要所属 World；普通组件也通过同一入口校验。
    #[cfg(feature = "rc")]
    pub(crate) world: pi_ecs_core::unsafe_world_cell::UnsafeWorldCell<'a>,
    /// 类型与订阅组件必须一致，由 SystemParam 初始化校验。
    pub(crate) marker: PhantomData<fn() -> T>,
}
impl<T: pi_ecs_core::component::Component> RemovedReader<'_, T> {
    /// 查询当前有无未读项，不消费，不保证之后仍为空；失效订阅报错不改游标。
    pub fn is_empty(&self) -> Result<bool, EcsError> {
        Self::removed_empty_inner(self)
    }
    /// 放弃调用时流尾以前的本读者未读项，不清共享流、不影响其他读者或此后追加项。
    /// 活批次/旧值借用阻止本可变借用；失效订阅错误不推进。无 ack/事务或业务成功含义。
    /// 全部读者通过且无活借用后才可回收记录。
    pub fn clear(&mut self) -> Result<(), EcsError> {
        Self::removed_clear_inner(self)
    }

    /// 全 World，从订阅尾起的未读记录；固定批次上界，next 返回即消费，早停保留剩余。
    /// 类型校验失败不推进；旧值只读引用存活期间不得回收。
    #[cfg(feature = "rc")]
    pub fn read(&mut self) -> Result<pi_ecs_core::notification::RemovedBatch<'_, T>, EcsError> {
        Self::removed_read_inner(self)
    }
}
// 配置为 ()，初始化从流尾订阅，恢复历史游标；无 ack/peek/自动重试或去重。
// 全部 Reader 监听全 World；阶段前可注销，最后订阅注销后该流不再推送。
// 三种 Reader 的 RC SystemParam 初始化/准备/fetch/注销声明均见 system_param_impls.rs。
// 配置与运行上下文沿统一参数协议传递，配对芯尚未实现，不提供伪造业务参数的构造器。

#[cfg(all(test, feature = "rc"))]
mod tests {
    use super::*;
    use crate::{
        system::{System, SystemFunction, SystemId, SystemRunContext},
        system_builder::SystemBuilder,
        system_param::{ParamRegistration, SystemParam},
    };
    use pi_ecs_core::{
        access::{AccessDeclaration, EntityScope, RunAccess, StructuralAccess},
        bundle::ComponentValue,
        change_detection::{ChangeTick, ChangeWindow},
        component::{Component, PlainAccess, StoragePolicy},
        world::World,
    };
    use std::{cell::Cell, num::NonZeroU64, rc::Rc};

    struct Tag;

    impl Component for Tag {
        type Access = PlainAccess;
    }

    #[test]
    fn added_reader_param_subscribes_reads_and_clears() {
        let mut world = World::new().unwrap();
        let tag = world
            .register_component::<Tag>(StoragePolicy::TableEligible)
            .unwrap();
        let table = world.register_table(&[tag]).unwrap();
        world.finish_registration().unwrap();

        let mut state = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|access| {
                    let mut registration = ParamRegistration::new(access);
                    AddedReader::<Tag>::initialize((), &mut registration)
                })
            })
            .unwrap();

        let mut spawn = |world: &mut World| {
            let values = ComponentValue(Tag);
            let owned = world
                .spawn(pi_ecs_core::entity::AllocationDomain(0), values)
                .unwrap();
            let entity = owned.entity();
            let _ = owned.forget();
            entity
        };

        let first = spawn(&mut world);
        {
            let mut reader = AddedReader::<Tag> {
                notifications: world.notifications(),
                subscription: &mut state.subscription,
                marker: PhantomData,
            };
            let mut batch = reader.read().unwrap();
            assert_eq!(batch.next().unwrap().entity, first);
        }
        let _second = spawn(&mut world);
        {
            let mut reader = AddedReader::<Tag> {
                notifications: world.notifications(),
                subscription: &mut state.subscription,
                marker: PhantomData,
            };
            reader.clear().unwrap();
            assert!(reader.is_empty().unwrap());
        }

        world
            .parameter_unregistration_scope(|access| {
                let mut registration = ParamRegistration::new(access);
                AddedReader::<Tag>::unregister(state, &mut registration)
            })
            .unwrap();
        world.reclaim_notifications().unwrap();
    }

    struct Counter(Rc<Cell<usize>>);

    impl SystemFunction<AddedReader<'static, Tag>> for Counter {
        type Param = AddedReader<'static, Tag>;
        fn call<'run>(&mut self, mut item: AddedReader<'run, Tag>) {
            self.0.set(self.0.get() + 1);
            assert!(!item.is_empty().unwrap());
            item.clear().unwrap();
        }
    }

    #[test]
    fn stage_runs_reader_system_through_executor() {
        let mut world = World::new().unwrap();
        let tag = world
            .register_component::<Tag>(StoragePolicy::TableEligible)
            .unwrap();
        let table = world.register_table(&[tag]).unwrap();
        world.finish_registration().unwrap();

        let count = Rc::new(Cell::new(0));
        let system = SystemBuilder::<Counter, AddedReader<'static, Tag>>::with_config(
            Counter(count.clone()),
            (),
        )
        .build(SystemId(2))
        .unwrap();
        let mut stage =
            crate::stage::Stage::new(crate::stage::StageId(0), crate::executor::ExecutorId(0));
        stage.add_system(system).unwrap();
        world
            .schedule_scope(|world| stage.prepare(world).map(drop))
            .unwrap();

        let values = ComponentValue(Tag);
        let owned = world
            .spawn(pi_ecs_core::entity::AllocationDomain(0), values)
            .unwrap();
        let _ = owned.forget();

        let mut executor =
            crate::executor::Executor::new(crate::executor::ExecutionMode::Serial).unwrap();
        world
            .schedule_scope(|world| stage.run(world, &mut executor))
            .unwrap();
        assert_eq!(count.get(), 1);

        stage.unregister_system(SystemId(2), &mut world).unwrap();
    }

    #[test]
    fn function_system_runs_added_reader_param() {
        let mut world = World::new().unwrap();
        let tag = world
            .register_component::<Tag>(StoragePolicy::TableEligible)
            .unwrap();
        let table = world.register_table(&[tag]).unwrap();
        world.finish_registration().unwrap();

        let count = Rc::new(Cell::new(0));
        let mut system = SystemBuilder::<Counter, AddedReader<'static, Tag>>::with_config(
            Counter(count.clone()),
            (),
        )
        .build(SystemId(1))
        .unwrap();
        world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|access| {
                    let mut registration = ParamRegistration::new(access);
                    system.initialize(&mut registration)
                })?;
                world.apply_tracking()?;
                system.prepare(world)?;
                Ok(())
            })
            .unwrap();

        let values = ComponentValue(Tag);
        let owned = world
            .spawn(pi_ecs_core::entity::AllocationDomain(0), values)
            .unwrap();
        let _ = owned.forget();

        let declaration = AccessDeclaration {
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
            observations: Box::new([(tag, pi_ecs_core::notification::NoticeKind::Added)]),
        };
        let declaration = pi_ecs_core::access::AccessTree::Leaf(declaration);
        let access = unsafe { RunAccess::from_executor(&mut world, &declaration, 0) };
        let context = SystemRunContext::new(
            access,
            ChangeWindow {
                last_run: None,
                this_run: ChangeTick::new(NonZeroU64::new(2).unwrap()),
            },
        )
        .unwrap();
        unsafe { system.run(context) };
        assert_eq!(count.get(), 1);

        world
            .parameter_unregistration_scope(|access| {
                let mut registration = ParamRegistration::new(access);
                system.unregister(&mut registration)
            })
            .unwrap();
        world.reclaim_notifications().unwrap();
    }

    #[test]
    fn removed_reader_param_subscribes_reads_and_unregisters() {
        let mut world = World::new().unwrap();
        let tag = world
            .register_component::<Tag>(StoragePolicy::TableEligible)
            .unwrap();
        let table = world.register_table(&[tag]).unwrap();
        world.finish_registration().unwrap();

        let state = world
            .schedule_scope(|world| {
                world.parameter_registration_scope(|access| {
                    let mut registration = ParamRegistration::new(access);
                    RemovedReader::<Tag>::initialize((), &mut registration)
                })
            })
            .unwrap();
        let mut state = state;

        let values = ComponentValue(Tag);
        let owned = world
            .spawn(pi_ecs_core::entity::AllocationDomain(0), values)
            .unwrap();
        let entity = owned.entity();
        let _ = owned.forget();

        {
            let mut reader = RemovedReader::<Tag> {
                notifications: world.notifications(),
                subscription: &mut state.subscription,
                world: pi_ecs_core::unsafe_world_cell::UnsafeWorldCell::from_ref(&world),
                marker: PhantomData,
            };
            assert!(reader.is_empty().unwrap());
        }

        assert!(unsafe { world.despawn(entity) }.unwrap());
        world.flush_destruction().unwrap();

        {
            let mut reader = RemovedReader::<Tag> {
                notifications: world.notifications(),
                subscription: &mut state.subscription,
                world: pi_ecs_core::unsafe_world_cell::UnsafeWorldCell::from_ref(&world),
                marker: PhantomData,
            };
            assert!(!reader.is_empty().unwrap());
            let mut batch = reader.read().unwrap();
            assert_eq!(batch.next().unwrap().header.entity, entity);
        }

        world
            .parameter_unregistration_scope(|access| {
                let mut registration = ParamRegistration::new(access);
                RemovedReader::<Tag>::unregister(state, &mut registration)
            })
            .unwrap();
        world.reclaim_notifications().unwrap();
    }
}
