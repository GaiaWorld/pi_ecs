use std::marker::PhantomData;

use pi_ecs_macros::{setup, setup1};

pub struct CalcText<F>(PhantomData<F>);

#[setup]
impl<F: Default + 'static + Send + Sync> CalcText<F> {
	#[system]
	pub fn ff() {}
}

pub struct CalcText1;

#[setup]
impl CalcText1 {
	#[system]
	pub fn ff() {}
}
// impl < F : Default > CalcText < F > {} impl < F : Default > pi_ecs :: SystemParamFieldAttributes:: Setup for CalcText < F > < F >
// {
// 	  fn
// 	     setup(world : & mut pi_ecs :: prelude :: World, stage_builder : & mut
// 			    pi_ecs :: prelude :: StageBuilder) {}
// 		}