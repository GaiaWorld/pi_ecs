use std::borrow::Cow;
use std::sync::Arc;
use std::{collections::HashSet, io::Result};

use futures::future::BoxFuture;
use pi_async::rt::{AsyncRuntime, AsyncTaskPool, AsyncTaskPoolExt};
use fixedbitset::FixedBitSet;
use thiserror::Error;

use pi_async_graph::{async_graph, Runnble, Runner};
use pi_graph::{DirectedGraph, DirectedGraphNode, NGraph, NGraphBuilder};
use crate::{
	query::Access,
	archetype::ArchetypeComponentId,
	world::World,
	storage::Local,
};

pub trait Arrange {
    fn arrange(&self) -> Option<GraphNode>;
}

/// Stage 是 由 可执行节点 组成的 图
type Stage = Arc<NGraph<usize, ExecNode>>;

/// 派发器 接口
pub trait Dispatcher {
    /// 只有 run 方法
    fn run(&self);
}

/// 串行 派发器
pub struct SingleDispatcher<P>
where
    P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>,
{
    /// 异步运行时
    rt: AsyncRuntime<(), P>,
    /// 派发器 包含 一组 Stage
    vec: Arc<Vec<Stage>>,
}

impl<P> SingleDispatcher<P>
where
    P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>,
{
    pub fn init(&mut self, vec: Vec<Stage>, arrange: &World) {
        let mut v1 = Vec::new();
        for i in vec.into_iter() {
            v1.push(i);

            // arrange node
            if let Some(node) = arrange.arrange() {
                let mut stage = StageBuilder::new();
                stage.add_node(node);

                v1.push(Arc::new(stage.build(arrange)))
            }
        }
		self.vec =  Arc::new(v1);
    }

	pub fn new(rt: AsyncRuntime<(), P>) -> Self {
        SingleDispatcher {
            vec: Arc::new(Vec::new()),
            rt,
        }
    }

    /// 执行指定阶段的指定节点
    pub fn exec(
        vec: Arc<Vec<Stage>>,
        rt: AsyncRuntime<(), P>,
        mut stage_index: usize,
        mut node_index: usize,
    ) {
        while stage_index < vec.len() {
            let g = &vec[stage_index];
            let arr = g.topological_sort();
            if node_index >= arr.len() {
                // stage结束，apply
                for elem in arr {
                    let node = g.get(elem).unwrap().value();
                    node.apply();
                }
                stage_index += 1;
                node_index = 0;
                continue;
            }
            let node = g.get(&arr[node_index]).unwrap().value();
            node_index += 1;
            if let Some(sync) = node.is_sync() {
                if sync {
                    node.get_sync().run();
                } else {
                    let f = node.get_async();
                    let vec1 = vec.clone();
                    let rt1 = rt.clone();
                    rt.spawn(rt.alloc(), async move {
                        f.await.unwrap();
                        SingleDispatcher::exec(vec1, rt1, stage_index, node_index);
                    })
                    .unwrap();
                    return;
                }
            }
        }
    }
}

impl<P> Dispatcher for SingleDispatcher<P>
where
    P: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P>,
{
    /// 同步节点自己执行， 如果有异步节点，则用单线程运行时执行
    fn run(&self) {
        Self::exec(self.vec.clone(), self.rt.clone(), 0, 0);
    }
}
pub struct MultiDispatcher<P1, P2>(Arc<MultiInner<P1, P2>>)
where
    P1: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P1>,
    P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>;

impl<P1, P2> MultiDispatcher<P1, P2>
where
    P1: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P1>,
    P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>,
{
    pub fn new(
        vec: Vec<(Stage, Option<AsyncRuntime<(), P2>>)>,
        multi: AsyncRuntime<(), P1>,
    ) -> Self {
        MultiDispatcher(Arc::new(MultiInner::new(vec, multi)))
    }
}

impl<
        P1: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P1>,
        P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>,
    > Dispatcher for MultiDispatcher<P1, P2>
{
    /// 根据阶段是单线程还是多线程，
    /// 如果多线程阶段，同步节点和异步节点，则用多线程运行时并行执行
    /// 如果单线程阶段，同步节点自己执行， 如果有异步节点，则用单线程运行时执行
    /// 一般为了线程安全，第一个阶段都是单线程执行
    fn run(&self) {
        let c = self.0.clone();
        exec(c, 0)
    }
}

