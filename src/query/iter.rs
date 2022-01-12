
use std::marker::PhantomData;

use crate::{
    archetype::{ArchetypeId, ArchetypeIdent},
    query::{Fetch, FilterFetch, QueryState, WorldQuery, MianFetch},
    storage::{LocalVersion, Keys},
    world::WorldInner,
};

pub struct EntityIter<'a>(pub(crate) Vec<Keys<'a, LocalVersion, ()>>, pub(crate) Keys<'a, LocalVersion, ()>);

impl<'a> EntityIter<'a> {
	pub fn next(&mut self) -> Option<LocalVersion> {
		if let Some(r) = self.1.next() {
			return Some(r);
		}

		if self.0.len() == 0 {
			return None;
		}

		self.1 = self.0.pop().unwrap();
		return self.1.next();
	}
}

pub struct QueryIter<'w, 's,  A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery>
where
    F::Fetch: FilterFetch,
{
	archetype_id: ArchetypeId,
	matchs: bool,
    world: &'w WorldInner,
    fetch: &'s mut Q::Fetch,
    filter:&'s mut F::Fetch,
	entity: EntityIter<'s>,
	mark: PhantomData<A>,
}

impl<'w, 's,  A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery> QueryIter<'w, 's, A, Q, F>
where
    F::Fetch: FilterFetch,
{
    pub(crate) unsafe fn new(
        world: &'w WorldInner,
        query_state: &'s mut QueryState<A, Q, F>,
        last_change_tick: u32,
        change_tick: u32,
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
		let fetch = &mut query_state.fetch_fetch;
		

		// let mut entity = EntityFetch::init(world,
        //     &query_state.entity_state);
		fetch.setting(world, last_change_tick, change_tick);
		query_state.filter_fetch.setting(world, last_change_tick, change_tick);

		let filter = & query_state.filter_fetch;
		let iter = match filter.main_fetch(&query_state.filter_state, last_change_tick, change_tick) {
			Some(r) => r,
			None => MianFetch{
				value: std::mem::transmute(world.archetypes()[query_state.archetype_id].entities.keys()),
				next: None,
			} 
		};

		let (value, mut next) = (iter.value,iter.next);

		let mut iter1 = EntityIter(Vec::new(), value);
		loop {
			match next {
				Some(r) => {
					let r = Box::into_inner(r);
					let (value, next1) = (r.value,r.next);
					iter1.0.push(value);
					next = next1;
				},
				None => break
			}
		} 
		
		#[allow(mutable_transmutes)]
        QueryIter {
            // is_dense: fetch.is_dense() && filter.is_dense(),
            world,
			matchs: query_state.matchs,
            fetch: std::mem::transmute(fetch),
            filter: std::mem::transmute(filter),
			entity: iter1,
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
				let entity = self.entity.next()?;

				if !self.filter.archetype_filter_fetch(entity) {
					continue;
				}

				let item = self.fetch.archetype_fetch(entity);
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
