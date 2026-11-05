//! 错误的暂定承载形状，不代表错误分类已经冻结。
/// ECS 操作失败；恢复阶段的状态保证见对应方法契约。
/// @to_design 在复制、安装、重建协议确定后细化为可匹配的错误分类，确定 panic 与析构失败规则。
#[derive(Debug)]
pub struct EcsError {
    /// 中文诊断信息；当前不作为稳定的程序匹配协议。
    pub message: String,
}
