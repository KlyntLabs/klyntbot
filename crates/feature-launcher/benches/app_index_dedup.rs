use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use feature_launcher::{
    new_attention_signals, new_running_signals, AppEntry, AppIndex, AttentionStat, RunningSignal,
    RunningSignals,
};
use smol_str::SmolStr;

const QUERIES: &[&str] = &["s", "saf", "safari", "vsc", "fin"];

fn synth_apps(n: usize) -> Vec<AppEntry> {
    (0..n)
        .map(|i| AppEntry {
            name: format!("App{i}"),
            path: PathBuf::from(format!("/Applications/App{i}.app")),
            bundle_id: Some(SmolStr::new(format!("com.example.App{i}"))),
            icon_path: None,
            icon_data_url: None,
        })
        .collect()
}

fn build_index_with_density(n: usize, density: f64) -> AppIndex {
    let apps = synth_apps(n);
    let running = new_running_signals();
    let attention = new_attention_signals();

    let pop_count = ((n as f64) * density) as usize;
    for app in apps.iter().take(pop_count) {
        let bid = app.bundle_id.clone().unwrap();
        running.insert(
            bid.clone(),
            RunningSignal {
                pid: 1,
                path: app.path.clone(),
            },
        );
        attention.insert(
            bid,
            AttentionStat {
                attention_secs: 1800,
                category: Some(SmolStr::new("misc")),
                last_used_at: jiff::Timestamp::now(),
            },
        );
    }

    let idx = AppIndex::new()
        .with_running_signals(running)
        .with_attention_signals(attention);
    idx.set_apps(apps);
    idx
}

fn search_bench(c: &mut Criterion) {
    let app_counts = [100usize, 500, 2_000];
    let signal_density = [0.0_f64, 0.25, 1.0];

    for &n in &app_counts {
        for &density in &signal_density {
            let idx = build_index_with_density(n, density);
            let mut group = c.benchmark_group(format!("app_index_search_n{n}_d{density:.2}"));
            group.throughput(Throughput::Elements(n as u64));

            for &q in QUERIES {
                group.bench_with_input(BenchmarkId::from_parameter(q), q, |b, q| {
                    b.iter(|| {
                        let r = idx.search(black_box(q), 20);
                        black_box(r.len());
                    });
                });
            }
            group.finish();
        }
    }
}

fn signals_refresh_bench(c: &mut Criterion) {
    use feature_launcher::apply_running_snapshot_for_bench as apply;
    use platform_macos::apps::RunningApp;

    let sizes = [10usize, 50, 200];
    let mut group = c.benchmark_group("running_signals_refresh");

    for &n in &sizes {
        let signals: RunningSignals = new_running_signals();
        for i in 0..n {
            signals.insert(
                SmolStr::new(format!("com.app.{i}")),
                RunningSignal {
                    pid: i as u32,
                    path: PathBuf::new(),
                },
            );
        }

        // Snapshot drops one app and adds one.
        let mut snapshot: Vec<RunningApp> = (0..n)
            .filter(|&i| i != 0)
            .map(|i| RunningApp {
                name: format!("App{i}"),
                bundle_id: Some(format!("com.app.{i}")),
                pid: i as i32,
                path: None,
            })
            .collect();
        snapshot.push(RunningApp {
            name: "NewApp".into(),
            bundle_id: Some(format!("com.app.{n}")),
            pid: n as i32,
            path: None,
        });

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                apply(&signals, &snapshot);
                black_box(signals.len());
            });
        });
    }
    group.finish();
}

criterion_group!(benches, search_bench, signals_refresh_bench);
criterion_main!(benches);
