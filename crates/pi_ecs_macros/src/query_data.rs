//! 具名 QueryData 派生设计壳。
use proc_macro::TokenStream;
#[path = "query_data_inner.rs"]
mod query_data_inner;

/// # 用途
/// 为具名字段结构体生成同可见性的 `<Name>Item`，并保留字段可见性。
/// 声明、匹配和取值共享同一字段元组；具名 Fetch 只重组结果，保留双寿命和真实策略类型。
/// 只读能力取决于所有字段的实际协议。支持生命周期、类型及常量参数；不跳过字段。
/// 容器可用 #[query_data(crate = path)] 指定 core 路径，字段不接受 query_data 属性。
pub(crate) fn derive_query_data(input: TokenStream) -> TokenStream {
    query_data_inner::derive_inner(input)
}
