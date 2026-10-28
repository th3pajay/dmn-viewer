use dmn_viewer::lint::{self, Severity};
use dmn_viewer::report::{FailOn, Report};
use dmn_viewer::targets::{expand, matches};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn example(name: &str) -> String {
    format!("{}/examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// A fresh directory with copies of the example, so tests can add broken files next to it.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dmn-viewer-report-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(r: &Report, coverage: bool) -> (String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    r.write_text(coverage, &mut out, &mut err).unwrap();
    (String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// The example with one failing case: rule 4's case expects a different fee.
fn failing_example(dir: &Path) -> String {
    let dmn = dir.join("fees.dmn");
    std::fs::copy(example("shipping-fees.dmn"), &dmn).unwrap();
    let suite = std::fs::read_to_string(example("shipping-fees.tests.json")).unwrap();
    std::fs::write(dir.join("fees.tests.json"), suite.replace("\"Fee\": 19.5", "\"Fee\": 1")).unwrap();
    dmn.to_string_lossy().into_owned()
}

#[test]
fn text_is_the_classic_report() {
    let path = example("shipping-fees.dmn");
    let r = Report::check(std::slice::from_ref(&path), None);
    let (out, err) = text(&r, false);
    let defs = dmn_viewer::parse::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let lint = lint::report(&defs, &lint::lint(&defs), &path);
    assert!(out.starts_with(&format!("{lint}\nTests: ")), "{out}");
    assert!(out.contains("\n  PASS EU small parcel [rules 1]\n"));
    assert!(out.contains("\n  PASS Missing OrderDate breaks the JavaScript label [rules -]\n"));
    assert!(out.ends_with("\n  9 passed, 0 failed\n\n"));
    assert!(err.is_empty());
    assert!(!r.failed(FailOn::Error));

    let (with, _) = text(&r, true);
    assert!(with.ends_with("\n  9 passed, 0 failed\n  Coverage (Shipping Fee): 7/7 rules covered (100%)\n  Never fired: none\n\n"), "{with}");
}

#[test]
fn text_sends_unreadable_files_and_suites_to_err() {
    let dir = scratch("errors");
    let bad = dir.join("bad.dmn");
    std::fs::write(&bad, "<definitions").unwrap();
    let r = Report::check(&[bad.to_string_lossy().into_owned(), example("shipping-fees.dmn")], Some(&dir.join("none.json")));
    let (out, err) = text(&r, false);
    assert!(err.starts_with(&format!("{}: ", bad.display())), "{err}");
    assert!(err.lines().nth(1).unwrap().contains("none.json"), "{err}");
    assert!(out.starts_with("DMN check: "));
    assert!(r.failed(FailOn::Never), "unreadable files and suites fail whatever --fail-on says");
}

#[test]
fn fail_on_levels() {
    assert_eq!(FailOn::parse("WARN"), Some(FailOn::Warn));
    assert_eq!(FailOn::parse("bogus"), None);
    assert!(FailOn::Error.fails(Severity::Error) && !FailOn::Error.fails(Severity::Warning));
    assert!(FailOn::Warn.fails(Severity::Warning) && !FailOn::Warn.fails(Severity::Info));
    assert!(!FailOn::Never.fails(Severity::Error));
    // The example has warnings but no errors.
    let r = Report::check(&[example("shipping-fees.dmn")], None);
    assert!(!r.failed(FailOn::Error) && r.failed(FailOn::Warn) && !r.failed(FailOn::Never));
}

#[test]
fn json_has_issues_tests_and_coverage() {
    let dir = scratch("json");
    let r = Report::check(&[failing_example(&dir)], None);
    let j: Value = serde_json::from_str(&r.json(FailOn::Error)).unwrap();
    assert_eq!(j["failed"], true);
    let f = &j["files"][0];
    assert_eq!(f["summary"]["warnings"], 2);
    let js = f["issues"].as_array().unwrap().iter().find(|i| i["code"] == "JAVASCRIPT").unwrap();
    assert_eq!((js["severity"].as_str(), js["decision"].as_str()), (Some("warning"), Some("Shipping Fee")));
    assert_eq!(js["rules"], serde_json::json!([3, 4, 5]), "rules are 1-based");
    assert!(js["line"].as_u64().unwrap() > 1);
    let only = f["issues"].as_array().unwrap().iter().find(|i| i["code"] == "INPUT_ONLY_IN_OUTPUT").unwrap();
    assert_eq!(only["column"]["kind"], "input");
    assert!(only.get("line").is_none());
    let t = &f["tests"];
    assert_eq!((t["passed"].as_u64(), t["failed"].as_u64()), (Some(8), Some(1)));
    let failing = t["cases"].as_array().unwrap().iter().find(|c| c["pass"] == false).unwrap();
    assert_eq!(failing["name"], "NA express");
    assert_eq!(failing["fired"], serde_json::json!([4]));
    assert_eq!(t["coverage"]["covered"], 7);
    assert_eq!(t["coverage"]["onlyFailing"], serde_json::json!([4]));
    assert_eq!(t["coverage"]["neverFired"], serde_json::json!([]));
}

#[test]
fn junit_is_well_formed_and_follows_fail_on() {
    let dir = scratch("junit");
    let bad = dir.join("broken.dmn");
    std::fs::write(&bad, "not xml & <").unwrap();
    let r = Report::check(&[failing_example(&dir), bad.to_string_lossy().into_owned()], None);
    for (fail_on, lint_failures) in [(FailOn::Error, 0), (FailOn::Warn, 1)] {
        let x = r.junit(fail_on);
        let doc = roxmltree::Document::parse(&x).unwrap_or_else(|e| panic!("{e}\n{x}"));
        let root = doc.root_element();
        assert_eq!(root.tag_name().name(), "testsuites");
        // Lint: one case for the decision; tests: 9 cases; the broken file: one erroring case.
        assert_eq!(root.attribute("tests"), Some("11"));
        assert_eq!(root.attribute("failures"), Some((1 + lint_failures).to_string().as_str()));
        assert_eq!(root.attribute("errors"), Some("1"));
        let suites: Vec<_> = root.children().filter(|n| n.has_tag_name("testsuite")).collect();
        assert_eq!(suites.len(), 3);
        let tests = suites.iter().find(|s| s.attribute("name").unwrap().starts_with("tests: ")).unwrap();
        let failure = tests.descendants().find(|n| n.has_tag_name("failure")).unwrap();
        assert_eq!(failure.parent().unwrap().attribute("name"), Some("NA express"));
        assert!(failure.attribute("message").unwrap().starts_with("Fee: expected 1"));
        let props: Vec<_> = tests.descendants().filter(|n| n.has_tag_name("property")).map(|p| (p.attribute("name").unwrap(), p.attribute("value").unwrap())).collect();
        assert!(props.contains(&("coverage", "7/7 rules covered (100%), 1 only by failing cases")), "{props:?}");
        assert!(x.contains("expressionLanguage=&quot;javascript&quot;"), "messages are escaped");
    }
}

#[test]
fn sarif_uses_codes_files_and_lines() {
    let dir = scratch("sarif");
    let bad = dir.join("broken.dmn");
    std::fs::write(&bad, "<x>").unwrap();
    let r = Report::check(&[example("shipping-fees.dmn"), bad.to_string_lossy().into_owned()], None);
    let s: Value = serde_json::from_str(&r.sarif()).unwrap();
    assert_eq!(s["version"], "2.1.0");
    let run = &s["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "dmn-check");
    let rules = run["tool"]["driver"]["rules"].as_array().unwrap();
    let results = run["results"].as_array().unwrap();
    assert_eq!(results.len(), 4, "three issues and the broken file");
    for res in results {
        let idx = res["ruleIndex"].as_u64().unwrap() as usize;
        assert_eq!(rules[idx]["id"], res["ruleId"]);
    }
    let js = results.iter().find(|x| x["ruleId"] == "JAVASCRIPT").unwrap();
    assert_eq!(js["level"], "warning");
    let loc = &js["locations"][0]["physicalLocation"];
    assert!(loc["artifactLocation"]["uri"].as_str().unwrap().ends_with("examples/shipping-fees.dmn"));
    assert!(loc["region"]["startLine"].as_u64().unwrap() > 1);
    // No line: the message names where the issue is instead.
    let info = results.iter().find(|x| x["ruleId"] == "INPUT_ONLY_IN_OUTPUT").unwrap();
    assert_eq!(info["level"], "note");
    assert!(info["locations"][0]["physicalLocation"].get("region").is_none());
    assert!(info["message"]["text"].as_str().unwrap().ends_with("(decision Shipping Fee, input column OrderDate)"), "{}", info["message"]);
    let file = results.iter().find(|x| x["ruleId"] == "FILE_ERROR").unwrap();
    assert_eq!(file["level"], "error");
}

#[test]
fn globs_and_directories_expand_once() {
    let dir = scratch("globs");
    for f in ["a.dmn", "b.DMN", "notes.txt", "sub/c.dmn", "sub/deep/d.dmn", "sub/deep/e.xml", ".hidden/h.dmn"] {
        let p = dir.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "").unwrap();
    }
    let d = dir.to_string_lossy().replace('\\', "/");
    let names = |pattern: &str| expand(pattern).map(|v| v.into_iter().map(|p| p[d.len() + 1..].to_string()).collect::<Vec<_>>());
    let ci = cfg!(windows);
    let mut all = vec!["a.dmn", "sub/c.dmn", "sub/deep/d.dmn"];
    if ci {
        all.insert(1, "b.DMN");
    }
    assert_eq!(names(&d).unwrap(), all, "a directory means every .dmn below it, hidden ones excepted");
    assert_eq!(names(&format!("{d}/**/*.dmn")).unwrap(), all);
    assert_eq!(names(&format!("{d}/*/*.dmn")).unwrap(), vec!["sub/c.dmn"]);
    assert_eq!(names(&format!("{d}/sub/**")).unwrap(), vec!["sub/c.dmn", "sub/deep/d.dmn", "sub/deep/e.xml"]);
    assert_eq!(names(&format!("{d}/?.txt")).unwrap(), Vec::<String>::new());
    assert_eq!(names(&format!("{d}/.hidden/*.dmn")).unwrap(), vec![".hidden/h.dmn"]);
    assert_eq!(names(&format!("{d}/nothing*")).unwrap(), Vec::<String>::new());
    assert_eq!(expand(&format!("{d}/a.dmn")), None, "a plain file is used as given");
    assert_eq!(expand(&format!("{d}/missing.dmn")), None);

    let back = dir.to_string_lossy().replace('/', "\\");
    if ci {
        assert!(expand(&format!("{back}\\*.dmn")).unwrap().iter().all(|p| p.starts_with(&back) && !p[back.len()..].contains('/')), "backslash patterns keep backslashes");
    }

    // Overlapping patterns check each file once; a pattern without matches is reported.
    let r = Report::check(&[format!("{d}/a.dmn"), format!("{d}/**/*.dmn"), format!("{d}/*.nope")], None);
    let paths: Vec<_> = r.files.iter().map(|f| f.path[d.len() + 1..].to_string()).collect();
    let mut want = vec!["a.dmn".to_string()];
    want.extend(all[1..].iter().map(|s| s.to_string()));
    want.push("*.nope".into());
    assert_eq!(paths, want);
    assert_eq!(r.files.last().unwrap().result.as_ref().err().map(String::as_str), Some("no files match"));
}

#[test]
fn wildcard_matching() {
    assert!(matches("*.dmn", "a.dmn"));
    assert!(matches("a*b*c", "aXXbYYc"));
    assert!(matches("a?c", "abc") && !matches("a?c", "ac"));
    assert!(matches("*", "") && matches("**", "x"));
    assert!(!matches("*.dmn", "a.dmn.bak"));
    assert!(matches("*a*a", "banana"));
    assert_eq!(matches("*.DMN", "x.dmn"), cfg!(windows));
}
