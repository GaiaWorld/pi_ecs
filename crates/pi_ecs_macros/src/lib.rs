//! 过程宏编译单元。生成代码应遵循 core 和 schedule 的真实声明。
//! 已注册的派生入口只转发到各自的展开模块。
mod bundle;
mod component;
mod query_data;
mod query_filter;
mod system_param;

/// # 用途
/// 从结构体 input 按字段顺序派生直接写入 Bundle 及系统参数的静态同形声明。
/// 非结构体、未知属性或不满足组件约束的字段会产生编译诊断。
#[proc_macro_derive(Bundle, attributes(bundle))]
pub fn derive_bundle(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    bundle::derive_bundle(input)
}

/// # 用途
/// 为具名字段结构体派生 QueryData，保留字段名及各字段实际返回类型。
#[proc_macro_derive(QueryData, attributes(query_data))]
pub fn derive_query_data(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    query_data::derive_query_data(input)
}
