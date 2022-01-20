use std::{io::Result, collections::HashSet};
use std::sync::Arc;

use async_graph::{async_graph, Runnble, Runner};
use futures::future::BoxFuture;
use graph::{DirectedGraph, DirectedGraphNode, NGraph, NGraphBuilder};
use r#async::rt::{AsyncRuntime, AsyncTaskPool, AsyncTaskPoolExt};

pub trait Dispatcher {
    fn run(&self);
}

pub struct SingleDispatcher<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> {
    vec: Arc<Vec<Arc<NGraph<usize, ExecNode>>>>,
    rt: AsyncRuntime<(), P>,
}

impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>> SingleDispatcher<P> {
    pub fn new<T: Arrange>(vec: Vec<Arc<NGraph<usize, ExecNode>>>, arrange: &T, rt: AsyncRuntime<(), P>) -> Self {
		let mut v1 = Vec::new();
		for i in vec.into_iter() {
			v1.push(i);

			// arrange node
			if let Some(node) = arrange.arrang() {
				let mut stage = StageBuilder::new();
				stage.add_node(node);
				
				v1.push(Arc::new(stage.build()))
			}
		}
        SingleDispatcher {
            vec: Arc::new(v1),
            rt,
        }
    }

    /// 执行指定阶段的指定节点
    pub fn exec(
        vec: Arc<Vec<Arc<NGraph<usize, ExecNode>>>>,
        rt: AsyncRuntime<(), P>,
        mut stage_index: usize,
        mut node_index: usize,
    ) {
        while stage_index < vec.len() {
            let g = &vec[stage_index];
            let arr = g.topological_sort();
            if node_index >= arr.len() {

				// stage结束，apply
				for i in 0..arr.len() {
					let node = g.get(&arr[i]).unwrap().value();
					node.apply();
				}
                stage_index += 1;
                node_index = 0;
                continue;
            }
            let node = g.get(&arr[node_index]).unwrap().value();
            node_index += 1;
            match node.is_sync() {
                Some(sync) => if sync {
                    node.get_sync().run();
                }else{
                    let f = node.get_async();
                    let vec1= vec.clone();
                    let rt1= rt.clone();
                    rt.spawn(rt.alloc(), async move {
                        let _ = f.await;
                        // println!("ok");
                        SingleDispatcher::exec(vec1, rt1, stage_index, node_index + 1);
                    }).unwrap();
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
pub struct MultiDispatcher<P1: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P1>, P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>>(
    Arc<MultiInner<P1, P2>>,
);
impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>, P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>> MultiDispatcher<P, P2> {
    pub fn new(
        vec: Vec<(Arc<NGraph<usize, ExecNode>>, Option<AsyncRuntime<(), P2>>)>,
        multi: AsyncRuntime<(), P>,
    ) -> Self {
        MultiDispatcher(Arc::new(MultiInner::new(vec, multi)))
    }
}
impl<P1: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P1>, P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>> Dispatcher for MultiDispatcher<P1, P2> {
    /// 根据阶段是单线程还是多线程，
    /// 如果多线程阶段，同步节点和异步节点，则用多线程运行时并行执行
    /// 如果单线程阶段，同步节点自己执行， 如果有异步节点，则用单线程运行时执行
    /// 一般为了线程安全，第一个阶段都是单线程执行
    fn run(&self) {
        let c = self.0.clone();
        exec(c, 0)
    }
}
struct MultiInner<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>, P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>> {
    vec: Vec<(Arc<NGraph<usize, ExecNode>>, Option<AsyncRuntime<(), P2>>)>,
    multi: AsyncRuntime<(), P>,
}
impl<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>, P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>> MultiInner<P, P2> {
    pub fn new(
        vec: Vec<(Arc<NGraph<usize, ExecNode>>, Option<AsyncRuntime<(), P2>>)>,
        multi: AsyncRuntime<(), P>,
    ) -> Self {
        MultiInner { vec, multi }
    }
}

/// 执行指定阶段
fn exec<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>, P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>>(
    d: Arc<MultiInner<P, P2>>,
    stage_index: usize,
) {
    if stage_index >= d.vec.len() {
        return;
    }
    if let Some(single) = &d.vec[stage_index].1 {
        let s = single.clone();
        single_exec(d, stage_index, 0, s);
    } else {
        multi_exec(d, stage_index);
    }
}

/// 单线程执行, 尽量本线程运行， 遇到异步节点则用单线程运行时运行
fn single_exec<P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>, P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>>(
    d: Arc<MultiInner<P, P2>>,
    stage_index: usize,
    mut node_index: usize,
    single: AsyncRuntime<(), P2>,
) {
    let g = &d.vec[stage_index].0;
    let single1= single.clone();
    loop {
		let arr = g.topological_sort();
        if node_index >= g.node_count() {

			// stage结束，apply
			for i in 0..g.node_count() {
				let node = g.get(&arr[i]).unwrap().value();
				node.apply();
			}

            // 本阶段执行完毕，执行下一阶段
            return exec(d, stage_index + 1);
        }
        
        let node = g.get(&arr[node_index]).unwrap().value();
        node_index += 1;
        match node.is_sync() {
            Some(sync) => {
                if sync {
                    if stage_index > 0 && node_index == 1 {
                        let f = node.get_sync();
                        let d1 = d.clone();
                        single1.spawn(single1.alloc(), async move {
                            f.run();
                            single_exec(d1, stage_index, node_index, single);
                        }).unwrap();
                        return;
                    }
                    // 如果是最开始的阶段， 或者非起始节点，则立即同步执行
                    node.get_sync().run();
                } else {
                    let f = node.get_async();
                    let d1 = d.clone();
                    single1.spawn(single1.alloc(), async move {
                        let _ = f.await;
                        single_exec(d1, stage_index, node_index, single);
                    }).unwrap();
                    return;
                }
            }
            None => (),
        }
    }
}
/// 多线程执行
fn multi_exec<P1: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P1>, P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>>(
    d: Arc<MultiInner<P1, P2>>,
    stage_index: usize,
) {
    let d1 = d.clone();
    d.multi.spawn(d.multi.alloc(), async move {
        let g = &d1.vec[stage_index].0;
        let r = async_graph(d1.multi.clone(), g.clone()).await;
        if r.is_ok() {

			// stage结束，apply
			let arr = g.topological_sort();
			for i in 0..g.node_count() {
				let node = g.get(&arr[i]).unwrap().value();
				node.apply();
			}

            exec(d1, stage_index + 1);
        }
    }).unwrap();
}
pub struct GraphNode {
    pub(crate) id: usize,
    pub(crate) reads: Vec<usize>,
    pub(crate) writes: Vec<usize>,
    pub(crate) node: ExecNode,
}

pub trait Operate {
	type R;
	fn run(&self) -> Self::R;
	fn apply(&self);
}

pub struct Run(pub(crate) Arc<dyn Operate<R=()>>);
unsafe impl Send for Run {}
unsafe impl Sync for Run {}

impl Runner for Run {
    fn run(self) {
        self.0.run()
    }
}

pub enum ExecNode {
    None,
    Sync(Run),
    Async(Box<dyn Operate<R=BoxFuture<'static, Result<()>>> + 'static + Send + Sync>),
}
impl Runnble for ExecNode {
    type R = Run;
    fn is_sync(&self) -> Option<bool> {
        match self {
            ExecNode::None => None,
            ExecNode::Sync(_) => Some(true),
            _ => Some(false),
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
            ExecNode::Async(f) => f.run(),
            _ => panic!(),
        }
    }
}

impl ExecNode {
	fn apply(&self) {
		match self {
			ExecNode::Sync(f) => f.0.apply(),
            ExecNode::Async(f) => f.apply(),
            _ => (),
        };
	}
}

pub struct StageBuilder {
    components: HashSet<usize>,
    systems: Vec<GraphNode>,
    edges: Vec<(usize, usize)>,
}

impl StageBuilder {
    pub fn new() -> Self {
        StageBuilder {
            components: HashSet::default(),
            systems: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_node<T: Into<GraphNode>>(&mut self, node: T) -> &mut Self {
        let node = node.into();

        for k in &node.reads {
            self.components.insert(*k);
            self.edges.push((*k, node.id));
        }
        for k in &node.writes {
            self.components.insert(*k);
            self.edges.push((node.id, *k));
        }
        self.systems.push(node);
        self
    }

    pub fn order(mut self, before: usize, after: usize) -> Self {
        self.edges.push((before, after));
        self
    }
    pub fn build(self) -> NGraph<usize, ExecNode> {
        let mut builder = NGraphBuilder::new();
        for id in self.components {
            builder = builder.node(id, ExecNode::None);
        }
        for n in self.systems {
            builder = builder.node(n.id, n.node);
        }
        for n in self.edges {
            builder = builder.edge(n.0, n.1);
        }
        builder.build().unwrap()
    }
}

pub trait Arrange {
	fn arrang(&self) -> Option<GraphNode>;
}
