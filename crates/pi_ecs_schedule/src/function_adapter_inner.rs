#[cfg(test)]
mod tests {
    use crate::{function_adapter::FunctionSignature, local::Local, system::SystemFunction};

    #[test]
    fn arity_2_preserves_argument_order_and_borrows() {
        let mut p0 = 1u32;
        let mut p1 = 2u32;
        let mut function = |mut p0: Local<'_, u32>, mut p1: Local<'_, u32>| {
            *p0 += 10;
            *p1 += 20;
        };
        SystemFunction::<FunctionSignature<(Local<'static, u32>, Local<'static, u32>)>>::call(
            &mut function,
            (Local { value: &mut p0 }, Local { value: &mut p1 }),
        );
        assert_eq!(p0, 11);
        assert_eq!(p1, 22);
    }

    #[test]
    fn arity_3_preserves_argument_order_and_borrows() {
        let mut p0 = 1u32;
        let mut p1 = 2u32;
        let mut p2 = 3u32;
        let mut function =
            |mut p0: Local<'_, u32>, mut p1: Local<'_, u32>, mut p2: Local<'_, u32>| {
                *p0 += 10;
                *p1 += 20;
                *p2 += 30;
            };
        SystemFunction::<
            FunctionSignature<(
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
            )>,
        >::call(
            &mut function,
            (
                Local { value: &mut p0 },
                Local { value: &mut p1 },
                Local { value: &mut p2 },
            ),
        );
        assert_eq!(p0, 11);
        assert_eq!(p1, 22);
        assert_eq!(p2, 33);
    }

    #[test]
    fn arity_4_preserves_argument_order_and_borrows() {
        let mut p0 = 1u32;
        let mut p1 = 2u32;
        let mut p2 = 3u32;
        let mut p3 = 4u32;
        let mut function = |mut p0: Local<'_, u32>,
                            mut p1: Local<'_, u32>,
                            mut p2: Local<'_, u32>,
                            mut p3: Local<'_, u32>| {
            *p0 += 10;
            *p1 += 20;
            *p2 += 30;
            *p3 += 40;
        };
        SystemFunction::<
            FunctionSignature<(
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
            )>,
        >::call(
            &mut function,
            (
                Local { value: &mut p0 },
                Local { value: &mut p1 },
                Local { value: &mut p2 },
                Local { value: &mut p3 },
            ),
        );
        assert_eq!(p0, 11);
        assert_eq!(p1, 22);
        assert_eq!(p2, 33);
        assert_eq!(p3, 44);
    }

    #[test]
    fn arity_5_preserves_argument_order_and_borrows() {
        let mut p0 = 1u32;
        let mut p1 = 2u32;
        let mut p2 = 3u32;
        let mut p3 = 4u32;
        let mut p4 = 5u32;
        let mut function = |mut p0: Local<'_, u32>,
                            mut p1: Local<'_, u32>,
                            mut p2: Local<'_, u32>,
                            mut p3: Local<'_, u32>,
                            mut p4: Local<'_, u32>| {
            *p0 += 10;
            *p1 += 20;
            *p2 += 30;
            *p3 += 40;
            *p4 += 50;
        };
        SystemFunction::<
            FunctionSignature<(
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
            )>,
        >::call(
            &mut function,
            (
                Local { value: &mut p0 },
                Local { value: &mut p1 },
                Local { value: &mut p2 },
                Local { value: &mut p3 },
                Local { value: &mut p4 },
            ),
        );
        assert_eq!(p0, 11);
        assert_eq!(p1, 22);
        assert_eq!(p2, 33);
        assert_eq!(p3, 44);
        assert_eq!(p4, 55);
    }

    #[test]
    fn arity_6_preserves_argument_order_and_borrows() {
        let mut p0 = 1u32;
        let mut p1 = 2u32;
        let mut p2 = 3u32;
        let mut p3 = 4u32;
        let mut p4 = 5u32;
        let mut p5 = 6u32;
        let mut function = |mut p0: Local<'_, u32>,
                            mut p1: Local<'_, u32>,
                            mut p2: Local<'_, u32>,
                            mut p3: Local<'_, u32>,
                            mut p4: Local<'_, u32>,
                            mut p5: Local<'_, u32>| {
            *p0 += 10;
            *p1 += 20;
            *p2 += 30;
            *p3 += 40;
            *p4 += 50;
            *p5 += 60;
        };
        SystemFunction::<
            FunctionSignature<(
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
            )>,
        >::call(
            &mut function,
            (
                Local { value: &mut p0 },
                Local { value: &mut p1 },
                Local { value: &mut p2 },
                Local { value: &mut p3 },
                Local { value: &mut p4 },
                Local { value: &mut p5 },
            ),
        );
        assert_eq!(p0, 11);
        assert_eq!(p1, 22);
        assert_eq!(p2, 33);
        assert_eq!(p3, 44);
        assert_eq!(p4, 55);
        assert_eq!(p5, 66);
    }

    #[test]
    fn arity_7_preserves_argument_order_and_borrows() {
        let mut p0 = 1u32;
        let mut p1 = 2u32;
        let mut p2 = 3u32;
        let mut p3 = 4u32;
        let mut p4 = 5u32;
        let mut p5 = 6u32;
        let mut p6 = 7u32;
        let mut function = |mut p0: Local<'_, u32>,
                            mut p1: Local<'_, u32>,
                            mut p2: Local<'_, u32>,
                            mut p3: Local<'_, u32>,
                            mut p4: Local<'_, u32>,
                            mut p5: Local<'_, u32>,
                            mut p6: Local<'_, u32>| {
            *p0 += 10;
            *p1 += 20;
            *p2 += 30;
            *p3 += 40;
            *p4 += 50;
            *p5 += 60;
            *p6 += 70;
        };
        SystemFunction::<
            FunctionSignature<(
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
            )>,
        >::call(
            &mut function,
            (
                Local { value: &mut p0 },
                Local { value: &mut p1 },
                Local { value: &mut p2 },
                Local { value: &mut p3 },
                Local { value: &mut p4 },
                Local { value: &mut p5 },
                Local { value: &mut p6 },
            ),
        );
        assert_eq!(p0, 11);
        assert_eq!(p1, 22);
        assert_eq!(p2, 33);
        assert_eq!(p3, 44);
        assert_eq!(p4, 55);
        assert_eq!(p5, 66);
        assert_eq!(p6, 77);
    }

    #[test]
    fn arity_8_preserves_argument_order_and_borrows() {
        let mut p0 = 1u32;
        let mut p1 = 2u32;
        let mut p2 = 3u32;
        let mut p3 = 4u32;
        let mut p4 = 5u32;
        let mut p5 = 6u32;
        let mut p6 = 7u32;
        let mut p7 = 8u32;
        let mut function = |mut p0: Local<'_, u32>,
                            mut p1: Local<'_, u32>,
                            mut p2: Local<'_, u32>,
                            mut p3: Local<'_, u32>,
                            mut p4: Local<'_, u32>,
                            mut p5: Local<'_, u32>,
                            mut p6: Local<'_, u32>,
                            mut p7: Local<'_, u32>| {
            *p0 += 10;
            *p1 += 20;
            *p2 += 30;
            *p3 += 40;
            *p4 += 50;
            *p5 += 60;
            *p6 += 70;
            *p7 += 80;
        };
        SystemFunction::<
            FunctionSignature<(
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
                Local<'static, u32>,
            )>,
        >::call(
            &mut function,
            (
                Local { value: &mut p0 },
                Local { value: &mut p1 },
                Local { value: &mut p2 },
                Local { value: &mut p3 },
                Local { value: &mut p4 },
                Local { value: &mut p5 },
                Local { value: &mut p6 },
                Local { value: &mut p7 },
            ),
        );
        assert_eq!(p0, 11);
        assert_eq!(p1, 22);
        assert_eq!(p2, 33);
        assert_eq!(p3, 44);
        assert_eq!(p4, 55);
        assert_eq!(p5, 66);
        assert_eq!(p6, 77);
        assert_eq!(p7, 88);
    }

    #[test]
    fn one_parameter_receives_the_current_borrow() {
        let mut stored = String::from("first");
        let mut function = |mut value: Local<'_, String>| value.push_str(" updated");
        SystemFunction::<FunctionSignature<(Local<'static, String>,)>>::call(
            &mut function,
            (Local { value: &mut stored },),
        );
        assert_eq!(stored, "first updated");
    }
}

