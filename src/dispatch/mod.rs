
use graph::{NGraph};
use async_graph::{ExecNode};

pub trait DisPatcher {
    fn setStage(self, name: Atom, graph: Arc<NGraph<TypeId, ExecNode>>, single: bool) -> Self;
    fn build(self) -> Self;
    fn run(&self);
}
struct Sys(usize);
impl RunFactory for Sys {
    type R = A;
    fn create(&self) -> A {
        A(self.0)
    }
}
pub struct Access {
    reads: Vec<TypeId>,
    writes: Vec<TypeId>,
    node: ExecNode<(), ()>,
}