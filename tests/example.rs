use dmn_viewer::{lint, parse, suite};
use std::path::PathBuf;

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join(name)
}

#[test]
fn example_parses_without_errors() {
    let xml = std::fs::read_to_string(example("shipping-fees.dmn")).unwrap();
    let defs = parse::parse(&xml).unwrap();
    assert_eq!(defs.decisions.len(), 1);
    let issues = lint::lint(&defs);
    let errors: Vec<_> = issues.iter().filter(|i| i.severity == lint::Severity::Error).collect();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn example_suite_passes() {
    let xml = std::fs::read_to_string(example("shipping-fees.dmn")).unwrap();
    let defs = parse::parse(&xml).unwrap();
    let s = suite::load(&example("shipping-fees.tests.json")).unwrap();
    let (_, table) = suite::find_table(&defs, &s.decision).unwrap();
    for case in &s.cases {
        let r = suite::run_case(table, case);
        assert!(r.pass, "{}: {:?}", case.name, r.problems);
    }
}

#[test]
fn feel_method_call_is_reported() {
    let xml = std::fs::read_to_string(example("shipping-fees.dmn")).unwrap();
    let broken = xml.replacen(r#"<outputEntry id="Rule_EuHeavy_Label" expressionLanguage="javascript">"#, r#"<outputEntry id="Rule_EuHeavy_Label">"#, 1);
    let defs = parse::parse(&broken).unwrap();
    let issues = lint::lint(&defs);
    assert!(issues.iter().any(|i| i.code == "NON_FEEL_SYNTAX" && i.rules == vec![2]));
}
