use pi_share::cell::TrustCell;

use crate::{
    archetype::{Archetype, ArchetypeId, ArchetypeComponentId, ArchetypeIdent},
    component::{Component, ComponentId, MultiCaseImpl},
    entity::Id,
	query::{
		access::FilteredAccess,
		filter::FilterFetch,
	},
    storage::LocalVersion,
    world::World,
};

use super::interface::{WorldQuery, Fetch, FetchState, ReadOnlyFetch};

use std::{
    marker::PhantomData,
    // ptr::{NonNull},
	// mem::MaybeUninit,
	ops::Deref, sync::Arc,
};


pub struct Join<C: Component + Deref<Target = Id<A>>, A, Q: WorldQuery, F: WorldQuery = ()>(PhantomData<(C, A, Q, F)>) where F::Fetch: FilterFetch;


impl<C: Component + Deref<Target = Id<A>>, A: ArchetypeIdent, Q: WorldQuery, F: WorldQuery> WorldQuery for Join<C, A, Q, F> where F::Fetch: FilterFetch {
    type Fetch = JoinFetch<C, A, Q::Fetch, F::Fetch>;
    type State = JoinState<C, A, Q::State, F::State>;
}

pub struct JoinFetch<C: Component + Deref<Target = Id<A>>, A: ArchetypeIdent, Q, F> {
	fetch: Q,
	filter: F,
	// container: MaybeUninit<NonNull<u8>>,
	container: usize,
	mark: PhantomData<(C, A)>,
}

unsafe impl<C, A, Q, F> ReadOnlyFetch for JoinFetch<C, A, Q, F> 
	where Q: ReadOnlyFetch,
		  A: ArchetypeIdent,
		  C: Component + Deref<Target = Id<A>> {}


impl<'s, C: Component + Deref<Target = Id<A>>, A: ArchetypeIdent, Q: Fetch<'s>, F: FilterFetch> Fetch<'s> for JoinFetch<C, A, Q, F> {
    type Item = Q::Item;
    type State = JoinState<C, A, Q::State, <F as Fetch<'s>>::State>;

    unsafe fn init(
        world: &World,
        state: &Self::State,
    ) -> Self {
		
        Self {
            fetch: Q::init(world, &state.fetch_state),
			filter: F::init(world, &state.filter_state),
			container: 0,
			mark: PhantomData,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
		world: &World,
    ) {
		let container = archetype.get_component(state.component_id);
		match container.clone().downcast() {
			Ok(r) => {
				let r: Arc<TrustCell<MultiCaseImpl<C>>> = r;
				self.container = (*r).as_ptr() as usize;
				let inner_archetype = &world.archetypes()[state.archetype_id];
				self.fetch.set_archetype(&state.fetch_state, inner_archetype, world);
				self.filter.set_archetype(&state.filter_state, inner_archetype, world);
			},
			Err(_) => panic!("downcast error"),
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
		let c: Option<&C> = std::mem::transmute((&mut *(self.container as *mut MultiCaseImpl<C>)).get_mut(local));
		match c {
			Some(r) => if self.filter.archetype_filter_fetch((**r).0){
				self.fetch.archetype_fetch((**r).0)
			} else {
				None
			},
			None => None
		}
    }

	#[inline]
    unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
		let c: &C = std::mem::transmute((&mut *(self.container as *mut MultiCaseImpl<C>)).get_unchecked_mut(local));
		// if self.filter.archetype_filter_fetch((**r).local()){
			self.fetch.archetype_fetch_unchecked((**c).0)
		// }
		// match c {
		// 	Some(r) => if self.filter.archetype_filter_fetch((**r).local()){
		// 		self.fetch.archetype_fetch((**r).local())
		// 	} else {
		// 		None
		// 	},
		// 	None => None
		// }
    }
}



pub struct JoinState<C: Component + Deref<Target = Id<A>>, A, Q, F> {
	fetch_state: Q,
	filter_state: F,
	world: World,
	component_id: ComponentId,
	archetype_id: ArchetypeId,
	mark: PhantomData<(A, C)>
}

unsafe impl<C: Component + Deref<Target = Id<A>>, A: ArchetypeIdent, Q: FetchState, F: FetchState> FetchState for JoinState<C, A, Q, F> {
    fn init(world: &mut World, query_id: usize, archetype_id: ArchetypeId) -> Self {
		let archetype_id_next = world.archetypes_mut().get_or_create_archetype::<A>();
		let component_id = world.get_or_register_component::<C>(archetype_id);
        let component_info = world.components.get_info(component_id).unwrap();
        Self {
			world: world.clone(),
			archetype_id: archetype_id_next,
			component_id: component_info.id(),
            fetch_state: Q::init(world, query_id, archetype_id_next),
			filter_state: F::init(world, query_id, archetype_id_next),
			mark: PhantomData,
        }
    }

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut FilteredAccess<ArchetypeComponentId>) {
		let archetype_component_id = unsafe { archetype.archetype_component_id(self.component_id)};
        access.add_read(archetype_component_id);
		let a = &self.world.archetypes()[self.archetype_id];
        self.fetch_state.update_archetype_component_access(a, access);
		self.filter_state.update_archetype_component_access(a, access);
		access.add_read(a.entity_archetype_component_id());
    }

    fn matches_archetype(&self, archetype: &Archetype) -> bool {
		let inner_archetype = &self.world.archetypes()[self.archetype_id];
		archetype.contains(self.component_id) && self.fetch_state.matches_archetype(inner_archetype) && self.filter_state.matches_archetype(inner_archetype)
	}
}