struct MultiInner<P1, P2>
where
    P1: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P1>,
    P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>,
{
    vec: Vec<(Stage, Option<AsyncRuntime<(), P2>>)>,
    multi: AsyncRuntime<(), P1>,
}

impl<P1, P2> MultiInner<P1, P2>
where
    P1: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P1>,
    P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>,
{
    pub fn new(
        vec: Vec<(Stage, Option<AsyncRuntime<(), P2>>)>,
        multi: AsyncRuntime<(), P1>,
    ) -> Self {
        MultiInner { vec, multi }
    }
}

/// 执行指定阶段
fn exec<P1, P2>(d: Arc<MultiInner<P1, P2>>, stage_index: usize)
where
    P1: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P1>,
    P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>,
{
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

/// 单线程执行, 尽量本线程运行，遇到异步节点则用单线程运行时运行
fn single_exec<P1, P2>(
    d: Arc<MultiInner<P1, P2>>,
    stage_index: usize,
    mut node_index: usize,
    single: AsyncRuntime<(), P2>,
) where
    P1: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P1>,
    P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>,
{
    let g = &d.vec[stage_index].0;
    let single1 = single.clone();
    loop {
        let arr = g.topological_sort();
        if node_index >= g.node_count() {
            // stage结束，apply
            for elem in arr {
                let node = g.get(elem).unwrap().value();
                node.apply();
            }

            // 本阶段执行完毕，执行下一阶段
            return exec(d, stage_index + 1);
        }

        let node = g.get(&arr[node_index]).unwrap().value();
        node_index += 1;
        if let Some(sync) = node.is_sync() {
            if sync {
                if stage_index > 0 && node_index == 1 {
                    let f = node.get_sync();
                    let d1 = d.clone();
                    single1
                        .spawn(single1.alloc(), async move {
                            f.run();
                            single_exec(d1, stage_index, node_index, single);
                        })
                        .unwrap();
                    return;
                }
                // 如果是最开始的阶段， 或者非起始节点，则立即同步执行
                node.get_sync().run();
            } else {
                let f = node.get_async();
                let d1 = d.clone();
                single1
                    .spawn(single1.alloc(), async move {
                        let _ = f.await;
                        single_exec(d1, stage_index, node_index, single);
                    })
                    .unwrap();
                return;
            }
        }
    }
}

/// 多线程执行
fn multi_exec<P1, P2>(d: Arc<MultiInner<P1, P2>>, stage_index: usize)
where
    P1: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P1>,
    P2: AsyncTaskPoolExt<()> + AsyncTaskPool<(), Pool = P2>,
{
    let d1 = d.clone();
    d.multi
        .spawn(d.multi.alloc(), async move {
            let g = &d1.vec[stage_index].0;
            let r = async_graph(d1.multi.clone(), g.clone()).await;
            if r.is_ok() {
                // stage结束，apply
                let arr = g.topological_sort();
                for elem in arr {
                    let node = g.get(elem).unwrap().value();
                    node.apply();
                }

                exec(d1, stage_index + 1);
            }
        })
        .unwrap();
}

/// 图 的 节点
pub struct GraphNode {
    // 节点id，每个节点有 独一无二的 id
    pub(crate) id: usize,
    // // 节点的输入，意义是 它 依赖 的 节点，决定 执行关系
    // pub(crate) reads: Vec<usize>,
    // // 节点的输出，意义是 依赖 它 的 节点，决定 执行关系
    // pub(crate) writes: Vec<usize>,
	pub(crate) access: Access<ArchetypeComponentId>,
    // 执行节点
    pub(crate) node: ExecNode,

	pub(crate) label: String,
}

/// 操作
pub trait Operate: Send + 'static {
    /// 返回类型
    type R;

    /// 执行
    /// 执行结果，会缓冲到 内部，等当前Stage全部执行结束后，再统一调用apply，刷新到world上
    fn run(&self) -> Self::R;

    /// 应用
    /// 在该stage所有的system run 结束之后 执行
    /// 扫描所有的system，将当前缓冲的数据 刷新到 world 上
    fn apply(&self);

	fn name(&self) -> Cow<'static, str>;
}

/// 对操作的 封装
pub struct Run(pub(crate) Arc<dyn Operate<R = ()>>);
unsafe impl Send for Run {}

impl Runner for Run {
    fn run(self) {
        self.0.run()
    }
}

