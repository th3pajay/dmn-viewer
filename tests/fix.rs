use dmn_viewer::edit::{Column, Edit, apply};
use dmn_viewer::fix::{self, Candidate};
use dmn_viewer::model::Definitions;
use dmn_viewer::suite::{Case, Suite};
use dmn_viewer::{lint, parse};
use std::collections::HashSet;

/// A FIRST table with one of every fixable problem, plus a placeholder that needs a person.
fn table_xml(hit_policy: &str, rules: &[(&str, [&str; 4], [&str; 2])]) -> String {
    let mut x = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="Defs" name="Fixes" namespace="http://example.com/fix">
  <decision id="Route" name="Route">
    <decisionTable id="Table" hitPolicy="{hit_policy}">
      <input id="In_Region" label="Region">
        <inputExpression id="IE_Region" typeRef="string"><text>Region</text></inputExpression>
        <inputValues id="IV_Region"><text>"EU","NA","APAC"</text></inputValues>
      </input>
      <input id="In_Weight" label="Weight">
        <inputExpression id="IE_Weight" typeRef="number"><text>Weight</text></inputExpression>
      </input>
      <input id="In_Zone" label="Zone">
        <inputExpression id="IE_Zone" typeRef="string"><text>Zone</text></inputExpression>
      </input>
      <input id="In_Channel" label="Channel">
        <inputExpression id="IE_Channel" typeRef="string"><text>Channel</text></inputExpression>
      </input>
      <output id="Out_Carrier" name="Carrier" typeRef="string" />
      <output id="Out_Label" name="Label" typeRef="string" />
"#
    );
    for (k, (id, ins, outs)) in rules.iter().enumerate() {
        x.push_str(&format!("      <rule id=\"{id}\">\n"));
        for (c, t) in ins.iter().enumerate() {
            x.push_str(&format!("        <inputEntry id=\"I{k}_{c}\"><text>{}</text></inputEntry>\n", t.replace('<', "&lt;").replace('>', "&gt;")));
        }
        for (c, t) in outs.iter().enumerate() {
            x.push_str(&format!("        <outputEntry id=\"O{k}_{c}\"><text>{t}</text></outputEntry>\n"));
        }
        x.push_str("      </rule>\n");
    }
    x.push_str("    </decisionTable>\n  </decision>\n</definitions>\n");
    x
}

