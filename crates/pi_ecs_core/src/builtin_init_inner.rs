//! 内置数组特殊释放配对芯。
use crate::{
    destruction::{ReleaseContext, ReleaseMode},
    entity::Entity,
    ptr::OwningPtr,
};

pub(super) unsafe fn release_strong_inner<const N: usize, T>(
    context: &ReleaseContext<'_>,
    value: OwningPtr<'_>,
) {
    // SAFETY: the release entry owns a detached value of this exact type.
    let value = unsafe {
        value
            .into_raw()
            .cast::<crate::owned_entity::StrongEntities<N, T>>()
            .read()
    };
    release_members(context, value.entities, false);
}

pub(super) unsafe fn release_weak_inner<const N: usize, T>(
    context: &ReleaseContext<'_>,
    value: OwningPtr<'_>,
) {
    // SAFETY: the release entry owns a detached value of this exact type.
    let value = unsafe {
        value
            .into_raw()
            .cast::<crate::owned_entity::WeakEntities<N, T>>()
            .read()
    };
    release_members(context, value.entities, true);
}

fn release_members(
    context: &ReleaseContext<'_>,
    members: impl IntoIterator<Item = Entity>,
    weak: bool,
) {
    // SAFETY: ReleaseContext holds the original World alive and permits reading these fields.
    let (entities, pending, access) = unsafe {
        let root = context.world.pointer.as_ptr();
        (
            &*std::ptr::addr_of!((*root).entities),
            &*std::ptr::addr_of!((*root).pending_destruction),
            &*std::ptr::addr_of!((*root).access),
        )
    };
    let failure = || -> ! {
        #[cfg(feature = "rc")]
        access.mode.set(crate::world::AccessMode::Stopped);
        #[cfg(not(feature = "rc"))]
        access.0.store(4, std::sync::atomic::Ordering::Release);
        std::process::abort()
    };
    for entity in members {
        if weak && entity == Entity::default() {
            continue;
        }
        let released = {
            #[cfg(feature = "rc")]
            let mut store = entities.try_borrow_mut().unwrap_or_else(|_| failure());
            #[cfg(not(feature = "rc"))]
            let store = entities;
            if weak {
                store.release_weak(entity)
            } else {
                store.release_strong(entity)
            }
        };
        match released {
            Ok(true) if context.mode == ReleaseMode::Normal => {
                if weak {
                    pending.reclamation.push(entity);
                } else {
                    pending.enqueue(entity);
                }
            }
            Ok(_) => {}
            Err(_) => failure(),
        }
    }
}
