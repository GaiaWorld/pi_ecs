// #![feature(proc_macro_hygiene)]
// #![feature(stmt_expr_attributes)]
// use pi_ecs::{monitor::Event, prelude::Local};
// use pi_ecs_macros::{struct_listen, listen};
// use pi_share::{ShareRefCell, cell::TrustCell};
// use std::{borrow::Borrow, sync::Arc, marker::PhantomData};

// pub struct Aa {}
// pub struct Node;
// pub struct Layer;
// pub struct Position;

// #[derive(Default)]
// pub struct Local1;

// pub trait XXX {
	
// }

// pub struct ZZ<B>(PhantomData<B>);

// impl Aa {
// 	// #[struct_listen(component=(Node, Position, Modify))]
// 	fn listener_component_entity(
// 		&self,
// 		input: Event,
// 		// local: Local<Local1>,
// 	) {
		
// 	}
// }

// // impl Aa {
// // 	// #[struct_listen(component=(Node, Position, Modify))]
// // 	fn listener_component_entity(
// // 		&self,
// // 		input: Event,
// // 		// local: Local<Local1>,
// // 	){
// // 		let rr = || {

// // 		};
// // 		let bb = rr;
		
// // 	}
// // }

// fn xx() {
// 	let r = ShareRefCell::new(Aa{});
	
// 	let l = Aa::get_listener_component_entity(r.clone());
// 	let zz = l.clone();
// 	let ll = l.listener();
// }

// fn ss() {

// }

// // FnMut(Event, Listen<(ComponentListen<Node, Position, Modify>,)
// // #[listen(component=(Node, Position, Modify))]
// // fn listener_component_entity(
// // 	input: Event,
// // 	local: Local<Local1>,
// // ) -> impl Fn() {
// // 	#[listen(component=(Node, Position, Modify))]
// // 	move|e: Event| {
// // 		let aa = 0;
// // 	}
// // }

