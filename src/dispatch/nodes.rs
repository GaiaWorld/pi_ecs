use super::interface::Run;
use crate::dispatch::interface::{Arrange, ExecNode, GraphNode, Operate};
use crate::{
    sys::{
        param::SystemParam,
        system::{
            func_sys::{FunctionSystem, SystemParamFunction},
            System,
        },
    },
	query::Access,
    world::World,
};
use futures::Future;
use futures::future::BoxFuture;
use pi_share::cell::TrustCell;
use std::borrow::Cow;
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

	fn name(&self) -> Cow<'static, str> {
        self.0.borrow().name()
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

	fn name(&self) -> Cow<'static, str> {
        self.0.borrow().name()
    }
}

impl<Param: SystemParam + 'static, Out: 'static + Send + Sync, F> Into<GraphNode> for FunctionSystem<(), Out, Param, (), F>
where
    F: Send + Sync + SystemParamFunction<(), Out, Param, ()>,
{
    default fn into(self) -> GraphNode {
        let id = self.id();
		let name = self.name().to_string();
        let component_access = self.archetype_component_access().clone();

        let sys = TrustCell::new(self);
        GraphNode {
            id: id.id(),
            // reads,
            // writes,
            node: ExecNode::Sync(Run(Arc::new(SyncRun(sys)))),
			access: component_access,
			label: name,
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
		let name = self.name().to_string();
        let component_access = self.archetype_component_access().clone();

        let sys = Arc::new(TrustCell::new(self));
        GraphNode {
            id: id.id(),
            node: ExecNode::Async(Box::new(AsyncRun(sys))),
			access: component_access,
			label: name,
        }
    }
}

impl Arrange for World {
    fn arrange(&self) -> Option<GraphNode> {
        let mut w = self.clone();
        let id = w.archetype_component_grow("arrange".to_string());
        let sys = move || {
			for l in w.listeners.iter() {
				l.apply();
			}
            w.increment_change_tick();
        };
        Some(GraphNode {
            id,
            node: ExecNode::Sync(Run(Arc::new(FnSys(Box::new(sys))))),
			access: Access::default(),
			label: "increment_change_tick".to_string(),
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

	fn name(&self) -> Cow<'static, str> {
		"FnSys".into()
    }
}

