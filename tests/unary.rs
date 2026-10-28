use dmn_viewer::eval::{Context, Value};
use dmn_viewer::model::ExpressionLanguage;
use dmn_viewer::unary::UnaryTests;

fn feel(text: &str) -> UnaryTests {
    UnaryTests::parse(text, &ExpressionLanguage::default())
}

fn lang(text: &str, l: &str) -> UnaryTests {
    UnaryTests::parse(text, &ExpressionLanguage::resolve(Some(l), None))
}

fn check(t: &UnaryTests, v: Value) -> bool {
    t.matches(&v, &Context::new()).unwrap()
}

fn num(n: f64) -> Value {
    Value::Num(n)
}

fn s(x: &str) -> Value {
    Value::Str(x.into())
}

#[test]
fn any_matches_everything() {
    for text in ["", "  ", "-", " - "] {
        let t = feel(text);
        assert!(t.is_any());
        assert!(check(&t, Value::Null));
        assert!(t.literals().is_none() && t.sample().is_none());
    }
}

#[test]
fn literal_lists() {
    let t = feel(r#""EU", "NA""#);
    assert!(check(&t, s("NA")) && !check(&t, s("APAC")) && !check(&t, Value::Null));
    let l = t.literals().unwrap();
    assert_eq!((l.negated, l.items.clone()), (false, vec![r#""EU""#.to_string(), r#""NA""#.to_string()]));
    assert_eq!(t.sample().as_deref(), Some("EU"));
}

#[test]
fn negated_lists() {
    let t = feel(r#"not("EU","NA")"#);
    assert!(!check(&t, s("EU")) && check(&t, s("APAC")) && check(&t, Value::Null));
    assert!(t.literals().unwrap().negated);
    assert_eq!(t.sample().as_deref(), Some(""));
}

#[test]
fn comparisons_and_ranges() {
    let le = feel("<= 5");
    assert!(check(&le, num(5.0)) && !check(&le, num(5.5)));
    assert!(check(&feel("!= 3"), num(4.0)));
    let closed_open = feel("[1..10[");
    assert!(check(&closed_open, num(1.0)) && check(&closed_open, num(9.9)) && !check(&closed_open, num(10.0)));
    let open = feel("]1..10]");
    assert!(!check(&open, num(1.0)) && check(&open, num(10.0)));
    assert!(!check(&feel("[1..10]"), s("x")));
    assert!(feel("<= 5").literals().is_none() && feel("<= 5").sample().is_none());
}

#[test]
fn null_and_booleans() {
    assert!(check(&feel("null"), Value::Null) && !check(&feel("null"), num(0.0)));
    assert!(check(&feel("true"), Value::Bool(true)) && !check(&feel("true"), Value::Bool(false)));
}

#[test]
fn expressions_see_the_input_as_question_mark() {
    let t = feel("? * 2 > 10");
    assert!(check(&t, num(6.0)) && !check(&t, num(5.0)));
    let t = feel(r#"starts with(?, "E")"#);
    assert!(check(&t, s("EU")) && !check(&t, s("NA")));
}

#[test]
fn expressions_see_variables() {
    let vars = Context::from([("limit".to_string(), num(3.0))]);
    assert!(feel("< limit").matches(&num(2.0), &vars).unwrap());
    assert!(!feel("< limit").matches(&num(4.0), &vars).unwrap());
}

#[test]
fn script_languages_test_cell_input() {
    let t = lang(r#"cellInput != null && cellInput.startsWith("E")"#, "javascript");
    assert!(check(&t, s("EU")) && !check(&t, Value::Null));
    assert!(t.literals().is_none());
    assert!(check(&lang("cellInput > 3", "juel"), num(4.0)));
}

#[test]
fn errors_surface_when_matching() {
    let unsupported = lang("x > 1", "groovy");
    assert_eq!(unsupported.matches(&num(1.0), &Context::new()), Err("expression language \"groovy\" is not supported by the tester".into()));
    assert!(lang("-", "groovy").is_any());
    assert!(feel("< \"unterminated").matches(&num(1.0), &Context::new()).is_err());
    // An earlier item that matches short-circuits a later broken one.
    let t = feel(r#""EU", < "oops"#);
    assert!(check(&t, s("EU")));
    assert!(t.matches(&s("NA"), &Context::new()).is_err());
}
