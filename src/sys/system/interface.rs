pub use pi_listener::FnListener;
use std::borrow::Cow;

use crate::sys::param::SystemParamFetch;
use crate::world::World;
use crate::archetype::ArchetypeComponentId;
use crate::query::{Access, FilteredAccessSet};
use pi_share::ThreadSend;

/// An ECS system that can be added to a [Schedule](crate::schedule::Schedule)
///
/// Systems are functions with all arguments implementing [SystemParam](crate::system::SystemParam).
///
/// Systems are added to an application using `AppBuilder::add_system(my_system.system())`
/// or similar methods, and will generally run once per pass of the main loop.
///
/// Systems are executed in parallel, in opportunistic order; data access is managed automatically.
/// It's possible to specify explicit execution order between specific systems,
/// see [SystemDescriptor](crate::schedule::SystemDescriptor).
pub trait System: ThreadSend + 'static {
    /// The system's input. See [`In`](crate::system::In) for [`FunctionSystem`](crate::system::FunctionSystem)s.
    type In;
    /// The system's output.
    type Out;
    /// Returns the system's name.
    fn name(&self) -> Cow<'static, str>;
    /// Returns the system's [`SystemId`].
    fn id(&self) -> SystemId;
    /// Returns the system's archetype component [`Access`].
    fn archetype_component_access(&self) -> &Access<ArchetypeComponentId>;
    /// Returns true if the system is [`Send`].
    fn is_send(&self) -> bool;
    /// Runs the system with the given input in the world. Unlike [`System::run`], this function
    /// takes a shared reference to [`World`] and may therefore break Rust's aliasing rules, making
    /// it unsafe to call.
    ///
    /// # Safety
    ///
    /// This might access world and resources in an unsafe manner. This should only be called in one
    /// of the following contexts:
    ///     1. This system is the only system running on the given world across all threads.
    ///     2. This system only runs in parallel with other systems that do not conflict with the
    ///        [`System::archetype_component_access()`].
    unsafe fn run_unsafe(&mut self, input: Self::In) -> Self::Out;
    /// Runs the system with the given input in the world.
    fn run(&mut self, input: Self::In) -> Self::Out {
        // SAFE: world and resources are exclusively borrowed
        unsafe { self.run_unsafe(input) }
    }
    fn apply_buffers(&mut self);
    fn check_change_tick(&mut self, change_tick: u32);
}

/// 数据状态
pub struct DataState<PramState> {
	system_state: SystemState,
	param_state: PramState,
}

impl<'w, 's, PramState: SystemParamFetch<'w, 's>> DataState<PramState> {
	pub fn new(world: &mut World) -> Self {
		let mut system_state = SystemState::new::<Self>();
		Self {
			param_state: PramState::init(
				world,
				&mut system_state,
				PramState::default_config(),
			),
			system_state,
		}
	}

	pub fn get_param(&'s mut self, world: &'w mut World) -> PramState::Item {
		unsafe { 
			let change_tick = world.change_tick();
			PramState::get_param(&mut self.param_state, &self.system_state, world, change_tick)
		}
	}
}

#[derive(Clone, Copy)]
pub struct SystemId(usize);

impl SystemId {
	pub fn new(v: usize) -> Self {
		SystemId(v)
	}

	pub fn id(&self) -> usize {
		self.0
	}
}

/// A convenience type alias for a boxed [`System`] trait object.
pub type BoxedSystem<In = (), Out = ()> = Box<dyn System<In = In, Out = Out>>;

/// 系统状态
pub struct SystemState {
    pub(crate) name: Cow<'static, str>, // 系统名称
    pub(crate) archetype_component_access: FilteredAccessSet<ArchetypeComponentId>,
    // NOTE: this must be kept private. making a SystemState non-send is irreversible to prevent
    // SystemParams from overriding each other
    pub(crate) is_send: bool,
    pub last_change_tick: u32,
}

impl SystemState {
    pub fn new<T>() -> Self {
        Self {
            name: std::any::type_name::<T>().into(),
            archetype_component_access: FilteredAccessSet::default(),
            is_send: true,
            last_change_tick: 0,
        }
    }

	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn last_change_tick(&self) -> u32 {
		self.last_change_tick
	}

	pub fn archetype_component_access(&self) -> &FilteredAccessSet<ArchetypeComponentId>{
		&self.archetype_component_access
	}

	pub fn archetype_component_access_mut(&mut self) -> &mut FilteredAccessSet<ArchetypeComponentId>{
		&mut self.archetype_component_access
	}

    /// Returns true if the system is [`Send`].
    #[inline]
    pub fn is_send(&self) -> bool {
        self.is_send
    }

    /// Sets the system to be not [`Send`].
    ///
    /// This is irreversible.
    #[inline]
    pub fn set_non_send(&mut self) {
        self.is_send = false;
    }
}

/// Conversion trait to turn something into a [`System`].
///
/// Use this to get a system from a function. Also note that every system implements this trait as well.
///
/// # Examples
///
/// ```
/// use bevy_ecs::system::IntoSystem;
/// use bevy_ecs::system::Res;
///
/// fn my_system_function(an_usize_resource: Res<usize>) {}
///
/// let system = my_system_function.system();
/// ```
pub trait IntoSystem<Params, SystemType: System> {
    /// Turns this value into its corresponding [`System`].
    fn system(self, world: &mut World) -> SystemType;
}

// Systems implicitly implement IntoSystem
impl<Sys: System> IntoSystem<(), Sys> for Sys {
    fn system(self, _world: &mut World) -> Sys {
        self
    }
}

/// Wrapper type to mark a [`SystemParam`] as an input.
///
/// [`System`]s may take an optional input which they require to be passed to them when they
/// are being [`run`](System::run). For [`FunctionSystems`](FunctionSystem) the input may be marked
/// with this `In` type, but only the first param of a function may be tagged as an input. This also
/// means a system can only have one or zero input paramaters.
///
/// # Examples
///
/// Here is a simple example of a system that takes a [`usize`] returning the square of it.
///
/// ```
/// use bevy_ecs::prelude::*;
///
/// fn main() {
///     let mut square_system = square.system();
///
///     let mut world = World::default();
///     square_system.initialize(&mut world);
///     assert_eq!(square_system.run(12, &mut world), 144);
/// }
///
/// fn square(In(input): In<usize>) -> usize {
///     input * input
/// }
/// ```

pub struct In<In>(pub In);
pub struct InputMarker;

pub struct OutputMarker;
