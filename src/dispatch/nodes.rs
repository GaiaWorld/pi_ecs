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
use pi_futures::BoxFuture;
use pi_share::cell::TrustCell;
use std::borrow::Cow;
use std::io::Result;
use std::sync::Arc;
use pi_share::{ThreadSync, ThreadSend};

pub struct SyncRun<Param: SystemParam + 'static, Out, F>(
    pub(crate) TrustCell<FunctionSystem<(), Out, Param, (), F>>,
);

pub struct AsyncRun<Param: SystemParam, Out: Future<Output=Result<()>> + ThreadSend + 'static, F: ThreadSync + 'static>(
    pub(crate) Arc<TrustCell<FunctionSystem<(), Out, Param, (), F>>>,
);

unsafe impl<Param, Out, F> Send for AsyncRun<Param, Out, F> where 
	Param: SystemParam, 
	Out: Future<Output=Result<()>> + ThreadSend + 'static, 
	F: ThreadSync + 'static {}
unsafe impl<Param, Out, F> Sync for AsyncRun<Param, Out, F> where 
Param: SystemParam, 
Out: Future<Output=Result<()>> + ThreadSend + 'static, 
F: ThreadSync + 'static {}

impl<Param: SystemParam, Out: ThreadSend + 'static, F> Operate for SyncRun<Param, Out, F>
where
    F: SystemParamFunction<(), Out, Param, ()> + ThreadSync + 'static,
{
    type R = ();

    fn run(&self) {
		// println!("run============{:?}", std::any::type_name::<F>());
		
        self.0.borrow_mut().run(());
		// println!("run end============{:?}", std::any::type_name::<F>());
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
    F: SystemParamFunction<(), Out, Param, ()> + ThreadSync + 'static,
	Out: Future<Output = Result<()>  > + ThreadSend + 'static,
	Param: SystemParam + 'static
{
    type R = BoxFuture<'static, Result<()>>;

    fn run(&self) -> BoxFuture<'static, Result<()>> {
		// println!("async============{:?}", std::any::type_name::<F>());
		let context: AsyncRun<Param, Out, F> = Self(self.0.clone());
		Box::pin(async move {
			// 将context捕获，使得future在执行时，system始终存在，保证future执行的安全性
			let mut b = context.0.borrow_mut();
			b.run(()).await
		})
    }
    fn apply(&self) {
        self.0.borrow_mut().apply_buffers();
    }

	fn name(&self) -> Cow<'static, str> {
        self.0.borrow().name()
    }
}

impl<Param: SystemParam + 'static, Out: ThreadSend + 'static, F> Into<GraphNode> for FunctionSystem<(), Out, Param, (), F>
where
    F: SystemParamFunction<(), Out, Param, ()> + ThreadSync + 'static,
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
    F: SystemParamFunction<(), Out, Param, ()> + ThreadSync + 'static,
	Param: SystemParam + 'static,
	Out: Future<Output = Result<()>> + ThreadSend + 'static
{
    fn into(self) -> GraphNode {
        let id = self.id();
		let name = self.name().to_string();
        let component_access = self.archetype_component_access().clone();

        let sys = Arc::new(TrustCell::new(self));
        GraphNode {
            id: id.id(),
            node: ExecNode::Async(super::interface::AsyncRun(Arc::new(AsyncRun(sys)))),
			access: component_access,
			label: name,
        }
    }
}

impl Arrange for World {
    fn arrange(&self) -> Option<GraphNode> {
        let mut w = self.clone();
        let id = w.archetype_component_grow("arrange", false);
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

pub trait FnSysTrait: Fn() + ThreadSync + 'static {}
impl<T: Fn() + ThreadSync + 'static> FnSysTrait for T {}

pub struct FnSys(pub(crate) Box<dyn FnSysTrait>);

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

