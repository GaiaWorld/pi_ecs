use std::io::Result;
use std::sync::Arc;

use async_graph::{Runnble, Runner};
use futures::future::BoxFuture;
use graph::{NGraph, NGraphBuilder, DirectedGraph, DirectedGraphNode};
use r#async::rt::{AsyncRuntime, AsyncTaskPool, AsyncTaskPoolExt};

pub trait Dispatcher {
    fn run(&self);
}

pub struct SingleDispatcher<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> {
    vec: Arc<Vec<(Arc<NGraph<usize, ExecNode>>, bool)>>,
    rt: AsyncRuntime<(), P>,
}

impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> SingleDispatcher<P> {
    pub fn new(vec: Vec<(Arc<NGraph<usize, ExecNode>>, bool)>, rt: AsyncRuntime<(), P>) -> Self {
        SingleDispatcher { vec: Arc::new(vec), rt }
    }

    /// 执行指定阶段的指定节点
    pub fn exec(vec: Arc<Vec<(Arc<NGraph<usize, ExecNode>>, bool)>>,
    rt: AsyncRuntime<(), P>, mut stage_index: usize, mut node_index: usize) {
        while stage_index < vec.len() {
            let g = &vec[stage_index].0;
            let arr = g.topological_sort();
            if node_index >= arr.len() {
                stage_index += 1;
                continue
            }
            let node = g.get(&arr[node_index]).unwrap().value();
            node_index += 1;
            match node.is_async() {
                Some(r#async) => if !r#async {
                    node.get_sync().run();
                }else{
                    let f = node.get_async();
                    let vec1= vec.clone();
                    let rt1= rt.clone();
                    rt.spawn(rt.alloc(), async move {
                        let _ = f.await;
                        // println!("ok");
                        SingleDispatcher::exec(vec1, rt1, stage_index, node_index + 1);
                    });
                },
                None => (),
            }
        }
    }
}
impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> Dispatcher for SingleDispatcher<P> {
    /// 同步节点自己执行， 如果有异步节点，则用单线程运行时执行
    fn run(&self) {
        Self::exec(self.vec.clone(), self.rt.clone(), 0, 0);
    }
}
pub struct MultiDispatcher<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> {
    vec: Vec<(Arc<NGraph<usize, ExecNode>>, bool)>,
    single: AsyncRuntime<(), P>,
    multi: AsyncRuntime<(), P>,
}
impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> MultiDispatcher<P> {
    pub fn new(
        vec: Vec<(Arc<NGraph<usize, ExecNode>>, bool)>,
        single: AsyncRuntime<(), P>,
        multi: AsyncRuntime<(), P>,
    ) -> Self {
        MultiDispatcher { vec, single, multi }
    }
}
impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> MultiDispatcher<P> {
    /// 执行
    fn exec(&self, index: usize) {
        if index >= self.vec.len() {
            return
        }
        let (g, single) = &self.vec[index];
        if *single {
            self.sync_exec(g)
        }else{
            self.async_exec(g.clone())
        }
    }
    /// 异步执行
    fn sync_exec(&self, g: &Arc<NGraph<usize, ExecNode>>) {
        
    }
    /// 异步执行
    fn async_exec(&self, g: Arc<NGraph<usize, ExecNode>>) {
        
    }
}
impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> Dispatcher for MultiDispatcher<P> {
    /// 根据阶段是单线程还是多线程，
    /// 如果多线程阶段，同步节点和异步节点，则用多线程运行时并行执行
    /// 如果单线程阶段，同步节点自己执行， 如果有异步节点，则用单线程运行时执行
    fn run(&self) {}
}
pub struct Node {
    pub(crate) id: usize,
    pub(crate) reads: Vec<usize>,
    pub(crate) writes: Vec<usize>,
    pub(crate) node: ExecNode,
}

pub struct Run(pub(crate) Arc<dyn Fn() -> ()>);
unsafe impl Send for Run {}
unsafe impl Sync for Run {}

impl Runner for Run {
    fn run(self) {
        self.0()
    }
}

pub enum ExecNode {
    None,
    Sync(Run),
    Async(Box<dyn Fn() -> BoxFuture<'static, Result<()>> + 'static + Send + Sync>),
}
impl Runnble for ExecNode {
    type R = Run;
    fn is_async(&self) -> Option<bool> {
        match self {
            ExecNode::None => None,
            ExecNode::Sync(_) => Some(false),
            _ => Some(true),
        }
    }
    /// 获得需要执行的同步函数
    fn get_sync(&self) -> Run {
        match self {
            ExecNode::Sync(f) => Run(f.0.clone()),
            _ => panic!(),
        }
    }
    /// 获得需要执行的异步块
    fn get_async(&self) -> BoxFuture<'static, Result<()>> {
        match self {
            ExecNode::Async(f) => f(),
            _ => panic!(),
        }
    }
}

pub struct StageBuilder {
    nodes: Vec<Node>,
    edges: Vec<(usize, usize)>,
}

impl StageBuilder {
    pub fn new() -> Self {
        StageBuilder {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
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
