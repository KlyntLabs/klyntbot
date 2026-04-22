use ai_core::{RecallProvider, RecallQuery};
use feature_finance::FinanceFeature;
use feature_tasks::TasksFeature;

#[test]
fn tasks_feature_scores_deadline_queries_higher() {
    let f = TasksFeature::default();
    let hot = RecallQuery { message: "when is the deadline?".into(), intent_summary: None };
    let cold = RecallQuery { message: "what is machine learning".into(), intent_summary: None };
    assert!(f.score_query(&hot) > f.score_query(&cold));
}

#[test]
fn finance_feature_scores_money_queries_higher() {
    let f = FinanceFeature::default();
    let hot = RecallQuery { message: "how much did I spend on food".into(), intent_summary: None };
    let cold = RecallQuery { message: "what time is it".into(), intent_summary: None };
    assert!(f.score_query(&hot) > f.score_query(&cold));
}
