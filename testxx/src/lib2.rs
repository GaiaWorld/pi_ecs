#![feature(prelude_import)]
#![feature(proc_macro_hygiene)]
#[prelude_import]
use std::prelude::rust_2018::*;
#[macro_use]
extern crate std;
use pi_ecs::{monitor::Event, prelude::Local};
use pi_ecs_macros::struct_listen;
use pi_share::ShareRefCell;
use std::borrow::Borrow;
pub struct Aa {}
pub struct Node;
pub struct Layer;
pub struct Position;
pub struct Local1;
impl Aa {
    pub fn get_listener_component_entity(
        __context: ShareRefCell<Self>,
        input: Event,
        local: Local<Local1>,
    ) -> impl FnMut(Event, Local<Local1>) {
        move |input: Event, local: Local<Local1>| {
            self.listener_component_entity(__context.borrow(), input, local);    
        }
    }
    fn listener_component_entity(&self, input: Event, local: Local<Local1>) {}   
}