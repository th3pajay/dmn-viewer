use dmn_viewer::edit::{Column, Edit, apply};
use dmn_viewer::model::{DecisionTable, Definitions, Language};
use dmn_viewer::{lint, parse};
use std::path::PathBuf;

fn example() -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join("shipping-fees.dmn")).unwrap()
}

fn defs(xml: &str) -> Definitions {
    parse::parse(xml).unwrap()
}

fn table(d: &Definitions) -> &DecisionTable {
    d.table(0).unwrap()
}

/// Bytes that differ between `a` and `b`, as (start, len in a, len in b), after trimming the common prefix and suffix.
fn changed(a: &str, b: &str) -> (usize, usize, usize) {
    let p = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    let s = a[p..].bytes().rev().zip(b[p..].bytes().rev()).take_while(|(x, y)| x == y).count();
    (p, a.len() - p - s, b.len() - p - s)
}

fn assert_no_shape_issues(xml: &str) {
    let issues = lint::lint(&defs(xml));
    assert!(!issues.iter().any(|i| i.code == "RULE_SHAPE" || i.code == "DUPLICATE_ID"), "{issues:#?}");
}

fn cell(rule: usize, col: Column, text: &str) -> Edit {
    Edit::Cell { decision: 0, rule, col, text: text.into() }
}

#[test]
fn cell_edit_touches_only_that_cell() {
    let xml = example();
    let out = apply(&xml, &cell(0, Column::Input(1), "<= 6")).unwrap();
    assert_eq!(table(&defs(&out)).rules[0].inputs[1].text, "<= 6");
    let (at, old, new) = changed(&xml, &out);
    assert_eq!((&xml[at..at + old], &out[at..at + new]), ("5", "6"));
    assert!(out.contains("&lt;= 6"));
}

#[test]
fn unchanged_values_keep_the_bytes() {
    let xml = example();
    let text = table(&defs(&xml)).rules[0].inputs[1].text.clone();
    assert_eq!(apply(&xml, &cell(0, Column::Input(1), &text)).unwrap(), xml);
    let note = table(&defs(&xml)).note(&table(&defs(&xml)).rules[0]).to_string();
    assert_eq!(apply(&xml, &Edit::Note { decision: 0, rule: 0, text: note }).unwrap(), xml);
}

#[test]
fn special_characters_round_trip() {
    let xml = example();
    let text = r#"a < b && c > "d" & 'e'"#;
    let out = apply(&xml, &cell(1, Column::Output(2), text)).unwrap();
    assert_eq!(table(&defs(&out)).rules[1].outputs[2].text, text);
    let out = apply(&out, &Edit::OutputHeader { decision: 0, output: 0, label: text.into(), name: "Fee".into(), type_ref: Some("double".into()), allowed: None }).unwrap();
    assert_eq!(table(&defs(&out)).outputs[0].label, text);
}

const TINY: &str = r#"<?xml version="1.0"?>
<dmn:definitions xmlns:dmn="https://www.omg.org/spec/DMN/20191111/MODEL/" id="d" name="d">
  <dmn:decision id="dec" name="Dec">
    <dmn:decisionTable id="t">
      <dmn:input id="i1"><dmn:inputExpression id="ie1" typeRef="string"><dmn:text>a</dmn:text></dmn:inputExpression></dmn:input>
      <dmn:output id="o1" name="out" typeRef="string" />
      <dmn:rule id="r1">
        <dmn:inputEntry id="e1"><dmn:text><![CDATA["x" < "y"]]></dmn:text></dmn:inputEntry>
        <dmn:outputEntry id="e2"><dmn:text/></dmn:outputEntry>
      </dmn:rule>
      <dmn:rule id="r2">
        <dmn:inputEntry id="e3"/>
        <dmn:outputEntry id="e4"></dmn:outputEntry>
      </dmn:rule>
    </dmn:decisionTable>
  </dmn:decision>
</dmn:definitions>
"#;

