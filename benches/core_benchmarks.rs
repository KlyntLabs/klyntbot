//! Performance benchmarks for core klyntbot operations.
//!
//! Run with: `cargo bench`
//! Results are stored in `target/criterion/` for comparison across runs.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};

use klyntbot::bus::{InboundMessage, MessageBus, OutboundMessage};
use klyntbot::config::Config;
use klyntbot::session::{Session, SessionManager};

// ─────────────────────────────────────────────────────────────
// Session benchmarks
// ─────────────────────────────────────────────────────────────

fn bench_session_add_message(c: &mut Criterion) {
    let mut group = c.benchmark_group("session");

    group.bench_function("add_message", |b| {
        b.iter_batched(
            || Session::new("bench:chat"),
            |mut session| {
                session.add_message("user", black_box("Hello, how are you?"));
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("add_100_messages", |b| {
        b.iter_batched(
            || Session::new("bench:chat"),
            |mut session| {
                for i in 0..100 {
                    session.add_message("user", format!("Message {}", i));
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("get_history_from_1000", |b| {
        let mut session = Session::new("bench:chat");
        for i in 0..1000 {
            session.add_message("user", format!("Message {}", i));
        }
        b.iter(|| {
            black_box(session.get_history(20));
        });
    });

    group.finish();
}

fn bench_session_save_load(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("session_io");

    group.bench_function("save_50_messages", |b| {
        b.iter_batched(
            || {
                let temp_dir = tempfile::TempDir::new().unwrap();
                let manager = rt.block_on(SessionManager::new(temp_dir.path()));
                let mut session = Session::new("bench:chat");
                for i in 0..50 {
                    session.add_message(
                        if i % 2 == 0 { "user" } else { "assistant" },
                        format!("Message number {} with some realistic content length", i),
                    );
                }
                (manager, session, temp_dir)
            },
            |(manager, session, _temp_dir)| {
                rt.block_on(async {
                    manager.save(&session).await.unwrap();
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("load_50_messages", |b| {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let mut manager = rt.block_on(SessionManager::new(temp_dir.path()));
        rt.block_on(async {
            let session = manager.get_or_create("bench:chat").await.unwrap();
            for i in 0..50 {
                session.add_message(
                    if i % 2 == 0 { "user" } else { "assistant" },
                    format!("Message number {} with some realistic content length", i),
                );
            }
            let session_clone = session.clone();
            manager.save(&session_clone).await.unwrap();
        });

        b.iter(|| {
            let mut fresh_manager = rt.block_on(SessionManager::new(temp_dir.path()));
            rt.block_on(async {
                black_box(fresh_manager.get_or_create("bench:chat").await.unwrap());
            });
        });
    });

    group.finish();
}

// ─────────────────────────────────────────────────────────────
// MessageBus benchmarks
// ─────────────────────────────────────────────────────────────

fn bench_message_bus(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("message_bus");

    group.bench_function("publish_inbound", |b| {
        let bus = MessageBus::new(10_000);
        let _rx = bus.take_inbound_rx();

        b.iter(|| {
            rt.block_on(async {
                let msg = InboundMessage::new("telegram", "user", "chat", "Hello");
                bus.publish_inbound(black_box(msg)).await.unwrap();
            });
        });
    });

    group.bench_function("publish_outbound", |b| {
        let bus = MessageBus::new(10_000);
        let _rx = bus.take_outbound_rx();

        b.iter(|| {
            rt.block_on(async {
                let msg = OutboundMessage::new("telegram", "chat", "Reply");
                bus.publish_outbound(black_box(msg)).await.unwrap();
            });
        });
    });

    group.bench_function("publish_consume_roundtrip", |b| {
        let bus = MessageBus::new(1_000);
        let mut rx = bus.take_inbound_rx().unwrap();

        b.iter(|| {
            rt.block_on(async {
                let msg = InboundMessage::new("telegram", "user", "chat", "roundtrip");
                bus.publish_inbound(msg).await.unwrap();
                black_box(rx.recv().await.unwrap());
            });
        });
    });

    group.bench_function("throughput_1000_messages", |b| {
        b.iter(|| {
            rt.block_on(async {
                let bus = MessageBus::new(2_000);
                let mut rx = bus.take_inbound_rx().unwrap();

                for i in 0..1_000 {
                    let msg =
                        InboundMessage::new("telegram", "user", "chat", format!("msg {}", i));
                    bus.publish_inbound(msg).await.unwrap();
                }

                for _ in 0..1_000 {
                    black_box(rx.recv().await.unwrap());
                }
            });
        });
    });

    group.finish();
}

// ─────────────────────────────────────────────────────────────
// Config benchmarks
// ─────────────────────────────────────────────────────────────

fn bench_config(c: &mut Criterion) {
    let mut group = c.benchmark_group("config");

    group.bench_function("default_creation", |b| {
        b.iter(|| {
            black_box(Config::default());
        });
    });

    group.bench_function("serialize_json", |b| {
        let config = Config::default();
        b.iter(|| {
            black_box(serde_json::to_string(&config).unwrap());
        });
    });

    group.bench_function("deserialize_json", |b| {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        b.iter(|| {
            black_box(serde_json::from_str::<Config>(&json).unwrap());
        });
    });

    group.bench_function("active_provider_detection", |b| {
        let config = Config::default();
        b.iter(|| {
            black_box(config.active_provider_name());
        });
    });

    group.finish();
}

// ─────────────────────────────────────────────────────────────
// InboundMessage benchmarks
// ─────────────────────────────────────────────────────────────

fn bench_inbound_message(c: &mut Criterion) {
    let mut group = c.benchmark_group("inbound_message");

    group.bench_function("creation", |b| {
        b.iter(|| {
            black_box(InboundMessage::new(
                "telegram",
                "user_12345",
                "chat_67890",
                "Hello, this is a test message with some content.",
            ));
        });
    });

    group.bench_function("session_key", |b| {
        let msg = InboundMessage::new("telegram", "user_12345", "chat_67890", "Hello");
        b.iter(|| {
            black_box(msg.session_key());
        });
    });

    group.bench_function("validate", |b| {
        let msg = InboundMessage::new("telegram", "user", "chat", "Normal message");
        b.iter(|| {
            black_box(msg.validate());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_session_add_message,
    bench_session_save_load,
    bench_message_bus,
    bench_config,
    bench_inbound_message,
);
criterion_main!(benches);
