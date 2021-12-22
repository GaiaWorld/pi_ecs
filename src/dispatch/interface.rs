
use std::sync::Arc;
use std::io::Result;

use futures::future::BoxFuture;
use graph::{NGraph, NGraphBuilder};
use async_graph::{Runner, Runnble, RunFactory};
use r#async::rt::{AsyncRuntime, AsyncTaskPool, AsyncTaskPoolExt};

pub trait Dispatcher {
    fn run(&self);
}

pub struct SingleDispatcher<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> {
    rt: AsyncRuntime<(), P>,
    vec: Vec<(Arc<NGraph<usize, ExecNode>>, bool)>,
}

impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> SingleDispatcher<P> {
    pub fn new(rt: AsyncRuntime<(), P>, vec: Vec<(Arc<NGraph<usize, ExecNode>>, bool)>) -> Self {
        SingleDispatcher {
            rt,
            vec,
        }
    }
}

impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> Dispatcher for SingleDispatcher<P> {
   fn run(&self) {

   }
}
pub struct MultiDispatcher<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> {
    single: AsyncRuntime<(), P>,
    multi: AsyncRuntime<(), P>,
    vec: Vec<(Arc<NGraph<usize, ExecNode>>, bool)>,
}
impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> MultiDispatcher<P> {
    pub fn new(single: AsyncRuntime<(), P>, multi: AsyncRuntime<(), P>, vec: Vec<(Arc<NGraph<usize, ExecNode>>, bool)>) -> Self {
        MultiDispatcher {
            single,
            multi,
            vec,
        }
    }
}
impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> Dispatcher for MultiDispatcher<P> {
   fn run(&self) {
       
   } 
}
pub struct Node {
    pub(crate) id: usize,
    pub(crate) reads: Vec<usize>,
    pub(crate) writes: Vec<usize>,
    pub(crate) node: ExecNode,
}

pub struct Run(pub(crate) Arc<dyn Fn() -> ()>);
unsafe impl Send for  Run{}
unsafe impl Sync for  Run{}

impl Runner for Run {
    fn run(self){
		self.0()
	}
}


impl RunFactory for Run {
	type R = Run;
    fn create(&self) -> Self::R {
		Run(self.0.clone())
	}
}
pub enum ExecNode {
	None,
	Sync(Run),
	Async(Box<dyn Fn() -> BoxFuture<'static, Result<()>> + 'static + Send + Sync>),
}
impl Runnble<Run> for ExecNode {
    fn is_async(&self) -> Option<bool> {
        match self {
            ExecNode::None => None,
            ExecNode::Sync(_) => Some(false),
            _ => Some(true)
        }
    }
    /// 获得需要执行的同步函数
    fn get_sync(&self) -> Run{
        match self {
            ExecNode::Sync(f) => f.create(),
            _ => panic!()
        }
    }
    /// 获得需要执行的异步块
    fn get_async(&self) -> BoxFuture<'static, Result<()>> {
        match self {
            ExecNode::Async(f) => f(),
            _ => panic!()
        }
    }
}


pub struct StageBuilder {
    nodes: Vec<Node>,
    edges: Vec<(usize, usize)>,
}

impl StageBuilder {
    pub fn new() -> Self {
        StageBuilder { nodes: Vec::new(), edges: Vec::new() }
    }

	pub fn add_node<T: Into<Node>>(&mut self, node: T) -> &mut Self {
		let node = node.into();

        for k in &node.reads {
            self.edges.push((*k, node.id));
        }
        for k in &node.writes {
            self.edges.push((node.id, *k));
        }
        self.nodes.push(node);
        self
    }

    pub fn order(mut self, before: usize, after: usize) -> Self {
        self.edges.push((before, after));
        self
    }
    pub fn build(self) -> NGraph<usize, ExecNode> {
        let mut builder = NGraphBuilder::new();
        for n in self.nodes {
            builder = builder.node(n.id, n.node);
        }
        for n in self.edges {
            builder = builder.edge(n.0, n.1);
        }
        builder.build().unwrap()
    }
}