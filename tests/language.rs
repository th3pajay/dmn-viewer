use dmn_viewer::model::{DecisionLogic, ExpressionLanguage, Language, LanguageSource};
use dmn_viewer::{lint, parse};
use std::path::PathBuf;

fn example() -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples").join("shipping-fees.dmn")).unwrap()
}

#[test]
fn cell_overrides_file_default() {
    let l = ExpressionLanguage::resolve(Some("JUEL"), Some("javascript"));
    assert_eq!((l.language, l.source), (Language::Juel, LanguageSource::Cell));
    let l = ExpressionLanguage::resolve(None, Some("javascript"));
    assert_eq!((l.language, l.source), (Language::Js, LanguageSource::FileDefault));
    let l = ExpressionLanguage::resolve(None, None);
    assert_eq!((l.language, l.source, l.declared), (Language::Feel, LanguageSource::Default, None));
    assert_eq!(ExpressionLanguage::resolve(Some("https://www.omg.org/spec/DMN/20191111/FEEL/"), None).language, Language::Feel);
}

#[test]
fn unsupported_language_keeps_its_name() {
    let l = ExpressionLanguage::resolve(Some("groovy"), None);
    assert_eq!(l.language, Language::Unsupported);
    assert_eq!(l.short_name(), "GROOVY");
}

#[test]
fn file_default_javascript_is_linted_as_javascript() {
    let xml = example().replacen("<definitions ", r#"<definitions expressionLanguage="javascript" "#, 1);
    let defs = parse::parse(&xml).unwrap();
    let DecisionLogic::Table(t) = &defs.decisions[0].logic else { panic!() };
    assert_eq!(t.rules[0].outputs[0].lang.source, LanguageSource::FileDefault);
    assert_eq!(t.rules[0].outputs[0].lang.language, Language::Js);
    let issues = lint::lint(&defs);
    let js = issues.iter().find(|i| i.code == "JAVASCRIPT").expect("JAVASCRIPT issue");
    assert!(js.rules.len() > 1, "{:?}", js.rules);
}
