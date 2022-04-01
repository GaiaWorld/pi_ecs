use super::interface::Run;
use crate::dispatch::interface::{Arrange, ExecNode, GraphNode, Operate};
use crate::storage::Local;
use crate::{
    sys::{
        param::SystemParam,
        system::{
            func_sys::{FunctionSystem, SystemParamFunction},
            System,
        },
    },
    world::World,
};
use futures::future::BoxFuture;
use pi_share::cell::TrustCell;
use std::io::Result;
use std::sync::Arc;

pub struct SyncRun<Param: SystemParam + 'static, F>(
    pub(crate) TrustCell<FunctionSystem<(), (), Param, (), F>>,
);

pub struct AsyncRun<Param: SystemParam + 'static, F>(
    pub(crate) TrustCell<FunctionSystem<(), BoxFuture<'static, Result<()>>, Param, (), F>>,
);

impl<Param: SystemParam + 'static, F> Operate for SyncRun<Param, F>
where
    F: Send + Sync + SystemParamFunction<(), (), Param, ()>,
{
    type R = ();

    fn run(&self) {
        self.0.borrow_mut().run(());
    }

    fn apply(&self) {
        self.0.borrow_mut().apply_buffers();
    }
}

impl<Param: SystemParam + 'static, F> Operate for AsyncRun<Param, F>
where
    F: Send + Sync + SystemParamFunction<(), BoxFuture<'static, Result<()>>, Param, ()>,
{
    type R = BoxFuture<'static, Result<()>>;

    fn run(&self) -> BoxFuture<'static, Result<()>> {
        self.0.borrow_mut().run(())
    }
    fn apply(&self) {
        self.0.borrow_mut().apply_buffers();
    }
}

impl<Param: SystemParam + 'static, F> Into<GraphNode> for FunctionSystem<(), (), Param, (), F>
where
    F: Send + Sync + SystemParamFunction<(), (), Param, ()>,
{
    default fn into(self) -> GraphNode {
        let id = self.id();
        let component_access = self.archetype_component_access();
        let access = component_access.clone();
        let mut reads = component_access.get_reads_and_writes().clone();
        reads.symmetric_difference_with(access.get_writes());
        let mut writes = component_access.get_writes().clone();

        let w = self.world();

        for write in component_access.get_writes().ones().into_iter() {
            let v = w.listener_access.get(Local::new(write));
            if let Some(r) = v {
                for access1 in r.iter() {
                    // 有相同元素
                    if component_access
                        .get_writes()
                        .is_disjoint(access1.get_writes())
                    {
                        panic!("write conflict");
                    }

                    // 扩充读
                    let mut r = access1.get_reads_and_writes().clone();
                    r.symmetric_difference_with(access1.get_writes());
                    reads.union_with(&r);

                    writes.union_with(access1.get_writes());
                }
            }
        }

        let reads = reads.ones().into_iter().collect();
        let writes = writes.ones().into_iter().collect();
        let sys = TrustCell::new(self);
        GraphNode {
            id: id.id(),
            reads,
            writes,
            node: ExecNode::Sync(Run(Arc::new(SyncRun(sys)))),
        }
    }
}

impl<Param: SystemParam + 'static, F> Into<GraphNode>
    for FunctionSystem<(), BoxFuture<'static, Result<()>>, Param, (), F>
where
    F: Send + Sync + SystemParamFunction<(), BoxFuture<'static, Result<()>>, Param, ()>,
{
    fn into(self) -> GraphNode {
        let id = self.id();
        let component_access = self.archetype_component_access();

        let reads = component_access
            .get_reads_and_writes()
            .difference(component_access.get_writes())
            .collect();
        let writes = component_access.get_writes().ones().into_iter().collect();
        let sys = TrustCell::new(self);
        GraphNode {
            id: id.id(),
            reads: reads,
            writes: writes,
            node: ExecNode::Async(Box::new(AsyncRun(sys))),
        }
    }
}

impl Arrange for World {
    fn arrange(&self) -> Option<GraphNode> {
        let mut w = self.clone();
        let id = w.archetype_component_grow();
        let sys = move || {
            w.increment_change_tick();
        };
        Some(GraphNode {
            id,
            reads: Vec::new(),
            writes: Vec::new(),
            node: ExecNode::Sync(Run(Arc::new(FnSys(Box::new(sys))))),
        })
    }
}

pub struct FnSys(pub(crate) Box<dyn Fn()>);

impl Operate for FnSys {
    type R = ();

    fn run(&self) {
        self.0();
    }
    fn apply(&self) {}
}
