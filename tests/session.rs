use dmn_viewer::edit::{Column, Edit};
use dmn_viewer::workspace::Workspace;
use std::path::PathBuf;
use std::sync::Arc;

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join(name)
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dmn-viewer-session-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fresh(name: &str) -> Workspace {
    let path = scratch_dir(name).join("shipping-fees.dmn");
    std::fs::copy(example("shipping-fees.dmn"), &path).unwrap();
    Workspace::open(path).unwrap()
}

fn fields(region: &str, weight: &str, express: &str, date: &str) -> Vec<String> {
    [region, weight, express, date].map(String::from).to_vec()
}

fn set_fields(ws: &mut Workspace, values: Vec<String>) {
    *ws.tests().fields_mut() = values;
}

fn cell(rule: usize, text: &str) -> Edit {
    Edit::Cell { decision: 0, rule, col: Column::Output(0), text: text.into() }
}

#[test]
fn live_outcome_follows_the_inputs() {
    let mut ws = fresh("live");
    assert!(!ws.tests().is_active());
    set_fields(&mut ws, fields("\"EU\"", "2.5", "false", "\"20260314\""));
    assert!(ws.tests().is_active());
    let o = ws.tests().outcome().unwrap();
    assert_eq!(o.fired, vec![0]);
    assert!(Arc::ptr_eq(&o, &ws.tests().outcome().unwrap()), "unchanged inputs reuse the cached outcome");
    ws.tests().fields_mut()[1] = "12".into();
    assert_eq!(ws.tests().outcome().unwrap().fired, vec![1]);
    ws.tests().clear_inputs();
    assert!(!ws.tests().is_active());
}

#[test]
fn add_accept_and_delete_cases() {
    let mut ws = fresh("add");
    set_fields(&mut ws, fields("\"EU\"", "2.5", "false", "\"20260314\""));
    let mut tests = ws.tests();
    tests.add_case("  ");
    tests.add_case(" small ");
    let names: Vec<&str> = tests.suite().cases.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Case 1", "small"]);
    assert_eq!(tests.suite().decision, tests.defs.decisions[0].name, "an unnamed suite takes the decision's name");
    assert_eq!(tests.suite().cases[0].inputs["Weight"], serde_json::json!(2.5));
    assert_eq!(tests.suite().cases[0].expect_rules, Some(vec![1]));

    let r = tests.results().unwrap();
    assert!(r.iter().all(|r| r.pass));
    assert!(Arc::ptr_eq(&r, &tests.results().unwrap()), "unchanged cases reuse the cached results");

    tests.delete(0);
    tests.delete(99);
    assert_eq!(tests.suite().cases.len(), 1);
}

#[test]
fn accept_replaces_expectations_with_the_actual_result() {
    let mut ws = Workspace::open(example("shipping-fees.dmn")).unwrap();
    let mut broken = ws.tests().suite().clone();
    broken.cases[0].expected.insert("Fee".into(), serde_json::json!(999));
    let dir = scratch_dir("accept");
    std::fs::write(dir.join("broken.json"), serde_json::to_string(&broken).unwrap()).unwrap();
    let mut tests = ws.tests();
    tests.load(&dir.join("broken.json")).unwrap();
    assert!(!tests.results().unwrap()[0].pass);
    tests.accept(0);
    assert!(tests.results().unwrap()[0].pass);
}

#[test]
fn fill_inputs_from_a_case_or_a_rule() {
    let mut ws = Workspace::open(example("shipping-fees.dmn")).unwrap();
    let mut tests = ws.tests();
    tests.fill_from_case(0);
    assert_eq!(tests.fields(), fields("EU", "2.5", "false", "20260314"));

    let defs = tests.defs;
    let table = defs.table(0).unwrap();
    *tests.fields_mut() = fields("", "", "", "keep");
    tests.fill_from_rule(&table.rules[0]);
    assert_eq!(tests.fields()[0], "EU");
    assert_eq!(tests.fields()[2], "false");
    assert_eq!(tests.fields()[3], "keep", "an any (-) condition leaves the input alone");
}

