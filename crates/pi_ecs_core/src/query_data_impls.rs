//! 内置 QueryData 适配：RC 通过受限 fetch 访问活槽。
use crate::{
    archetype::Archetype,
    component::{Component, ComponentRead, ComponentWrite, PlainAccess},
    entity::{Entity, EntityLocation},
    error::EcsError,
    query::{
        private::{DataSealed, Sealed},
        QueryData, QueryDataDeclaration, ReadOnlyQueryData,
    },
    query_data::*,
    query_fetch::*,
    world::World,
};
use std::marker::PhantomData;
#[path = "query_data_impls_inner.rs"]
mod query_data_impls_inner;
impl DataSealed for Entity {}
unsafe impl QueryData for Entity {
    type Item<'a, 'w> = Entity;
    type State = ();
    type FetchState = ();
    type Fetch<'w> = MetadataFetch<'w, Self>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    /// Ref/OrDefaultRef 声明真实 tick_reads，Has/普通共享读取不因存在追踪就声明 tick 观察。
    /// 可变项的自身 tick 读取由 Write 覆盖，不能因调用方可能不修改就省略写授权。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl ReadOnlyQueryData for Entity {}
impl DataSealed for EntityLocation {}
unsafe impl QueryData for EntityLocation {
    type Item<'a, 'w> = EntityLocation;
    type State = ();
    type FetchState = ();
    type Fetch<'w> = MetadataFetch<'w, Self>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl ReadOnlyQueryData for EntityLocation {}
impl DataSealed for () {}
unsafe impl QueryData for () {
    type Item<'a, 'w> = ();
    type State = ();
    type FetchState = ();
    type Fetch<'w> = MetadataFetch<'w, Self>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl ReadOnlyQueryData for () {}
impl DataSealed for &'static Archetype {}
unsafe impl QueryData for &'static Archetype {
    type Item<'a, 'w> = &'a Archetype;
    type State = ();
    type FetchState = ();
    type Fetch<'w> = MetadataFetch<'w, Self>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl ReadOnlyQueryData for &'static Archetype {}
impl<T: ?Sized> DataSealed for PhantomData<T> {}
unsafe impl<T: ?Sized> QueryData for PhantomData<T> {
    type Item<'a, 'w> = ();
    type State = ();
    type FetchState = ();
    type Fetch<'w> = MetadataFetch<'w, Self>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl<T: ?Sized> ReadOnlyQueryData for PhantomData<T> {}
impl<C: Component> DataSealed for &'static C {}
unsafe impl<C: Component> QueryData for &'static C {
    type Item<'a, 'w> = ComponentRead<'a, 'w, C>;
    type State = ComponentQueryState;
    type FetchState = ComponentQueryFetchState;
    type Fetch<'w> = ComponentFetch<'w, Self, C>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl<C: Component> ReadOnlyQueryData for &'static C {}
impl<C: Component> DataSealed for Option<&'static C> {}
unsafe impl<C: Component> QueryData for Option<&'static C> {
    type Item<'a, 'w> = Option<ComponentRead<'a, 'w, C>>;
    type State = ComponentQueryState;
    type FetchState = ComponentQueryFetchState;
    type Fetch<'w> = ComponentFetch<'w, Self, C>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl<C: Component> ReadOnlyQueryData for Option<&'static C> {}
impl<C: Component> DataSealed for &'static mut C {}
unsafe impl<C: Component> QueryData for &'static mut C {
    type Item<'a, 'w> = ComponentWrite<'a, 'w, C>;
    type State = ComponentQueryState;
    type FetchState = ComponentQueryFetchState;
    type Fetch<'w> = ComponentFetch<'w, Self, C>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
impl<C: Component> DataSealed for Option<&'static mut C> {}
unsafe impl<C: Component> QueryData for Option<&'static mut C> {
    type Item<'a, 'w> = Option<ComponentWrite<'a, 'w, C>>;
    type State = ComponentQueryState;
    type FetchState = ComponentQueryFetchState;
    type Fetch<'w> = ComponentFetch<'w, Self, C>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
impl<C: Component<Access = PlainAccess>> DataSealed for Ref<C> {}
unsafe impl<C: Component<Access = PlainAccess>> QueryData for Ref<C> {
    type Item<'a, 'w> = ValueRef<'a, C>;
    type State = ComponentQueryState;
    type FetchState = ComponentQueryFetchState;
    type Fetch<'w> = ComponentFetch<'w, Self, C>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl<C: Component<Access = PlainAccess>> ReadOnlyQueryData for Ref<C> {}
impl<C: Component<Access = PlainAccess>> DataSealed for OrDefault<C> {}
unsafe impl<C: Component<Access = PlainAccess>> QueryData for OrDefault<C> {
    type Item<'a, 'w> = &'a C;
    type State = ComponentQueryState;
    type FetchState = ComponentQueryFetchState;
    type Fetch<'w> = ComponentFetch<'w, Self, C>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl<C: Component<Access = PlainAccess>> ReadOnlyQueryData for OrDefault<C> {}
impl<C: Component<Access = PlainAccess>> DataSealed for OrDefaultRef<C> {}
unsafe impl<C: Component<Access = PlainAccess>> QueryData for OrDefaultRef<C> {
    type Item<'a, 'w> = ValueRef<'a, C>;
    type State = ComponentQueryState;
    type FetchState = ComponentQueryFetchState;
    type Fetch<'w> = ComponentFetch<'w, Self, C>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl<C: Component<Access = PlainAccess>> ReadOnlyQueryData for OrDefaultRef<C> {}
impl<C: Component> DataSealed for Has<C> {}
unsafe impl<C: Component> QueryData for Has<C> {
    type Item<'a, 'w> = bool;
    type State = ComponentQueryState;
    type FetchState = ComponentQueryFetchState;
    type Fetch<'w> = PresenceFetch<'w, C>;
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl<C: Component> ReadOnlyQueryData for Has<C> {}
impl<Q0: QueryData> DataSealed for (Q0,) {}
unsafe impl<Q0: QueryData> QueryData for (Q0,) {
    type Item<'a, 'w> = (Q0::Item<'a, 'w>,);
    type State = (Q0::State,);
    type FetchState = (Q0::FetchState,);
    type Fetch<'w> = (Q0::Fetch<'w>,);
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
impl<Q0: QueryData, Q1: QueryData> DataSealed for (Q0, Q1) {}
unsafe impl<Q0: QueryData, Q1: QueryData> QueryData for (Q0, Q1) {
    type Item<'a, 'w> = (Q0::Item<'a, 'w>, Q1::Item<'a, 'w>);
    type State = (Q0::State, Q1::State);
    type FetchState = (Q0::FetchState, Q1::FetchState);
    type Fetch<'w> = (Q0::Fetch<'w>, Q1::Fetch<'w>);
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
impl<Q0: QueryData, Q1: QueryData, Q2: QueryData> DataSealed for (Q0, Q1, Q2) {}
unsafe impl<Q0: QueryData, Q1: QueryData, Q2: QueryData> QueryData for (Q0, Q1, Q2) {
    type Item<'a, 'w> = (Q0::Item<'a, 'w>, Q1::Item<'a, 'w>, Q2::Item<'a, 'w>);
    type State = (Q0::State, Q1::State, Q2::State);
    type FetchState = (Q0::FetchState, Q1::FetchState, Q2::FetchState);
    type Fetch<'w> = (Q0::Fetch<'w>, Q1::Fetch<'w>, Q2::Fetch<'w>);
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
impl<Q0: QueryData, Q1: QueryData, Q2: QueryData, Q3: QueryData> DataSealed for (Q0, Q1, Q2, Q3) {}
unsafe impl<Q0: QueryData, Q1: QueryData, Q2: QueryData, Q3: QueryData> QueryData
    for (Q0, Q1, Q2, Q3)
{
    type Item<'a, 'w> = (
        Q0::Item<'a, 'w>,
        Q1::Item<'a, 'w>,
        Q2::Item<'a, 'w>,
        Q3::Item<'a, 'w>,
    );
    type State = (Q0::State, Q1::State, Q2::State, Q3::State);
    type FetchState = (
        Q0::FetchState,
        Q1::FetchState,
        Q2::FetchState,
        Q3::FetchState,
    );
    type Fetch<'w> = (Q0::Fetch<'w>, Q1::Fetch<'w>, Q2::Fetch<'w>, Q3::Fetch<'w>);
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
impl<Q0: QueryData, Q1: QueryData, Q2: QueryData, Q3: QueryData, Q4: QueryData> DataSealed
    for (Q0, Q1, Q2, Q3, Q4)
{
}
unsafe impl<Q0: QueryData, Q1: QueryData, Q2: QueryData, Q3: QueryData, Q4: QueryData> QueryData
    for (Q0, Q1, Q2, Q3, Q4)
{
    type Item<'a, 'w> = (
        Q0::Item<'a, 'w>,
        Q1::Item<'a, 'w>,
        Q2::Item<'a, 'w>,
        Q3::Item<'a, 'w>,
        Q4::Item<'a, 'w>,
    );
    type State = (Q0::State, Q1::State, Q2::State, Q3::State, Q4::State);
    type FetchState = (
        Q0::FetchState,
        Q1::FetchState,
        Q2::FetchState,
        Q3::FetchState,
        Q4::FetchState,
    );
    type Fetch<'w> = (
        Q0::Fetch<'w>,
        Q1::Fetch<'w>,
        Q2::Fetch<'w>,
        Q3::Fetch<'w>,
        Q4::Fetch<'w>,
    );
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
impl<Q0: QueryData, Q1: QueryData, Q2: QueryData, Q3: QueryData, Q4: QueryData, Q5: QueryData>
    DataSealed for (Q0, Q1, Q2, Q3, Q4, Q5)
{
}
unsafe impl<Q0: QueryData, Q1: QueryData, Q2: QueryData, Q3: QueryData, Q4: QueryData, Q5: QueryData>
    QueryData for (Q0, Q1, Q2, Q3, Q4, Q5)
{
    type Item<'a, 'w> = (
        Q0::Item<'a, 'w>,
        Q1::Item<'a, 'w>,
        Q2::Item<'a, 'w>,
        Q3::Item<'a, 'w>,
        Q4::Item<'a, 'w>,
        Q5::Item<'a, 'w>,
    );
    type State = (
        Q0::State,
        Q1::State,
        Q2::State,
        Q3::State,
        Q4::State,
        Q5::State,
    );
    type FetchState = (
        Q0::FetchState,
        Q1::FetchState,
        Q2::FetchState,
        Q3::FetchState,
        Q4::FetchState,
        Q5::FetchState,
    );
    type Fetch<'w> = (
        Q0::Fetch<'w>,
        Q1::Fetch<'w>,
        Q2::Fetch<'w>,
        Q3::Fetch<'w>,
        Q4::Fetch<'w>,
        Q5::Fetch<'w>,
    );
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
impl<
        Q0: QueryData,
        Q1: QueryData,
        Q2: QueryData,
        Q3: QueryData,
        Q4: QueryData,
        Q5: QueryData,
        Q6: QueryData,
    > DataSealed for (Q0, Q1, Q2, Q3, Q4, Q5, Q6)
{
}
unsafe impl<
        Q0: QueryData,
        Q1: QueryData,
        Q2: QueryData,
        Q3: QueryData,
        Q4: QueryData,
        Q5: QueryData,
        Q6: QueryData,
    > QueryData for (Q0, Q1, Q2, Q3, Q4, Q5, Q6)
{
    type Item<'a, 'w> = (
        Q0::Item<'a, 'w>,
        Q1::Item<'a, 'w>,
        Q2::Item<'a, 'w>,
        Q3::Item<'a, 'w>,
        Q4::Item<'a, 'w>,
        Q5::Item<'a, 'w>,
        Q6::Item<'a, 'w>,
    );
    type State = (
        Q0::State,
        Q1::State,
        Q2::State,
        Q3::State,
        Q4::State,
        Q5::State,
        Q6::State,
    );
    type FetchState = (
        Q0::FetchState,
        Q1::FetchState,
        Q2::FetchState,
        Q3::FetchState,
        Q4::FetchState,
        Q5::FetchState,
        Q6::FetchState,
    );
    type Fetch<'w> = (
        Q0::Fetch<'w>,
        Q1::Fetch<'w>,
        Q2::Fetch<'w>,
        Q3::Fetch<'w>,
        Q4::Fetch<'w>,
        Q5::Fetch<'w>,
        Q6::Fetch<'w>,
    );
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
impl<
        Q0: QueryData,
        Q1: QueryData,
        Q2: QueryData,
        Q3: QueryData,
        Q4: QueryData,
        Q5: QueryData,
        Q6: QueryData,
        Q7: QueryData,
    > DataSealed for (Q0, Q1, Q2, Q3, Q4, Q5, Q6, Q7)
{
}
unsafe impl<
        Q0: QueryData,
        Q1: QueryData,
        Q2: QueryData,
        Q3: QueryData,
        Q4: QueryData,
        Q5: QueryData,
        Q6: QueryData,
        Q7: QueryData,
    > QueryData for (Q0, Q1, Q2, Q3, Q4, Q5, Q6, Q7)
{
    type Item<'a, 'w> = (
        Q0::Item<'a, 'w>,
        Q1::Item<'a, 'w>,
        Q2::Item<'a, 'w>,
        Q3::Item<'a, 'w>,
        Q4::Item<'a, 'w>,
        Q5::Item<'a, 'w>,
        Q6::Item<'a, 'w>,
        Q7::Item<'a, 'w>,
    );
    type State = (
        Q0::State,
        Q1::State,
        Q2::State,
        Q3::State,
        Q4::State,
        Q5::State,
        Q6::State,
        Q7::State,
    );
    type FetchState = (
        Q0::FetchState,
        Q1::FetchState,
        Q2::FetchState,
        Q3::FetchState,
        Q4::FetchState,
        Q5::FetchState,
        Q6::FetchState,
        Q7::FetchState,
    );
    type Fetch<'w> = (
        Q0::Fetch<'w>,
        Q1::Fetch<'w>,
        Q2::Fetch<'w>,
        Q3::Fetch<'w>,
        Q4::Fetch<'w>,
        Q5::Fetch<'w>,
        Q6::Fetch<'w>,
        Q7::Fetch<'w>,
    );
    /// 解析已登记类型并展开访问，拒绝内部别名；默认来源缺失报错，不在 fetch 隐式创建。
    fn init_state(world: &World) -> Result<Self::State, EcsError> {
        QueryDataBinding::<Self>::init_inner(world)
    }
    /// 报告数据描述的实际组件访问与变化需求；不读取业务值或取得权限。
    /// # 用途
    /// state/scope/binding 同源，一次完成匹配和字段地址绑定；Table 未能统一为 NeedsVariants，具体原型不匹配为 NotMatched。
    /// 元组逐字段组合，真实错误不被排除掩盖；Ready 是完整状态，不持临时业务引用。
    fn init_fetch_state(
        state: &Self::State,
        scope: FetchScope,
        binding: &FetchBinding<'_>,
    ) -> Result<crate::query::FetchStateResult<Self::FetchState>, EcsError> {
        QueryDataBinding::<Self>::init_fetch_inner(state, scope, binding)
    }
    /// # 用途
    /// 返回完整静态访问与追踪需求，不读取业务值或新增运行权限。
    fn declare_access(state: &Self::State) -> Result<QueryDataDeclaration, EcsError> {
        QueryDataBinding::<Self>::declare_inner(state)
    }
}
unsafe impl<Q0: ReadOnlyQueryData> ReadOnlyQueryData for (Q0,) {}
unsafe impl<Q0: ReadOnlyQueryData, Q1: ReadOnlyQueryData> ReadOnlyQueryData for (Q0, Q1) {}
unsafe impl<Q0: ReadOnlyQueryData, Q1: ReadOnlyQueryData, Q2: ReadOnlyQueryData> ReadOnlyQueryData
    for (Q0, Q1, Q2)
{
}
unsafe impl<
        Q0: ReadOnlyQueryData,
        Q1: ReadOnlyQueryData,
        Q2: ReadOnlyQueryData,
        Q3: ReadOnlyQueryData,
    > ReadOnlyQueryData for (Q0, Q1, Q2, Q3)
{
}
unsafe impl<
        Q0: ReadOnlyQueryData,
        Q1: ReadOnlyQueryData,
        Q2: ReadOnlyQueryData,
        Q3: ReadOnlyQueryData,
        Q4: ReadOnlyQueryData,
    > ReadOnlyQueryData for (Q0, Q1, Q2, Q3, Q4)
{
}
unsafe impl<
        Q0: ReadOnlyQueryData,
        Q1: ReadOnlyQueryData,
        Q2: ReadOnlyQueryData,
        Q3: ReadOnlyQueryData,
        Q4: ReadOnlyQueryData,
        Q5: ReadOnlyQueryData,
    > ReadOnlyQueryData for (Q0, Q1, Q2, Q3, Q4, Q5)
{
}
unsafe impl<
        Q0: ReadOnlyQueryData,
        Q1: ReadOnlyQueryData,
        Q2: ReadOnlyQueryData,
        Q3: ReadOnlyQueryData,
        Q4: ReadOnlyQueryData,
        Q5: ReadOnlyQueryData,
        Q6: ReadOnlyQueryData,
    > ReadOnlyQueryData for (Q0, Q1, Q2, Q3, Q4, Q5, Q6)
{
}
unsafe impl<
        Q0: ReadOnlyQueryData,
        Q1: ReadOnlyQueryData,
        Q2: ReadOnlyQueryData,
        Q3: ReadOnlyQueryData,
        Q4: ReadOnlyQueryData,
        Q5: ReadOnlyQueryData,
        Q6: ReadOnlyQueryData,
        Q7: ReadOnlyQueryData,
    > ReadOnlyQueryData for (Q0, Q1, Q2, Q3, Q4, Q5, Q6, Q7)
{
}

// SAFETY: 准备仅绑定既有声明，取值仅兑现有效范围内唯一目标的单槽授权。
unsafe impl<'w, Q: query_data_impls_inner::ComponentResult<C>, C: Component> QueryFetch<'w, Q>
    for ComponentFetch<'w, Q, C>
{
    /// # 用途
    /// access/scope/state/fetch_state 同源且已匹配，准备直接后端描述和需要的默认/元数据，不读业务值。
    /// # Safety
    /// 原授权覆盖全部结果期，调用方排除结构变化及冲突写入；缓存身份已核对。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &<Q as QueryData>::State,
        fetch_state: &<Q as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        Self::prepare_live_inner(access, scope, state, fetch_state)
    }
    /// # 用途
    /// entity 已验证且通过过滤，直接使用本 fetch 的已绑定描述，不搜索目录/类型/缓存表。
    /// Ref/默认 Ref 用视图 ticks 复制快照；写项用视图 write→change_marker→component::write_component。
    /// 同实体的过滤 tick 短读须已结束，返回结果不与只读 tick 引用重叠；包装初始 marked=false。
    /// # Safety
    /// 完整身份/位置/范围有效，写结果在全部活结果之间唯一，结果由原授权而非 self 保活。
    unsafe fn fetch_at(&mut self, entity: &FetchEntity) -> <Q as QueryData>::Item<'w, 'w> {
        self.fetch_live_inner(entity)
    }
}

// SAFETY: 准备仅绑定既有声明，取值仅兑现有效范围内唯一目标的单槽授权。
unsafe impl<'w, Q: query_data_impls_inner::MetadataResult> QueryFetch<'w, Q>
    for MetadataFetch<'w, Q>
{
    /// # 用途
    /// access/scope/state/fetch_state 同源且已匹配，准备直接后端描述和需要的默认/元数据，不读业务值。
    /// # Safety
    /// 原授权覆盖全部结果期，调用方排除结构变化及冲突写入；缓存身份已核对。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &<Q as QueryData>::State,
        fetch_state: &<Q as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        Self::prepare_live_inner(access, scope, state, fetch_state)
    }
    /// # 用途
    /// entity 已验证且通过过滤，直接使用本 fetch 的已绑定描述，不搜索目录/类型/缓存表。
    /// # Safety
    /// 完整身份/位置/范围有效，写结果在全部活结果之间唯一，结果由原授权而非 self 保活。
    unsafe fn fetch_at(&mut self, entity: &FetchEntity) -> <Q as QueryData>::Item<'w, 'w> {
        self.fetch_live_inner(entity)
    }
}

// SAFETY: 准备仅绑定既有声明，取值仅兑现有效范围内唯一目标的单槽授权。
unsafe impl<'w, C: Component> QueryFetch<'w, Has<C>> for PresenceFetch<'w, C> {
    /// # 用途
    /// access/scope/state/fetch_state 同源且已匹配，准备直接后端描述和需要的默认/元数据，不读业务值。
    /// # Safety
    /// 原授权覆盖全部结果期，调用方排除结构变化及冲突写入；缓存身份已核对。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &<Has<C> as QueryData>::State,
        fetch_state: &<Has<C> as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        Self::prepare_live_inner(access, scope, state, fetch_state)
    }
    /// # 用途
    /// entity 已验证且通过过滤，直接使用本 fetch 的已绑定描述，不搜索目录/类型/缓存表。
    /// # Safety
    /// 完整身份/位置/范围有效，写结果在全部活结果之间唯一，结果由原授权而非 self 保活。
    unsafe fn fetch_at(&mut self, entity: &FetchEntity) -> <Has<C> as QueryData>::Item<'w, 'w> {
        self.fetch_live_inner(entity)
    }
}

// SAFETY: 元组按字段转发既有准备与取值协议，含写重复声明在构造前已拒绝。
unsafe impl<'w, Q0: QueryData> QueryFetch<'w, (Q0,)> for (Q0::Fetch<'w>,) {
    /// # 用途
    /// 各字段在同一 access/scope 上逐项准备，失败丢弃已准备描述，不释放业务值或发布部分 fetch。
    /// # Safety
    /// 整体权限/别名已校验，所有字段从同一原授权派生，不能逐项新增授权。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &<(Q0,) as QueryData>::State,
        fetch_state: &<(Q0,) as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        QueryDataBinding::<(Q0,)>::prepare_tuple_inner(access, scope, state, fetch_state)
    }
    /// # 用途
    /// 按字段顺序从各子 fetch 取得同一 entity 的结果，不创建中转数组或再次定位后端。
    /// # Safety
    /// entity 有效且过滤已通过，整体声明保证字段间没有可变别名，返回结果不借用元组临时内存。
    unsafe fn fetch_at(&mut self, entity: &FetchEntity) -> <(Q0,) as QueryData>::Item<'w, 'w> {
        QueryDataBinding::<(Q0,)>::fetch_tuple_inner(self, entity)
    }
}

// SAFETY: 元组按字段转发既有准备与取值协议，含写重复声明在构造前已拒绝。
unsafe impl<'w, Q0: QueryData, Q1: QueryData> QueryFetch<'w, (Q0, Q1)>
    for (Q0::Fetch<'w>, Q1::Fetch<'w>)
{
    /// # 用途
    /// 各字段在同一 access/scope 上逐项准备，失败丢弃已准备描述，不释放业务值或发布部分 fetch。
    /// # Safety
    /// 整体权限/别名已校验，所有字段从同一原授权派生，不能逐项新增授权。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &<(Q0, Q1) as QueryData>::State,
        fetch_state: &<(Q0, Q1) as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        QueryDataBinding::<(Q0, Q1)>::prepare_tuple_inner(access, scope, state, fetch_state)
    }
    /// # 用途
    /// 按字段顺序从各子 fetch 取得同一 entity 的结果，不创建中转数组或再次定位后端。
    /// # Safety
    /// entity 有效且过滤已通过，整体声明保证字段间没有可变别名，返回结果不借用元组临时内存。
    unsafe fn fetch_at(&mut self, entity: &FetchEntity) -> <(Q0, Q1) as QueryData>::Item<'w, 'w> {
        QueryDataBinding::<(Q0, Q1)>::fetch_tuple_inner(self, entity)
    }
}

// SAFETY: 元组按字段转发既有准备与取值协议，含写重复声明在构造前已拒绝。
unsafe impl<'w, Q0: QueryData, Q1: QueryData, Q2: QueryData> QueryFetch<'w, (Q0, Q1, Q2)>
    for (Q0::Fetch<'w>, Q1::Fetch<'w>, Q2::Fetch<'w>)
{
    /// # 用途
    /// 各字段在同一 access/scope 上逐项准备，失败丢弃已准备描述，不释放业务值或发布部分 fetch。
    /// # Safety
    /// 整体权限/别名已校验，所有字段从同一原授权派生，不能逐项新增授权。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &<(Q0, Q1, Q2) as QueryData>::State,
        fetch_state: &<(Q0, Q1, Q2) as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        QueryDataBinding::<(Q0, Q1, Q2)>::prepare_tuple_inner(access, scope, state, fetch_state)
    }
    /// # 用途
    /// 按字段顺序从各子 fetch 取得同一 entity 的结果，不创建中转数组或再次定位后端。
    /// # Safety
    /// entity 有效且过滤已通过，整体声明保证字段间没有可变别名，返回结果不借用元组临时内存。
    unsafe fn fetch_at(
        &mut self,
        entity: &FetchEntity,
    ) -> <(Q0, Q1, Q2) as QueryData>::Item<'w, 'w> {
        QueryDataBinding::<(Q0, Q1, Q2)>::fetch_tuple_inner(self, entity)
    }
}

// SAFETY: 元组按字段转发既有准备与取值协议，含写重复声明在构造前已拒绝。
unsafe impl<'w, Q0: QueryData, Q1: QueryData, Q2: QueryData, Q3: QueryData>
    QueryFetch<'w, (Q0, Q1, Q2, Q3)>
    for (Q0::Fetch<'w>, Q1::Fetch<'w>, Q2::Fetch<'w>, Q3::Fetch<'w>)
{
    /// # 用途
    /// 各字段在同一 access/scope 上逐项准备，失败丢弃已准备描述，不释放业务值或发布部分 fetch。
    /// # Safety
    /// 整体权限/别名已校验，所有字段从同一原授权派生，不能逐项新增授权。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &<(Q0, Q1, Q2, Q3) as QueryData>::State,
        fetch_state: &<(Q0, Q1, Q2, Q3) as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        QueryDataBinding::<(Q0, Q1, Q2, Q3)>::prepare_tuple_inner(access, scope, state, fetch_state)
    }
    /// # 用途
    /// 按字段顺序从各子 fetch 取得同一 entity 的结果，不创建中转数组或再次定位后端。
    /// # Safety
    /// entity 有效且过滤已通过，整体声明保证字段间没有可变别名，返回结果不借用元组临时内存。
    unsafe fn fetch_at(
        &mut self,
        entity: &FetchEntity,
    ) -> <(Q0, Q1, Q2, Q3) as QueryData>::Item<'w, 'w> {
        QueryDataBinding::<(Q0, Q1, Q2, Q3)>::fetch_tuple_inner(self, entity)
    }
}

// SAFETY: 元组按字段转发既有准备与取值协议，含写重复声明在构造前已拒绝。
unsafe impl<'w, Q0: QueryData, Q1: QueryData, Q2: QueryData, Q3: QueryData, Q4: QueryData>
    QueryFetch<'w, (Q0, Q1, Q2, Q3, Q4)>
    for (
        Q0::Fetch<'w>,
        Q1::Fetch<'w>,
        Q2::Fetch<'w>,
        Q3::Fetch<'w>,
        Q4::Fetch<'w>,
    )
{
    /// # 用途
    /// 各字段在同一 access/scope 上逐项准备，失败丢弃已准备描述，不释放业务值或发布部分 fetch。
    /// # Safety
    /// 整体权限/别名已校验，所有字段从同一原授权派生，不能逐项新增授权。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &<(Q0, Q1, Q2, Q3, Q4) as QueryData>::State,
        fetch_state: &<(Q0, Q1, Q2, Q3, Q4) as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        QueryDataBinding::<(Q0, Q1, Q2, Q3, Q4)>::prepare_tuple_inner(
            access,
            scope,
            state,
            fetch_state,
        )
    }
    /// # 用途
    /// 按字段顺序从各子 fetch 取得同一 entity 的结果，不创建中转数组或再次定位后端。
    /// # Safety
    /// entity 有效且过滤已通过，整体声明保证字段间没有可变别名，返回结果不借用元组临时内存。
    unsafe fn fetch_at(
        &mut self,
        entity: &FetchEntity,
    ) -> <(Q0, Q1, Q2, Q3, Q4) as QueryData>::Item<'w, 'w> {
        QueryDataBinding::<(Q0, Q1, Q2, Q3, Q4)>::fetch_tuple_inner(self, entity)
    }
}

// SAFETY: 元组按字段转发既有准备与取值协议，含写重复声明在构造前已拒绝。
unsafe impl<
        'w,
        Q0: QueryData,
        Q1: QueryData,
        Q2: QueryData,
        Q3: QueryData,
        Q4: QueryData,
        Q5: QueryData,
    > QueryFetch<'w, (Q0, Q1, Q2, Q3, Q4, Q5)>
    for (
        Q0::Fetch<'w>,
        Q1::Fetch<'w>,
        Q2::Fetch<'w>,
        Q3::Fetch<'w>,
        Q4::Fetch<'w>,
        Q5::Fetch<'w>,
    )
{
    /// # 用途
    /// 各字段在同一 access/scope 上逐项准备，失败丢弃已准备描述，不释放业务值或发布部分 fetch。
    /// # Safety
    /// 整体权限/别名已校验，所有字段从同一原授权派生，不能逐项新增授权。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &<(Q0, Q1, Q2, Q3, Q4, Q5) as QueryData>::State,
        fetch_state: &<(Q0, Q1, Q2, Q3, Q4, Q5) as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        QueryDataBinding::<(Q0, Q1, Q2, Q3, Q4, Q5)>::prepare_tuple_inner(
            access,
            scope,
            state,
            fetch_state,
        )
    }
    /// # 用途
    /// 按字段顺序从各子 fetch 取得同一 entity 的结果，不创建中转数组或再次定位后端。
    /// # Safety
    /// entity 有效且过滤已通过，整体声明保证字段间没有可变别名，返回结果不借用元组临时内存。
    unsafe fn fetch_at(
        &mut self,
        entity: &FetchEntity,
    ) -> <(Q0, Q1, Q2, Q3, Q4, Q5) as QueryData>::Item<'w, 'w> {
        QueryDataBinding::<(Q0, Q1, Q2, Q3, Q4, Q5)>::fetch_tuple_inner(self, entity)
    }
}

