use crate::prelude::{World, StageBuilder};

/// 定义trait Setup
pub trait Setup {
	// 如果存在system，返回system的id
	fn setup(world: &mut World, stage_builder: &mut StageBuilder) -> Option<usize>;
}