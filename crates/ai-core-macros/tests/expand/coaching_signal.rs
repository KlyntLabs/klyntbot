use ai_core::{AiEventMeta, AiSignal};
use ai_core_macros::AiEvent;

#[derive(AiEvent)]
pub enum TestEvent {
    #[ai(
        importance = 0.9,
        salience = "extract",
        observation_template = "alert {category} {spent}/{limit}",
        coaching_signal(
            app_from = "category",
            amount_from = "spent",
            category_from = "category",
            rule = "Review spending when budget pressure rises",
        ),
    )]
    BudgetAlert { category: String, spent: f64, limit: f64 },

    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "distraction {app}",
    )]
    Distraction { app: String },
}

fn main() {
    let sig: AiSignal = TestEvent::BudgetAlert {
        category: "food".into(),
        spent: 450.0,
        limit: 500.0,
    }.to_signal();
    assert!(sig.coaching_signal);
    assert_eq!(sig.coaching_rule.as_deref(), Some("Review spending when budget pressure rises"));
    assert_eq!(sig.metrics.amount, Some(450.0));
    assert_eq!(sig.metrics.category.as_deref(), Some("food"));
    assert_eq!(sig.metrics.app.as_deref(), Some("food"));

    let sig: AiSignal = TestEvent::Distraction { app: "reddit".into() }.to_signal();
    assert!(!sig.coaching_signal);
    assert!(sig.coaching_rule.is_none());
    assert!(sig.metrics.app.is_none());
}
