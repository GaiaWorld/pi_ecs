use pi_ecs_macros::all_tuples;


use crate::{
	query::{
		fetch::interface::{Fetch, WorldQuery, MianFetch, FetchState},
		access::FilteredAccess,
	},
	archetype::{Archetype, ArchetypeId, ArchetypeComponentId, ArchetypeIdent},
	storage::LocalVersion,
	world::{World, WorldInner},
};
/// Fetch methods used by query filters. This trait exists to allow "short circuit" behaviors for
/// relevant query filter fetches.
pub trait FilterFetch: for<'s> Fetch<'s> {
    /// # Safety
    /// Must always be called _after_ [Fetch::set_archetype]. `archetype_index` must be in the range
    /// of the current archetype
    unsafe fn archetype_filter_fetch(&mut self, local: LocalVersion) -> bool;
}


pub struct Or<T>(pub T);
pub struct OrFetch<T: FilterFetch> {
    pub fetch: T,
    matches: bool,
}

macro_rules! impl_query_filter_tuple {
    ($(($filter: ident, $state: ident)),*) => {
        #[allow(unused_variables)]
        #[allow(non_snake_case)]
        impl<'a, $($filter: FilterFetch),*> FilterFetch for ($($filter,)*) {

            #[inline]
            unsafe fn archetype_filter_fetch(&mut self, local: LocalVersion) -> bool {
                let ($($filter,)*) = self;
                true $(&& $filter.archetype_filter_fetch(local))*
            }
        }

        impl<$($filter: WorldQuery),*> WorldQuery for Or<($($filter,)*)>
            where $($filter::Fetch: FilterFetch),*
        {
            type Fetch = Or<($(OrFetch<$filter::Fetch>,)*)>;
            type State = Or<($($filter::State,)*)>;
        }


        #[allow(unused_variables)]
        #[allow(non_snake_case)]
        impl<'s, $($filter: FilterFetch),*> Fetch<'s> for Or<($(OrFetch<$filter>,)*)> {
            type State = Or<($(<$filter as Fetch<'s>>::State,)*)>;
            type Item = bool;

            unsafe fn init(world: &World, state: &Self::State) -> Self {
                let ($($filter,)*) = &state.0;
                Or(($(OrFetch {
                    fetch: $filter::init(world, $filter),
                    matches: false,
                },)*))
            }

			unsafe fn setting(&mut self, world: &WorldInner, last_change_tick: u32, change_tick: u32) {
				let ($($filter,)*) = &mut self.0;
				$(
					$filter.fetch.setting(world, last_change_tick, change_tick);
				)*
			}

			#[allow(unused_mut)]
			unsafe fn main_fetch<'x>(&'x self, state: &Self::State, last_change_tick: u32, change_tick: u32) -> Option<MianFetch<'x>> {
				$crate::paste::item! {
					let ($([<state $filter>],)*) = &state.0;
					let ($($filter,)*) = &self.0;
					let mut k: Option<MianFetch<'x>> = None;
					$(
						if let Some(r) = $filter.fetch.main_fetch([<state $filter>], last_change_tick, change_tick) {
							if let Some(next) = &k {
								if next.value.len() == 0 {
									k = Some(r);
								} else if r.value.len() == 0 {

								} else {
									return None; // 如果有多个脏，直接遍历整个实体列表， 所以返回None
								}
							} else {
								k = Some(r);
							}
						};
					)*
					k
				}
			}

            #[inline]
            unsafe fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype, world: &World) {
                let ($($filter,)*) = &mut self.0;
                let ($($state,)*) = &state.0;
                $(
                    $filter.matches = $state.matches_archetype(archetype);
                    if $filter.matches {
                        $filter.fetch.set_archetype($state, archetype, world);
                    }
                )*
            }

            #[inline]
            unsafe fn archetype_fetch(&mut self, archetype_index: LocalVersion) -> Option<bool> {
                Some(true)
            }

			#[inline]
			unsafe fn archetype_fetch_unchecked(&mut self, _local: LocalVersion) -> Self::Item {
				true
			}
        }

        // SAFE: update_component_access and update_archetype_component_access are called for each item in the tuple
        #[allow(unused_variables)]
        #[allow(non_snake_case)]
        unsafe impl<$($filter: FetchState),*> FetchState for Or<($($filter,)*)> {
            fn init(world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self {
                Or(($($filter::init(world, query_id, archetype_id),)*))
            }
			fn init_archetype<A: ArchetypeIdent>(&self, world: &mut World)  {
				let ($($filter,)*) = &self.0;
                // let ($($state,)*) = &state.0;
                $(
                    // $filter.matches = $state.matches_archetype(archetype);
                    // if $filter.matches {
                        $filter.init_archetype::<A>( world);
                    // }
                )*
			}

            fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut FilteredAccess<ArchetypeComponentId>) {
                let ($($filter,)*) = &self.0;
                $($filter.update_archetype_component_access(archetype, access);)*
            }

            fn matches_archetype(&self, archetype: &Archetype) -> bool {
                let ($($filter,)*) = &self.0;
                false $(|| $filter.matches_archetype(archetype))*
            }
        }

		#[allow(unused_variables)]
        #[allow(non_snake_case)]
		impl<'a, $($filter: FilterFetch),*> FilterFetch for Or<($(OrFetch<$filter>,)*)> {
			unsafe fn archetype_filter_fetch(&mut self, local: LocalVersion) -> bool {
				let ($($filter,)*) = &mut self.0;
                false $(|| ($filter.matches && $filter.fetch.archetype_filter_fetch(local)))*
			}
		}
    };
}

all_tuples!(impl_query_filter_tuple, 0, 16, F, S);