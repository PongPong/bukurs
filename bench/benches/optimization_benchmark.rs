use bukurs::db::BukuDb;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn bench_statement_caching(c: &mut Criterion) {
    let mut group = c.benchmark_group("statement_caching");

    // Benchmark: get_rec_by_id with cached statements (current implementation)
    group.bench_function("get_rec_by_id_cached", |b| {
        b.iter_with_setup(
            || {
                let db = BukuDb::init_in_memory().unwrap();
                for i in 1..=100 {
                    db.add_rec(
                        &format!("https://example.com/{}", i),
                        &format!("Title {}", i),
                        ",rust,programming,",
                        "Description",
                        None,
                    )
                    .unwrap();
                }
                db
            },
            |db| {
                // Call get_rec_by_id multiple times - benefits from caching
                for i in 1..=100 {
                    black_box(db.get_rec_by_id(i).unwrap());
                }
            },
        );
    });

    group.finish();
}

fn bench_search_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_operations");

    // Benchmark: search with cached FTS5 statement
    group.bench_function("search_cached", |b| {
        b.iter_with_setup(
            || {
                let db = BukuDb::init_in_memory().unwrap();
                for i in 1..=1000 {
                    db.add_rec(
                        &format!("https://example.com/{}", i),
                        &format!("Title {}", i),
                        ",rust,programming,systems,",
                        &format!("Description for item {}", i),
                        None,
                    )
                    .unwrap();
                }
                db
            },
            |db| {
                // Multiple searches benefit from statement caching
                for keyword in &["rust", "programming", "systems", "Title", "Description"] {
                    black_box(
                        db.search(&[keyword.to_string()], true, false, false)
                            .unwrap(),
                    );
                }
            },
        );
    });

    // Benchmark: search_tags with cached statements
    group.bench_function("search_tags_cached", |b| {
        b.iter_with_setup(
            || {
                let db = BukuDb::init_in_memory().unwrap();
                for i in 1..=1000 {
                    db.add_rec(
                        &format!("https://example.com/{}", i),
                        &format!("Title {}", i),
                        ",rust,programming,systems,web,",
                        &format!("Description {}", i),
                        None,
                    )
                    .unwrap();
                }
                db
            },
            |db| {
                // Multiple tag searches benefit from statement caching
                for tag in &["rust", "programming", "systems", "web"] {
                    black_box(db.search_tags(&[tag.to_string()]).unwrap());
                }
            },
        );
    });

    // Benchmark: get_all_tags with cached statement
    group.bench_function("get_all_tags_cached", |b| {
        b.iter_with_setup(
            || {
                let db = BukuDb::init_in_memory().unwrap();
                for i in 1..=500 {
                    db.add_rec(
                        &format!("https://example.com/{}", i),
                        &format!("Title {}", i),
                        &format!(",tag{},tag{},tag{},", i % 10, i % 20, i % 30),
                        &format!("Description {}", i),
                        None,
                    )
                    .unwrap();
                }
                db
            },
            |db| {
                // Call get_all_tags multiple times
                for _ in 0..10 {
                    black_box(db.get_all_tags().unwrap());
                }
            },
        );
    });

    group.finish();
}

fn bench_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_operations");

    // Benchmark: Batch delete (benefits from cached statements in loop)
    for batch_size in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("delete_batch", batch_size),
            batch_size,
            |b, &size| {
                b.iter_with_setup(
                    || {
                        let db = BukuDb::init_in_memory().unwrap();
                        let mut ids = Vec::new();
                        for i in 1..=size {
                            let id = db
                                .add_rec(
                                    &format!("https://example.com/{}", i),
                                    &format!("Title {}", i),
                                    ",tag,",
                                    "Description",
                                    None,
                                )
                                .unwrap();
                            ids.push(id);
                        }
                        (db, ids)
                    },
                    |(db, ids)| {
                        // Batch delete benefits from statement caching
                        black_box(db.delete_rec_batch(&ids).unwrap());
                    },
                );
            },
        );
    }

    group.finish();
}

fn bench_index_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_performance");

    // Benchmark: get_all_tags with index
    group.bench_function("get_all_tags_with_index", |b| {
        b.iter_with_setup(
            || {
                let db = BukuDb::init_in_memory().unwrap();
                // Create bookmarks with diverse tags
                for i in 1..=1000 {
                    db.add_rec(
                        &format!("https://example.com/{}", i),
                        &format!("Title {}", i),
                        &format!(",tag{},category{},type{},", i % 50, i % 30, i % 20),
                        &format!("Description {}", i),
                        None,
                    )
                    .unwrap();
                }
                db
            },
            |db| {
                black_box(db.get_all_tags().unwrap());
            },
        );
    });

    group.finish();
}

fn bench_no_clone_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("no_clone_optimization");

    // Simulate the tag search scenario
    group.bench_function("search_tags_slice_from_ref", |b| {
        b.iter_with_setup(
            || {
                let db = BukuDb::init_in_memory().unwrap();
                for i in 1..=100 {
                    db.add_rec(
                        &format!("https://example.com/{}", i),
                        &format!("Title {}", i),
                        ",rust,programming,",
                        "Description",
                        None,
                    )
                    .unwrap();
                }
                (db, "rust".to_string())
            },
            |(db, selected_tag)| {
                // Using slice::from_ref (no clone)
                black_box(db.search_tags(std::slice::from_ref(&selected_tag)).unwrap());
            },
        );
    });

    // Compare with cloning approach (for reference)
    group.bench_function("search_tags_with_clone", |b| {
        b.iter_with_setup(
            || {
                let db = BukuDb::init_in_memory().unwrap();
                for i in 1..=100 {
                    db.add_rec(
                        &format!("https://example.com/{}", i),
                        &format!("Title {}", i),
                        ",rust,programming,",
                        "Description",
                        None,
                    )
                    .unwrap();
                }
                (db, "rust".to_string())
            },
            |(db, selected_tag)| {
                // Cloning approach
                black_box(db.search_tags(&[selected_tag.clone()]).unwrap());
            },
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_statement_caching,
    bench_search_operations,
    bench_batch_operations,
    bench_index_performance,
    bench_no_clone_optimization
);
criterion_main!(benches);