#[test]
fn save_and_load_round_trip() {
    let path = scratch_dir("roundtrip").join("suite.tests.json");
    let mut ws = fresh("roundtrip-a");
    set_fields(&mut ws, fields("\"NA\"", "3", "true", "\"20260401\""));
    let mut s = ws.tests();
    s.new_suite();
    s.add_case("na");
    assert_eq!(s.save(&path), Ok(1));
    assert_eq!(s.suite_path(), Some(path.as_path()));
    let saved = s.suite().clone();

    let mut other = fresh("roundtrip-b");
    let mut t = other.tests();
    assert_eq!(t.load(&path), Ok(1));
    assert_eq!(t.suite(), &saved);
    assert!(t.results().unwrap()[0].pass);

    assert!(t.load(&scratch_dir("roundtrip").join("missing.json")).is_err());
    assert_eq!(t.suite_path(), Some(path.as_path()), "a failed load leaves the session unchanged");
}

#[test]
fn non_table_decisions_have_no_results() {
    let xml = r#"<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="d"><decision id="x"><literalExpression><text>1</text></literalExpression></decision></definitions>"#;
    let path = scratch_dir("literal").join("literal.dmn");
    std::fs::write(&path, xml).unwrap();
    let mut ws = Workspace::open(path).unwrap();
    let mut tests = ws.tests();
    assert!(tests.outcome().is_none() && tests.results().is_none());
    tests.add_case("x");
    assert!(tests.suite().cases.is_empty());
}

#[test]
fn results_follow_document_edits_without_any_sync_call() {
    let mut ws = Workspace::open(example("shipping-fees.dmn")).unwrap();
    let before = ws.tests().results().unwrap();
    assert!(Arc::ptr_eq(&before, &ws.tests().results().unwrap()), "no edit, same run");
    assert!(before[0].pass);

    assert!(ws.commit_group(&[cell(0, "999")], None));
    let after = ws.tests().results().unwrap();
    assert!(!Arc::ptr_eq(&before, &after), "an edit forces a re-run");
    assert!(!after[0].pass, "the first case expects the old fee");

    ws.undo();
    assert!(ws.tests().results().unwrap()[0].pass, "undo brings the old result back");
}

#[test]
fn the_live_outcome_follows_document_edits() {
    let mut ws = Workspace::open(example("shipping-fees.dmn")).unwrap();
    set_fields(&mut ws, fields("\"EU\"", "2.5", "false", "\"20260314\""));
    let a = ws.tests().outcome().unwrap();
    assert!(Arc::ptr_eq(&a, &ws.tests().outcome().unwrap()));
    ws.commit_group(&[cell(0, "999")], None);
    let b = ws.tests().outcome().unwrap();
    assert!(!Arc::ptr_eq(&a, &b), "an edit forces a re-evaluation");
    assert_ne!(a.rows, b.rows);
}

#[test]
fn coverage_is_cached_with_the_results_and_follows_cases_and_edits() {
    let mut ws = Workspace::open(example("shipping-fees.dmn")).unwrap();
    let a = ws.tests().coverage().unwrap();
    assert!(Arc::ptr_eq(&a, &ws.tests().coverage().unwrap()), "same cases, same run");
    assert_eq!(ws.tests().results().unwrap().len(), 9);
    assert!(Arc::ptr_eq(&a, &ws.tests().coverage().unwrap()), "results and coverage share one run");

    ws.tests().delete(0);
    ws.tests().delete(0);
    let b = ws.tests().coverage().unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    assert_eq!(b.fired_by[0], (0, 0), "both rule-1 cases are gone");

    ws.commit_group(&[cell(0, "999")], None);
    assert!(!Arc::ptr_eq(&b, &ws.tests().coverage().unwrap()), "an edit forces a re-run");
}

#[test]
fn reloading_the_file_keeps_the_suite_and_refreshes_the_results() {
    let mut ws = fresh("reload");
    ws.tests().new_suite();
    let a = ws.tests().results().unwrap();
    ws.reload().unwrap();
    assert!(!Arc::ptr_eq(&a, &ws.tests().results().unwrap()), "a new document is a new revision");
}
