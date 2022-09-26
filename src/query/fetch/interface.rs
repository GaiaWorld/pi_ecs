use derive_deref::{Deref, DerefMut};
use pi_ecs_macros::all_tuples;

use crate::{
	world::{WorldInner, World},
	archetype::{Archetype, ArchetypeId, ArchetypeComponentId, ArchetypeIdent},
	storage::LocalVersion,
	component::{ComponentId, Component},
	query::access::FilteredAccess,
};
use pi_share::ThreadSync;

/// WorldQuery 从world上fetch组件、实体、资源，需要实现该triat
pub trait WorldQuery {
    type Fetch: for<'s> Fetch<'s, State = Self::State>;
    type State: FetchState;
}

pub trait Fetch<'s>: Sized + ThreadSync + 'static {
    type Item;
    type State: FetchState;

    /// 创建一个新的fetch实例.
    ///
    /// # Safety
    /// `state` must have been initialized (via [FetchState::init]) using the same `world` passed in
    /// to this function.
    unsafe fn init(
        world: &World,
        state: &Self::State
    ) -> Self;

	unsafe fn setting(&mut self, _world: &WorldInner, _last_change_tick: u32, _change_tick: u32) {

	}

    /// Adjusts internal state to account for the next [Archetype]. This will always be called on
    /// archetypes that match this [Fetch]
    ///
    /// # Safety
    /// `archetype` and `tables` must be from the [World] [Fetch::init] was called on. `state` must
    /// be the [Self::State] this was initialized with.
	/// be the [Self::State::match_archetype] was called and return `true`.
    unsafe fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype, world: &World);

    /// # Safety
    /// Must always be called _after_ [Fetch::set_archetype]. `archetype_index` must be in the range
    /// of the current archetype
    unsafe fn archetype_fetch(&mut self, archetype_index: LocalVersion) -> Option<Self::Item>;

	/// # fetch fail will panic
	unsafe fn archetype_fetch_unchecked(&mut self, archetype_index: LocalVersion) -> Self::Item;

	unsafe fn main_fetch<'a>(&'a self, _state: &Self::State, _last_change_tick: u32, _change_tick: u32) -> Option<MianFetch<'a>> {
		None
	}
}
/// State used to construct a Fetch. This will be cached inside QueryState, so it is best to move as
/// much data / computation here as possible to reduce the cost of constructing Fetch.
/// SAFETY:
/// Implementor must ensure that [FetchState::update_component_access] and
/// [FetchState::update_archetype_component_access] exactly reflects the results of
/// [FetchState::matches_archetype], [FetchState::matches_table], [Fetch::archetype_fetch], and
/// [Fetch::table_fetch]
pub unsafe trait FetchState: ThreadSync + 'static + Sized {
	/// 创建FetchState实例
    fn init(world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self;
    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut FilteredAccess<ArchetypeComponentId>);
    fn matches_archetype(&self, archetype: &Archetype) -> bool;
	// fn get_matches(&self) -> bool;
	fn init_archetype<A: ArchetypeIdent>(&self, _world: &mut World) {}
    // fn matches_table(&self, table: &Table) -> bool;

	fn apply(&self, _world: &mut World) {

	}
}

/// A fetch that is read only. This must only be implemented for read-only fetches.
pub unsafe trait ReadOnlyFetch {}

pub struct MianFetch<'a> {
	pub(crate) value: pi_slotmap::secondary::Keys<'a, LocalVersion, ()>,
	pub(crate) next: Option<Box<MianFetch<'a>>>,
}

#[derive(Deref, DerefMut)]
pub struct DefaultComponent<T: Component>(pub T);


macro_rules! impl_tuple_fetch {
    ($(($name: ident, $state: ident)),*) => {
        #[allow(non_snake_case)]
        impl<'s, $($name: Fetch<'s>),*> Fetch<'s> for ($($name,)*) {
            type Item = ($($name::Item,)*);
            type State = ($($name::State,)*);

            unsafe fn init(_world: &World, state: &Self::State) -> Self {
                let ($($name,)*) = state;
                ($($name::init(_world, $name),)*)
            }

			unsafe fn main_fetch<'x>(&'x self, state: &Self::State, _last_change_tick: u32, _change_tick: u32) -> Option<MianFetch<'x>> {
				$crate::paste::item! {
					let ($([<state $name>],)*) = state;
					let ($($name,)*) = self;
					$(
						if let Some(r) = $name.main_fetch([<state $name>], _last_change_tick, _change_tick) {
							return Some(r)
						};
					)*
					None
				}
			}

			#[allow(unused_variables)]
			#[inline]
			unsafe fn setting(&mut self, world: &WorldInner, last_change_tick: u32, change_tick: u32) {
				let ($($name,)*) = self;
				$(
					$name.setting(world, last_change_tick, change_tick);
				)*
			}

            #[inline]
			#[allow(unused_variables)]
            unsafe fn set_archetype(&mut self, _state: &Self::State, _archetype: &Archetype, world: &World) {
                let ($($name,)*) = self;
                let ($($state,)*) = _state;
                $($name.set_archetype($state, _archetype, world);)*
            }

            #[inline]
			#[allow(unused_variables)]
            unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
                let ($($name,)*) = self;
                Some(($(match $name.archetype_fetch(local) {
					Some(r) => r,
					None => return None
				},)*))
            }

			#[inline]
			#[allow(unused_variables)]
            unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
                let ($($name,)*) = self;
                ($($name.archetype_fetch_unchecked(local),)*)
            }
        }

        // SAFE: update_component_access and update_archetype_component_access are called for each item in the tuple
        #[allow(non_snake_case)]
		#[allow(unused_variables)]
        unsafe impl<$($name: FetchState),*> FetchState for ($($name,)*) {
            fn init(_world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self {
                ($($name::init(_world, query_id, archetype_id),)*)
            }

			fn init_archetype<A: ArchetypeIdent>(&self, _world: &mut World)  {
				let ($($name,)*) = self;
                $($name.init_archetype::<A>(_world);)*
			}

            fn update_archetype_component_access(&self, archetype: &Archetype, _access: &mut FilteredAccess<ComponentId>) {
                let ($($name,)*) = self;
                $($name.update_archetype_component_access(archetype, _access);)*
            }


			#[allow(unused_variables)]
            fn matches_archetype(&self, _archetype: &Archetype) -> bool {
                let ($($name,)*) = self;
                true $(&& $name.matches_archetype(_archetype))*
            }
        }

        impl<$($name: WorldQuery),*> WorldQuery for ($($name,)*) {
            type Fetch = ($($name::Fetch,)*);
            type State = ($($name::State,)*);
        }

        /// SAFE: each item in the tuple is read only
        unsafe impl<$($name: ReadOnlyFetch),*> ReadOnlyFetch for ($($name,)*) {}

    };
}

all_tuples!(impl_tuple_fetch, 0, 16, F, S);