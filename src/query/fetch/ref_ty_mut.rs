use std::{
	marker::PhantomData,
	sync::Arc,
};

use pi_share::cell::TrustCell;

use super::interface::{WorldQuery, FetchState, Fetch};

use crate::{
	archetype::{Archetype, ArchetypeId, ArchetypeComponentId},
	storage::LocalVersion,
	component::{ComponentId, Component, MultiCaseImpl},
	query::access::FilteredAccess,
	world::World,
	pointer::Mut,
};

impl<T: Component> WorldQuery for &mut T {
    type Fetch = MutFetch<T>;
    type State = MutState<T>;
}
pub struct MutFetch<T> {
	container: usize,
	mark: PhantomData<T>,
}

impl<'s, T: Component> Fetch<'s> for MutFetch<T> {
    type Item = Mut<'s, T>;
    type State = MutState<T>;

    unsafe fn init(
        _world: &World,
        _state: &Self::State
    ) -> Self {
        Self {
			container: 0,
			mark: PhantomData,
        }
    }

    #[inline]
    unsafe fn set_archetype(
        &mut self,
        state: &Self::State,
        archetype: &Archetype,
		_world: &World,
    ) {
		let c = archetype.get_component(state.component_id);
		match c.clone().downcast() {
			Ok(r) => {
				let r: Arc<TrustCell<MultiCaseImpl<T>>> = r;
				self.container = (*r).as_ptr() as usize;
			},
			Err(_) => panic!("downcast fail")
		}
    }

    #[inline]
    unsafe fn archetype_fetch(&mut self, local: LocalVersion) -> Option<Self::Item> {
        let value = std::mem::transmute((&mut *(self.container as *mut MultiCaseImpl<T>)).get_mut(local));
		match value {
			Some(r) => Some(Mut {
				value: r,
			}),
			None => None,
		}
    }

	#[inline]
    unsafe fn archetype_fetch_unchecked(&mut self, local: LocalVersion) -> Self::Item {
        let value = std::mem::transmute((&mut *(self.container as *mut MultiCaseImpl<T>)).get_unchecked_mut(local));
		Mut {
			value,
		}
    }
}

pub struct MutState<T> {
    component_id: ComponentId,
    marker: PhantomData<T>,
}

// SAFE: component access and archetype component access are properly updated to reflect that T IS
// read
unsafe impl<T: Component> FetchState for MutState<T> {
    fn init(world: &mut World, _query_id: usize, archetype_id: ArchetypeId) -> Self {
		let component_id = world.get_or_register_component::<T>(archetype_id);
        MutState {
            component_id,
            marker: PhantomData,
        }
    }

    fn update_archetype_component_access(&self, archetype: &Archetype, access: &mut FilteredAccess<ArchetypeComponentId>) {
		let archetype_component_id = unsafe { archetype.archetype_component_id(self.component_id)};
        if access.has_write(archetype_component_id) {
            panic!("&{} conflicts with a previous access in this query. Shared access cannot coincide with exclusive access.",
                std::any::type_name::<T>());
        }
		access.add_write(archetype_component_id)
    }


    fn matches_archetype(&self, archetype: &Archetype) -> bool {
        archetype.contains(self.component_id)
    }
}