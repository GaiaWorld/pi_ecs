/// 测试系统参数: LayerDirty
/// 该参数监听组件的变化，根据其实体在Tree上的层次，来记录层次脏
/// 该参数提供iter方法，用于迭代已经脏的实体
/// LayerDirty<A, F, N=()>, A为原型类型、F为过滤器、N默认为(),其是Tree<K, N>上的一个泛型

use std::sync::Arc;

use pi_ecs::{prelude::{Query, IntoSystem, StageBuilder, SingleDispatcher, Dispatcher, LayerDirty}, entity::Entity, world::World, storage::{Offset, LocalVersion, Local}, monitor::Notify};
use pi_ecs::query::filter_change::Changed;
use pi_async::rt::{AsyncRuntime, multi_thread::{MultiTaskRuntimeBuilder, StealableTaskPool}};
use pi_tree::Tree;


pub struct Node;

#[derive(Debug)]
/// 定义一个组件类型
pub struct Name(pub String);


/// 测试组件脏
///迭代出脏的Position和对应的entity
pub fn iter_dirty(
	q: Query<Node, (Entity, &Name)>,
	dirtys: LayerDirty<Node, Changed<Name>>, // 该声明隐意：world上必须存在Tree<LocalVersion, ()>资源
) {
	for k in dirtys.iter() {
		let position = q.get(k.0.clone());
		println!("modify entity_index: {:?}, layer: {:?}, position:{:?}", k.0.local().offset(), k.1, position);
	}
}

#[test]
fn test() {
	
	// 创建world
	let mut world = World::new();

	// 创建一个名为Node的原型，为该原型注册组件类型（一旦注册，不可修改）
	world.new_archetype::<Node>()
		.register::<Name>()
		.create();
	world.insert_resource(Tree::<LocalVersion, ()>::default());
	let tree_id = world.get_resource_id::<Tree<LocalVersion, ()>>().unwrap().clone();

	let dispatcher = create_dispatcher(&mut world);

	let mut entitys = Vec::new();
	// 创建原型为Node的实体，并为该实体添加组件（必须是在Node中注册过的组件， 否则无法插入）
	let name = format!("{}", 0);
	let root = world.spawn::<Node>()
		.insert(Name(name.clone()))
		.id();
	unsafe{world.archetypes().get_resource_mut::<Tree<LocalVersion, ()>>(tree_id)}.unwrap().create(root.local());
	unsafe{world.archetypes().get_resource_mut::<Tree<LocalVersion, ()>>(tree_id)}.unwrap().insert_child(root.local(), None, 0);
	entitys.push(root.clone());

	unsafe{world.archetypes().get_resource_notify::<Tree<LocalVersion, ()>>(tree_id)}.unwrap().create_event(root.clone());

	create_tree(
		name,
		root.local(),
		1,
		4,
		3,
		&mut world,
		&mut entitys,
		tree_id);

	println!("change all(0 and children): ");
	dispatcher.run();
	std::thread::sleep(std::time::Duration::new(1, 0));

	world.insert_component(entitys[1].clone(), Name("00".to_string()));
	println!("change 00 and children: ");
	dispatcher.run();

	std::thread::sleep(std::time::Duration::new(2, 0));
}

fn create_tree (
	parent_name: String, 
	parent: LocalVersion,
	cur_layer_count: usize, 
	layer_count: usize, 
	each_layer_count: usize,
	world: &mut World,
	entitys: &mut Vec<Entity>,
	tree_id: Local,
) {
	if cur_layer_count < layer_count {
		for i in 0..each_layer_count {
			let name = parent_name.clone() + format!("{}", i).as_str();
			let id = world.spawn::<Node>()
			.insert(Name(name.clone()))
			.id();

			unsafe{world.archetypes().get_resource_mut::<Tree<LocalVersion, ()>>(tree_id)}.unwrap().create(id.local());
			entitys.push(id.clone());
			create_tree(name, id.local(), cur_layer_count + 1, layer_count, each_layer_count, world, entitys, tree_id);

			unsafe{world.archetypes().get_resource_mut::<Tree<LocalVersion, ()>>(tree_id)}.unwrap().insert_child(id.local(), Some(parent), std::usize::MAX);
			unsafe{world.archetypes().get_resource_notify::<Tree<LocalVersion, ()>>(tree_id)}.unwrap().create_event(id.clone());
		}
	}
}


fn create_dispatcher(world: &mut World) -> SingleDispatcher<StealableTaskPool<()>> {
	let rt = AsyncRuntime::Multi(MultiTaskRuntimeBuilder::default().build());
	let iter_dirty_system = iter_dirty.system(world);

	let mut stage = StageBuilder::new();
	stage.add_node(iter_dirty_system);
	
	let mut stages = Vec::new();
	stages.push(Arc::new(stage.build()));
	let dispatcher = SingleDispatcher::new(stages, world, rt);

	return dispatcher;
}