// SAFETY: 元组按字段转发既有准备与取值协议，含写重复声明在构造前已拒绝。
unsafe impl<
        'w,
        Q0: QueryData,
        Q1: QueryData,
        Q2: QueryData,
        Q3: QueryData,
        Q4: QueryData,
        Q5: QueryData,
        Q6: QueryData,
    > QueryFetch<'w, (Q0, Q1, Q2, Q3, Q4, Q5, Q6)>
    for (
        Q0::Fetch<'w>,
        Q1::Fetch<'w>,
        Q2::Fetch<'w>,
        Q3::Fetch<'w>,
        Q4::Fetch<'w>,
        Q5::Fetch<'w>,
        Q6::Fetch<'w>,
    )
{
    /// # 用途
    /// 各字段在同一 access/scope 上逐项准备，失败丢弃已准备描述，不释放业务值或发布部分 fetch。
    /// # Safety
    /// 整体权限/别名已校验，所有字段从同一原授权派生，不能逐项新增授权。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &<(Q0, Q1, Q2, Q3, Q4, Q5, Q6) as QueryData>::State,
        fetch_state: &<(Q0, Q1, Q2, Q3, Q4, Q5, Q6) as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        QueryDataBinding::<(Q0, Q1, Q2, Q3, Q4, Q5, Q6)>::prepare_tuple_inner(
            access,
            scope,
            state,
            fetch_state,
        )
    }
    /// # 用途
    /// 按字段顺序从各子 fetch 取得同一 entity 的结果，不创建中转数组或再次定位后端。
    /// # Safety
    /// entity 有效且过滤已通过，整体声明保证字段间没有可变别名，返回结果不借用元组临时内存。
    unsafe fn fetch_at(
        &mut self,
        entity: &FetchEntity,
    ) -> <(Q0, Q1, Q2, Q3, Q4, Q5, Q6) as QueryData>::Item<'w, 'w> {
        QueryDataBinding::<(Q0, Q1, Q2, Q3, Q4, Q5, Q6)>::fetch_tuple_inner(self, entity)
    }
}

