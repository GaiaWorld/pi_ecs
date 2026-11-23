//! 命名组合派生；参考 bevy_ecs_macros 0.17.2 lib.rs::derive_system_param。
use proc_macro::TokenStream;
#[path = "system_param_inner.rs"]
mod system_param_inner;
/// 为命名结构体生成直接 unsafe impl SystemParam，不生成额外 Param 描述别名或强制 NamedParam 包装。
/// input 保留类型/const 泛型和 where；首版恰好一个运行借用生命周期，最多八个字段，不支持忽略字段。
/// 每字段必须是已实现 SystemParam 的描述，其 Item 在重绑运行寿命后须等于该字段的实际类型。
/// Config/State/Pending 分别为字段关联类型元组；Item<'run> 是把结构体运行寿命替换为 'run 的自身。
/// initialize/prepare/declaration/ordering_access/begin/finish/unregister 均委托字段元组；
/// declaration 保持原 Together/Exclusive 树形，ordering_access 保留字段完整潜在访问，不从 prepare 结果反推。
/// fetch 先调用元组 fetch，再安全按字段重组，不漏 trait 必需方法。
/// 注销及初始化失败清理继承元组完整清偿契约，不生成遇 Err 提前跳过后续字段的清理逻辑。
/// begin/fetch 直接返回工作区/参数，生命周期错误直接 panic；不生成捕获、失败标记或展开时补调用 finish。
/// 嵌套直接使用另一个派生参数结构体；访问树保留同时借用关系，ParamSet 字段保留自身互斥边界。
/// 不生成自定义 fetch 权限、生命周期 transmute、Send/Sync、默认业务值或提前提交；不得绕过 Item 类型等式。
/// 生成 unsafe impl 的安全理由是全部权限/生命周期行为委托现有合法字段参数，仅重组返回值。
/// crate 路径支持重命名及 #[system_param(crate = path)]；未知/重复属性、enum/union、错误寿命和超限字段定位报错。
/// 带自定义 Drop 的结果不得自动执行结构提交；与现有显式 finish 协议相同，panic 停止而非回滚。
/// 显式构建使用 NamedStruct<'static,...> 描述，函数接收 NamedStruct<'_,...>；static 描述不代表结果为 static。
pub(crate) fn derive_system_param(input: TokenStream) -> TokenStream {
    system_param_inner::derive_system_param_inner(input)
}
// 验收包含嵌套、元组字段、Local 配置、Reader 独立游标、Alter finish、ParamSet、别名拒绝、泛型/where、
// 注销首成员报错后仍清理后续字段、初始化部分失败的完整清偿、错误生命周期和 Item 等式诊断；
// 命令字段覆盖 &CommandSender<C>/CommandRead<C>：消费视图不暴露拥有句柄，失败清理归还唯一资格。
// 宏未注册、展开与编译验收尚未执行。
