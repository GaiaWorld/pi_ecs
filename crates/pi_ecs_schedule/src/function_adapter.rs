//! 普通函数签名适配；零至八个参数，声明类型与运行 Item 双约束辅助类型推导。
#[path = "function_adapter_inner.rs"]
mod function_adapter_inner;
use crate::{system::SystemFunction, system_param::SystemParam};
/// # 用途
/// 普通函数适配的类型标记，与高级手写 SystemFunction 的 Marker 隔离；不产生运行字段。
pub struct FunctionSignature<P>(
    /// 仅表达参数元组类型，不拥有参数或配置。
    pub(crate) std::marker::PhantomData<fn() -> P>,
);
/// 双重 FnMut 约束让声明参数可推导，实际调用只接收当前运行 Item；返回必须为 ()。
impl<F> SystemFunction<FunctionSignature<()>> for F
where
    F: FnMut(),
{
    type Param = ();
    /// # 用途
    /// item 为本次已授权参数元组；只拆分调用，不提前 fetch ParamSet，不允许返回借用。
    fn call<'run>(&mut self, item: <Self::Param as SystemParam>::Item<'run>) {
        FunctionSignature::<Self::Param>::call_0::<F>(self, item)
    }
}
/// 双重 FnMut 约束让声明参数可推导，实际调用只接收当前运行 Item；返回必须为 ()。
impl<F, P0> SystemFunction<FunctionSignature<(P0,)>> for F
where
    F: FnMut(P0) + for<'run> FnMut(P0::Item<'run>),
    P0: SystemParam + 'static,
{
    type Param = (P0,);
    /// # 用途
    /// item 为本次已授权参数元组；只拆分调用，不提前 fetch ParamSet，不允许返回借用。
    fn call<'run>(&mut self, item: <Self::Param as SystemParam>::Item<'run>) {
        FunctionSignature::<Self::Param>::call_1::<F, P0>(self, item)
    }
}
/// 双重 FnMut 约束让声明参数可推导，实际调用只接收当前运行 Item；返回必须为 ()。
impl<F, P0, P1> SystemFunction<FunctionSignature<(P0, P1)>> for F
where
    F: FnMut(P0, P1) + for<'run> FnMut(P0::Item<'run>, P1::Item<'run>),
    P0: SystemParam + 'static,
    P1: SystemParam + 'static,
{
    type Param = (P0, P1);
    /// # 用途
    /// item 为本次已授权参数元组；只拆分调用，不提前 fetch ParamSet，不允许返回借用。
    fn call<'run>(&mut self, item: <Self::Param as SystemParam>::Item<'run>) {
        FunctionSignature::<Self::Param>::call_2::<F, P0, P1>(self, item)
    }
}
/// 双重 FnMut 约束让声明参数可推导，实际调用只接收当前运行 Item；返回必须为 ()。
impl<F, P0, P1, P2> SystemFunction<FunctionSignature<(P0, P1, P2)>> for F
where
    F: FnMut(P0, P1, P2) + for<'run> FnMut(P0::Item<'run>, P1::Item<'run>, P2::Item<'run>),
    P0: SystemParam + 'static,
    P1: SystemParam + 'static,
    P2: SystemParam + 'static,
{
    type Param = (P0, P1, P2);
    /// # 用途
    /// item 为本次已授权参数元组；只拆分调用，不提前 fetch ParamSet，不允许返回借用。
    fn call<'run>(&mut self, item: <Self::Param as SystemParam>::Item<'run>) {
        FunctionSignature::<Self::Param>::call_3::<F, P0, P1, P2>(self, item)
    }
}
/// 双重 FnMut 约束让声明参数可推导，实际调用只接收当前运行 Item；返回必须为 ()。
impl<F, P0, P1, P2, P3> SystemFunction<FunctionSignature<(P0, P1, P2, P3)>> for F
where
    F: FnMut(P0, P1, P2, P3)
        + for<'run> FnMut(P0::Item<'run>, P1::Item<'run>, P2::Item<'run>, P3::Item<'run>),
    P0: SystemParam + 'static,
    P1: SystemParam + 'static,
    P2: SystemParam + 'static,
    P3: SystemParam + 'static,
{
    type Param = (P0, P1, P2, P3);
    /// # 用途
    /// item 为本次已授权参数元组；只拆分调用，不提前 fetch ParamSet，不允许返回借用。
    fn call<'run>(&mut self, item: <Self::Param as SystemParam>::Item<'run>) {
        FunctionSignature::<Self::Param>::call_4::<F, P0, P1, P2, P3>(self, item)
    }
}
/// 双重 FnMut 约束让声明参数可推导，实际调用只接收当前运行 Item；返回必须为 ()。
impl<F, P0, P1, P2, P3, P4> SystemFunction<FunctionSignature<(P0, P1, P2, P3, P4)>> for F
where
    F: FnMut(P0, P1, P2, P3, P4)
        + for<'run> FnMut(
            P0::Item<'run>,
            P1::Item<'run>,
            P2::Item<'run>,
            P3::Item<'run>,
            P4::Item<'run>,
        ),
    P0: SystemParam + 'static,
    P1: SystemParam + 'static,
    P2: SystemParam + 'static,
    P3: SystemParam + 'static,
    P4: SystemParam + 'static,
{
    type Param = (P0, P1, P2, P3, P4);
    /// # 用途
    /// item 为本次已授权参数元组；只拆分调用，不提前 fetch ParamSet，不允许返回借用。
    fn call<'run>(&mut self, item: <Self::Param as SystemParam>::Item<'run>) {
        FunctionSignature::<Self::Param>::call_5::<F, P0, P1, P2, P3, P4>(self, item)
    }
}
/// 双重 FnMut 约束让声明参数可推导，实际调用只接收当前运行 Item；返回必须为 ()。
impl<F, P0, P1, P2, P3, P4, P5> SystemFunction<FunctionSignature<(P0, P1, P2, P3, P4, P5)>> for F
where
    F: FnMut(P0, P1, P2, P3, P4, P5)
        + for<'run> FnMut(
            P0::Item<'run>,
            P1::Item<'run>,
            P2::Item<'run>,
            P3::Item<'run>,
            P4::Item<'run>,
            P5::Item<'run>,
        ),
    P0: SystemParam + 'static,
    P1: SystemParam + 'static,
    P2: SystemParam + 'static,
    P3: SystemParam + 'static,
    P4: SystemParam + 'static,
    P5: SystemParam + 'static,
{
    type Param = (P0, P1, P2, P3, P4, P5);
    /// # 用途
    /// item 为本次已授权参数元组；只拆分调用，不提前 fetch ParamSet，不允许返回借用。
    fn call<'run>(&mut self, item: <Self::Param as SystemParam>::Item<'run>) {
        FunctionSignature::<Self::Param>::call_6::<F, P0, P1, P2, P3, P4, P5>(self, item)
    }
}
/// 双重 FnMut 约束让声明参数可推导，实际调用只接收当前运行 Item；返回必须为 ()。
impl<F, P0, P1, P2, P3, P4, P5, P6> SystemFunction<FunctionSignature<(P0, P1, P2, P3, P4, P5, P6)>>
    for F
