use dmn_viewer::edit::{Column, Edit};
use dmn_viewer::fix;
use dmn_viewer::lint::{Issue, Severity};
use dmn_viewer::model::Definitions;
use dmn_viewer::parse;
use dmn_viewer::rules::{self, RuleClip};

fn xml() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/shipping-fees.dmn")).unwrap()
}

fn defs() -> Definitions {
    parse::parse(&xml()).unwrap()
}

fn issue(decision: usize, rules: &[usize]) -> Issue {
    Issue { severity: Severity::Warning, code: "TEST", decision: Some(decision), rules: rules.to_vec(), column: None, message: String::new() }
}

#[test]
fn copy_reads_a_rules_cells_and_note() {
    let d = defs();
    let t = d.table(0).unwrap();
    let clip = rules::copy(t, 0).unwrap();
    assert_eq!(clip.inputs.len(), t.inputs.len());
    assert_eq!(clip.outputs.len(), t.outputs.len());
    assert_eq!(clip.inputs[0], t.rules[0].inputs[0].text);
    assert_eq!(clip.note, t.note(&t.rules[0]));
    assert!(rules::copy(t, t.rules.len()).is_none());
}

#[test]
fn pasting_adds_a_rule_with_the_copied_cells() {
    let d = defs();
    let t = d.table(0).unwrap();
    let clip = rules::copy(t, 2).unwrap();
    let (edits, at) = rules::paste(t, 0, Some(0), &clip);
    assert_eq!(at, 1);
    let pasted = parse::parse(&fix::apply(&xml(), &edits).unwrap()).unwrap();
    let p = pasted.table(0).unwrap();
    assert_eq!(p.rules.len(), t.rules.len() + 1);
    let texts = |r: usize| (p.rules[r].inputs.iter().map(|e| e.text.clone()).collect::<Vec<_>>(), p.rules[r].outputs.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    assert_eq!(texts(at), texts(3), "the pasted rule matches the copied one, now at index 3");
}

#[test]
fn pasting_without_a_selection_appends() {
    let d = defs();
    let t = d.table(0).unwrap();
    let (_, at) = rules::paste(t, 0, None, &rules::copy(t, 0).unwrap());
    assert_eq!(at, t.rules.len());
}

#[test]
fn pasting_drops_columns_the_table_lacks_and_skips_blank_cells() {
    let d = defs();
    let t = d.table(0).unwrap();
    let clip = RuleClip { inputs: vec!["\"EU\"".into(), " ".into(), "x".into(), "y".into(), "extra".into()], outputs: vec!["1".into()], note: String::new() };
    let (edits, at) = rules::paste(t, 0, Some(0), &clip);
    let cells: Vec<_> = edits.iter().filter_map(|e| if let Edit::Cell { col, .. } = e { Some(*col) } else { None }).collect();
    assert_eq!(cells, [Column::Input(0), Column::Input(2), Column::Input(3), Column::Output(0)]);
    assert!(!edits.iter().any(|e| matches!(e, Edit::Note { .. })));
    assert!(matches!(edits[0], Edit::InsertRule { at: 1, .. }));
    assert_eq!(at, 1);
}

#[test]
fn replace_text_keeps_the_rest_of_the_cell() {
    assert_eq!(rules::replace_text("x > 5 or x < 2", "x", "age", true).as_deref(), Some("age > 5 or age < 2"));
    assert_eq!(rules::replace_text("\"Gold\"", "gold", "\"Silver\"", false).as_deref(), Some("\"\"Silver\"\""));
    assert_eq!(rules::replace_text("\"Gold\"", "gold", "x", true), None);
    assert_eq!(rules::replace_text("x", "", "y", false), None);
}

#[test]
fn replace_edits_every_matching_cell_and_applies_cleanly() {
    let d = defs();
    let t = d.table(0).unwrap();
    let edits = rules::replace(t, 0, "EU", "XX", true);
    assert!(!edits.is_empty());
    let replaced = parse::parse(&fix::apply(&xml(), &edits).unwrap()).unwrap();
    let r = replaced.table(0).unwrap();
    let cells = r.rules.iter().flat_map(|x| x.inputs.iter().map(|e| &e.text).chain(x.outputs.iter().map(|e| &e.text)));
    assert!(cells.clone().all(|c| !c.contains("EU")));
    assert!(cells.clone().any(|c| c.contains("XX")));
    assert!(rules::replace(t, 0, "eu", "XX", true).is_empty(), "case matters when asked");
    assert_eq!(rules::replace(t, 0, "eu", "XX", false).len(), edits.len(), "and is ignored otherwise");
}

#[test]
fn next_issue_wraps_and_stays_in_the_decision() {
    let issues = [issue(0, &[4]), issue(0, &[1, 4]), issue(1, &[2]), issue(0, &[])];
    assert_eq!(rules::next_issue(&issues, 0, None, true), Some(1));
    assert_eq!(rules::next_issue(&issues, 0, None, false), Some(4));
    assert_eq!(rules::next_issue(&issues, 0, Some(1), true), Some(4));
    assert_eq!(rules::next_issue(&issues, 0, Some(4), true), Some(1), "wraps forward");
    assert_eq!(rules::next_issue(&issues, 0, Some(1), false), Some(4), "wraps backward");
    assert_eq!(rules::next_issue(&issues, 0, Some(3), false), Some(1));
    assert_eq!(rules::next_issue(&issues, 2, Some(0), true), None);
    assert_eq!(rules::next_issue(&[], 0, None, true), None);
}
