
use std::marker::PhantomData;

use crate::{
    archetype::{ArchetypeId, ArchetypeIdent},
    query::{Fetch, FilterFetch, QueryState, WorldQuery, EntityFetch},
    storage::Offset,
    world::World,
};

pub struct QueryIter<'w, 's,  A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery>
where
    F::Fetch: FilterFetch,
{
	archetype_id: ArchetypeId,
	matchs: bool,
    world: &'w World,
    fetch: &'s mut Q::Fetch,
    filter:&'s mut F::Fetch,
	entity: EntityFetch,
	mark: PhantomData<A>,
}

impl<'w, 's,  A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery> QueryIter<'w, 's, A, Q, F>
where
    F::Fetch: FilterFetch,
{
    pub(crate) unsafe fn new(
        world: &'w World,
        query_state: &'s QueryState<A, Q, F>,
        _last_change_tick: u32,
        _change_tick: u32,
    ) -> Self {
        // let mut fetch = <Q::Fetch as Fetch>::init(
        //     world,
        //     &query_state.fetch_state,
        //     // last_change_tick,
        //     // change_tick,
        // );
        // let mut filter = <F::Fetch as Fetch>::init(
        //     world,
        //     &query_state.filter_state,
        //     // last_change_tick,
        //     // change_tick,
        // );
		let fetch = &query_state.fetch_fetch;
		let filter = &query_state.filter_fetch;
		let mut entity = EntityFetch::init(world,
            &query_state.entity_state);
		
		if query_state.matchs {
			entity.set_archetype(
				&query_state.entity_state,
				&world.archetypes()[query_state.archetype_id],
				&world,
			);
		}
		
		#[allow(mutable_transmutes)]
        QueryIter {
            // is_dense: fetch.is_dense() && filter.is_dense(),
            world,
			matchs: query_state.matchs,
            fetch: std::mem::transmute(fetch),
            filter: std::mem::transmute(filter),
			entity,
			archetype_id: query_state.archetype_id,
            mark: PhantomData,
        }
    }
}

impl<'w, 's, A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery> Iterator for QueryIter<'w, 's, A, Q, F>
where
    F::Fetch: FilterFetch,
{
    type Item = <Q::Fetch as Fetch>::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
			if !self.matchs {
				return  None;
			}
			
			loop {
				let entity = self.entity.archetype_fetch(0)?;

				if !self.filter.archetype_filter_fetch(entity.local().offset()) {
					continue;
				}

				let item = self.fetch.archetype_fetch(entity.local().offset());
				if let None = item {
					continue;
				}
				return item;
			}
        }
    }

    // NOTE: For unfiltered Queries this should actually return a exact size hint,
    // to fulfil the ExactSizeIterator invariant, but this isn't practical without specialization.
    // For more information see Issue #1686.
    fn size_hint(&self) -> (usize, Option<usize>) {
		let max_size = self.world.archetypes[self.archetype_id].len();

        (0, Some(max_size))
    }
}

// NOTE: We can cheaply implement this for unfiltered Queries because we have:
// (1) pre-computed archetype matches
// (2) each archetype pre-computes length
// (3) there are no per-entity filters
// TODO: add an ArchetypeOnlyFilter that enables us to implement this for filters like With<T>
impl<'w, 's, A: ArchetypeIdent, Q: WorldQuery> ExactSizeIterator for QueryIter<'w, 's, A, Q, ()> {
    fn len(&self) -> usize {
		self.world.archetypes[self.archetype_id].len()
    }
}
