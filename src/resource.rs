use std::ops::{Deref, DerefMut};

use any::ArcAny;
use share::cell::TrustCell;

use crate::{
	monitor::{Notify, NotifyImpl, Listener},
	entity::Entity, component::Component,
};

pub trait SingleCase: Notify + ArcAny {}
impl_downcast_arc!(SingleCase);

pub type CellSingleCase<T> = TrustCell<SingleCaseImpl<T>>;

impl<T: Component> SingleCase for CellSingleCase<T> {}

// TODO 以后用宏生成
impl<T: Component> Notify for CellSingleCase<T> {
    fn add_create(&self, listener: Listener) {
        self.borrow_mut().notify.add_create(listener);
    }
    fn add_delete(&self, listener: Listener) {
        self.borrow_mut().notify.add_delete(listener)
    }
    fn add_modify(&self, listener: Listener) {
        self.borrow_mut().notify.add_modify(listener)
    }
    fn create_event(&self, id: Entity) {
        self.borrow().notify.create_event(id);
    }
    fn delete_event(&self, id: Entity) {
        self.borrow().notify.delete_event(id);
    }
    fn modify_event(&self, id: Entity, field: &'static str, index: usize) {
        self.borrow().notify.modify_event(id, field, index);
    }
    fn remove_create(&self, listener: &Listener) {
        self.borrow_mut().notify.remove_create(listener);
    }
    fn remove_delete(&self, listener: &Listener) {
        self.borrow_mut().notify.remove_delete(listener);
    }
    fn remove_modify(&self, listener: &Listener) {
        self.borrow_mut().notify.remove_modify(listener);
    }
}

pub struct SingleCaseImpl<T: 'static> {
    value: T,
    notify: NotifyImpl,
}

impl<T: 'static> Deref for SingleCaseImpl<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: 'static> DerefMut for SingleCaseImpl<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T: 'static> SingleCaseImpl<T> {
    pub fn new(value: T) -> TrustCell<Self> {
        TrustCell::new(SingleCaseImpl {
            value,
            notify: NotifyImpl::default(),
        })
    }
    pub fn get_notify(&self) -> NotifyImpl {
        self.notify.clone()
    }

    pub fn get_notify_ref(&self) -> &NotifyImpl {
        &self.notify
    }
}