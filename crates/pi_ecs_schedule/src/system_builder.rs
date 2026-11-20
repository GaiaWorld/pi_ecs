//! 函数和逐位置参数配置构造；不访问 World，不登记订阅，不构建调度图。
#[path = "system_builder_inner.rs"]
mod system_builder_inner;
use crate::{
    system::{FunctionSystem, SystemFunction, SystemId},
    system_param::SystemParam,
};
use pi_ecs_core::error::EcsError;
/// # 用途
/// 拥有完整参数配置，不允许用任意 State 工厂绕过 initialize。
pub struct ParamConfig<P: SystemParam> {
    /// 每参数位置独立配置，首次 initialize 消费。
    pub(crate) value: P::Config,
}

#[cfg(all(test, feature = "rc"))]
mod compile_checks {
    use super::*;
    use crate::{
        local::{Local, LocalConfig},
        resource_param::{MultiResConfig, MultiResMut, Res},
        stage::Stage,
    };
    use pi_ecs_core::alter::QueryAlter;
    // 只编译，不执行构造/运行失败桩；验证普通函数推导及 Stage 登记完整链。
    fn no_args() {}
    fn resource(_value: Res<'_, u32>) {}
    fn locals(_first: Local<'_, u32>, _second: Local<'_, u32>) {}
    fn configured(
        _value: Local<'_, u32>,
        _output: MultiResMut<'_, u32>,
        _alter: QueryAlter<'_, ()>,
    ) {
    }
    fn build(stage: &mut Stage) -> Result<(), EcsError> {
        stage.add_system(SystemBuilder::new(no_args).build(SystemId(100))?)?;
        stage.add_system(SystemBuilder::new(resource).build(SystemId(101))?)?;
        stage.add_system(
            SystemBuilder::with_config(locals, (LocalConfig::new(1), LocalConfig::new(2)))
                .build(SystemId(102))?,
        )?;
        stage.add_system(
            SystemBuilder::with_config(
                configured,
                (LocalConfig::new(0), MultiResConfig::new(0), ()),
            )
            .build(SystemId(103))?,
        )?;
        let value = std::rc::Rc::new(std::cell::Cell::new(0));
        let closure = move |_counter: Local<'_, u32>| {
            value.set(value.get() + 1);
        };
        stage.add_system(SystemBuilder::new(closure).build(SystemId(104))?)?;
        Ok(())
    }
}
impl<P: SystemParam> ParamConfig<P> {
    /// # 用途
    /// value 是完整配置，接管所有权，不调用初始化或用户工厂。
    pub fn new(value: P::Config) -> Self {
        Self::config_inner(value)
    }
}
/// # 用途
/// 拥有函数及其完整配置，Marker 为静态函数签名标记，构造不绑定 World。
/// RC 允许 !Send 捕获；登记 Stage 时函数及配置状态所需类型必须满足 static，不能捕获外部短借用。
pub struct SystemBuilder<F: SystemFunction<Marker>, Marker> {
    /// 普通 FnMut 函数/闭包或高级手写适配，首版返回 ()。
    pub(crate) function: F,
    /// 与 F::Param 对应，元组/NamedParam/ParamSet 按位置同形，Local 不按类型合并。
    pub(crate) config: ParamConfig<F::Param>,
    /// 不拥有 Marker，不施加额外线程约束。
    pub(crate) signature: std::marker::PhantomData<fn() -> Marker>,
}
impl<F: SystemFunction<Marker>, Marker> SystemBuilder<F, Marker> {
    /// # 用途
    /// function 对应完整参数签名，config 为其关联 Config；这里只接管值，不登记或访问 World。
    /// 不构造 State；配置不匹配编译拒绝，未知组件/缺资源等留给 initialize/prepare。
    pub fn with_config(function: F, config: <F::Param as SystemParam>::Config) -> Self {
        Self::with_config_inner(function, config)
    }
    /// # 用途
    /// function 使用默认参数配置，要求整个 Config:Default；构建时调用 Default，panic 原样传播。
    /// Local/MultiRes 的默认初值只构造一次，不在每次运行重置；不隐式配置快照策略。
    pub fn new(function: F) -> Self
    where
        <F::Param as SystemParam>::Config: Default,
    {
        Self::new_inner(function)
    }
    /// # 用途
    /// id 为调用方分配身份，重复由 Stage 登记拒绝；得到未初始化系统，不执行函数或参数初始化。
    /// 未初始化销毁仅释放函数及配置；初始化后必须由所属调度先 unregister 再释放。
    pub fn build(self, id: SystemId) -> Result<FunctionSystem<F, F::Param, Marker>, EcsError> {
        self.build_inner(id)
    }
}
