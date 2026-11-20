//! 插件只组装，数据归 World，执行状态归调度。
use crate::app::App;
use pi_ecs_core::error::EcsError;
/// 插件是可带字段的安装配置，不持独立身份分配器，不提供每帧 update。
/// 同具体类型仅安装一次，顺序显式；业务状态归组件/资源/Local，无热卸载或解冻。
pub trait Plugin: 'static {
    /// app 处于 Building；登记类型、资源、快照能力、调度/阶段和系统，可安装子插件。
    /// 依赖通过 App::require_plugin 显式检查，不自动安装/排序；失败可部分登记并停止 App。
    /// 自己尚未算成功依赖；嵌套子插件先成功先登记，finish 顺序也在父插件之前。
    fn build(&self, app: &mut App) -> Result<(), EcsError>;
    /// app 处于 Finishing；所有 build 完成后按成功安装顺序补连接及检查依赖。
    /// 禁止安装插件，不执行初始化系统；失败停止且不撤销。无收束需求的插件默认无操作。
    /// 回调对象由 App 临时移出至调用栈，当前类型仍标记已安装；不得据此移除或替换插件槽。
    fn finish(&self, _app: &mut App) -> Result<(), EcsError> {
        Ok(())
    }
}
