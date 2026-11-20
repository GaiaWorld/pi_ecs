//! 命名过滤派生设计，参考 Bevy 0.17.2 QueryFilter 派生。
use proc_macro::TokenStream;
#[path = "query_filter_inner.rs"]
mod query_filter_inner;
/// 为结构体生成 unsafe impl QueryFilter，字段必须各自实现 QueryFilter，按声明顺序 AND 组合。
/// 复用字段元组的 State/FetchState/Fetch 和全部方法，IS_ARCHETYPAL 为字段标记合取。
/// 嵌套组合保留 Or/AND 边界；不构造字段业务值、不开放可写过滤、不以变化条件证明互斥。
/// 保留泛型/where/span，支持重命名路径及 #[query_filter(crate = path)]；未知属性定位报错。
/// 空结构等价 ()，大于八字段使用嵌套元组降低适配层数限制，不添加运行期管理器。
/// 尚未注册宏或执行展开验收；编译壳不表示派生已实现。
pub(crate) fn derive_query_filter(input: TokenStream) -> TokenStream {
    query_filter_inner::derive_inner(input)
}
