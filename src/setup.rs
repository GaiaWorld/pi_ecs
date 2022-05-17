use crate::prelude::{World, StageBuilder};

/// 定义trait Setup
pub trait Setup {
	fn setup(world: &mut World, stage_builder: &mut StageBuilder);
}