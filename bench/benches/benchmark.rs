use bukurs::db::BukuDb;
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_db_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_operations");

    group.bench_function("add_rec", |b| {
        b.iter_with_setup(
            || BukuDb::init_in_memory().unwrap(),
            |db| {
                // We can't easily reuse DB across iterations in criterion's iter_with_setup
                // without changing how we measure.
                // Instead, let's measure adding MANY records in one go, which will benefit from caching.
                for i in 0..100 {
                    db.add_rec(
                        &format!("https://example.com/{}", i),
                        &format!("Example Title {}", i),
                        ",tag1,tag2,",
                        "Description",
                        None,
                    )
                    .unwrap();
                }
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

    group.bench_function("add_rec_file_default", |b| {
        b.iter_with_setup(
            || {
                let tmp_dir = tempfile::tempdir().unwrap();
                let db_path = tmp_dir.path().join("test.db");
                (tmp_dir, BukuDb::init(&db_path).unwrap())
            },
            |(_tmp_dir, db)| {
                db.add_rec("https://example.com", "Title", ",tags,", "Desc", None)
                    .unwrap();
            },
        );
    });

    group.bench_function("add_rec_file_optimized", |b| {
        b.iter_with_setup(
            || {
                let tmp_dir = tempfile::tempdir().unwrap();
                let db_path = tmp_dir.path().join("test.db");
                let db = BukuDb::init(&db_path).unwrap();
                // Apply optimizations manually for benchmark
                db.execute("PRAGMA synchronous = NORMAL", []).unwrap();
                db.set_journal_mode("WAL").unwrap();
                (tmp_dir, db)
            },
            |(_tmp_dir, db)| {
                db.add_rec("https://example.com", "Title", ",tags,", "Desc", None)
                    .unwrap();
            },
        );
    });

    group.finish();
}

criterion_group!(benches, bench_db_ops);
criterion_main!(benches);
