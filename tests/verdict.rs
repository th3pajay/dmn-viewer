mod common;

use common::*;
use dmn_viewer::eval::{Context, Value, evaluate};
use dmn_viewer::model::DecisionTable;
use dmn_viewer::suite::{Case, Suite, run_case};
use dmn_viewer::verdict::{Expectation, Verdict, check};
use serde_json::{Value as J, json};

fn at(x: f64) -> Context {
    Context::from([("x".to_string(), Value::Num(x))])
}

fn expect(pairs: &[(&str, J)]) -> Expectation {
    Expectation { expected: pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(), ..Expectation::default() }
}

fn judge(t: &DecisionTable, x: f64, e: &Expectation) -> Verdict {
    check(t, &evaluate(t, &at(x)), e)
}

fn two_outputs(attrs: &str) -> DecisionTable {
    table(&dmn(attrs, &[LEVEL, AMOUNT], &[(">= 0", &[r#""low""#, "1"]), (">= 5", &[r#""mid""#, "2"]), (">= 8", &[r#""high""#, "3"])]))
}

#[test]
fn unlisted_outputs_are_not_checked() {
    let t = two_outputs(r#"hitPolicy="FIRST""#);
    let e = expect(&[("level", json!("low"))]);
    let v = judge(&t, 9.0, &e);
    assert!(v.pass(), "{:?}", v.problems);
    assert_eq!((v.checks, v.cells.iter().filter(|c| c.checked).count()), (1, 1));
    assert!(!v.cells[1].checked && v.cells[1].matches());
    let case = Case { name: "partial".into(), inputs: [("x".to_string(), json!(9))].into(), expect: e };
    assert!(run_case(&t, &case).pass);
}

#[test]
fn multi_row_results_compare_every_row() {
    let t = levels(r#"hitPolicy="COLLECT""#);
    assert!(judge(&t, 9.0, &expect(&[("level", json!(["low", "mid", "high"]))])).pass());
    let v = judge(&t, 9.0, &expect(&[("level", json!(["low", "mid"]))]));
    assert_eq!(v.problems, vec!["level: expected 2 rows, got 3".to_string()]);
    assert_eq!(v.rows, 3);
    assert!(!v.cells[2].matches() && v.cells[2].expected.is_none());
    let v = judge(&t, 9.0, &expect(&[("level", json!(["low", "high", "mid"]))]));
    assert_eq!(v.problems, vec![r#"level #2: expected "high", got "mid""#.to_string(), r#"level #3: expected "mid", got "high""#.to_string()]);
}

#[test]
fn a_scalar_means_exactly_one_row() {
    let t = levels(r#"hitPolicy="RULE ORDER""#);
    assert!(judge(&t, 2.0, &expect(&[("level", json!("low"))])).pass());
    assert!(!judge(&t, 9.0, &expect(&[("level", json!("low"))])).pass());
    assert!(judge(&t, -1.0, &expect(&[("level", J::Null)])).pass(), "null means no rows");
}

#[test]
fn single_result_policies_reject_lists() {
    let v = judge(&levels(r#"hitPolicy="FIRST""#), 9.0, &expect(&[("level", json!(["low"]))]));
    assert_eq!(v.problems, vec!["level: expected one value for hit policy FIRST, got a list".to_string()]);
    let sum = table(&dmn(r#"hitPolicy="COLLECT" aggregation="SUM""#, &[AMOUNT], &AMOUNTS));
    assert!(judge(&sum, 9.0, &expect(&[("amount", json!(6))])).pass());
    assert!(!judge(&sum, 9.0, &expect(&[("amount", json!([6]))])).pass());
}

#[test]
fn an_accepted_outcome_always_passes() {
    for attrs in [r#"hitPolicy="FIRST""#, r#"hitPolicy="PRIORITY""#, r#"hitPolicy="COLLECT""#, r#"hitPolicy="RULE ORDER""#, r#"hitPolicy="OUTPUT ORDER""#, r#"hitPolicy="UNIQUE""#] {
        let t = two_outputs(attrs);
        for x in [-1.0, 2.0, 6.0, 9.0] {
            let o = evaluate(&t, &at(x));
            let e = Expectation::of_outcome(&t, &o);
            let v = check(&t, &o, &e);
            assert!(v.pass(), "{attrs} x={x}: {:?}", v.problems);
            if t.hit_policy.returns_rows() && o.error.is_none() {
                assert!(e.expected["level"].is_array(), "{attrs}");
            }
        }
    }
}

#[test]
fn testing_a_rule_checks_that_rule_fired_with_its_own_row() {
    let first = levels(r#"hitPolicy="FIRST""#);
    let shadowed = Expectation::of_rule(&first, 1, &at(9.0)).unwrap();
    let v = judge(&first, 9.0, &shadowed);
    assert!(v.problems.contains(&"rule 2 did not fire".to_string()), "{:?}", v.problems);
    assert!(judge(&first, 9.0, &Expectation::of_rule(&first, 0, &at(9.0)).unwrap()).pass());
    let collect = levels(r#"hitPolicy="COLLECT""#);
    let v = judge(&collect, 9.0, &Expectation::of_rule(&collect, 1, &at(9.0)).unwrap());
    assert!(v.pass(), "{:?}", v.problems);
    assert_eq!(v.cells[0].actual, Some(Value::Str("mid".into())));
}

#[test]
fn expected_errors_are_checked_too() {
    let t = levels(r#"hitPolicy="UNIQUE""#);
    let wants_error = Expectation { expect_error: true, ..Expectation::default() };
    assert!(judge(&t, 9.0, &wants_error).pass());
    assert!(!judge(&t, 2.0, &wants_error).pass());
    let v = judge(&t, 9.0, &expect(&[("level", json!("low"))]));
    assert!(v.problems[0].starts_with("evaluation failed"));
}

#[test]
fn fired_rules_are_part_of_the_verdict() {
    let t = levels(r#"hitPolicy="FIRST""#);
    let mut e = expect(&[("level", json!("low"))]);
    e.expect_rules = Some(vec![2]);
    let v = judge(&t, 9.0, &e);
    assert_eq!(v.rules, Some((vec![2], vec![1])));
    assert_eq!((v.checks, v.failed), (2, 1));
}

#[test]
fn the_tester_and_the_suite_agree_on_partial_cases() {
    let xml = dmn(r#"hitPolicy="FIRST""#, &[LEVEL, AMOUNT], &[(">= 0", &[r#""low""#, "1"]), (">= 5", &[r#""mid""#, "2"])]);
    let mut ws = workspace("verdict-agree", &xml);
    let dir = ws.path().parent().unwrap().to_path_buf();
    let suite = Suite { decision: String::new(), cases: vec![Case { name: "partial".into(), inputs: [("x".to_string(), json!(2))].into(), expect: expect(&[("level", json!("low"))]) }] };
    let file = dir.join("partial.tests.json");
    dmn_viewer::suite::save(&file, &suite).unwrap();
    let mut tests = ws.tests();
    tests.load(&file).unwrap();
    assert!(tests.results().unwrap()[0].pass);
    tests.fill_from_case(0);
    let v = tests.verdict().unwrap();
    assert!(v.pass(), "{:?}", v.problems);
    assert_eq!(v.against.as_deref(), Some("case \"partial\""));
}

#[test]
fn the_tester_saves_multi_row_cases_as_arrays() {
    let mut ws = workspace("verdict-rows", &dmn(r#"hitPolicy="COLLECT""#, &[LEVEL], &LEVELS));
    let mut tests = ws.tests();
    tests.set_inputs(&["9"]);
    tests.add_case("all three");
    assert_eq!(tests.suite().cases[0].expect.expected["level"], json!(["low", "mid", "high"]));
    assert!(tests.results().unwrap()[0].pass);
    assert!(tests.verdict().unwrap().pass());
}

fn levels(attrs: &str) -> DecisionTable {
    table(&dmn(attrs, &[LEVEL], &LEVELS))
}

#[test]
fn suites_round_trip_through_the_file_format() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join("shipping-fees.tests.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let suite: Suite = serde_json::from_str(&text).unwrap();
    let written = serde_json::to_string_pretty(&suite).unwrap();
    assert_eq!(serde_json::from_str::<Suite>(&written).unwrap(), suite);
    let at = |k: &str| written.find(k).unwrap();
    assert!(at("\"name\"") < at("\"inputs\"") && at("\"inputs\"") < at("\"expected\"") && at("\"expected\"") < at("\"expectRules\""));
    assert!(written.contains("\"expectError\": true") && !written.contains("\"rule\""));
}
