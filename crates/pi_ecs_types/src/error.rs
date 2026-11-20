//! ECS 错误单源使用 crates.io 的 pi_result，不自建字符串错误容器。
pub use pi_result;
/// # 用途
/// Report 扩展及 typed context/附件工具使用真实依赖门面，不另包装错误栈。
pub use pi_result::prelude;
pub use pi_result::{Error as EcsError, ErrorKind, Report, Result as EcsResult};
/// # 用途
/// kind 为调用分支显式选择的公共分类，message 为保留的诊断附件，不解析文本推断分类。
/// 不吞底层来源；已有 Report 应直接传播或补 context，不能用本入口格式化重建错误链。
/// 分类不代表 World 可继续运行，停止/关闭状态以实际生命周期门禁为准。
pub fn classified_error(kind: ErrorKind, message: impl Into<String>) -> EcsError {
    Report::new(kind).attach(message.into())
}
