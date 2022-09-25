use {
    criterion::{black_box, criterion_group, criterion_main, Criterion},
    lazy_transform::Atomic,
    parking_lot::Mutex,
    std::{mem, sync::Arc},
};
// use lazy_transform::{
//     v000_st_naive, v005_st_is_some, v010_mt_naive, v015_mt_is_some, v020_mt_atomic_ptr,
//     v030_mt_atomic_ptr, v035_mt_atomic_ptr,
// };
//
// fn v000(c: &mut Criterion) {
//     c.bench_function("v000", |b| {
//         let mut lt = v000_st_naive::LazyTransform::<u32, _, _>::new(|v| v);
//         lt.set_source(Some(1));
//         b.iter(|| {
//             let res = black_box(&mut lt).get_value();
//             let _ = black_box(res);
//         })
//     });
// }
//
// fn v005(c: &mut Criterion) {
//     c.bench_function("v005", |b| {
//         let mut lt = v005_st_is_some::LazyTransform::<u32, _, _>::new(|v| v);
//         lt.set_source(Some(1));
//         b.iter(|| {
//             let res = black_box(&mut lt).get_value();
//             let _ = black_box(res);
//         })
//     });
// }
//
// fn v010(c: &mut Criterion) {
//     c.bench_function("v010", |b| {
//         let lt = v010_mt_naive::LazyTransform::<u32, _, _>::new(|v| v);
//         lt.set_source(Some(1));
//         b.iter(|| {
//             let res = black_box(&lt).get_value();
//             let _ = black_box(res);
//         })
//     });
// }
//
// fn v015(c: &mut Criterion) {
//     c.bench_function("v015", |b| {
//         let lt = v015_mt_is_some::LazyTransform::<u32, _, _>::new(|v| v);
//         lt.set_source(Some(1));
//         b.iter(|| {
//             let res = black_box(&lt).get_value();
//             let _ = black_box(res);
//         })
//     });
// }
//
// fn v020(c: &mut Criterion) {
//     c.bench_function("v020", |b| {
//         let mut lt = v020_mt_atomic_ptr::LazyTransform::<u32, _, _>::new(|v| v);
//         lt.set_source(Some(1));
//         b.iter(|| {
//             let res = black_box(&mut lt).get_value();
//             let _ = black_box(res);
//         })
//     });
// }
//
// fn v025(c: &mut Criterion) {
//     c.bench_function("v025", |b| {
//         let mut lt = v020_mt_atomic_ptr::LazyTransform::<u32, _, _>::new(|v| v);
//         lt.set_source(Some(1));
//         b.iter(|| {
//             let res = black_box(&mut lt).get_value();
//             let _ = black_box(res);
//         })
//     });
// }
//
// fn v030(c: &mut Criterion) {
//     c.bench_function("v030", |b| {
//         let mut lt = v030_mt_atomic_ptr::LazyTransform::<u32, _, _>::new(|v| v);
//         lt.set_source(Some(1));
//         b.iter(|| {
//             let res = black_box(&mut lt).get_value();
//             let _ = black_box(res);
//         })
//     });
// }
//
// fn v035(c: &mut Criterion) {
//     c.bench_function("v035", |b| {
//         let mut lt = v035_mt_atomic_ptr::LazyTransform::<u32, _, _>::new(|v| v);
//         lt.set_source(Some(1));
//         b.iter(|| {
//             let res = black_box(&mut lt).get_value();
//             let _ = black_box(res);
//         })
//     });
// }

fn atomic(c: &mut Criterion) {
    c.bench_function("gn", |b| {
        let a = Atomic::new(Arc::new(1));
        b.iter(|| {
            let res = a.get();
            mem::forget(black_box(res));
        });
    });
}

fn locking(c: &mut Criterion) {
    c.bench_function("gn", |b| {
        let a = Mutex::new(Arc::new(1));
        b.iter(|| {
            let res = a.lock().clone();
            mem::forget(black_box(res));
        });
    });
}

criterion_group!(
    benches, // v000,
    atomic, locking,
);
criterion_main!(benches);
