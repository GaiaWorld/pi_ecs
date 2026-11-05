//! World 内的组件登记编号。
/// 当前 World 注册表内的组件编号；不得跨不兼容 World 混用。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ComponentId(
    /// 初始化登记时分配的编号，不是稳定序列化类型名。
    pub u32,
);
