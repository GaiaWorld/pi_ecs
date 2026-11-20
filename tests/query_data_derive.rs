#![cfg(feature = "rc")]

use pi_ecs::core::{bundle_input::EmptyQueue, entity_container::EntityQueue};
use pi_ecs::{core::query::QueryState, entity::AllocationDomain, prelude::*};

pub struct Position(u32);
impl Component for Position {
    type Access = PlainAccess;
}
struct Tag(u32);
impl Component for Tag {
    type Access = PlainAccess;
}

#[derive(QueryData)]
struct Motion {
    position: &'static mut Position,
    tag: Option<&'static Tag>,
}

#[derive(QueryData)]
pub struct PositionView<'q, Q = Entity, const N: usize = 1>
where
    Q: QueryData,
{
    pub position: &'q Position,
    pub extras: (Q, ()),
}

#[derive(QueryData)]
#[query_data(crate = pi_ecs::core)]
struct Wide {
    view: PositionView<'static>,
    a: (),
    b: (),
    c: (),
    d: (),
    e: (),
    f: (),
    g: (),
    last: Entity,
}

#[derive(QueryData)]
struct Empty {}

#[derive(QueryData)]
struct Aliased {
    shared: &'static Position,
    exclusive: &'static mut Position,
}

#[derive(QueryData)]
struct QueueWrite {
    queue: &'static mut EntityQueue<()>,
}

#[test]
fn named_field_preserves_the_component_policy_result() {
    let mut world = World::new().unwrap();
    world
        .register_component::<EntityQueue<()>>(StoragePolicy::SparseOnly)
        .unwrap();
    world.finish_registration().unwrap();
    let owner = world
        .spawn(AllocationDomain(0), EmptyQueue::<()>::new())
        .unwrap()
        .forget();
    let mut writer = world.write().unwrap();
    let mut result = writer.query_one_mut::<QueueWrite>(owner).unwrap();
    assert!(result.queue.pop_front().unwrap().is_none());
    drop(writer);
    // SAFETY: the test owns the initial strong transferred by forget.
    unsafe { world.despawn(owner).unwrap() };
    world.close().unwrap();
}

#[test]
fn named_items_work_for_table_and_sparse_reads_and_writes() {
    let mut world = World::new().unwrap();
    let position = world
        .register_component::<Position>(StoragePolicy::TableEligible)
        .unwrap();
    world
        .register_component::<Tag>(StoragePolicy::SparseOnly)
        .unwrap();
    world.register_table(&[position]).unwrap();
    world.finish_registration().unwrap();
    let plain = world
        .spawn(AllocationDomain(0), ComponentValue(Position(5)))
        .unwrap()
        .forget();
    let tagged = world
        .spawn(
            AllocationDomain(0),
            (ComponentValue(Position(8)), ComponentValue(Tag(3))),
        )
        .unwrap()
        .forget();

    let mut read = QueryState::<Wide>::new(&mut world).unwrap();
    {
        // SAFETY: sole host access; no structural mutation while the query is live.
        let query = unsafe { read.query(&mut world) }.unwrap();
        let item = query.get(tagged).unwrap();
        assert_eq!(
            (item.view.position.0, item.view.extras.0, item.last),
            (8, tagged, tagged)
        );
    }
    let mut empty = QueryState::<Empty>::new(&mut world).unwrap();
    // SAFETY: sole host access; the empty item escapes no storage borrow.
    let _: EmptyItem = unsafe { empty.query(&mut world) }
        .unwrap()
        .get(plain)
        .unwrap();
    assert!(QueryState::<Aliased>::new(&mut world).is_err());

    let mut write = world.write().unwrap();
    let MotionItem { mut position, tag } = write.query_one_mut::<Motion>(plain).unwrap();
    assert!(tag.is_none());
    position.0 += 1;
    let MotionItem { mut position, tag } = write.query_one_mut::<Motion>(tagged).unwrap();
    position.0 += tag.unwrap().0;
    drop(write);
    // SAFETY: the write token is gone and no structural access overlaps.
    let query = unsafe { read.query(&mut world) }.unwrap();
    assert_eq!(query.get(plain).unwrap().view.position.0, 6);
    assert_eq!(query.get(tagged).unwrap().view.position.0, 11);

    // SAFETY: both successful spawns transferred exactly one strong to this test.
    unsafe {
        world.despawn(plain).unwrap();
        world.despawn(tagged).unwrap();
    }
    world.close().unwrap();
}