// SAFETY: 元组按字段转发既有准备与取值协议，含写重复声明在构造前已拒绝。
unsafe impl<
        'w,
        Q0: QueryData,
        Q1: QueryData,
        Q2: QueryData,
        Q3: QueryData,
        Q4: QueryData,
        Q5: QueryData,
        Q6: QueryData,
        Q7: QueryData,
    > QueryFetch<'w, (Q0, Q1, Q2, Q3, Q4, Q5, Q6, Q7)>
    for (
        Q0::Fetch<'w>,
        Q1::Fetch<'w>,
        Q2::Fetch<'w>,
        Q3::Fetch<'w>,
        Q4::Fetch<'w>,
        Q5::Fetch<'w>,
        Q6::Fetch<'w>,
        Q7::Fetch<'w>,
    )
{
    /// # 用途
    /// 各字段在同一 access/scope 上逐项准备，失败丢弃已准备描述，不释放业务值或发布部分 fetch。
    /// # Safety
    /// 整体权限/别名已校验，所有字段从同一原授权派生，不能逐项新增授权。
    unsafe fn prepare(
        access: &FetchAccess<'w>,
        scope: FetchScope,
        state: &<(Q0, Q1, Q2, Q3, Q4, Q5, Q6, Q7) as QueryData>::State,
        fetch_state: &<(Q0, Q1, Q2, Q3, Q4, Q5, Q6, Q7) as QueryData>::FetchState,
    ) -> Result<Self, EcsError> {
        QueryDataBinding::<(Q0, Q1, Q2, Q3, Q4, Q5, Q6, Q7)>::prepare_tuple_inner(
            access,
            scope,
            state,
            fetch_state,
        )
    }
    /// # 用途
    /// 按字段顺序从各子 fetch 取得同一 entity 的结果，不创建中转数组或再次定位后端。
    /// # Safety
    /// entity 有效且过滤已通过，整体声明保证字段间没有可变别名，返回结果不借用元组临时内存。
    unsafe fn fetch_at(
        &mut self,
        entity: &FetchEntity,
    ) -> <(Q0, Q1, Q2, Q3, Q4, Q5, Q6, Q7) as QueryData>::Item<'w, 'w> {
        QueryDataBinding::<(Q0, Q1, Q2, Q3, Q4, Q5, Q6, Q7)>::fetch_tuple_inner(self, entity)
    }
}
