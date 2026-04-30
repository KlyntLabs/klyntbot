//! Generate expanded fixture sets for KCA benchmarks.
//! Usage: cargo run -p kca-bench --bin gen-fixtures -- --output tests/fixtures/kca

use kca_e2e::fixtures::{ConversationFixture, GroundTruthFact, QueryFixture, TurnFixture};
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let output = args
        .iter()
        .enumerate()
        .find_map(|(i, a)| {
            if a == "--output" {
                args.get(i + 1).cloned()
            } else {
                None
            }
        })
        .unwrap_or_else(|| "tests/fixtures/kca".into());
    let out = std::path::Path::new(&output);
    std::fs::create_dir_all(out).unwrap();

    gen_longmembench(out);
    gen_locobench(out);
    gen_klynt_coding(out);
    gen_multi_cli(out);
    gen_hallucination(out);
    gen_regression(out);

    println!("✅ Fixtures written to {}", out.display());
}

fn gen_longmembench(out: &std::path::Path) {
    let path = out.join("longmembench_subset.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    let personas = [
        "Alice", "Bob", "Carol", "Dan", "Eve", "Frank", "Grace", "Heidi", "Ivan", "Judy",
    ];
    let companies = [
        "Anthropic",
        "OpenAI",
        "Google",
        "Meta",
        "Microsoft",
        "Amazon",
        "Apple",
        "Netflix",
        "Stripe",
        "Shopify",
    ];
    let teams = [
        "Claude",
        "GPT",
        "Search",
        "Ads",
        "Cloud",
        "Prime",
        "iOS",
        "Content",
        "Payments",
        "Logistics",
    ];
    let cities = [
        "SF",
        "NYC",
        "London",
        "Berlin",
        "Tokyo",
        "Singapore",
        "Sydney",
        "Toronto",
        "Paris",
        "Amsterdam",
    ];

    for i in 0..100 {
        let p = personas[i % personas.len()];
        let c = companies[i % companies.len()];
        let t = teams[i % teams.len()];
        let city = cities[i % cities.len()];
        let fixture = ConversationFixture {
            id: format!("lmb_{i:03}"),
            source: "longmembench".into(),
            turns: vec![
                TurnFixture {
                    user: format!("Hi, I'm {p} and I work at {c} on the {t} team."),
                    assistant: format!("Nice to meet you, {p}!"),
                    ground_truth_facts: vec![
                        GroundTruthFact {
                            subject: p.into(),
                            predicate: "works_at".into(),
                            object: c.into(),
                        },
                        GroundTruthFact {
                            subject: p.into(),
                            predicate: "on_team".into(),
                            object: t.into(),
                        },
                    ],
                    ..Default::default()
                },
                TurnFixture {
                    user: format!("I moved to {city} last year."),
                    assistant: "Got it.".into(),
                    ground_truth_facts: vec![GroundTruthFact {
                        subject: p.into(),
                        predicate: "lives_in".into(),
                        object: city.into(),
                    }],
                    ..Default::default()
                },
            ],
            queries: vec![
                QueryFixture {
                    query: format!("Where does {p} work?"),
                    gold_answer: c.into(),
                    hop_type: "single".into(),
                    required_fact_subjects: vec![p.into()],
                },
                QueryFixture {
                    query: format!("Where does {p} live?"),
                    gold_answer: city.into(),
                    hop_type: "single".into(),
                    required_fact_subjects: vec![p.into()],
                },
            ],
            metadata: serde_json::Value::Null,
        };
        writeln!(f, "{}", serde_json::to_string(&fixture).unwrap()).unwrap();
    }
}

