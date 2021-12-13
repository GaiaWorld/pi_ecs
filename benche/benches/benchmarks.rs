use criterion::*;
use benches::*;

fn bench_simple_insert(c: &mut Criterion) {
	// 批量插入
	let mut group = c.benchmark_group("sample_insert");
    // group.bench_function("legion/batch", |b| {
    //     let mut bench = legion::batch_insert::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	// group.bench_function("legion4/batch", |b| {
    //     let mut bench = legion4::batch_insert::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	// group.bench_function("bevy/batch", |b| {
    //     let mut bench = bevy::batch_insert::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	// group.bench_function("bevy5/batch", |b| {
    //     let mut bench = bevy5::batch_insert::Benchmark::new();
    //     b.iter(move || bench.run());
    // });

	// 一个一个插入
	// group.bench_function("legion/each", |b| {
    //     let mut bench = legion::simple_insert::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	// group.bench_function("legion4/each", |b| {
    //     let mut bench = legion4::simple_insert::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	// group.bench_function("bevy/each", |b| {
    //     let mut bench = bevy::simple_insert::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	group.bench_function("bevy5/each", |b| {
        let mut bench = bevy5::simple_insert::Benchmark::new();
        b.iter(move || bench.run());
    });
	group.bench_function("bevy5/each_modify_2_archetype", |b| {
        let mut bench = bevy5::simple_insert::Benchmark::new();
        b.iter(move || bench.run());
    });
	group.bench_function("piecs/each", |b| {
        let mut bench = piecs::simple_insert::Benchmark::new();
        b.iter(move || bench.run());
    });
    // group.bench_function("hecs", |b| {
    //     let mut bench = hecs::simple_insert::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("planck_ecs", |b| {
    //     let mut bench = planck_ecs::simple_insert::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("shipyard", |b| {
    //     let mut bench = shipyard::simple_insert::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("specs/each", |b| {
    //     let mut bench = specs::simple_insert::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	// group.bench_function("pi_ecs/each", |b| {
    //     let mut bench = pi_ecs::simple_insert::SampleBenchmark::new();
    //     b.iter(move || bench.run());
    // });
	// group.bench_function("pi_ecs/each/quick", |b| {
    //     let mut bench = pi_ecs::simple_insert::QuickBenchmark::new();
    //     b.iter(move || bench.run());
    // });
}

fn bench_simple_iter(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_iter");
    // group.bench_function("legion", |b| {
    //     let mut bench = legion::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	// group.bench_function("legion4", |b| {
    //     let mut bench = legion4::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("legion (packed)", |b| {
    //     let mut bench = legion_packed::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("bevy", |b| {
    //     let mut bench = bevy::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	group.bench_function("bevy5", |b| {
        let mut bench = bevy5::simple_iter::Benchmark::new();
        b.iter(move || bench.run());
    });
	group.bench_function("piecs", |b| {
        let mut bench = piecs::simple_iter::Benchmark::new();
        b.iter(move || bench.run());
    });
    // group.bench_function("hecs", |b| {
    //     let mut bench = hecs::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("planck_ecs", |b| {
    //     let mut bench = planck_ecs::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("shipyard", |b| {
    //     let mut bench = shipyard::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("shipyard (packed)", |b| {
    //     let mut bench = shipyard_packed::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("pi_ecs", |b| {
    //     let mut bench = pi_ecs::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	// group.bench_function("specs", |b| {
    //     let mut bench = specs::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
}

fn bench_query_iter(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_iter");
    // group.bench_function("legion", |b| {
    //     let mut bench = legion::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	// group.bench_function("legion4", |b| {
    //     let mut bench = legion4::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("legion (packed)", |b| {
    //     let mut bench = legion_packed::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("bevy", |b| {
    //     let mut bench = bevy::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	group.bench_function("bevy5", |b| {
        let mut bench = bevy5::simple_iter::Benchmark::new();
        b.iter(move || bench.run_query());
    });
	group.bench_function("piecs", |b| {
        let mut bench = piecs::simple_iter::Benchmark::new();
        b.iter(move || bench.run_query());
    });
    // group.bench_function("hecs", |b| {
    //     let mut bench = hecs::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("planck_ecs", |b| {
    //     let mut bench = planck_ecs::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("shipyard", |b| {
    //     let mut bench = shipyard::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("shipyard (packed)", |b| {
    //     let mut bench = shipyard_packed::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
    // group.bench_function("pi_ecs", |b| {
    //     let mut bench = pi_ecs::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
	// group.bench_function("specs", |b| {
    //     let mut bench = specs::simple_iter::Benchmark::new();
    //     b.iter(move || bench.run());
    // });
}

