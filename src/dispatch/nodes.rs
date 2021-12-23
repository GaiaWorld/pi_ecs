use std::{sync::Arc};
use std::io::Result;

use share::cell::TrustCell;
use futures::future::BoxFuture;

use crate::{
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

use crate::dispatch::interface::{Node, ExecNode};

use super::interface::Run;

impl<Param: SystemParam + 'static, F> Into<Node> for FunctionSystem<(), (), Param, (), F> where 
	F: Send + Sync + SystemParamFunction<(), (), Param, ()> {

	default fn into(self) -> Node {
		let id = self.id();
		let component_access = self.archetype_component_access();
		let reads = component_access.get_reads().ones().into_iter().collect();
		let writes = component_access.get_writes().ones().into_iter().collect();
		let sys = TrustCell::new(self);
		Node{
			id: id.id(),
			reads,
			writes,
			node: ExecNode::Sync(Run(Arc::new(move || {
				sys.borrow_mut().run(())
			})) )
		}
	}
}

// impl<Param: SystemParam + 'static, F, R> Into<Node> for FunctionSystem<(), R, Param, (), F> where 
// 	R: std::future::Future<Output=Result<()>> + Send + 'static,
// 	F: Send + Sync + SystemParamFunction<(), R, Param, ()> {
// 	fn into(self) -> Node {
// 		let id = self.id();
// 		let component_access = self.archetype_component_access();
// 		let reads = component_access.get_reads().ones().into_iter().collect();
// 		let writes = component_access.get_writes().ones().into_iter().collect();
// 		let sys = TrustCell::new(self);
// 		Node{
// 			id: id.id(),
// 			reads: reads,
// 			writes: writes,
// 			node: ExecNode::Async(Box::new(move || {
// 				sys.borrow_mut().run(())
// 			}))
// 		}
// 	}
// }

impl<Param: SystemParam + 'static, F> Into<Node> for FunctionSystem<(), BoxFuture<'static, Result<()> >, Param, (), F> where 
	F: Send + Sync + SystemParamFunction<(), BoxFuture<'static, Result<()>>, Param, ()> {
	fn into(self) -> Node {
		let id = self.id();
		let component_access = self.archetype_component_access();
		let reads = component_access.get_reads().ones().into_iter().collect();
		let writes = component_access.get_writes().ones().into_iter().collect();
		let sys = TrustCell::new(self);
		Node{
			id: id.id(),
			reads: reads,
			writes: writes,
			node: ExecNode::Async(Box::new(move || {
				sys.borrow_mut().run(())
			}))
		}
	}
}