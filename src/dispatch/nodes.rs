use std::{sync::Arc};
use std::io::Result;

use share::cell::TrustCell;

use crate::{
	sys::{
		system::{
			async_func_sys::{
				AsyncFunctionSystem, 
				SystemAsyncParamFunction
			}, 
			func_sys::{
				FunctionSystem,
				SystemParamFunction
			}, 
			System
		}, 
		param::SystemParam
	},
};

use crate::dispatch::interface::{Node, ExecNode};

use super::interface::Run;

impl<Param: SystemParam + 'static, F> Into<Node> for AsyncFunctionSystem<(), Result<()>, Param, (), F> where 
	F: Send + Sync + SystemAsyncParamFunction<(), Result<()>, Param, ()> {
	fn into(self) -> Node {
		let sys = TrustCell::new(self);
		Node{
			id: 0,
			reads: Vec::new(),
			writes: Vec::new(),
			node: ExecNode::Async(Box::new(move || {
				sys.borrow_mut().run(())
			}))
		}
	}
}

impl<Param: SystemParam + 'static, F> Into<Node> for FunctionSystem<(), (), Param, (), F> where 
	F: Send + Sync + SystemParamFunction<(), (), Param, ()> {

	fn into(self) -> Node {
		let id = self.id();
		// let component_access
		let sys = TrustCell::new(self);
		Node{
			id: 0,
			reads: Vec::new(),
			writes: Vec::new(),
			node: ExecNode::Sync(Run(Arc::new(move || {
				sys.borrow_mut().run(())
			})) )
		}
	}
}