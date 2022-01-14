use std::{sync::Arc};
use std::io::Result;

use share::cell::TrustCell;
use futures::future::BoxFuture;

use crate::{
	world::World,
	sys::{
		system::{
			func_sys::{
				FunctionSystem,
				SystemParamFunction,
			}, 
			System
		}, 
		param::SystemParam
	},
};

use crate::dispatch::interface::{GraphNode, ExecNode, Arrange};

use super::interface::Run;


impl<Param: SystemParam + 'static, F> Into<GraphNode> for FunctionSystem<(), (), Param, (), F> where 
	F: Send + Sync + SystemParamFunction<(), (), Param, ()> {

	default fn into(self) -> GraphNode {
		let id = self.id();
		let component_access = self.archetype_component_access();
		let reads = component_access
								.get_reads_and_writes()
								.difference(component_access.get_writes())
								.collect();
		let writes = component_access.get_writes().ones().into_iter().collect();
		let sys = TrustCell::new(self);
		GraphNode{
			id: id.id(),
			reads,
			writes,
			node: ExecNode::Sync(Run(Arc::new(move || {
				sys.borrow_mut().run(())
			})) )
		}
	}
}

impl<Param: SystemParam + 'static, F> Into<GraphNode> for FunctionSystem<(), BoxFuture<'static, Result<()> >, Param, (), F> where 
	F: Send + Sync + SystemParamFunction<(), BoxFuture<'static, Result<()>>, Param, ()> {
	fn into(self) -> GraphNode {
		let id = self.id();
		let component_access = self.archetype_component_access();
		
		let reads = component_access
								.get_reads_and_writes()
								.difference(component_access.get_writes())
								.collect();
		let writes = component_access.get_writes().ones().into_iter().collect();
		let sys = TrustCell::new(self);
		GraphNode{
			id: id.id(),
			reads: reads,
			writes: writes,
			node: ExecNode::Async(Box::new(move || {
				sys.borrow_mut().run(())
			}))
		}
	}
}

impl Arrange for World {
	fn arrang(&self) -> Option<GraphNode> {
		let mut w = self.clone();
		let id = w.archetype_component_grow();
		Some(GraphNode {
			id,
			reads: Vec::new(),
			writes: Vec::new(),
			node: ExecNode::Sync(Run(Arc::new(move|| {
				w.increment_change_tick();
			})))
		})
	}
}