impl<P> super::FunctionSignature<P> {
    pub(super) fn call_0<'run, F>(
        _function: &mut F,
        _item: <() as crate::system_param::SystemParam>::Item<'run>,
    ) where
        F: FnMut(),
    {
        _function();
    }
    pub(super) fn call_1<'run, F, P0>(
        _function: &mut F,
        _item: <(P0,) as crate::system_param::SystemParam>::Item<'run>,
    ) where
        F: FnMut(P0) + for<'a> FnMut(P0::Item<'a>),
        P0: crate::system_param::SystemParam + 'static,
    {
        _function(_item.0);
    }
    pub(super) fn call_2<'run, F, P0, P1>(
        _function: &mut F,
        _item: <(P0, P1) as crate::system_param::SystemParam>::Item<'run>,
    ) where
        F: FnMut(P0, P1) + for<'a> FnMut(P0::Item<'a>, P1::Item<'a>),
        P0: crate::system_param::SystemParam + 'static,
        P1: crate::system_param::SystemParam + 'static,
    {
        _function(_item.0, _item.1);
    }
    pub(super) fn call_3<'run, F, P0, P1, P2>(
        _function: &mut F,
        _item: <(P0, P1, P2) as crate::system_param::SystemParam>::Item<'run>,
    ) where
        F: FnMut(P0, P1, P2) + for<'a> FnMut(P0::Item<'a>, P1::Item<'a>, P2::Item<'a>),
        P0: crate::system_param::SystemParam + 'static,
        P1: crate::system_param::SystemParam + 'static,
        P2: crate::system_param::SystemParam + 'static,
    {
        _function(_item.0, _item.1, _item.2);
    }
    pub(super) fn call_4<'run, F, P0, P1, P2, P3>(
        _function: &mut F,
        _item: <(P0, P1, P2, P3) as crate::system_param::SystemParam>::Item<'run>,
    ) where
        F: FnMut(P0, P1, P2, P3)
            + for<'a> FnMut(P0::Item<'a>, P1::Item<'a>, P2::Item<'a>, P3::Item<'a>),
        P0: crate::system_param::SystemParam + 'static,
        P1: crate::system_param::SystemParam + 'static,
        P2: crate::system_param::SystemParam + 'static,
        P3: crate::system_param::SystemParam + 'static,
    {
        _function(_item.0, _item.1, _item.2, _item.3);
    }
    pub(super) fn call_5<'run, F, P0, P1, P2, P3, P4>(
        _function: &mut F,
        _item: <(P0, P1, P2, P3, P4) as crate::system_param::SystemParam>::Item<'run>,
    ) where
        F: FnMut(P0, P1, P2, P3, P4)
            + for<'a> FnMut(P0::Item<'a>, P1::Item<'a>, P2::Item<'a>, P3::Item<'a>, P4::Item<'a>),
        P0: crate::system_param::SystemParam + 'static,
        P1: crate::system_param::SystemParam + 'static,
        P2: crate::system_param::SystemParam + 'static,
        P3: crate::system_param::SystemParam + 'static,
        P4: crate::system_param::SystemParam + 'static,
    {
        _function(_item.0, _item.1, _item.2, _item.3, _item.4);
    }
    pub(super) fn call_6<'run, F, P0, P1, P2, P3, P4, P5>(
        _function: &mut F,
        _item: <(P0, P1, P2, P3, P4, P5) as crate::system_param::SystemParam>::Item<'run>,
    ) where
        F: FnMut(P0, P1, P2, P3, P4, P5)
            + for<'a> FnMut(
                P0::Item<'a>,
                P1::Item<'a>,
                P2::Item<'a>,
                P3::Item<'a>,
                P4::Item<'a>,
                P5::Item<'a>,
            ),
        P0: crate::system_param::SystemParam + 'static,
        P1: crate::system_param::SystemParam + 'static,
        P2: crate::system_param::SystemParam + 'static,
        P3: crate::system_param::SystemParam + 'static,
        P4: crate::system_param::SystemParam + 'static,
        P5: crate::system_param::SystemParam + 'static,
    {
        _function(_item.0, _item.1, _item.2, _item.3, _item.4, _item.5);
    }
    pub(super) fn call_7<'run, F, P0, P1, P2, P3, P4, P5, P6>(
        _function: &mut F,
        _item: <(P0, P1, P2, P3, P4, P5, P6) as crate::system_param::SystemParam>::Item<'run>,
    ) where
        F: FnMut(P0, P1, P2, P3, P4, P5, P6)
            + for<'a> FnMut(
                P0::Item<'a>,
                P1::Item<'a>,
                P2::Item<'a>,
                P3::Item<'a>,
                P4::Item<'a>,
                P5::Item<'a>,
                P6::Item<'a>,
            ),
        P0: crate::system_param::SystemParam + 'static,
        P1: crate::system_param::SystemParam + 'static,
        P2: crate::system_param::SystemParam + 'static,
        P3: crate::system_param::SystemParam + 'static,
        P4: crate::system_param::SystemParam + 'static,
        P5: crate::system_param::SystemParam + 'static,
        P6: crate::system_param::SystemParam + 'static,
    {
        _function(
            _item.0, _item.1, _item.2, _item.3, _item.4, _item.5, _item.6,
        );
    }
    pub(super) fn call_8<'run, F, P0, P1, P2, P3, P4, P5, P6, P7>(
        _function: &mut F,
        _item: <(P0, P1, P2, P3, P4, P5, P6, P7) as crate::system_param::SystemParam>::Item<'run>,
    ) where
        F: FnMut(P0, P1, P2, P3, P4, P5, P6, P7)
            + for<'a> FnMut(
                P0::Item<'a>,
                P1::Item<'a>,
                P2::Item<'a>,
                P3::Item<'a>,
                P4::Item<'a>,
                P5::Item<'a>,
                P6::Item<'a>,
                P7::Item<'a>,
            ),
        P0: crate::system_param::SystemParam + 'static,
        P1: crate::system_param::SystemParam + 'static,
        P2: crate::system_param::SystemParam + 'static,
        P3: crate::system_param::SystemParam + 'static,
        P4: crate::system_param::SystemParam + 'static,
        P5: crate::system_param::SystemParam + 'static,
        P6: crate::system_param::SystemParam + 'static,
        P7: crate::system_param::SystemParam + 'static,
    {
        _function(
            _item.0, _item.1, _item.2, _item.3, _item.4, _item.5, _item.6, _item.7,
        );
    }
}
