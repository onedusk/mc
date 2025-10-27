use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use mc::{
    prune_nested_items, CleanItem, Config, ItemType, PatternCategory, PatternMatch, PatternMatcher,
    PatternSource, Scanner,
};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

fn setup_fixture_tree() -> TempDir {
    let temp = TempDir::new().expect("create temp fixture");

    for project_idx in 0..6 {
        let project_root = temp.path().join(format!("project_{project_idx}"));
        fs::create_dir_all(project_root.join("node_modules/pkg_a/lib")).unwrap();
        fs::create_dir_all(project_root.join("node_modules/pkg_b/dist")).unwrap();
        fs::create_dir_all(project_root.join("target/debug/deps")).unwrap();
        fs::create_dir_all(project_root.join("logs")).unwrap();

        for file_idx in 0..25 {
            fs::write(
                project_root.join(format!("logs/app_{file_idx}.log")),
                b"benchmark log payload",
            )
            .unwrap();
            fs::write(
                project_root.join(format!("node_modules/pkg_a/lib/file_{file_idx}.js")),
                b"console.log('hello');",
            )
            .unwrap();
        }
    }

    temp
}

fn bench_scanner(c: &mut Criterion) {
    let fixture = setup_fixture_tree();
    let config = Config::default();
    let matcher = Arc::new(PatternMatcher::new(&config.patterns).expect("compile patterns"));
    let scanner = Scanner::new(fixture.path().to_path_buf(), Arc::clone(&matcher))
        .with_max_depth(config.safety.max_depth)
        .with_symlinks(!config.options.preserve_symlinks);

    c.bench_function("scanner_scan_synthetic_tree", |b| {
        b.iter(|| {
            let (items, errors) = scanner.scan().expect("scan succeeds");
            black_box((items.len(), errors.len()));
        });
    });
}

fn generate_items(sample: usize) -> Vec<CleanItem> {
    let mut items = Vec::with_capacity(sample * 4);

    for idx in 0..sample {
        let root = PathBuf::from(format!("/repo/project_{idx}"));

        items.push(make_item(
            root.join("node_modules"),
            PatternCategory::Dependencies,
            ItemType::Directory,
        ));
        items.push(make_item(
            root.join("node_modules/pkg_a/dist"),
            PatternCategory::BuildOutputs,
            ItemType::Directory,
        ));
        items.push(make_item(
            root.join("target"),
            PatternCategory::BuildOutputs,
            ItemType::Directory,
        ));
        items.push(make_item(
            root.join(format!("logs/build_{idx}.log")),
            PatternCategory::Logs,
            ItemType::File,
        ));
    }

    items
}

fn make_item(path: PathBuf, category: PatternCategory, item_type: ItemType) -> CleanItem {
    CleanItem {
        path,
        size: 1024,
        item_type,
        pattern: PatternMatch {
            pattern: "bench".to_string(),
            priority: 0,
            source: PatternSource::Config,
            category,
        },
    }
}

fn bench_prune_nested_items(c: &mut Criterion) {
    let base_items = generate_items(2_000);

    c.bench_function("prune_nested_items_large_vector", |b| {
        b.iter_batched(
            || base_items.clone(),
            |items| {
                let pruned = prune_nested_items(items);
                black_box(pruned);
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(performance, bench_scanner, bench_prune_nested_items);
criterion_main!(performance);
