#![feature(box_into_inner)]
#![feature(core_intrinsics)]
#![feature(proc_macro_hygiene)]
#![feature(specialization)]
#![feature(if_let_guard)]
#![feature(associated_type_defaults)]

#[macro_use]
extern crate pi_any;
extern crate paste;

pub mod world;
pub mod component;
pub mod entity;
pub mod archetype;
pub mod sys;
pub mod storage;
pub mod query;
pub mod pointer;
pub mod resource;
pub mod dispatch;
pub mod monitor;
mod setup;

pub use world::WorldInner;

pub mod prelude {
    // #[cfg(feature = "bevy_reflect")]
    // pub use crate::reflect::ReflectComponent;
    pub use crate::{
        query::*,
        sys::{
			system::*,
			param::*,
		},
		setup::Setup,
		monitor::{Listeners, Monitor, Event, ListenSetup, ComponentListen, ResourceListen, EntityListen, Create, Modify, Delete, EventType},
        world::{World, FromWorld},
		dispatch::interface::*,
		component::Component,
		archetype::{ArchetypeId, Archetype},
		entity::{Entities, Id, Entity},
		storage::{LocalVersion, Offset},
    };
}




// use rand::{Rng, SeedableRng};


// pub struct Benchmark4{

// 	arr: Vec<usize>,
// 	dirtys: Vec<usize>,

// 	r: usize,
// }

// impl Benchmark4 {
//     pub fn new() -> Self {
// 		let mut arr = Vec::new();

// 		let len = 10000;

// 		for i in 0..len {
// 			arr.push(i);
// 		}

// 		let mut dirtys = Vec::new();
// 		let mut rng = rand::thread_rng();
// 		dirtys.push(rng.gen_range(0..len as u32) as usize);

// 		Self {
// 			dirtys,
// 			r: 0,
// 			arr,
// 		}
//     }

//     pub fn run(&mut self) {
// 		let mut k = 0;
// 		let mut len1 = self.arr.len() - 2;
// 		let mut len2 = self.arr.len() - 2;
// 		for i in 0..1000 {
// 			println!("zzzz{:?}, {:?}", self.dirtys.len(), self.arr.len());
// 			println!("xxxxxxxx{:?}, {:?}", self.dirtys[i], self.arr.len());
// 			self.r = self.arr[self.dirtys[i]]; 
// 		}
// 		self.r = k;
// 	}
// }

// #[test]
// fn aa() {
	

// 	let mut bench = Benchmark4::new();
//     bench.run();
// }
// #[test]
// fn aa() {
	

// 	let mut bench = Benchmark2::new();
//     bench.run();
// }