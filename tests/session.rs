use dmn_viewer::model::Definitions;
use dmn_viewer::parse;
use dmn_viewer::session::TestSession;
use std::path::PathBuf;
use std::sync::Arc;

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join(name)
}

fn defs() -> Definitions {
    parse::parse(&std::fs::read_to_string(example("shipping-fees.dmn")).unwrap()).unwrap()
}

fn fields(region: &str, weight: &str, express: &str, date: &str) -> Vec<String> {
    [region, weight, express, date].map(String::from).to_vec()
}

#[test]
fn live_outcome_follows_the_inputs() {
    let defs = defs();
    let mut s = TestSession::default();
    assert!(!s.is_active());
    s.fields = fields("\"EU\"", "2.5", "false", "\"20260314\"");
    assert!(s.is_active());
    let o = s.outcome(&defs, 0).unwrap();
    assert_eq!(o.fired, vec![0]);
    assert!(Arc::ptr_eq(&o, &s.outcome(&defs, 0).unwrap()), "unchanged inputs reuse the cached outcome");
    s.fields[1] = "12".into();
    assert_eq!(s.outcome(&defs, 0).unwrap().fired, vec![1]);
    s.clear_inputs();
    assert!(!s.is_active());
}

#[test]
fn add_accept_and_delete_cases() {
    let defs = defs();
    let mut s = TestSession::default();
    s.fields = fields("\"EU\"", "2.5", "false", "\"20260314\"");
    s.add_case(&defs, 0, "  ");
    s.add_case(&defs, 0, " small ");
    let names: Vec<&str> = s.suite().cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Case 1", "small"]);
    assert_eq!(s.suite().decision, defs.decisions[0].name, "an unnamed suite takes the decision's name");
    assert_eq!(s.suite().cases[0].inputs["Weight"], serde_json::json!(2.5));
    assert_eq!(s.suite().cases[0].expect_rules, Some(vec![1]));

    let r = s.results(&defs, 0).unwrap();
    assert!(r.iter().all(|r| r.pass));
    assert!(Arc::ptr_eq(&r, &s.results(&defs, 0).unwrap()), "unchanged cases reuse the cached results");

    s.delete(0);
    s.delete(99);
    assert_eq!(s.suite().cases.len(), 1);
}

#[test]
fn accept_replaces_expectations_with_the_actual_result() {
    let defs = defs();
    let mut s = TestSession::default();
    s.load(&example("shipping-fees.tests.json")).unwrap();
    let mut broken = s.suite().clone();
    broken.cases[0].expected.insert("Fee".into(), serde_json::json!(999));
    let dir = scratch_dir("accept");
    std::fs::write(dir.join("broken.json"), serde_json::to_string(&broken).unwrap()).unwrap();
    s.load(&dir.join("broken.json")).unwrap();
    assert!(!s.results(&defs, 0).unwrap()[0].pass);
    s.accept(&defs, 0, 0);
    assert!(s.results(&defs, 0).unwrap()[0].pass);
}

#[test]
fn fill_inputs_from_a_case_or_a_rule() {
    let defs = defs();
    let mut s = TestSession::default();
    s.load(&example("shipping-fees.tests.json")).unwrap();
    s.fill_from_case(&defs, 0, 0);
    assert_eq!(s.fields, fields("EU", "2.5", "false", "20260314"));

    let table = defs.table(0).unwrap();
    s.fields = fields("", "", "", "keep");
    s.fill_from_rule(&table.rules[0]);
    assert_eq!(s.fields[0], "EU");
    assert_eq!(s.fields[2], "false");
    assert_eq!(s.fields[3], "keep", "an any (-) condition leaves the input alone");
}

#[test]
fn save_and_load_round_trip() {
    let defs = defs();
    let dir = scratch_dir("roundtrip");
    let path = dir.join("suite.tests.json");
    let mut s = TestSession::default();
    s.new_suite("");
    s.fields = fields("\"NA\"", "3", "true", "\"20260401\"");
    s.add_case(&defs, 0, "na");
    assert_eq!(s.save(&path, "ignored, already named"), Ok(1));
    assert_eq!(s.suite_path(), Some(path.as_path()));

    let mut t = TestSession::default();
    assert_eq!(t.load(&path), Ok(1));
    assert_eq!(t.suite(), s.suite());
    assert!(t.results(&defs, 0).unwrap()[0].pass);

    assert!(t.load(&dir.join("missing.json")).is_err());
    assert_eq!(t.suite_path(), Some(path.as_path()), "a failed load leaves the session unchanged");
}

#[test]
fn non_table_decisions_have_no_results() {
    let xml = r#"<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="d"><decision id="x"><literalExpression><text>1</text></literalExpression></decision></definitions>"#;
    let defs = parse::parse(xml).unwrap();
    let mut s = TestSession::default();
    assert!(s.outcome(&defs, 0).is_none() && s.results(&defs, 0).is_none());
    s.add_case(&defs, 0, "x");
    assert!(s.suite().cases.is_empty());
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dmn-viewer-session-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