fn gen_locobench(out: &std::path::Path) {
    let path = out.join("locobench_subset.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    let personas = [
        "Alice", "Bob", "Carol", "Dan", "Eve", "Frank", "Grace", "Heidi", "Ivan", "Judy",
    ];
    let pets = [
        "dog", "cat", "parrot", "rabbit", "hamster", "turtle", "fish", "snake", "lizard", "frog",
    ];
    let pet_names = [
        "Max", "Bella", "Charlie", "Luna", "Cooper", "Daisy", "Milo", "Lucy", "Rocky", "Lily",
    ];
    let foods = [
        "pizza", "sushi", "tacos", "pasta", "burgers", "curry", "ramen", "salad", "steak",
        "dim sum",
    ];

    for i in 0..100 {
        let p = personas[i % personas.len()];
        let pet = pets[i % pets.len()];
        let name = pet_names[i % pet_names.len()];
        let food = foods[i % foods.len()];
        let fixture = ConversationFixture {
            id: format!("loco_{i:03}"),
            source: "locobench".into(),
            turns: vec![
                // Identity binding — without this turn, the conversation
                // says "I have a dog" but the query asks "what pet does
                // Alice have?", leaving the agent no way to bind I→Alice.
                TurnFixture {
                    user: format!("Hi, I'm {p}."),
                    assistant: format!("Nice to meet you, {p}!"),
                    ground_truth_facts: vec![],
                    ..Default::default()
                },
                TurnFixture {
                    user: format!("I have a {pet} named {name}."),
                    assistant: "Cute!".into(),
                    ground_truth_facts: vec![
                        GroundTruthFact {
                            subject: p.into(),
                            predicate: "has_pet".into(),
                            object: name.into(),
                        },
                        GroundTruthFact {
                            subject: name.into(),
                            predicate: "is_a".into(),
                            object: pet.into(),
                        },
                    ],
                    ..Default::default()
                },
                TurnFixture {
                    user: format!("{name} loves {food}."),
                    assistant: "Great.".into(),
                    ground_truth_facts: vec![GroundTruthFact {
                        subject: name.into(),
                        predicate: "loves".into(),
                        object: food.into(),
                    }],
                    ..Default::default()
                },
            ],
            queries: vec![
                QueryFixture {
                    query: format!("What kind of pet does {p} have?"),
                    gold_answer: pet.into(),
                    hop_type: "single".into(),
                    required_fact_subjects: vec![p.into()],
                },
                QueryFixture {
                    query: format!("What does {name} love?"),
                    gold_answer: food.into(),
                    hop_type: "multi".into(),
                    required_fact_subjects: vec![name.into()],
                },
            ],
            metadata: serde_json::Value::Null,
        };
        writeln!(f, "{}", serde_json::to_string(&fixture).unwrap()).unwrap();
    }
}