// fn bench_frag_iter_bc(c: &mut Criterion) {
//     let mut group = c.benchmark_group("fragmented_iter");
//     group.bench_function("legion", |b| {
//         let mut bench = legion::frag_iter::Benchmark::new();
//         b.iter(move || bench.run());
//     });
// 	group.bench_function("legion4", |b| {
//         let mut bench = legion4::frag_iter::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("bevy", |b| {
//         let mut bench = bevy::frag_iter::Benchmark::new();
//         b.iter(move || bench.run());
//     });
// 	group.bench_function("bevy5", |b| {
//         let mut bench = bevy5::frag_iter::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     // group.bench_function("hecs", |b| {
//     //     let mut bench = hecs::frag_iter::Benchmark::new();
//     //     b.iter(move || bench.run());
//     // });
//     // group.bench_function("planck_ecs", |b| {
//     //     let mut bench = planck_ecs::frag_iter::Benchmark::new();
//     //     b.iter(move || bench.run());
//     // });
//     // group.bench_function("shipyard", |b| {
//     //     let mut bench = shipyard::frag_iter::Benchmark::new();
//     //     b.iter(move || bench.run());
//     // });
//     group.bench_function("specs", |b| {
//         let mut bench = specs::frag_iter::Benchmark::new();
//         b.iter(move || bench.run());
//     });
// }

// fn bench_schedule(c: &mut Criterion) {
//     let mut group = c.benchmark_group("schedule");
//     group.bench_function("legion", |b| {
//         let mut bench = legion::schedule::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("legion (packed)", |b| {
//         let mut bench = legion_packed::schedule::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("bevy", |b| {
//         let mut bench = bevy::schedule::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("planck_ecs", |b| {
//         let mut bench = planck_ecs::schedule::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("shipyard", |b| {
//         let mut bench = shipyard::schedule::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("shipyard (packed)", |b| {
//         let mut bench = shipyard_packed::schedule::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("specs", |b| {
//         let mut bench = specs::schedule::Benchmark::new();
//         b.iter(move || bench.run());
//     });
// }

// fn bench_heavy_compute(c: &mut Criterion) {
//     let mut group = c.benchmark_group("heavy_compute");
//     group.bench_function("legion", |b| {
//         let mut bench = legion::heavy_compute::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("legion (packed)", |b| {
//         let mut bench = legion_packed::heavy_compute::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("bevy", |b| {
//         let mut bench = bevy::heavy_compute::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("hecs", |b| {
//         let mut bench = hecs::heavy_compute::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("shipyard", |b| {
//         let mut bench = shipyard::heavy_compute::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("shipyard (packed)", |b| {
//         let mut bench = shipyard_packed::heavy_compute::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("specs", |b| {
//         let mut bench = specs::heavy_compute::Benchmark::new();
//         b.iter(move || bench.run());
//     });
// }

// fn bench_add_remove(c: &mut Criterion) {
//     let mut group = c.benchmark_group("add_remove_component");
//     group.bench_function("legion", |b| {
//         let mut bench = legion::add_remove::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("hecs", |b| {
//         let mut bench = hecs::add_remove::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("planck_ecs", |b| {
//         let mut bench = planck_ecs::add_remove::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("shipyard", |b| {
//         let mut bench = shipyard::add_remove::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("specs", |b| {
//         let mut bench = specs::add_remove::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("bevy", |b| {
//         let mut bench = bevy::add_remove::Benchmark::new();
//         b.iter(move || bench.run());
//     });
// }

// fn bench_serialize_text(c: &mut Criterion) {
//     let mut group = c.benchmark_group("serialize_text");
//     group.bench_function("legion", |b| {
//         let mut bench = legion::serialize_text::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("hecs", |b| {
//         let mut bench = hecs::serialize_text::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     // group.bench_function("bevy", |b| {
//     //     let mut bench = bevy::serialize_text::Benchmark::new();
//     //     b.iter(move || bench.run());
//     // });
// }

// fn bench_serialize_binary(c: &mut Criterion) {
//     let mut group = c.benchmark_group("serialize_binary");
//     group.bench_function("legion", |b| {
//         let mut bench = legion::serialize_binary::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     group.bench_function("hecs", |b| {
//         let mut bench = hecs::serialize_binary::Benchmark::new();
//         b.iter(move || bench.run());
//     });
//     // group.bench_function("bevy", |b| {
//     //     let mut bench = bevy::serialize_text::Benchmark::new();
//     //     b.iter(move || bench.run());
//     // });
// }

