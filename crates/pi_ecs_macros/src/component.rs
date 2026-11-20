//! Component 派生设计；参考 bevy_ecs_macros 0.17.2 component.rs 的泛型展开与属性诊断。
use proc_macro::TokenStream;
#[path = "component_inner.rs"]
mod component_inner;
/// 普通组件派生入口；input 是完整 derive 输入，不读取 World、不登记组件、不生成编号。
/// 支持结构体/枚举/联合体及原有类型、生命周期、const 泛型与 where 子句；生成 Self: 'static 约束。
/// 展开只实现 Component，type Access = PlainAccess；不推导 Clone/Default/Send/Sync 或存储/恢复能力。
/// 编译器检查类型是否满足 static，不能删掉生命周期参数或擅自给所有字段添加 Component 约束。
/// 框架内置组件继续手写专用策略，不通过该宏重选策略；重复 impl 由编译器拒绝。
/// 路径解析支持 facade pi_ecs 与直接 core 依赖及 Cargo 重命名；歧义允许显式 #[component(crate = path)]。
/// 首版 component 属性仅支持 crate，重复/未知键或错误位置输出带原 span 的 compile_error，不静默忽略。
/// 错误输入不能 panic 或输出空 TokenStream 假成功；设计未实现阶段调用显式失败。
/// # Examples
/// 目标展开：impl Component for Position { type Access = PlainAccess; }，实际路径使用解析后的 crate 路径。
pub(crate) fn derive_component(input: TokenStream) -> TokenStream {
    /* 设计已确认：
    解析 derive 输入和 component 属性，保留标识符/泛型/where/span。
    解析依赖路径，生成只增加 Self: static 的 Component 实现及 PlainAccess 关联类型。
    错误按原位置生成编译诊断，不生成运行态注册或任何生命周期强转。
    */
    component_inner::derive_component_inner(input)
}
// 验收：结构体/枚举/union、泛型/const/where、static 与借用失败、非 Send/Sync 值、重命名依赖、
// 未知/重复属性、重复实现；对比展开并做编译成功/失败测试。当前未实现，尚未执行这些测试。
