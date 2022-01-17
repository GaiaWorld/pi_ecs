use std::{sync::Arc};
use std::io::Result;

use share::cell::TrustCell;
use futures::future::BoxFuture;

use crate::storage::Local;
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
					if component_access.get_writes().is_disjoint(access1.get_writes()) {
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
