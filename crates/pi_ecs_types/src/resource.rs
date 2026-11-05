//! 独立于组件命名空间的资源编号。
/// 资源编号，不与 ComponentId 共用命名空间。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ResourceId(
    /// 本 World 登记编号。
    pub(crate) u32,
);
