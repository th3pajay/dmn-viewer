use dmn_viewer::eval::{Context, Value, evaluate};
use dmn_viewer::{gaps, parse};

fn table(hp: &str, rules: &[[&str; 2]]) -> String {
    let mut x = format!(
        r#"<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="d"><decision id="D"><decisionTable id="T" hitPolicy="{hp}"><input id="a" label="Region"><inputExpression id="ae" typeRef="string"><text>Region</text></inputExpression></input><input id="b" label="Weight"><inputExpression id="be" typeRef="number"><text>Weight</text></inputExpression></input><output id="o" name="o" typeRef="string"/>"#
    );
    for (i, [r, w]) in rules.iter().enumerate() {
        x += &format!(
            r#"<rule id="R{i}"><inputEntry id="R{i}a"><text>{r}</text></inputEntry><inputEntry id="R{i}b"><text>{}</text></inputEntry><outputEntry id="R{i}o"><text>"r{i}"</text></outputEntry></rule>"#,
            w.replace('<', "&lt;").replace('>', "&gt;")
        );
    }
    x + "</decisionTable></decision></definitions>"
}

fn fired(xml: &str, rule: usize) -> (Vec<Option<Value>>, Vec<usize>) {
    let defs = parse::parse(xml).unwrap();
    let t = defs.table(0).unwrap();
    let w = gaps::witness(t, rule).unwrap();
    let ctx: Context = [("Region", &w[0]), ("Weight", &w[1])].into_iter().map(|(k, v)| (k.to_string(), v.clone().unwrap_or(Value::Null))).collect();
    let o = evaluate(t, &ctx);
    (w, o.fired)
}

#[test]
fn first_avoids_earlier_rules() {
    let x = table("FIRST", &[[r#""EU""#, "< 5"], [r#""EU""#, "< 10"], ["-", "-"]]);
    let (w, f) = fired(&x, 1);
    assert_eq!(f, vec![1], "{w:?}");
    assert_eq!(w, vec![Some(Value::Str("EU".into())), Some(Value::Num(5.0))]);
    let (w, f) = fired(&x, 2);
    assert_eq!(f, vec![2], "{w:?}");
}

#[test]
fn any_cells_stay_empty_when_nothing_needs_them() {
    let x = table("UNIQUE", &[[r#""EU""#, "-"], [r#""NA""#, "-"]]);
    let (w, f) = fired(&x, 1);
    assert_eq!(f, vec![1]);
    assert_eq!(w, vec![Some(Value::Str("NA".into())), None]);
}

#[test]
fn not_cells_get_a_readable_other_value() {
    let x = table("UNIQUE", &[[r#""EU""#, "-"], [r#"not("EU")"#, "-"]]);
    let (w, f) = fired(&x, 1);
    assert_eq!(f, vec![1]);
    assert_eq!(w[0], Some(Value::Str("other".into())));
}

#[test]
fn an_unreachable_rule_still_gets_matching_values() {
    let x = table("FIRST", &[["-", "< 10"], [r#""EU""#, "< 5"]]);
    let (w, f) = fired(&x, 1);
    assert_eq!(f, vec![0]);
    assert_eq!(w[0], Some(Value::Str("EU".into())));
}

#[test]
fn every_example_rule_fires_through_the_test_session() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join("shipping-fees.dmn");
    let mut ws = dmn_viewer::workspace::Workspace::open(path).unwrap();
    assert!(!ws.tests().suite().cases.is_empty());
    let n = ws.defs().table(0).unwrap().rules.len();
    for r in 0..n {
        let mut tests = ws.tests();
        tests.clear_inputs();
        assert!(tests.fill_to_fire(r));
        let o = tests.outcome().unwrap();
        assert_eq!(o.fired, vec![r], "rule {}: {:?} {:?}", r + 1, tests.inputs(), o.error);
    }
}

#[test]
fn rule_values_outside_the_allowed_list_still_fire() {
    let x = table("FIRST", &[[r#""C100""#, "-"], [r#""C200""#, "-"]]).replace(
        r#"<inputExpression id="ae" typeRef="string"><text>Region</text></inputExpression>"#,
        r#"<inputExpression id="ae" typeRef="string"><text>Region</text></inputExpression><inputValues><text>"alpha","beta"</text></inputValues>"#,
    );
    let (w, f) = fired(&x, 1);
    assert_eq!(f, vec![1], "{w:?}");
    assert_eq!(w[0], Some(Value::Str("C200".into())));
}
