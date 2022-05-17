use pi_ecs_macros::{setup, listen};
use pi_ecs::{prelude::{IntoSystem, World}, monitor::Event};

struct AA;

#[setup]
impl AA {
	#[system]
	pub async fn run1() {

	}
	#[system]
	fn run2() {

	}

	#[init]
	fn init(w: &mut World) {

	}

	#[listen(component=(usize, usize, Create))]
	fn listen1(e: Event) {

	}
	#[listen(component=(usize, usize, Modify))]
	fn listen2(e: Event) {

	}
}