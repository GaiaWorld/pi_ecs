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
use futures::Future;
use futures::future::BoxFuture;
use pi_share::cell::TrustCell;
use std::io::Result;
use std::sync::Arc;

pub struct SyncRun<Param: SystemParam + 'static, Out, F>(
    pub(crate) TrustCell<FunctionSystem<(), Out, Param, (), F>>,
);

pub struct AsyncRun<Param: SystemParam + 'static, Out: Future<Output=Result<()>>, F>(
    pub(crate) Arc<TrustCell<FunctionSystem<(), Out, Param, (), F>>>,
);

impl<Param: SystemParam + 'static, Out: 'static + Send + Sync, F> Operate for SyncRun<Param, Out, F>
where
    F: Send + Sync + SystemParamFunction<(), Out, Param, ()>,
{
    type R = ();

    fn run(&self) {
        self.0.borrow_mut().run(());
    }

    fn apply(&self) {
        self.0.borrow_mut().apply_buffers();
    }
}

impl<Param, Out, F> Operate for AsyncRun<Param, Out, F>
where
    F: Send + Sync + SystemParamFunction<(), Out, Param, ()>,
	Out: Future<Output = Result<()>  > + Send + Sync + 'static,
	Param: SystemParam + 'static
{
    type R = BoxFuture<'static, Result<()>>;

    fn run(&self) -> BoxFuture<'static, Result<()>> {
		Box::pin(self.0.borrow_mut().run(()))
    }
    fn apply(&self) {
        self.0.borrow_mut().apply_buffers();
    }
}

impl<Param: SystemParam + 'static, Out: 'static + Send + Sync, F> Into<GraphNode> for FunctionSystem<(), Out, Param, (), F>
where
    F: Send + Sync + SystemParamFunction<(), Out, Param, ()>,
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

impl<Param, Out, F> Into<GraphNode>
    for FunctionSystem<(), Out, Param, (), F>
where
    F: Send + Sync + SystemParamFunction<(), Out, Param, ()>,
	Param: SystemParam + 'static,
	Out: 'static + Send + Sync + Future<Output = Result<()>>
{
    fn into(self) -> GraphNode {
        let id = self.id();
        let component_access = self.archetype_component_access();

        let reads = component_access
            .get_reads_and_writes()
            .difference(component_access.get_writes())
            .collect();
        let writes = component_access.get_writes().ones().into_iter().collect();
        let sys = Arc::new(TrustCell::new(self));
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