fn messy() -> String {
    table_xml(
        "FIRST",
        &[
            ("R1", [r#""EU""#, "<= 5", "-", "-"], [r#""POSTAL""#, r#""EU-small""#]),
            ("R2", [r#""EU""#, "> 5", "1", "-"], [r#""DHL""#, "OrderDate.substring(2,8)"]),
            ("R2", [r#""NA","LATAM""#, "-", "-", "-"], [r#""UPS""#, r#""NA""#]),
            ("R4", [r#""EU""#, "<= 5", "-", "-"], [r#""POSTAL""#, r#""EU-small""#]),
            ("R5", [r#""APAC""#, "-", "-", "-"], [r#""FEDEX""#, r#""TBD""#]),
            ("R6", ["-", "-", "-", "-"], [r#""REVIEW""#, r#""manual""#]),
            ("R7", [r#""APAC""#, "> 30", "-", "-"], [r#""FREIGHT""#, r#""heavy""#]),
        ],
    )
}

fn defs(xml: &str) -> Definitions {
    parse::parse(xml).unwrap()
}

fn queue(xml: &str, skipped: &HashSet<String>) -> Vec<Candidate> {
    let d = defs(xml);
    fix::queue(&d, &lint::lint(&d), skipped, None, None)
}

fn find<'a>(q: &'a [Candidate], code: &str, rule: usize) -> &'a Candidate {
    q.iter().find(|c| c.issue.code == code && c.issue.rules.first() == Some(&rule)).unwrap_or_else(|| panic!("no {code} on rule {}", rule + 1))
}

fn case(name: &str, region: &str, weight: f64, carrier: &str) -> Case {
    let mut c = Case { name: name.into(), ..Case::default() };
    c.inputs.insert("Region".into(), region.into());
    c.inputs.insert("Weight".into(), weight.into());
    c.expected.insert("Carrier".into(), carrier.into());
    c
}

#[test]
fn the_queue_puts_errors_first_and_deletions_last() {
    let q = queue(&messy(), &HashSet::new());
    let shown: Vec<(&str, Vec<usize>, bool)> = q.iter().map(|c| (c.issue.code, c.issue.rules.clone(), c.destructive())).collect();
    assert_eq!(
        shown,
        vec![
            ("DUPLICATE_ID", vec![], false),
            ("NON_FEEL_SYNTAX", vec![1], false),
            ("NOT_IN_ALLOWED_VALUES", vec![2], false),
            ("UNREACHABLE_RULE", vec![6, 4], false),
            ("UNREACHABLE_RULE", vec![3, 0], true),
            ("TYPE_MISMATCH", vec![1], false),
            ("UNUSED_INPUT", vec![], true),
        ]
    );
    // The placeholder is listed by lint but has no automatic fix.
    let d = defs(&messy());
    assert!(lint::lint(&d).iter().any(|i| i.code == "PLACEHOLDER"));
    assert!(q.iter().all(|c| c.issue.code != "PLACEHOLDER"));
}

#[test]
fn each_fix_produces_the_expected_text() {
    let xml = messy();
    let q = queue(&xml, &HashSet::new());
    let d = defs(&xml);
    let after = |c: &Candidate, i: usize| defs(&fix::apply(&xml, &c.fixes[i].edits).unwrap());

    let feel = find(&q, "NON_FEEL_SYNTAX", 1);
    assert_eq!(feel.fixes.len(), 2);
    assert_eq!(after(feel, 0).table(0).unwrap().rules[1].outputs[1].text, "substring(OrderDate, 3, 6)");
    assert_eq!(after(feel, 1).table(0).unwrap().rules[1].outputs[1].lang.declared.as_deref(), Some("javascript"));

    let quote = find(&q, "TYPE_MISMATCH", 1);
    assert_eq!(after(quote, 0).table(0).unwrap().rules[1].inputs[2].text, r#""1""#);

    let allowed = find(&q, "NOT_IN_ALLOWED_VALUES", 2);
    assert_eq!(allowed.fixes.len(), 2);
    assert_eq!(after(allowed, 0).table(0).unwrap().inputs[0].input_values.as_deref(), Some(r#""EU","NA","APAC","LATAM""#));
    assert_eq!(after(allowed, 1).table(0).unwrap().rules[2].inputs[0].text, r#""NA""#);

    // Shadowed by a different rule: moving it up is offered first; identical conditions: only deleting.
    let shadowed = find(&q, "UNREACHABLE_RULE", 6);
    assert_eq!((shadowed.default_fix(), shadowed.fixes.len()), (Some(0), 2));
    assert_eq!(after(shadowed, 0).table(0).unwrap().rules[4].id, "R7");
    let copy = find(&q, "UNREACHABLE_RULE", 3);
    assert_eq!((copy.default_fix(), copy.fixes.len()), (None, 1));
    assert_eq!(copy.fixes[0].label, "Delete rule 4");

    let dup = q.iter().find(|c| c.issue.code == "DUPLICATE_ID").unwrap();
    let out = fix::apply(&xml, &dup.fixes[0].edits).unwrap();
    assert_eq!(defs(&out).table(0).unwrap().rules[2].id, "R2_2");
    assert_eq!(out.replacen("R2_2", "R2", 1), xml, "only the second id changes");

    // Every fix describes itself as from → to lines.
    for c in &q {
        for f in &c.fixes {
            let lines: Vec<_> = f.edits.iter().flat_map(|e| fix::describe(&d, e)).collect();
            assert!(!lines.is_empty() && lines.iter().all(|l| l.from != l.to), "{}: {lines:?}", f.label);
        }
    }
}

#[test]
fn applying_the_queue_leaves_only_what_needs_a_person() {
    let mut xml = messy();
    for _ in 0..20 {
        let q = queue(&xml, &HashSet::new());
        let Some(c) = q.first() else { break };
        let f = &c.fixes[c.default_fix().unwrap_or(0)];
        xml = fix::apply(&xml, &f.edits).unwrap();
    }
    let left: Vec<&str> = lint::lint(&defs(&xml)).iter().map(|i| i.code).collect();
    assert!(queue(&xml, &HashSet::new()).is_empty(), "{left:?}");
    assert!(left.contains(&"PLACEHOLDER"));
}

#[test]
fn keys_survive_renumbering_and_tell_duplicate_ids_apart() {
    let xml = messy();
    let q = queue(&xml, &HashSet::new());
    let shadowed = find(&q, "UNREACHABLE_RULE", 6).key.clone();
    // Deleting rule 4 makes rule 7 rule 6; its key stays the same.
    let after = apply(&xml, &Edit::DeleteRule { decision: 0, rule: 3 }).unwrap();
    let q2 = queue(&after, &HashSet::new());
    assert_eq!(find(&q2, "UNREACHABLE_RULE", 5).key, shadowed);
    // A skipped issue stays out of the queue.
    let skipped: HashSet<String> = [shadowed].into();
    assert!(queue(&after, &skipped).iter().all(|c| !(c.issue.code == "UNREACHABLE_RULE" && c.issue.rules[0] == 5)));
    // Rules 2 and 3 share the id R2, so their keys carry the position.
    let allowed = find(&q, "NOT_IN_ALLOWED_VALUES", 2);
    assert!(allowed.key.contains("|R2#2"), "{}", allowed.key);
    assert_ne!(find(&q, "NON_FEEL_SYNTAX", 1).key, allowed.key);
}

#[test]
fn a_shared_keys_helper_gives_the_same_keys_as_key() {
    let xml = table_xml(
        "FIRST",
        &[
            ("R1", [r#""LATAM""#, "-", "-", "-"], [r#""A""#, r#""a""#]),
            ("", ["-", "-", "-", "-"], [r#""B""#, r#""b""#]),
            ("R1", [r#""EU""#, "-", "-", "-"], [r#""C""#, r#""c""#]),
            ("", [r#""NA""#, "-", "-", "-"], [r#""D""#, r#""d""#]),
        ],
    );
    let d = defs(&xml);
    let issues = lint::lint(&d);
    let keys = fix::Keys::new(&d);
    assert!(!issues.is_empty());
    for is in &issues {
        assert_eq!(keys.key(&d, is), fix::key(&d, is));
    }
    let by_helper: Vec<String> = fix::queue_with(&keys, &d, &issues, &HashSet::new(), None, None).into_iter().map(|c| c.key).collect();
    let by_queue: Vec<String> = fix::queue(&d, &issues, &HashSet::new(), None, None).into_iter().map(|c| c.key).collect();
    assert_eq!(by_helper, by_queue);
    assert!(by_queue.iter().any(|k| k.contains("|R1#")), "a shared id carries the rule's position: {by_queue:?}");
    assert!(by_queue.iter().any(|k| k.contains("||") || k.contains("|#")), "an empty id carries the rule's position: {by_queue:?}");
}

#[test]
fn front_and_only_shape_the_queue() {
    let xml = messy();
    let d = defs(&xml);
    let issues = lint::lint(&d);
    let all = fix::queue(&d, &issues, &HashSet::new(), None, None);
    let last = all.last().unwrap().key.clone();
    let fronted = fix::queue(&d, &issues, &HashSet::new(), Some(&last), None);
    assert_eq!(fronted[0].key, last);
    let one = fix::queue(&d, &issues, &HashSet::new(), None, Some(&last));
    assert_eq!(one.len(), 1);
}

#[test]
fn test_effect_shows_broken_fixed_and_changed_cases() {
    let xml = messy();
    let before = defs(&xml);
    let mut suite = Suite { decision: "Route".into(), cases: vec![case("LATAM", "LATAM", 3.0, "UPS"), case("APAC heavy", "APAC", 40.0, "FREIGHT"), case("Unknown", "XX", 1.0, "nobody")] };
    let q = queue(&xml, &HashSet::new());

    let allowed = find(&q, "NOT_IN_ALLOWED_VALUES", 2);
    let removed = defs(&fix::apply(&xml, &allowed.fixes[1].edits).unwrap());
    let fx = fix::test_effect(&before, &removed, 0, &suite).unwrap();
    assert_eq!((fx.total, fx.passed_before, fx.passed_after), (3, 1, 0));
    assert_eq!(fx.broken, vec!["LATAM".to_string()]);

    let moved = defs(&fix::apply(&xml, &find(&q, "UNREACHABLE_RULE", 6).fixes[0].edits).unwrap());
    let fx = fix::test_effect(&before, &moved, 0, &suite).unwrap();
    assert_eq!(fx.fixed, vec!["APAC heavy".to_string()]);
    assert!(fx.broken.is_empty() && fx.changed.is_empty());

    // Failing before and after, for a different reason: a light APAC parcel gets FEDEX from rule 5,
    // and REVIEW once rule 5 is deleted.
    suite.cases = vec![case("APAC light", "APAC", 1.0, "FREIGHT")];
    let deleted = defs(&apply(&xml, &Edit::DeleteRule { decision: 0, rule: 4 }).unwrap());
    let fx = fix::test_effect(&before, &deleted, 0, &suite).unwrap();
    assert_eq!(fx.changed, vec!["APAC light".to_string()]);

    suite.decision = "Other".into();
    assert!(fix::test_effect(&before, &moved, 0, &suite).is_none());
}

#[test]
fn duplicate_rules_under_collect_can_only_be_deleted() {
    let xml = table_xml(
        "COLLECT",
        &[("A", [r#""EU""#, "-", "-", "-"], [r#""X""#, r#""Y""#]), ("B", [r#""EU""#, "-", "-", "-"], [r#""X""#, r#""Y""#]), ("C", [r#""NA""#, "-", r#""z""#, "-"], [r#""X""#, r#""Y""#])],
    );
    let q = queue(&xml, &HashSet::new());
    let dup = find(&q, "DUPLICATE_RULE", 0);
    assert!(dup.destructive());
    assert_eq!(dup.fixes[0].edits, vec![Edit::DeleteRule { decision: 0, rule: 1 }]);
}

#[test]
fn renaming_duplicate_ids_skips_ids_in_use() {
    let xml = table_xml(
        "FIRST",
        &[
            ("D", ["-", "-", r#""a""#, "-"], [r#""1""#, r#""2""#]),
            ("D", [r#""EU""#, "-", r#""b""#, "-"], [r#""1""#, r#""2""#]),
            ("D_2", [r#""NA""#, "-", r#""c""#, "-"], [r#""1""#, r#""2""#]),
            ("D", [r#""APAC""#, "-", r#""d""#, "-"], [r#""1""#, r#""2""#]),
        ],
    );
    let out = apply(&xml, &Edit::RenameDuplicateId { id: "D".into() }).unwrap();
    let ids: Vec<String> = defs(&out).table(0).unwrap().rules.iter().map(|r| r.id.clone()).collect();
    assert_eq!(ids, vec!["D", "D_3", "D_2", "D_4"]);
    assert!(defs(&out).duplicate_ids.is_empty());
}

#[test]
fn cell_fixes_keep_a_rule_column_shape() {
    // A FEEL input entry with a method call can only be switched to JavaScript.
    let xml = table_xml("FIRST", &[("A", [r#"Region.startsWith("E")"#, "-", r#""z""#, "-"], [r#""1""#, r#""2""#]), ("B", ["-", "-", "-", "-"], [r#""1""#, r#""2""#])]);
    let q = queue(&xml, &HashSet::new());
    let c = find(&q, "NON_FEEL_SYNTAX", 0);
    assert_eq!(c.fixes.len(), 1);
    assert_eq!(c.fixes[0].edits, vec![Edit::CellLanguage { decision: 0, rule: 0, col: Column::Input(0), lang: Some("javascript".into()) }]);
}
