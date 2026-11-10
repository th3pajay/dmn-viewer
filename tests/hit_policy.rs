mod common;

use common::*;
use dmn_viewer::eval::{Context, Value, evaluate};
use dmn_viewer::model::{Aggregation, HitPolicy};
use dmn_viewer::{gaps, lint};

fn at(x: f64) -> Context {
    Context::from([("x".to_string(), Value::Num(x))])
}

fn levels(attrs: &str) -> dmn_viewer::model::DecisionTable {
    table(&dmn(attrs, &[LEVEL], &LEVELS))
}

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

#[test]
fn parsing_is_case_insensitive_and_defaults_to_unique() {
    assert_eq!(HitPolicy::parse(None, None), HitPolicy::Unique);
    assert_eq!(HitPolicy::parse(Some("rule order"), None), HitPolicy::RuleOrder);
    assert_eq!(HitPolicy::parse(Some("COLLECT"), Some("sum")), HitPolicy::Collect(Some(Aggregation::Sum)));
    assert_eq!(HitPolicy::parse(Some("FIRST"), Some("SUM")), HitPolicy::First);
    assert_eq!(HitPolicy::parse(Some("SOMETIMES"), None), HitPolicy::Unknown("SOMETIMES".into()));
    assert!(HitPolicy::Collect(None).returns_rows() && HitPolicy::RuleOrder.returns_rows() && HitPolicy::OutputOrder.returns_rows());
    assert!(!HitPolicy::Collect(Some(Aggregation::Sum)).returns_rows() && !HitPolicy::First.returns_rows());
}

#[test]
fn each_policy_combines_matches_its_own_way() {
    let o = evaluate(&levels(r#"hitPolicy="UNIQUE""#), &at(9.0));
    assert!(o.error.as_deref().unwrap().contains("UNIQUE violated"));
    let o = evaluate(&levels(r#"hitPolicy="FIRST""#), &at(9.0));
    assert_eq!((o.fired, o.rows), (vec![0], vec![vec![s("low")]]));
    let o = evaluate(&levels(r#"hitPolicy="PRIORITY""#), &at(9.0));
    assert_eq!((o.fired, o.rows), (vec![2], vec![vec![s("high")]]));
    let o = evaluate(&levels(r#"hitPolicy="ANY""#), &at(9.0));
    assert!(o.error.as_deref().unwrap().contains("ANY violated"));
    let o = evaluate(&levels(r#"hitPolicy="COLLECT""#), &at(9.0));
    assert_eq!(o.fired, vec![0, 1, 2]);
    let o = evaluate(&levels(r#"hitPolicy="RULE ORDER""#), &at(9.0));
    assert_eq!(o.rows, vec![vec![s("low")], vec![s("mid")], vec![s("high")]]);
    let o = evaluate(&levels(r#"hitPolicy="OUTPUT ORDER""#), &at(9.0));
    assert_eq!((o.fired, o.rows), (vec![2, 1, 0], vec![vec![s("high")], vec![s("mid")], vec![s("low")]]));
}

#[test]
fn any_with_equal_outputs_fires_the_first() {
    let t = table(&dmn(r#"hitPolicy="ANY""#, &[LEVEL], &[(">= 0", &[r#""low""#]), (">= 5", &[r#""low""#])]));
    let o = evaluate(&t, &at(9.0));
    assert_eq!((o.error, o.fired), (None, vec![0]));
}

#[test]
fn collect_aggregations() {
    let run = |agg: &str| evaluate(&table(&dmn(&format!(r#"hitPolicy="COLLECT" aggregation="{agg}""#), &[AMOUNT], &AMOUNTS)), &at(9.0)).aggregate;
    assert_eq!(run("SUM"), Some(Value::Num(6.0)));
    assert_eq!(run("COUNT"), Some(Value::Num(3.0)));
    assert_eq!(run("MIN"), Some(Value::Num(1.0)));
    assert_eq!(run("MAX"), Some(Value::Num(3.0)));
}

#[test]
fn unknown_policy_or_aggregation_is_an_error_and_an_issue() {
    for attrs in [r#"hitPolicy="SOMETIMES""#, r#"hitPolicy="COLLECT" aggregation="AVG""#] {
        let xml = dmn(attrs, &[AMOUNT], &AMOUNTS);
        let o = evaluate(&table(&xml), &at(9.0));
        assert!(o.error.as_deref().unwrap().starts_with("Unsupported"), "{attrs}: {:?}", o.error);
        assert!(lint::lint(&defs(&xml)).iter().any(|i| i.code == "UNKNOWN_HIT_POLICY"), "{attrs}");
    }
}

#[test]
fn witness_fires_every_reachable_rule_under_priority() {
    let t = levels(r#"hitPolicy="PRIORITY""#);
    for rule in 0..3 {
        let w = gaps::witness(&t, rule).unwrap();
        let ctx = Context::from([("x".to_string(), w[0].clone().unwrap_or(Value::Null))]);
        assert_eq!(evaluate(&t, &ctx).fired, vec![rule], "rule {}", rule + 1);
    }
}

#[test]
fn witness_respects_any_with_equal_outputs() {
    let t = table(&dmn(r#"hitPolicy="ANY""#, &[LEVEL], &[("< 5", &[r#""low""#]), (">= 3", &[r#""low""#]), (">= 8", &[r#""high""#])]));
    let w = gaps::witness(&t, 1).unwrap();
    let ctx = Context::from([("x".to_string(), w[0].clone().unwrap_or(Value::Null))]);
    assert_eq!(evaluate(&t, &ctx).fired, vec![1]);
}

#[test]
fn witness_gives_up_on_unknown_policies() {
    assert!(gaps::witness(&levels(r#"hitPolicy="SOMETIMES""#), 0).is_none());
}
