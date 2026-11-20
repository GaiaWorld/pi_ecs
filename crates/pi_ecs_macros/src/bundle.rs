//! Bundle 派生设计壳；参考 bevy_ecs_macros 0.17.2 lib.rs::derive_bundle。
use proc_macro::TokenStream;
#[path = "bundle_inner.rs"]
mod bundle_inner;
/// # 用途
/// input 为完整 derive 输入；只面向结构体，保留泛型、const 参数、where 及字段原 span。
/// 目标实现 core::bundle::Bundle：类型描述和消费值必须使用同一字段顺序，不隐式注册。
/// 参考 Bevy 的命名/元组字段遍历与 split_for_impl，不复制 unsafe Bundle/MovingPtr 或逆向重建接口。
/// 未知属性、非法目标和无法合法移动字段的自定义 Drop 组合不得输出假成功代码。
/// 普通字段只能经 BundleWriter::write 的 PlainAccess 通道直接写入，不拆出内置头。
/// 字段默认要求 Component<Access = PlainAccess>，用 BundleWriter::write 移入；#[bundle(nested)] 要求 Bundle，
/// 递归调用 Bundle::component_types/validate/write；三者严格使用相同字段顺序，不生成拥有值暂存。
/// 保留输入结构的寿命泛型，允许 nested 强弱输入；不得添加 Self: static 或擦除拥有引用寿命。
/// 不提供忽略字段，未知/重复字段属性报错；容器仅接受 #[bundle(crate = path)]，路径解析规则同 Component。
/// () 空组合及 Bundle 成员的 1..8 元组由 core 提供；不生成 Component blanket 或附送叶子 Bundle 实现。
/// 对有 Drop 的结构体用安全字段移动，让 Rust 拒绝非法拆分，不使用 ManuallyDrop/裸读绕过。
/// 类型展平后的重复必须拒绝；通过类型别名或泛型形成的重复也不得仅靠宏文本比较遗漏。
/// 另为自身声明 SystemBundle：显式寿命映射为 'static，类型/常量参数保留；
/// 派生的系统形状适配要求类型参数静态；用作 B/Add 的静态形状本身仍须实现 Bundle。宿主不增加此限制。
pub(crate) fn derive_bundle(input: TokenStream) -> TokenStream {
    bundle_inner::derive_bundle_inner(input)
}
// 验收覆盖泛型/where/重命名依赖、空/命名/元组结构、嵌套与重复类型、非法字段及 Drop；尚未执行。