where
    F: FnMut(P0, P1, P2, P3, P4, P5, P6)
        + for<'run> FnMut(
            P0::Item<'run>,
            P1::Item<'run>,
            P2::Item<'run>,
            P3::Item<'run>,
            P4::Item<'run>,
            P5::Item<'run>,
            P6::Item<'run>,
        ),
    P0: SystemParam + 'static,
    P1: SystemParam + 'static,
    P2: SystemParam + 'static,
    P3: SystemParam + 'static,
    P4: SystemParam + 'static,
    P5: SystemParam + 'static,
    P6: SystemParam + 'static,
{
    type Param = (P0, P1, P2, P3, P4, P5, P6);
    /// # 用途
    /// item 为本次已授权参数元组；只拆分调用，不提前 fetch ParamSet，不允许返回借用。
    fn call<'run>(&mut self, item: <Self::Param as SystemParam>::Item<'run>) {
        FunctionSignature::<Self::Param>::call_7::<F, P0, P1, P2, P3, P4, P5, P6>(self, item)
    }
}
/// 双重 FnMut 约束让声明参数可推导，实际调用只接收当前运行 Item；返回必须为 ()。
impl<F, P0, P1, P2, P3, P4, P5, P6, P7>
    SystemFunction<FunctionSignature<(P0, P1, P2, P3, P4, P5, P6, P7)>> for F
where
    F: FnMut(P0, P1, P2, P3, P4, P5, P6, P7)
        + for<'run> FnMut(
            P0::Item<'run>,
            P1::Item<'run>,
            P2::Item<'run>,
            P3::Item<'run>,
            P4::Item<'run>,
            P5::Item<'run>,
            P6::Item<'run>,
            P7::Item<'run>,
        ),
    P0: SystemParam + 'static,
    P1: SystemParam + 'static,
    P2: SystemParam + 'static,
    P3: SystemParam + 'static,
    P4: SystemParam + 'static,
    P5: SystemParam + 'static,
    P6: SystemParam + 'static,
    P7: SystemParam + 'static,
{
    type Param = (P0, P1, P2, P3, P4, P5, P6, P7);
    /// # 用途
    /// item 为本次已授权参数元组；只拆分调用，不提前 fetch ParamSet，不允许返回借用。
    fn call<'run>(&mut self, item: <Self::Param as SystemParam>::Item<'run>) {
        FunctionSignature::<Self::Param>::call_8::<F, P0, P1, P2, P3, P4, P5, P6, P7>(self, item)
    }
}
