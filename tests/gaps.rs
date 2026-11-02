use dmn_viewer::{fix, gaps, lint, parse};

fn table(hp: &str, inputs: &[(&str, Option<&str>)], rules: &[&[&str]]) -> String {
    let mut x = format!(r#"<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="d" name="d" namespace="x"><decision id="D" name="D"><decisionTable id="T" hitPolicy="{hp}">"#);
    for (i, (name, allowed)) in inputs.iter().enumerate() {
        x += &format!(r#"<input id="I{i}" label="{name}"><inputExpression id="E{i}"><text>{name}</text></inputExpression>"#);
        if let Some(a) = allowed {
            x += &format!("<inputValues><text>{a}</text></inputValues>");
        }
        x += "</input>";
    }
    x += r#"<output id="O" name="Out" typeRef="string"/>"#;
    for (r, cells) in rules.iter().enumerate() {
        x += &format!(r#"<rule id="R{r}">"#);
        for (c, cell) in cells.iter().enumerate() {
            x += &format!(r#"<inputEntry id="R{r}I{c}"><text>{}</text></inputEntry>"#, cell.replace('<', "&lt;").replace('>', "&gt;"));
        }
        x += &format!(r#"<outputEntry id="R{r}O"><text>"r{r}"</text></outputEntry></rule>"#);
    }
    x + "</decisionTable></decision></definitions>"
}

fn gaps_of(xml: &str) -> Option<Vec<Vec<Option<String>>>> {
    let defs = parse::parse(xml).unwrap();
    gaps::gaps(defs.table(0).unwrap(), 10).map(|g| g.into_iter().map(|g| g.cells).collect())
}

fn codes(xml: &str) -> Vec<&'static str> {
    lint::lint(&parse::parse(xml).unwrap()).into_iter().map(|i| i.code).collect()
}

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

#[test]
fn finds_the_numbers_between_two_ranges() {
    let x = table("UNIQUE", &[("N", None)], &[&["< 5"], &["> 10"]]);
    assert_eq!(gaps_of(&x), Some(vec![vec![s("[5..10]")]]));
    assert!(codes(&x).contains(&"INCOMPLETE_TABLE") && !codes(&x).contains(&"NO_DEFAULT_RULE"));
}

#[test]
fn open_and_closed_bounds_follow_the_cells() {
    let x = table("FIRST", &[("N", None)], &[&["<= 5"], &["[7..9["], &[">= 9"]]);
    assert_eq!(gaps_of(&x), Some(vec![vec![s("(5..7)")]]));
    let x = table("FIRST", &[("N", None)], &[&["< 5"], &["> 5"]]);
    assert_eq!(gaps_of(&x), Some(vec![vec![s("5")]]));
}

#[test]
fn finds_strings_no_rule_lists() {
    let x = table("UNIQUE", &[("Region", None)], &[&[r#""EU""#], &[r#""NA""#]]);
    assert_eq!(gaps_of(&x), Some(vec![vec![s(r#"not("EU", "NA")"#)]]));
}

#[test]
fn allowed_values_close_the_domain() {
    let x = table("UNIQUE", &[("Region", Some(r#""EU","NA""#))], &[&[r#""EU""#], &[r#""NA""#]]);
    assert_eq!(gaps_of(&x), Some(vec![]));
    assert!(!codes(&x).contains(&"NO_DEFAULT_RULE") && !codes(&x).contains(&"INCOMPLETE_TABLE"));
    let x = table("UNIQUE", &[("Region", Some(r#""EU","NA","APAC""#))], &[&[r#""EU""#], &[r#""NA""#]]);
    assert_eq!(gaps_of(&x), Some(vec![vec![s(r#""APAC""#)]]));
}

#[test]
fn rule_values_outside_the_allowed_list_still_count() {
    let x = table("FIRST", &[("Client", Some(r#""alpha","beta""#))], &[&[r#""C100""#], &[r#""alpha""#]]);
    assert_eq!(gaps_of(&x), Some(vec![vec![s(r#""beta""#)]]));
    assert!(codes(&x).contains(&"NOT_IN_ALLOWED_VALUES"));
}

#[test]
fn a_gap_leaves_unconstrained_columns_free() {
    let x = table("FIRST", &[("Region", None), ("Weight", None)], &[&[r#""EU""#, "< 5"], &[r#""EU""#, ">= 5"], &[r#""NA""#, "-"]]);
    assert_eq!(gaps_of(&x), Some(vec![vec![s(r#"not("EU", "NA")"#), None]]));
    let x = table("FIRST", &[("Region", None), ("Weight", None)], &[&[r#""EU""#, "< 5"], &[r#"not("EU")"#, "-"]]);
    assert_eq!(gaps_of(&x), Some(vec![vec![s(r#""EU""#), s(">= 5")]]));
}

#[test]
fn a_complete_table_has_no_gap_and_no_catch_all_notice() {
    let x = table("UNIQUE", &[("N", None), ("B", None)], &[&["< 0", "true"], &["< 0", "false"], &[">= 0", "-"]]);
    assert_eq!(gaps_of(&x), Some(vec![]));
    assert!(!codes(&x).contains(&"NO_DEFAULT_RULE"));
}

#[test]
fn cells_it_cannot_analyse_keep_the_catch_all_notice() {
    let x = table("UNIQUE", &[("N", None)], &[&["< Limit"], &[">= 5"]]);
    assert_eq!(gaps_of(&x), None);
    assert!(codes(&x).contains(&"NO_DEFAULT_RULE"));
    let x = table("UNIQUE", &[("Region", None)], &[&[r#"< "M""#], &[r#""Z""#]]);
    assert_eq!(gaps_of(&x), None);
}

#[test]
fn multi_hit_tables_are_not_checked() {
    let x = table("COLLECT", &[("N", None)], &[&["< 5"]]);
    assert!(!codes(&x).contains(&"INCOMPLETE_TABLE"));
}

#[test]
fn the_fix_adds_a_rule_that_closes_the_gap() {
    let x = table("UNIQUE", &[("Region", None), ("Weight", None)], &[&[r#""EU""#, "< 5"], &[r#""EU""#, "> 10"], &[r#"not("EU")"#, "-"]]);
    let defs = parse::parse(&x).unwrap();
    let issue = lint::lint(&defs).into_iter().find(|i| i.code == "INCOMPLETE_TABLE").unwrap();
    assert!(issue.message.contains(r#"Region: "EU", Weight: [5..10]"#), "{}", issue.message);
    let f = fix::fixes(&defs, &issue);
    assert_eq!(f.len(), 1);
    assert!(!f[0].destructive);
    let after = parse::parse(&fix::apply(&x, &f[0].edits).unwrap()).unwrap();
    assert_eq!(after.table(0).unwrap().rules.len(), 4);
    assert_eq!(gaps::gaps(after.table(0).unwrap(), 10), Some(vec![]));
    assert!(!lint::lint(&after).iter().any(|i| i.code == "INCOMPLETE_TABLE" || i.code == "OVERLAPPING_RULES"));
}

#[test]
fn a_big_table_stays_within_budget() {
    let rows: Vec<String> = (0..400).map(|i| format!("[{}..{}[", i * 10, i * 10 + 10)).collect();
    let cells: Vec<[&str; 2]> = rows.iter().map(|r| [r.as_str(), "-"]).collect();
    let rules: Vec<&[&str]> = cells.iter().map(|c| c.as_slice()).collect();
    let x = table("UNIQUE", &[("N", None), ("M", None)], &rules);
    assert_eq!(gaps_of(&x), Some(vec![vec![s("< 0, >= 4000"), None]]));
}

#[test]
fn agrees_with_brute_force_on_random_tables() {
    use dmn_viewer::eval::{Context, Value};
    use dmn_viewer::model::ExpressionLanguage;
    use dmn_viewer::unary::UnaryTests;
    let mut seed = 0x2545F4914F6CDD1Du64;
    let mut rnd = |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n) as usize
    };
    let nums: Vec<Value> = (-2..=40).map(|i| Value::Num(i as f64 / 2.0)).collect();
    let strs: Vec<Value> = ["A", "B", "C", "Z"].iter().map(|s| Value::Str(s.to_string())).collect();
    let lang = ExpressionLanguage::resolve(None, None);
    let ctx = Context::new();
    for _ in 0..300 {
        let rules: Vec<[String; 2]> = (0..1 + rnd(5))
            .map(|_| {
                let a = rnd(10);
                let b = a + 1 + rnd(4);
                let num = match rnd(6) {
                    0 => "-".to_string(),
                    1 => format!("< {a}"),
                    2 => format!(">= {a}"),
                    3 => format!("[{a}..{b}["),
                    4 => format!("]{a}..{b}]"),
                    _ => format!("{a}, > {b}"),
                };
                let s = ["\"A\"", "\"B\"", "\"A\",\"C\"", "not(\"B\")", "-"][rnd(5)].to_string();
                [num, s]
            })
            .collect();
        let refs: Vec<[&str; 2]> = rules.iter().map(|r| [r[0].as_str(), r[1].as_str()]).collect();
        let rows: Vec<&[&str]> = refs.iter().map(|r| r.as_slice()).collect();
        let x = table("FIRST", &[("N", None), ("S", None)], &rows);
        let defs = parse::parse(&x).unwrap();
        let t = defs.table(0).unwrap();
        let covered = |n: &Value, s: &Value| t.rules.iter().any(|r| r.inputs[0].tests.matches(n, &ctx).unwrap() && r.inputs[1].tests.matches(s, &ctx).unwrap());
        let open: Vec<(&Value, &Value)> = nums.iter().flat_map(|n| strs.iter().map(move |s| (n, s))).filter(|(n, s)| !covered(n, s)).collect();
        let found = gaps::gaps(t, 50).expect("analysable");
        assert_eq!(found.is_empty(), open.is_empty(), "{rules:?} {found:?}");
        for g in &found {
            let tests: Vec<UnaryTests> = g.cells.iter().map(|c| UnaryTests::parse(c.as_deref().unwrap_or("-"), &lang)).collect();
            let inside = |n: &Value, s: &Value| tests[0].matches(n, &ctx).unwrap() && tests[1].matches(s, &ctx).unwrap();
            assert!(open.iter().any(|(n, s)| inside(n, s)), "{rules:?} {g:?}");
            assert!(nums.iter().flat_map(|n| strs.iter().map(move |s| (n, s))).filter(|(n, s)| inside(n, s)).all(|(n, s)| !covered(n, s)), "{rules:?} {g:?}");
        }
    }
}

#[test]
fn reads_the_dmn_version_from_the_namespace() {
    for (ns, v) in [
        ("http://www.omg.org/spec/DMN/20151101/dmn.xsd", Some("1.1")),
        ("http://www.omg.org/spec/DMN/20180521/MODEL/", Some("1.2")),
        ("https://www.omg.org/spec/DMN/20191111/MODEL/", Some("1.3")),
        ("https://www.omg.org/spec/DMN/20211108/MODEL/", Some("1.4")),
        ("https://www.omg.org/spec/DMN/20230324/MODEL/", Some("1.5")),
        ("urn:other", None),
    ] {
        let defs = parse::parse(&format!(r#"<dmn:definitions xmlns:dmn="{ns}" id="d"/>"#)).unwrap();
        assert_eq!(defs.dmn_version, v, "{ns}");
    }
}
