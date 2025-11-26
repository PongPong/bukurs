use bukurs::db::BukuDb;
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_db_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_operations");

    group.bench_function("add_rec", |b| {
        b.iter_with_setup(
            || {
                // Setup: Create a fresh in-memory DB for each iteration to avoid unique constraint violations
                // or growing DB size affecting performance.
                BukuDb::init_in_memory().unwrap()
            },
            |db| {
                // Benchmark: Add a single record
                db.add_rec(
                    "https://example.com",
                    "Example Title",
                    ",tag1,tag2,",
                    "Description",
                    None,
                )
                .unwrap();
            },
        );
    });

    group.bench_function("undo_last (add)", |b| {
        b.iter_with_setup(
            || {
                // Setup: Create DB and add a record to undo
                let db = BukuDb::init_in_memory().unwrap();
                db.add_rec(
                    "https://example.com",
                    "Example Title",
                    ",tag1,tag2,",
                    "Description",
                    None,
                )
                .unwrap();
                db
            },
            |db| {
                // Benchmark: Undo the add operation
                db.undo_last().unwrap();
            },
        );
    });

    group.bench_function("search (single keyword)", |b| {
        b.iter_with_setup(
            || {
                let db = BukuDb::init_in_memory().unwrap();
                // Add some data to search
                for i in 0..100 {
                    db.add_rec(
                        &format!("https://example.com/{}", i),
                        &format!("Title {}", i),
                        ",tag1,tag2,",
                        "Description",
                        None,
                    )
                    .unwrap();
                }
                db
            },
            |db| {
                // Search with a keyword that triggers the clone path (contains OR)
                db.search(&["Title OR Description".to_string()], true, false, false)
                    .unwrap();
            },
        );
    });

    group.finish();
}

criterion_group!(benches, bench_db_ops);
criterion_main!(benches);
