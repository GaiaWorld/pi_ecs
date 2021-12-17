
pub struct StageBuilder<T> {
    builder: NGraphBuilder<TypeId, T>,
}

impl<T> StageBuilder<T> {
    pub fn new() -> Self {
        let builder = NGraphBuilder::new();
        StageBuilder { builder }
    }
    pub fn sys(mut self, state: SystemState) -> Self {
        self.builder.node();
        self
    }
    pub fn set(mut self, from: K, to: K) -> Self {
        let node = self.graph.map.get_mut(&from).unwrap();
        node.to.push(to.clone());
        let node = self.graph.map.get_mut(&to).unwrap();
        node.from.push(from);
        self
    }
    pub fn build(mut self) -> NGraph<K, T> {
        for (k, v) in self.graph.map.iter() {
            if v.from.is_empty() {
                self.graph.from.push(k.clone());
            }
            if v.to.is_empty() {
                self.graph.to.push(k.clone());
            }
        }
        self.graph
    }
}