fn gen_klynt_coding(out: &std::path::Path) {
    let path = out.join("klynt_coding_bench.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    let clis = ["ClaudeCode", "Codex", "KimiCli", "OpenCode"];
    let frameworks = ["cargo-nextest", "jest", "pytest", "mocha", "vitest"];
    let dead_ends = ["unwrap", "clone", "mutex", "arc", "channel"];
    let fixes = ["refactor", "extract", "inline", "split", "merge"];

    for i in 0..50 {
        let cli = clis[i % clis.len()];
        let fw = frameworks[i % frameworks.len()];
        let fixture = ConversationFixture {
            id: format!("kcb_dead_{i:03}"),
            source: "klynt-coding".into(),
            turns: vec![
                TurnFixture {
                    user: format!("Fix the failing test related to {fw}."),
                    assistant: format!(
                        "Checking error... the panic is at the {} in agent_loop. Let me trace.",
                        dead_ends[i % dead_ends.len()]
                    ),
                    cli_source: Some(cli.into()),
                    ..Default::default()
                },
                TurnFixture {
                    user: format!(
                        "Tried changing the {}, still fails",
                        dead_ends[i % dead_ends.len()]
                    ),
                    assistant: format!(
                        "This is the same dead end as last week — {} isn't the root cause.",
                        dead_ends[i % dead_ends.len()]
                    ),
                    cli_source: Some(cli.into()),
                    ..Default::default()
                },
            ],
            queries: vec![QueryFixture {
                query: format!(
                    "Have we tried changing the {} before?",
                    dead_ends[i % dead_ends.len()]
                ),
                gold_answer: "yes".into(),
                hop_type: "open".into(),
                required_fact_subjects: vec![],
            }],
            metadata: serde_json::Value::Null,
        };
        writeln!(f, "{}", serde_json::to_string(&fixture).unwrap()).unwrap();
    }

    for i in 0..50 {
        let cli = clis[i % clis.len()];
        let fix = fixes[i % fixes.len()];
        let fixture = ConversationFixture {
            id: format!("kcb_fix_{i:03}"),
            source: "klynt-coding".into(),
            turns: vec![
                TurnFixture {
                    user: format!("The {fix} didn't work last time."),
                    assistant: "Let me check the previous attempt.".into(),
                    cli_source: Some(cli.into()),
                    ..Default::default()
                },
                TurnFixture {
                    user: "What did we try?".into(),
                    assistant: format!("You attempted a {fix} two days ago but reverted it."),
                    cli_source: Some(cli.into()),
                    ..Default::default()
                },
            ],
            queries: vec![QueryFixture {
                query: format!("Did we try a {fix} before?"),
                gold_answer: "yes".into(),
                hop_type: "open".into(),
                required_fact_subjects: vec![],
            }],
            metadata: serde_json::Value::Null,
        };
        writeln!(f, "{}", serde_json::to_string(&fixture).unwrap()).unwrap();
    }

    for i in 0..30 {
        let cli = clis[i % clis.len()];
        let fixture = ConversationFixture {
            id: format!("kcb_xcli_{i:03}"),
            source: "klynt-coding".into(),
            turns: vec![TurnFixture {
                user: "Why did the build fail?".into(),
                assistant: "Looking at recent changes...".into(),
                cli_source: Some(cli.into()),
                ..Default::default()
            }],
            queries: vec![QueryFixture {
                query: format!("What pattern did {cli} find?"),
                gold_answer: "recent changes inspection".into(),
                hop_type: "single".into(),
                required_fact_subjects: vec![],
            }],
            metadata: serde_json::Value::Null,
        };
        writeln!(f, "{}", serde_json::to_string(&fixture).unwrap()).unwrap();
    }
}

fn gen_multi_cli(out: &std::path::Path) {
    let path = out.join("multi_cli_replay.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    let clis = ["ClaudeCode", "Codex", "KimiCli", "OpenCode"];
    let topics = [
        "Rust deadlock",
        "Python GIL",
        "Go channel leak",
        "JS async await",
        "SQL injection",
    ];
    let answers = [
        "parking_lot",
        "pyo3",
        "select",
        "Promise.all",
        "prepared statements",
    ];

    for i in 0..100 {
        let cli = clis[i % clis.len()];
        let topic = topics[i % topics.len()];
        let ans = answers[i % answers.len()];
        let fixture = ConversationFixture {
            id: format!("mcli_{i:03}"),
            source: "multi-cli".into(),
            turns: vec![TurnFixture {
                user: format!("How do I debug a {topic}?"),
                assistant: format!("Consider using {ans} to resolve it."),
                cli_source: Some(cli.into()),
                ..Default::default()
            }],
            queries: vec![QueryFixture {
                query: format!("What CLI suggested {ans}?"),
                gold_answer: cli.into(),
                hop_type: "single".into(),
                required_fact_subjects: vec![],
            }],
            metadata: serde_json::Value::Null,
        };
        writeln!(f, "{}", serde_json::to_string(&fixture).unwrap()).unwrap();
    }
}

fn gen_hallucination(out: &std::path::Path) {
    let path = out.join("hallucination_planted.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    let names = [
        "Alice", "Bob", "Carol", "Dan", "Eve", "Frank", "Grace", "Heidi", "Ivan", "Judy",
    ];
    let items = [
        "coffee", "tea", "pizza", "sushi", "books", "music", "hiking", "yoga", "coding", "design",
    ];

    for i in 0..100 {
        let name = names[i % names.len()];
        let item = items[i % items.len()];
        let fixture = ConversationFixture {
            id: format!("hal_{i:03}"),
            source: "synthetic-hallucination".into(),
            turns: vec![
                TurnFixture {
                    user: format!("My name is {name}."),
                    assistant: format!("Hi {name}!"),
                    ground_truth_facts: vec![GroundTruthFact {
                        subject: "user".into(),
                        predicate: "name".into(),
                        object: name.into(),
                    }],
                    ..Default::default()
                },
                TurnFixture {
                    user: format!("I love {item}."),
                    assistant: "Got it.".into(),
                    ground_truth_facts: vec![GroundTruthFact {
                        subject: "user".into(),
                        predicate: "loves".into(),
                        object: item.into(),
                    }],
                    ..Default::default()
                },
            ],
            queries: vec![],
            metadata: serde_json::Value::Null,
        };
        writeln!(f, "{}", serde_json::to_string(&fixture).unwrap()).unwrap();
    }
}

fn gen_regression(out: &std::path::Path) {
    let path = out.join("regression_panel.jsonl");
    let mut f = std::fs::File::create(&path).unwrap();
    for i in 0..30 {
        let bug_id = format!("bug_{i:03}");
        let fixture = ConversationFixture {
            id: format!("reg_{i:03}"),
            source: "regression".into(),
            turns: vec![TurnFixture {
                user: format!("Regression test for {bug_id}."),
                assistant: "Acknowledged.".into(),
                ..Default::default()
            }],
            queries: vec![],
            metadata: serde_json::json!({"bug_id": bug_id, "asserts": ["no_panic"]}),
        };
        writeln!(f, "{}", serde_json::to_string(&fixture).unwrap()).unwrap();
    }
}
