//! Generate 100 base ConversationFixtures by composing personas + topics + actions.
//! Run via: cargo run -p kca-bench --bin gen-soak > tests/fixtures/kca/soak_10k.jsonl

use kca_e2e::fixtures::{ConversationFixture, TurnFixture};

fn main() {
    let personas = ["Alice", "Bob", "Carol", "Dan", "Eve"];
    let topics = [
        "Rust",
        "Python",
        "tokio",
        "FastAPI",
        "kubernetes",
        "PostgreSQL",
    ];
    let actions = ["uses", "tested", "deployed", "debugged"];
    for (i, p) in personas.iter().enumerate() {
        for (j, t) in topics.iter().enumerate() {
            for (k, a) in actions.iter().enumerate() {
                let id = format!("soak_{i}_{j}_{k}");
                let f = ConversationFixture {
                    id,
                    source: "soak".into(),
                    turns: vec![TurnFixture {
                        user: format!("{p} {a} {t} today."),
                        assistant: "Got it.".into(),
                        ..Default::default()
                    }],
                    queries: vec![],
                    metadata: serde_json::Value::Null,
                };
                println!("{}", serde_json::to_string(&f).unwrap());
            }
        }
    }
}
