//! 显式参数配置与系统构建，不构建执行图，不用任意 World 闭包伪造参数状态。
use crate::{
    system::{FunctionSystem, SystemFunction, SystemId},
    system_param::SystemParam,
};
use pi_ecs_core::error::EcsError;
/// 待初始化的参数配置，支持显式 Local/多例初值和恢复策略，配置内容由框架参数实现提供。
/// 不向外部暴露“任意 P::State 工厂”，防止绕过访问声明和订阅登记。
pub struct ParamConfig<P: SystemParam> {
    /// 按参数位置拥有配置；初始化时消费，不直接保存业务借用。
    pub(crate) value: P::Config,
    // @to_design Local/多例的具体配置类型及默认配置便捷构造待细化。
}
impl<P: SystemParam> ParamConfig<P> {
    /// value 为完整 P::Config，按参数位置组织；仅接管配置，不初始化或登记 World 订阅。
    pub fn new(value: P::Config) -> Self {
        crate::system_builder_inner::config_inner::<P>(value)
    }
}
/// 显式构建器拥有函数和参数配置；构建不执行系统或偷偷登记 World 订阅。
pub struct SystemBuilder<F, P: SystemParam> {
    /// 用户函数适配。
    pub(crate) function: F,
    /// 等待阶段前 initialize 使用的配置。
    pub(crate) config: ParamConfig<P>,
}
impl<F: SystemFunction<P>, P: SystemParam> SystemBuilder<F, P> {
    /// function 对应 P 的完整参数签名，config 提供其初始化配置。
    /// 只接管所有权，不访问业务数据，普通错误不丢失未初始化责任。
    pub fn new(function: F, config: ParamConfig<P>) -> Self {
        crate::system_builder_inner::new_inner(function, config)
    }
    /// id 为调度登记分配的身份，重复校验由登记入口执行；返回未初始化系统。
    /// 参数在阶段前才建立状态/订阅，失败须清理本次部分登记，不在此构图。
    pub fn build(self, id: SystemId) -> Result<FunctionSystem<F, P>, EcsError> {
        crate::system_builder_inner::build_inner(self, id)
    }
}
// @to_design Local/多例初值配置入口、函数推导及 Schedule 登记入口待闭合，宏后置。
