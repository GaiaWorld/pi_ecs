use share::cell::TrustCell;

use crate::{
    archetype::{Archetype, ArchetypeId, ArchetypeComponentId},
    component::{Component, ComponentId, MultiCaseImpl},
    entity::Entity,
    query::{ FilteredAccess, Access, FilterFetch, WorldQuery, Fetch, FetchState, ReadOnlyFetch},
    storage::LocalVersion,
    world::World,
};

use std::{
    marker::PhantomData,
    // ptr::{NonNull},
	// mem::MaybeUninit,
	ops::Deref, any::TypeId, sync::Arc,
};


pub struct Join<C: Component + Deref<Target = Entity>, A, Q: WorldQuery, F: WorldQuery = ()>(PhantomData<(C, A, Q, F)>) where F::Fetch: FilterFetch;


impl<C: Component + Deref<Target = Entity>, A: Send + Sync + 'static, Q: WorldQuery, F: WorldQuery> WorldQuery for Join<C, A, Q, F> where F::Fetch: FilterFetch {
    type Fetch = JoinFetch<C, A, Q::Fetch, F::Fetch>;
    type State = JoinState<C, A, Q::State, F::State>;
}

pub struct JoinFetch<C: Component + Deref<Target = Entity>, A: Send + Sync + 'static, Q, F> {
	fetch: Q,
	filter: F,
	// container: MaybeUninit<NonNull<u8>>,
	container: usize,
	mark: PhantomData<(C, A)>,
}

unsafe impl<C, A, Q, F> ReadOnlyFetch for JoinFetch<C, A, Q, F> 
	where Q: ReadOnlyFetch,
		  A: Send + Sync + 'static,
		  C: Component + Deref<Target = Entity> {}


impl<C: Component + Deref<Target = Entity>, A: Send + Sync + 'static, Q: Fetch, F: FilterFetch> Fetch for JoinFetch<C, A, Q, F> {
    type Item = Q::Item;
    type State = JoinState<C, A, Q::State, <F as Fetch>::State>;

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
			Some(r) => if self.filter.archetype_filter_fetch((**r).local()){
				self.fetch.archetype_fetch((**r).local())
			} else {
				None
			},
			None => None
		}
    }
}



pub struct JoinState<C: Component + Deref<Target = Entity>, A, Q, F> {
	fetch_state: Q,
	filter_state: F,
	world: World,
	component_id: ComponentId,
	archetype_id: ArchetypeId,
	mark: PhantomData<(A, C)>
}

unsafe impl<C: Component + Deref<Target = Entity>, A: Send + Sync + 'static, Q: FetchState, F: FetchState> FetchState for JoinState<C, A, Q, F> {
    fn init(world: &mut World) -> Self {
		let archetype_id = match world.archetypes().get_id_by_ident(TypeId::of::<A>()) {
			Some(r) => r.clone(),
			None => panic!("JoinState fetch archetype ${} fail", std::any::type_name::<A>()),
		};
		let component_id = match world.components.get_id(TypeId::of::<C>()) {
			Some(r) => r,
			None => panic!("JoinState component fetch ${} fail", std::any::type_name::<C>()),
		};
        let component_info = world.components.get_info(component_id).unwrap();
        Self {
			world: world.clone(),
			archetype_id,
			component_id: component_info.id(),
            fetch_state: Q::init(world),
			filter_state: F::init(world),
			mark: PhantomData,
        }
    }

	fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
		if access.access().has_write(self.component_id) {
            panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<C>());
        }
        access.add_read(self.component_id);
		self.fetch_state.update_component_access(access);
		self.filter_state.update_component_access(access);
	}

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>) {
		let archetype_component_id = unsafe { archetype.archetype_component_id(self.component_id)};
        access.add_read(archetype_component_id);
		let a = &self.world.archetypes()[self.archetype_id];
        self.fetch_state.update_archetype_component_access(a, access);
		self.filter_state.update_archetype_component_access(a, access);
    }

    fn matches_archetype(&mut self, archetype: &Archetype, world: &World) -> bool {
		let inner_archetype = &world.archetypes()[self.archetype_id];
		archetype.contains(self.component_id) && self.fetch_state.matches_archetype(inner_archetype, world) && self.filter_state.matches_archetype(inner_archetype, world)
	}
}