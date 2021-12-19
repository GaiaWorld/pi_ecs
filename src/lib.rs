#![feature(core_intrinsics)]
#![feature(proc_macro_hygiene)]
#![feature(min_specialization)]

extern crate atom;
extern crate listener;
extern crate map;
extern crate slab;

extern crate any;
extern crate hash;
extern crate share;
// #[cfg(feature = "wasm-bindgen")]
// extern crate wasm_bindgen_cross_performance;
// #[cfg(feature = "native")]
// extern crate native_cross_performance;
// extern crate im;
pub extern crate paste;

pub extern crate time;
extern crate log;

// pub extern crate web_sys;

// #[cfg(feature = "wasm-bindgen")]
// pub crate use wasm_bindgen_cross_performance as cross_performance;
// #[cfg(feature = "native")]
// pub crate use native_cross_performance as cross_performance;

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

pub use world::World;

pub mod prelude {
    // #[cfg(feature = "bevy_reflect")]
    // pub use crate::reflect::ReflectComponent;
    pub use crate::{
        entity::Entity,
        query::QueryState,
        // system::{
        //     Commands, In, IntoChainSystem, IntoExclusiveSystem, IntoSystem, Local, NonSend,
        //     NonSendMut, Query, QuerySet, RemovedComponents, Res, ResMut, System,
        // },
        world::World,
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