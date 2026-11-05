//! 创建和组合修改的显式值载体，不隐式注册组件类型。
use crate::{entity_values_inner, error::EcsError};
use std::any::{Any, TypeId};
/// 操作拥有普通组件值及内置创建请求；`insert<T>` 仅接受 PlainAccess，类型和值保持一致。
/// 尚未移入 World 的值由输入集合拥有，不设置事务 guard 或列逐槽标记。
pub struct EntityValues<'w> {
    /// spawn/Alter 内置创建请求，不是可逃逸的容器头；解析于受授权 World 中。
    pub(crate) builtin_requests: Vec<BuiltinRequest<'w>>,
    /// 按 Rust 类型去重的拥有值；ComponentId 在目标 World 校验时解析。
    pub(crate) values: Vec<(TypeId, Box<dyn Any>)>,
}
/// 强数组请求拒绝时原样交还所有拥有输入，不留下部分转移。
pub struct StrongArrayInputError<'w, const N: usize> {
    /// 重复类型、World/分支不一致或准备失败原因。
    pub error: EcsError,
    /// 每位置原有的强责任，顺序与输入一致。
    pub values: [crate::owned_entity::OwnedEntity<'w>; N],
}
/// 仅输入集合拥有的内置请求，不是可逃逸的存储头；安装逐份消费责任。
pub(crate) enum BuiltinRequest<'w> {
    /// 空链头请求，不拥有目标引用。
    Empty {
        /// 具体容器泛型类型，目标 World 必须已登记。
        type_id: TypeId,
    },
    /// 已接受的强数组输入，Drop 未安装部分会交还 strong。
    Strong {
        /// 包含 N/T 的具体数组组件类型。
        type_id: TypeId,
        /// 按位置保存的原强责任，不额外增持；长度必须等于注册 N。
        values: Vec<crate::owned_entity::OwnedEntity<'w>>,
    },
    /// 已准备的弱计数责任，安装时转入 WeakEntities。
    Weak {
        /// 包含 N/T 的具体弱数组类型。
        type_id: TypeId,
        /// 失败/遗弃时由请求释放仍持有的弱责任。
        values: WeakArrayRequest<'w>,
    },
}
/// 请求独占的弱责任，非普通弱存储值，也不对用户公开临时 WeakEntity。
pub(crate) struct WeakArrayRequest<'w> {
    /// 同一 World/授权；N=0 无目标，不需 World 绑定。
    pub(crate) world: Option<&'w crate::world::World>,
    /// 每项一份已取得但尚未安装的 weak，重复身份逐份保存。
    pub(crate) entities: Vec<crate::entity::Entity>,
}
impl Drop for WeakArrayRequest<'_> {
    /// 仅释放请求仍拥有的 weak；安装移交的项不得再次释放，归零只登记延迟槽回收。
    fn drop(&mut self) {
        entity_values_inner::drop_weak_request_inner(self)
    }
}
impl<'w> EntityValues<'w> {
    /// values 按位置移交原 strong，不增持；重复目标按份计，N=0 合法。
    /// 重复类型/异 World 或分支/准备拒绝交回完整数组且集合不变；不留下部分转移。
    /// 此处不隐式注册；spawn/Alter 安装前校验类型、目标 World/分支及授权，安装不重复加计数。
    pub fn strong_entities<const N: usize, T: 'static>(
        &mut self,
        values: [crate::owned_entity::OwnedEntity<'w>; N],
    ) -> Result<(), StrongArrayInputError<'w, N>> {
        entity_values_inner::strong_entities_inner::<N, T>(self, values)
    }
    /// values 的强引用仅用于证明活身份，不消费也不增持 strong；逐位置新增 weak 给请求拥有。
    /// N=0 合法；重复类型/异 World 或分支/weak 溢出/准备拒绝不改集合，交还已取得的临时 weak。
    /// 请求可晚于这些强引用对象本身存在，但不得越过 World/授权；不得擦除为 static。
    pub fn weak_entities<const N: usize, T: 'static>(
        &mut self,
        values: [&crate::owned_entity::OwnedEntity<'w>; N],
    ) -> Result<(), EcsError> {
        entity_values_inner::weak_entities_inner::<N, T>(self, values)
    }

    /// 创建空值集合，可用于空原型；不登记任何类型。
    pub fn new() -> Self {
        entity_values_inner::new_inner()
    }
    /// 为 `EntityList<T>` 登记空头创建请求，仅 spawn/Alter 执行，重复类型错误不改集合。
    pub fn empty_list<T: 'static>(&mut self) -> Result<(), EcsError> {
        entity_values_inner::builtin_empty_inner::<crate::entity_container::EntityList<T>>(self)
    }
    /// 为 `EntityQueue<T>` 登记空头创建请求，不构造可被用户替换的组件值。
    pub fn empty_queue<T: 'static>(&mut self) -> Result<(), EcsError> {
        entity_values_inner::builtin_empty_inner::<crate::entity_container::EntityQueue<T>>(self)
    }
    /// 为 `EntityDeque<T>` 登记空头创建请求，重复类型错误不改集合。
    pub fn empty_deque<T: 'static>(&mut self) -> Result<(), EcsError> {
        entity_values_inner::builtin_empty_inner::<crate::entity_container::EntityDeque<T>>(self)
    }
    /// 把 value 移入集合；同 T 已有值时返回错误并保留原条目，新 value 正常释放。
    /// T 必须选择 PlainAccess，内置头编译期拒绝；不承诺跨线程或调用复制函数。
    pub fn insert<T: crate::component::Component<Access = crate::component::PlainAccess>>(
        &mut self,
        value: T,
    ) -> Result<(), EcsError> {
        entity_values_inner::insert_inner(self, value)
    }
}
