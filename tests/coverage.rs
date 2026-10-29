use dmn_viewer::suite::{self, Coverage, Suite};
use dmn_viewer::{model::Definitions, parse};
use serde_json::json;
use std::path::PathBuf;

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join(name)
}

fn defs() -> Definitions {
    parse::parse(&std::fs::read_to_string(example("shipping-fees.dmn")).unwrap()).unwrap()
}

fn coverage(defs: &Definitions, s: &Suite) -> Coverage {
    let (_, t) = suite::find_table(defs, &s.decision).unwrap();
    let results: Vec<_> = s.cases.iter().map(|c| suite::run_case(t, c)).collect();
    Coverage::new(t.rules.len(), &results)
}

#[test]
fn the_example_suite_covers_every_rule() {
    let defs = defs();
    let cov = coverage(&defs, &suite::load(&example("shipping-fees.tests.json")).unwrap());
    assert_eq!((cov.covered(), cov.total()), (7, 7));
    assert_eq!(cov.summary(), "7/7 rules covered (100%)");
    assert_eq!(cov.never_fired_label(), "none");
    assert!(cov.only_failing().is_empty());
}

#[test]
fn counts_cases_per_rule_and_rules_fired_only_by_failing_cases() {
    let defs = defs();
    let mut s = suite::load(&example("shipping-fees.tests.json")).unwrap();
    // Keep: two passing cases on rule 1, one on rule 2, and the NA express case (rule 4) made to fail.
    s.cases.retain(|c| ["EU small parcel", "EU parcel at the 5 kg boundary", "EU medium parcel", "NA express"].contains(&c.name.as_str()));
    s.cases[3].expected.insert("Fee".into(), json!(1));
    let cov = coverage(&defs, &s);
    assert_eq!(&cov.fired_by[..4], &[(2, 0), (1, 0), (0, 0), (0, 1)]);
    assert_eq!(cov.covered(), 3, "a rule only failing cases fire still counts as covered");
    assert!(cov.is_covered(3) && !cov.is_covered(2) && !cov.is_covered(99));
    assert_eq!(cov.only_failing(), vec![3]);
    assert_eq!(cov.never_fired(), vec![2, 4, 5, 6]);
    assert_eq!(cov.never_fired_label(), "3, 5-7");
    assert_eq!(cov.summary(), "3/7 rules covered (42%), 1 only by failing cases");
}

#[test]
fn an_empty_suite_covers_nothing() {
    let defs = defs();
    let cov = coverage(&defs, &Suite::default());
    assert_eq!((cov.covered(), cov.total()), (0, 7));
    assert_eq!(cov.never_fired_label(), "1-7");
    assert_eq!(Coverage::new(0, &[]).summary(), "0/0 rules covered (100%)");
}