#[test]
fn awkward_text_forms_and_prefixes() {
    let rule = |xml: &str, r: usize| table(&defs(xml)).rules[r].clone();
    assert_eq!(rule(TINY, 0).inputs[0].text, r#""x" < "y""#);
    let out = apply(TINY, &cell(0, Column::Input(0), "\"z\"")).unwrap();
    assert_eq!(rule(&out, 0).inputs[0].text, "\"z\"", "CDATA content is replaced");
    let out = apply(TINY, &cell(0, Column::Output(0), "1")).unwrap();
    assert!(out.contains("<dmn:text>1</dmn:text></dmn:outputEntry>"), "<text/> is expanded with its prefix");
    let out = apply(TINY, &cell(1, Column::Input(0), "\"q\"")).unwrap();
    assert!(out.contains(r#"<dmn:inputEntry id="e3"><dmn:text>"q"</dmn:text></dmn:inputEntry>"#), "self-closing entry gets a <text>");
    let out = apply(TINY, &cell(1, Column::Output(0), "2")).unwrap();
    assert_eq!(rule(&out, 1).outputs[0].text, "2", "entry without <text> gets one");
}

#[test]
fn crlf_and_bom_are_kept() {
    let xml = format!("\u{feff}{}", example().replace('\n', "\r\n"));
    let out = apply(&xml, &cell(0, Column::Input(1), "<= 7")).unwrap();
    assert!(out.starts_with('\u{feff}'));
    assert_eq!(out.matches("\r\n").count(), xml.matches("\r\n").count());
    let out = apply(&xml, &Edit::InsertRule { decision: 0, at: 1, copy_of: Some(0) }).unwrap();
    assert!(!out.replace("\r\n", "").contains('\n'), "inserted rule uses CRLF too");
}

#[test]
fn cell_language() {
    let xml = example();
    let out = apply(&xml, &Edit::CellLanguage { decision: 0, rule: 0, col: Column::Output(2), lang: Some("javascript".into()) }).unwrap();
    assert_eq!(table(&defs(&out)).rules[0].outputs[2].lang.language, Language::Js);
    let back = apply(&out, &Edit::CellLanguage { decision: 0, rule: 0, col: Column::Output(2), lang: None }).unwrap();
    assert_eq!(back, xml);
}

#[test]
fn rule_notes() {
    let xml = example();
    let out = apply(&xml, &Edit::Note { decision: 0, rule: 0, text: "Tiny parcels".into() }).unwrap();
    assert_eq!(table(&defs(&out)).rules[0].description, "Tiny parcels");
    let out = apply(&xml, &Edit::Note { decision: 0, rule: 0, text: String::new() }).unwrap();
    assert!(table(&defs(&out)).rules[0].description.is_empty() && !out.contains("Small parcels"));
    let created = apply(&out, &Edit::Note { decision: 0, rule: 0, text: "Small parcels inside the EU".into() }).unwrap();
    assert_eq!(created, xml, "re-adding the description restores the original bytes");

    let annotated = TINY.replace(r#"<dmn:rule id="r1">"#, r#"<dmn:annotation name="Notes"/><dmn:rule id="r1">"#);
    let out = apply(&annotated, &Edit::Note { decision: 0, rule: 0, text: "why".into() }).unwrap();
    let t = defs(&out).table(0).unwrap().clone();
    assert_eq!((t.rules[0].annotations.clone(), t.rules[0].description.clone()), (vec!["why".to_string()], String::new()));
}

#[test]
fn hit_policy_and_aggregation() {
    let xml = example();
    let out = apply(&xml, &Edit::HitPolicy { decision: 0, hit_policy: "COLLECT".into(), aggregation: Some("SUM".into()) }).unwrap();
    let t = table(&defs(&out)).clone();
    assert_eq!((t.hit_policy.as_str(), t.aggregation.as_deref()), ("COLLECT", Some("SUM")));
    let out = apply(&out, &Edit::HitPolicy { decision: 0, hit_policy: "FIRST".into(), aggregation: Some("SUM".into()) }).unwrap();
    assert_eq!(out, xml, "aggregation only applies to COLLECT");
    let out = apply(TINY, &Edit::HitPolicy { decision: 0, hit_policy: "UNIQUE".into(), aggregation: None }).unwrap();
    assert_eq!(out, TINY, "the default policy is not written out");
}

#[test]
fn input_and_output_headers() {
    let xml = example();
    let edit = Edit::InputHeader { decision: 0, input: 0, label: "Zone".into(), expression: "zone".into(), type_ref: Some("string".into()), lang: None, allowed: Some(r#""EU","NA""#.into()) };
    let out = apply(&xml, &edit).unwrap();
    let i = table(&defs(&out)).inputs[0].clone();
    assert_eq!((i.label.as_str(), i.expression.as_str(), i.input_values.as_deref()), ("Zone", "zone", Some(r#""EU","NA""#)));
    assert!(!i.lang.is_explicit(), "expressionLanguage was removed");

    let out =
        apply(&xml, &Edit::OutputHeader { decision: 0, output: 1, label: "Carrier".into(), name: "Carrier".into(), type_ref: Some("string".into()), allowed: Some(r#""POSTAL","ROADLINE""#.into()) })
            .unwrap();
    assert_eq!(table(&defs(&out)).outputs[1].output_values.as_deref(), Some(r#""POSTAL","ROADLINE""#), "self-closing <output> gets <outputValues>");
    let back = apply(&out, &Edit::OutputHeader { decision: 0, output: 1, label: "Carrier".into(), name: "Carrier".into(), type_ref: Some("string".into()), allowed: None }).unwrap();
    assert!(table(&defs(&back)).outputs[1].output_values.is_none());
}

#[test]
fn insert_duplicate_and_delete_rules() {
    let xml = example();
    let n = table(&defs(&xml)).rules.len();
    let dup = apply(&xml, &Edit::InsertRule { decision: 0, at: 1, copy_of: Some(0) }).unwrap();
    let t = table(&defs(&dup)).clone();
    assert_eq!(t.rules.len(), n + 1);
    assert_eq!(t.rules[1].inputs.iter().map(|e| &e.text).collect::<Vec<_>>(), t.rules[0].inputs.iter().map(|e| &e.text).collect::<Vec<_>>());
    assert_ne!(t.rules[1].id, t.rules[0].id);
    assert_no_shape_issues(&dup);
    assert_eq!(apply(&dup, &Edit::DeleteRule { decision: 0, rule: 1 }).unwrap(), xml);

    let empty = apply(&xml, &Edit::InsertRule { decision: 0, at: n, copy_of: None }).unwrap();
    let t = table(&defs(&empty)).clone();
    let last = &t.rules[n];
    assert!(last.inputs.iter().all(|e| e.is_any()) && last.outputs.iter().all(|e| e.text.is_empty()) && last.description.is_empty());
    assert_no_shape_issues(&empty);
    assert_eq!(apply(&empty, &Edit::DeleteRule { decision: 0, rule: n }).unwrap(), xml);
}

#[test]
fn move_rules() {
    let xml = example();
    let ids = |x: &str| table(&defs(x)).rules.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
    let before = ids(&xml);
    let down = apply(&xml, &Edit::MoveRule { decision: 0, from: 1, to: 3 }).unwrap();
    let mut want = before.clone();
    let r = want.remove(1);
    want.insert(3, r);
    assert_eq!(ids(&down), want);
    assert_eq!(apply(&down, &Edit::MoveRule { decision: 0, from: 3, to: 1 }).unwrap(), xml);
}

#[test]
fn insert_and_delete_columns() {
    let xml = example();
    for (insert, delete) in [
        (Edit::InsertInput { decision: 0, at: 1 }, Edit::DeleteInput { decision: 0, input: 1 }),
        (Edit::InsertInput { decision: 0, at: 4 }, Edit::DeleteInput { decision: 0, input: 4 }),
        (Edit::InsertOutput { decision: 0, at: 0 }, Edit::DeleteOutput { decision: 0, output: 0 }),
        (Edit::InsertOutput { decision: 0, at: 3 }, Edit::DeleteOutput { decision: 0, output: 3 }),
    ] {
        let out = apply(&xml, &insert).unwrap();
        let t = table(&defs(&out)).clone();
        assert_eq!(t.inputs.len() + t.outputs.len(), 8, "{insert:?}");
        assert_no_shape_issues(&out);
        assert_eq!(apply(&out, &delete).unwrap(), xml, "{delete:?}");
    }
    let one = apply(TINY, &Edit::InsertOutput { decision: 0, at: 1 }).unwrap();
    assert_eq!(table(&defs(&one)).outputs[1].name, "output1");
    assert!(apply(TINY, &Edit::DeleteOutput { decision: 0, output: 0 }).is_err(), "the last output stays");
}

#[test]
fn empty_table_gets_a_rule() {
    let bare = r#"<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="d"><decision id="x"><decisionTable id="t"><input id="i"><inputExpression id="ie"><text>a</text></inputExpression></input><output id="o" name="b"/></decisionTable></decision></definitions>"#;
    let out = apply(bare, &Edit::InsertRule { decision: 0, at: 0, copy_of: None }).unwrap();
    let t = table(&defs(&out)).clone();
    assert_eq!((t.rules.len(), t.rules[0].inputs.len(), t.rules[0].outputs.len()), (1, 1, 1));
}

#[test]
fn bad_targets_are_errors() {
    let xml = example();
    assert!(apply(&xml, &cell(99, Column::Input(0), "x")).is_err());
    assert!(apply(&xml, &Edit::DeleteRule { decision: 5, rule: 0 }).is_err());
}