criterion_group!(
    benchmarks,
	// iter
    bench_simple_insert,
    bench_simple_iter,
	bench_query_iter
    // bench_frag_iter_bc,
    // bench_schedule,
    // bench_heavy_compute,
    // bench_add_remove,
    // bench_serialize_text,
    // bench_serialize_binary,
);
criterion_main!(benchmarks);


fn iter(c: &mut Criterion) {
    let mut group = c.benchmark_group("simple_iter");
    group.bench_function("顺序迭代10000次", |b| {
        let mut bench = Benchmark1::new();
        b.iter(move || bench.run());
    });
	group.bench_function("随机迭代5000次", |b| {
        let mut bench = Benchmark4::new(5000);
        b.iter(move || bench.run());
    });

	group.bench_function("随机迭代1000次", |b| {
        let mut bench = Benchmark4::new(1000);
        b.iter(move || bench.run());
    });
	group.bench_function("随机迭代100次", |b| {
        let mut bench = Benchmark4::new(100);
        b.iter(move || bench.run());
    });
	group.bench_function("随机迭代1次", |b| {
        let mut bench = Benchmark4::new(1);
        b.iter(move || bench.run());
    });
}
pub static mut ggg1:usize = 0;
pub static mut ggg4:usize = 0;
use rand::{Rng};
pub struct Benchmark1{
	r: usize,
	arr: Vec<usize>,
	tick: Vec<usize>,
	dirtys: Vec<usize>
}

impl Benchmark1 {
    pub fn new() -> Self {
		let mut arr = Vec::new();

		let len = 10000;
		let mut rng = rand::thread_rng();
		for i in 0..len {
			arr.push(i);
		}

		let mut dirtys = Vec::new();
		let mut rng = rand::thread_rng();
		for _i in 0..1000 {
			dirtys.push(rng.gen_range(0..len as u32) as usize);
		}

		
		Self {
			r: 0,
			arr,
			tick: Vec::with_capacity(len),
			dirtys: dirtys
		}
    }

    pub fn run(&mut self) {
		for i in 0..self.dirtys.len() {
			let r = &self.arr[i];

			// for _i in 0..1000 {
			// 	let rr = rng.gen_range(0..len as u32) as usize;
			// 	arr[rr].0 = 1;
			// }
			// self.r = if r.0 <1000 {
			// 	r.1
			// }else{
			// 	continue
			// };
			unsafe{ggg1 += self.r};
		}
	}
}




pub struct Benchmark4{

	arr: Vec<usize>,
	dirtys: Vec<usize>,

	r: usize,
	l: usize,
}

impl Benchmark4 {
    pub fn new(l: usize) -> Self {
		let mut arr = Vec::new();

		let len = 10000;

		for i in 0..len {
			arr.push(i);
		}

		let mut dirtys = Vec::new();
		let mut rng = rand::thread_rng();
		for _i in 0..l {
			dirtys.push(rng.gen_range(0..len as u32) as usize);
		}
		

		Self {
			dirtys,
			r: 0,
			arr,
			l: l,
		}
    }

    pub fn run(&mut self) {
		for i in 0..self.l {
			self.r = self.arr[self.dirtys[i]];
			unsafe{ggg4 += self.r}; 
		}
	}
}

#[test]
fn aa() {
	

	let mut bench = Benchmark2::new();
    bench.run();
}
// #[test]
// fn aa() {
// 	let mut arr = Vec::new();

// 	let len = 10000;

// 	for i in 0..len {
// 		arr.push(i);
// 	}
// 	let mut r = 0;
// 	let time = std::time::Instant::now();
// 	for i in 0..arr.len() {
// 		r = arr[i];
		
// 	}
// 	println!("t1:{:?}, {:?}", r, std::time::Instant::now() - time);

// 	let mut k = 0;
// 	let mut len1 = len-2;
// 	let mut len2 = len-2;
// 	let mut r = 0;
// 	let time = std::time::Instant::now();
// 	for _i in 0..1000 {
// 		k = len1 - k;
// 		len2 -= 1;
// 		len1 = ((len2 as f32/2.0).ceil() * 2.0) as usize;
// 		// r = arr[k];
		
// 	}
// 	println!("t3: {:?}, {:?}, {:?}",k, r, std::time::Instant::now() - time);

// 	let mut k = 0;
// 	let mut len1 = len-2;
// 	let mut len2 = len-2;
// 	let mut r = 0;
// 	let time = std::time::Instant::now();
// 	for _i in 0..1000 {
// 		k = len1 - k;
// 		len2 -= 1;
// 		len1 = ((len2 as f32/2.0).ceil() * 2.0) as usize;
// 		r = arr[k];
		
// 	}
// 	println!("t2: {:?}, {:?}, {:?}",k, r, std::time::Instant::now() - time);

	
// }