/// 执行节点
pub enum ExecNode {
    /// 不执行任何操作
    None,
    /// 同步函数
    Sync(Run),
    /// 异步函数
    Async(Box<dyn Operate<R = BoxFuture<'static, Result<()>>>>),
}

unsafe impl Sync for ExecNode {}

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

/// 阶段构造器
#[derive(Default)]
pub struct StageBuilder {
    // 所有的节点 id
    components: HashSet<usize>,
    // 节点
    systems: Vec<GraphNode>,
    // 边，(输入节点id, 输出节点id)
    edges: Vec<(usize, usize)>,
}

impl StageBuilder {
    /// 创建
    pub fn new() -> Self {
        StageBuilder::default()
    }

    /// 加入节点
    pub fn add_node<T: Into<GraphNode>>(&mut self, node: T) -> &mut Self {
        let node = node.into();

        // for k in &node.reads {
        //     self.components.insert(*k);

        //     // 边: 输入 --> 该节点
        //     self.edges.push((*k, node.id));
        // }

        // for k in &node.writes {
        //     self.components.insert(*k);

        //     // 边: 该节点 --> 输出
        //     self.edges.push((node.id, *k));
        // }

        // 加入 System
        self.systems.push(node);

        self
    }

    /// 显示指定 节点的依赖 关系
    pub fn order(mut self, before: usize, after: usize) -> Self {
        // 添加边: before --> after
        self.edges.push((before, after));

        self
    }

    /// 构建 拓扑 序
    pub fn build(mut self, w: &World) -> NGraph<usize, ExecNode> {
        // Stages --> NGraph
        let mut builder = NGraphBuilder::new();

		for s in self.systems.iter() {
			match write_depend(w, s.access.get_reads_and_writes(), s.access.get_writes(), s.access.get_modify()) {
				Ok((mut r, w)) => {
					r.difference_with(&w);

					// 边: 该节点 --> 输出
					for k in r.ones() {
					    self.components.insert(k);
					    self.edges.push((k, s.id));
					}

					for k in w.ones() {
					    self.components.insert(k);
					    self.edges.push((s.id, k));
					}
				},
				Err(c) => {
					let c: Vec<String> = c.ones().map(|i| {(*&w.archetypes().archetype_component_info[i]).clone()}).collect();
					panic!("{:?}", BuildErr::WriteConflict(s.label.clone(), c));
				}
			}
		}

        for id in self.components {
            // 每个 Component 都是一个节点
            builder = builder.node(id, ExecNode::None);
        }

        for n in self.systems {
            // 每个 System 都是 一个 可执行节点
            builder = builder.node(n.id, n.node);
        }

        for n in self.edges {
            // 边 对应 Graph 的 边
            builder = builder.edge(n.0, n.1);
        }

        builder.build().unwrap()
    }
}

#[derive(Debug, Error)]
pub enum BuildErr {
	#[error("build fail, node is circly: {0:?}")]
	Circly(Vec<usize>),
	#[error("build fail, write conflict, system: {0:?}, write access {1:?}")]
	WriteConflict(String, Vec<String>)
}


fn write_depend(w: &World, read_writes: &FixedBitSet, writes: &FixedBitSet, modifys: &FixedBitSet) -> std::result::Result<(FixedBitSet, FixedBitSet), FixedBitSet> {
	let (mut read_writes_new, mut write_new) = (read_writes.clone(), writes.clone());
	let conflict = FixedBitSet::default();

	for write in modifys.ones().into_iter() {
		// 取到对应监听器的写入，判断冲突
		let v = w.listener_access.get(Local::new(write));
		if let Some(r) = v {
			for access in r.iter() {
				// 收集监听器的写入是否与访问冲突
				let mut conflict = read_writes.clone();
				conflict.intersect_with(access.combined_access().get_writes());

				if conflict.count_ones(..) == 0 {
					let (mut r1, mut w1) = (read_writes.clone(), writes.clone());
					r1.union_with(access.combined_access().get_reads_and_writes());
					w1.union_with(access.combined_access().get_writes());
					match write_depend(w, &r1, &w1,  access.combined_access().get_modify()) {
						Ok((r, w)) => {
							read_writes_new.union_with(&r);
							write_new.union_with(&w);
						},
						Err(c) => conflict.union_with(&c),
					};
				}
			}
		}
	}

	if conflict.count_ones(..) > 0 {
		println!("len: {:?}, {:?}", conflict.count_ones(..), conflict);
		Err(conflict)
	} else {
		Ok((read_writes_new, write_new))
	}
}