use crate::{
    archetype::{Archetype, ArchetypeId, ArchetypeComponentId},
    component::{Component, ComponentId, StorageType},
    entity::Entity,
    query::{ FilteredAccess, Access, FilterFetch, WorldQuery, Fetch, FetchState},
    storage::{SecondaryMap, SparseSecondaryMap, Local, Offset},
    world::World,
};

use std::{
    marker::PhantomData,
    // ptr::{NonNull},
	// mem::MaybeUninit,
	ops::Deref, any::TypeId,
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
	storage_type: StorageType,
	mark: PhantomData<(C, A)>,
}


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
			storage_type: state.storage_type,
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
		self.container = archetype.get_component(state.component_id).as_ptr() as usize;
		let inner_archetype = &world.archetypes()[state.archetype_id];
        self.fetch.set_archetype(&state.fetch_state, inner_archetype, world);
		self.filter.set_archetype(&state.filter_state, inner_archetype, world);
		
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, archetype_index: usize) -> Option<Self::Item> {
		let c: Option<&C> = match self.storage_type {
            StorageType::Table => std::mem::transmute((&mut *(self.container as *mut SecondaryMap<Local, C>)).get_mut(Local::new(archetype_index))) ,
            StorageType::SparseSet => std::mem::transmute((&mut *(self.container as *mut SparseSecondaryMap<Local, C>)).get_mut(Local::new(archetype_index))),
        };
		match c {
			Some(r) => if self.filter.archetype_filter_fetch((**r).local().offset()){
				self.fetch.archetype_fetch((**r).local().offset())
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
	component_id: ComponentId,
	storage_type: StorageType,
	archetype_id: ArchetypeId,
	mark: PhantomData<(A, C)>
}

unsafe impl<C: Component + Deref<Target = Entity>, A: Send + Sync + 'static, Q: FetchState, F: FetchState> FetchState for JoinState<C, A, Q, F> {
    fn init(world: &World) -> Self {
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
			archetype_id,
			component_id: component_info.id(),
			storage_type: component_info.storage_type(),
            fetch_state: Q::init(world),
			filter_state: F::init(world),
			mark: PhantomData,
        }
    }

	fn update_component_access(&self, access: &mut FilteredAccess<ComponentId>) {
		self.fetch_state.update_component_access(access);
		self.filter_state.update_component_access(access);
	}

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut Access<ArchetypeComponentId>) {
        self.fetch_state.update_archetype_component_access(archetype, access);
		self.filter_state.update_archetype_component_access(archetype, access);
    }

    fn matches_archetype(&self, archetype: &Archetype, world: &World) -> bool {
		let inner_archetype = &world.archetypes()[self.archetype_id];
		archetype.contains(self.component_id) && self.fetch_state.matches_archetype(inner_archetype, world) && self.filter_state.matches_archetype(inner_archetype, world)
	}
}