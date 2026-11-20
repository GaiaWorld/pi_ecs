//! 原型只读声明访问的配对芯。
#[cfg(all(test, feature = "rc"))]
mod tests {
    use crate::{
        component::{Component, PlainAccess, StoragePolicy},
        world::World,
    };
    struct Value;
    impl Component for Value {
        type Access = PlainAccess;
    }
    // Seam: World 真实登记后公开原型元数据读取，不构造伪原型。
    #[test]
    fn rc_archetype_registered_metadata_identity() {
        let mut world = World::new().unwrap();
        let value = world
            .register_component::<Value>(StoragePolicy::TableEligible)
            .unwrap();
        let id = world.register_table(&[value]).unwrap();
        world.finish_registration().unwrap();
        let archetype = world.registry.archetype(id).unwrap();
        assert_eq!(archetype.id(), id);
        assert_eq!(archetype.table(), id);
        assert_eq!(archetype.components().len(), 1);
        assert_eq!(archetype.components()[0].component(), value);
        assert_eq!(
            archetype.components()[0].source(),
            super::ComponentSource::Table(crate::table::ColumnId(0))
        );
        world.close().unwrap();
    }
}
use super::*;
impl Archetype {
    pub(super) fn id_inner(&self) -> ArchetypeId {
        self.id
    }
    pub(super) fn table_inner(&self) -> ArchetypeId {
        self.table
    }
    pub(super) fn components_inner(&self) -> &[ArchetypeComponent] {
        &self.components
    }
}
impl ArchetypeComponent {
    pub(super) fn component_inner(&self) -> ComponentId {
        self.component
    }
    pub(super) fn source_inner(&self) -> ComponentSource {
        self.source
    }
}
