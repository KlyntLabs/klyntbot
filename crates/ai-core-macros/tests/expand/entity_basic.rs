use ai_core::AiEntity;
use ai_core_macros::AiEntity;

#[derive(AiEntity)]
#[ai(entity_type = "task", embed_on = ["title", "description"])]
struct Task {
    title: String,
    description: Option<String>,
    internal: i64,
}

fn main() {
    let t = Task {
        title: "Ship".into(),
        description: Some("it".into()),
        internal: 99,
    };
    assert_eq!(Task::entity_type(), "task");
    assert_eq!(t.embed_text(), "Ship\nit");

    let t2 = Task { title: "A".into(), description: None, internal: 0 };
    assert_eq!(t2.embed_text(), "A");
}
