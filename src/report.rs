//! One model of a check run (files, lint issues, test results and coverage), rendered as the
//! human-readable text report, JSON, JUnit XML or SARIF 2.1.0. `dmn-check` only parses its
//! arguments, builds a `Report` and picks a renderer.

use crate::lint::{self, Column, Issue, Severity};
use crate::model::{Decision, Definitions};
use crate::suite::{self, CaseResult, Coverage, Suite};
use crate::{parse, targets};
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::Path;

/// Which lint issues fail the run. Failing tests and unreadable files always do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FailOn {
    #[default]
    Error,
    Warn,
    Never,
}

impl FailOn {
    pub fn parse(s: &str) -> Option<FailOn> {
        match s.to_ascii_lowercase().as_str() {
            "error" => Some(FailOn::Error),
            "warn" | "warning" => Some(FailOn::Warn),
            "never" => Some(FailOn::Never),
            _ => None,
        }
    }

    pub fn fails(self, s: Severity) -> bool {
        match self {
            FailOn::Error => s == Severity::Error,
            FailOn::Warn => s <= Severity::Warning,
            FailOn::Never => false,
        }
    }
}

pub struct Report {
    pub files: Vec<FileReport>,
}

pub struct FileReport {
    /// As given on the command line, or as a pattern expanded it.
    pub path: String,
    /// `Err` when the file could not be read or parsed, or a pattern matched nothing.
    pub result: Result<Checked, String>,
}

pub struct Checked {
    pub defs: Definitions,
    pub issues: Vec<Issue>,
    pub tests: Option<SuiteReport>,
}

pub struct SuiteReport {
    pub path: String,
    /// `Err` holds the complete message: the suite file is unreadable or names no decision table.
    pub result: Result<SuiteRun, String>,
}

pub struct SuiteRun {
    pub decision: usize,
    pub suite: Suite,
    pub results: Vec<CaseResult>,
    pub coverage: Coverage,
}

impl SuiteRun {
    pub fn failed(&self) -> usize {
        self.results.iter().filter(|r| !r.pass).count()
    }
}

fn decision_name(d: &Decision) -> &str {
    if d.name.is_empty() { &d.id } else { &d.name }
}

impl Report {
    /// Checks every input: plain paths as given, directories and glob patterns expanded once.
    /// `tests` replaces the `<name>.tests.json` found next to each file.
    pub fn check(inputs: &[String], tests: Option<&Path>) -> Report {
        let mut seen = HashSet::new();
        let mut files = Vec::with_capacity(inputs.len());
        for a in inputs {
            match targets::expand(a) {
                None => {
                    seen.insert(a.clone());
                    files.push(check_file(a.clone(), tests));
                }
                Some(found) if found.is_empty() => files.push(FileReport { path: a.clone(), result: Err("no files match".into()) }),
                Some(found) => {
                    for f in found {
                        if seen.insert(f.clone()) {
                            files.push(check_file(f, tests));
                        }
                    }
                }
            }
        }
        Report { files }
    }

    pub fn failed(&self, fail_on: FailOn) -> bool {
        self.files.iter().any(|f| match &f.result {
            Err(_) => true,
            Ok(c) => c.issues.iter().any(|i| fail_on.fails(i.severity)) || c.tests.as_ref().is_some_and(|t| t.result.as_ref().map_or(true, |r| r.failed() > 0)),
        })
    }

    /// The human-readable report: problems reading files or suites go to `err`, the rest to `out`.
    pub fn write_text(&self, coverage: bool, out: &mut dyn Write, err: &mut dyn Write) -> io::Result<()> {
        for f in &self.files {
            let c = match &f.result {
                Ok(c) => c,
                Err(e) => {
                    writeln!(err, "{}: {e}", f.path)?;
                    continue;
                }
            };
            writeln!(out, "{}", lint::report(&c.defs, &c.issues, &f.path))?;
            let Some(t) = &c.tests else { continue };
            let run = match &t.result {
                Ok(r) => r,
                Err(e) => {
                    writeln!(err, "{e}")?;
                    continue;
                }
            };
            writeln!(out, "Tests: {}", t.path)?;
            for (case, r) in run.suite.cases.iter().zip(&run.results) {
                let fired = if r.outcome.fired.is_empty() { "-".to_string() } else { fired_list(&r.outcome.fired) };
                writeln!(out, "  {} {} [rules {fired}]", if r.pass { "PASS" } else { "FAIL" }, case.name)?;
                for p in &r.problems {
                    writeln!(out, "       {p}")?;
                }
            }
            let failed = run.failed();
            writeln!(out, "  {} passed, {failed} failed", run.results.len() - failed)?;
            if coverage {
                let cov = &run.coverage;
                writeln!(out, "  Coverage ({}): {}", decision_name(&c.defs.decisions[run.decision]), cov.summary())?;
                writeln!(out, "  Never fired: {}", cov.never_fired_label())?;
                let only = cov.only_failing();
                if !only.is_empty() {
                    writeln!(out, "  Only fired by failing cases: {}", lint::rules_label(&only))?;
                }
            }
            writeln!(out)?;
        }
        Ok(())
    }

    pub fn json(&self, fail_on: FailOn) -> String {
        let files = self
            .files
            .iter()
            .map(|f| {
                let mut jf = JFile { path: &f.path, error: None, name: None, summary: None, issues: vec![], tests: None };
                match &f.result {
                    Err(e) => jf.error = Some(e),
                    Ok(c) => {
                        let count = |s| c.issues.iter().filter(|i| i.severity == s).count();
                        jf.name = Some(if c.defs.name.is_empty() { &c.defs.id } else { &c.defs.name });
                        jf.summary = Some(JCounts { errors: count(Severity::Error), warnings: count(Severity::Warning), info: count(Severity::Info) });
                        jf.issues = c.issues.iter().map(|i| JIssue::new(&c.defs, i)).collect();
                        jf.tests = c.tests.as_ref().map(|t| JSuite::new(&c.defs, t));
                    }
                }
                jf
            })
            .collect();
        let report = JReport { tool: "dmn-check", version: env!("CARGO_PKG_VERSION"), failed: self.failed(fail_on), files };
        serde_json::to_string_pretty(&report).expect("the report serialises")
    }

    /// JUnit XML: per file, one test case per decision for its lint issues (failing when an issue
    /// meets `fail_on`); per suite, one test case per case. Coverage goes into suite properties.
    pub fn junit(&self, fail_on: FailOn) -> String {
        let estimate: usize = self
            .files
            .iter()
            .map(|f| f.result.as_ref().map_or(1, |c| c.issues.len() + c.defs.decisions.len() + c.tests.as_ref().and_then(|t| t.result.as_ref().ok()).map_or(1, |r| r.results.len())))
            .sum();
        let mut body = String::with_capacity(512 + 256 * estimate);
        let (mut tests, mut failures, mut errors) = (0, 0, 0);
        let mut suite = |body: &mut String, name: &str, props: &str, cases: &str, n: usize, f: usize, e: usize| {
            (tests, failures, errors) = (tests + n, failures + f, errors + e);
            let _ = write!(body, "  <testsuite name=\"{}\" tests=\"{n}\" failures=\"{f}\" errors=\"{e}\">\n{props}{cases}  </testsuite>\n", xml(name));
        };
        let mut cases = String::new();
        for file in &self.files {
            cases.clear();
            let c = match &file.result {
                Ok(c) => c,
                Err(e) => {
                    let _ = writeln!(cases, "    <testcase classname=\"{p}\" name=\"read and parse\">\n      <error message=\"{m}\">{m}</error>\n    </testcase>", p = xml(&file.path), m = xml(e));
                    suite(&mut body, &format!("lint: {}", file.path), "", &cases, 1, 0, 1);
                    continue;
                }
            };
            // One case per decision, plus one for file-level issues when there are any.
            let groups = (0..c.defs.decisions.len()).map(Some).chain(c.issues.iter().any(|i| i.decision.is_none()).then_some(None));
            let (mut n, mut failed) = (0, 0);
            for d in groups {
                n += 1;
                let name = d.map_or("file", |d| decision_name(&c.defs.decisions[d]));
                let (mut fails, mut fail_text, mut other_text) = (0, String::new(), String::new());
                for i in c.issues.iter().filter(|i| i.decision == d) {
                    let failing = fail_on.fails(i.severity);
                    fails += failing as usize;
                    let rules = if i.rules.is_empty() { String::new() } else { format!(" [rule {}]", lint::rules_label(&i.rules)) };
                    let _ = writeln!(if failing { &mut fail_text } else { &mut other_text }, "{} {}{rules}: {}", i.severity.label(), i.code, i.message);
                }
                let _ = writeln!(cases, "    <testcase classname=\"{}\" name=\"{}\">", xml(&file.path), xml(&format!("lint: {name}")));
                if fails > 0 {
                    failed += 1;
                    let _ = writeln!(cases, "      <failure message=\"{fails} issue{} at or above the fail-on level\" type=\"lint\">{}</failure>", if fails == 1 { "" } else { "s" }, xml(&fail_text));
                }
                if !other_text.is_empty() {
                    let _ = writeln!(cases, "      <system-out>{}</system-out>", xml(&other_text));
                }
                cases.push_str("    </testcase>\n");
            }
            suite(&mut body, &format!("lint: {}", file.path), "", &cases, n, failed, 0);

            let Some(t) = &c.tests else { continue };
            cases.clear();
            let run = match &t.result {
                Ok(r) => r,
                Err(e) => {
                    let _ = writeln!(cases, "    <testcase classname=\"{p}\" name=\"load\">\n      <error message=\"{m}\">{m}</error>\n    </testcase>", p = xml(&t.path), m = xml(e));
                    suite(&mut body, &format!("tests: {}", t.path), "", &cases, 1, 0, 1);
                    continue;
                }
            };
            let class = xml(decision_name(&c.defs.decisions[run.decision]));
            for (case, r) in run.suite.cases.iter().zip(&run.results) {
                let _ = write!(cases, "    <testcase classname=\"{class}\" name=\"{}\"", xml(&case.name));
                if r.pass {
                    cases.push_str("/>\n");
                } else {
                    let first = r.problems.first().map_or("failed", String::as_str);
                    let fired = if r.outcome.fired.is_empty() { "-".to_string() } else { fired_list(&r.outcome.fired) };
                    let _ = write!(cases, ">\n      <failure message=\"{}\" type=\"test\">{}\nfired rules: {fired}</failure>\n    </testcase>\n", xml(first), xml(&r.problems.join("\n")));
                }
            }
            let cov = &run.coverage;
            let props = format!(
                "    <properties>\n      <property name=\"coverage\" value=\"{}\"/>\n      <property name=\"coverage.covered\" value=\"{}\"/>\n      <property name=\"coverage.total\" value=\"{}\"/>\n      <property name=\"coverage.neverFired\" value=\"{}\"/>\n    </properties>\n",
                xml(&cov.summary()),
                cov.covered(),
                cov.total(),
                xml(&cov.never_fired_label())
            );
            suite(&mut body, &format!("tests: {}", t.path), &props, &cases, run.results.len(), run.failed(), 0);
        }
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites name=\"dmn-check\" tests=\"{tests}\" failures=\"{failures}\" errors=\"{errors}\">\n{body}</testsuites>\n")
    }

    /// SARIF 2.1.0 with one result per lint issue (the lint code is the rule id) and one per
    /// unreadable file. Issues tied to rules point at the first rule's line; the others name
    /// their decision, rules and column in the message instead. Test results are not included.
    pub fn sarif(&self) -> String {
        const FILE_ERROR: &str = "FILE_ERROR";
        let mut rules: Vec<SRule> = Vec::new();
        let mut rule_index = |id: &'static str, title: &'static str| {
            rules.iter().position(|r| r.id == id).unwrap_or_else(|| {
                rules.push(SRule { id, short_description: SText { text: Cow::Borrowed(title) } });
                rules.len() - 1
            })
        };
        let mut results = Vec::new();
        for f in &self.files {
            let uri = uri(&f.path);
            let location =
                |line: Option<u32>| [SLocation { physical_location: SPhysical { artifact_location: SArtifact { uri: uri.clone() }, region: line.map(|start_line| SRegion { start_line }) } }];
            match &f.result {
                Err(e) => results.push(SResult {
                    rule_id: FILE_ERROR,
                    rule_index: rule_index(FILE_ERROR, "File could not be read or parsed"),
                    level: "error",
                    message: SText { text: Cow::Borrowed(e) },
                    locations: location(None),
                }),
                Ok(c) => {
                    for i in &c.issues {
                        let at = Located::new(&c.defs, i);
                        let text = match at.line {
                            Some(_) => Cow::Borrowed(i.message.as_str()),
                            None => at.describe().map_or(Cow::Borrowed(i.message.as_str()), |w| Cow::Owned(format!("{} ({w})", i.message))),
                        };
                        let level = match i.severity {
                            Severity::Error => "error",
                            Severity::Warning => "warning",
                            Severity::Info => "note",
                        };
                        results.push(SResult { rule_id: i.code, rule_index: rule_index(i.code, i.title()), level, message: SText { text }, locations: location(at.line) });
                    }
                }
            }
        }
        let driver = SDriver { name: "dmn-check", version: env!("CARGO_PKG_VERSION"), rules };
        let log = Sarif { schema: "https://json.schemastore.org/sarif-2.1.0.json", version: "2.1.0", runs: [SRun { tool: STool { driver }, results }] };
        serde_json::to_string_pretty(&log).expect("the report serialises")
    }
}

fn check_file(path: String, tests: Option<&Path>) -> FileReport {
    let result = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|xml| parse::parse(&xml)).map(|defs| {
        let issues = lint::lint(&defs);
        let auto = suite::default_path(Path::new(&path));
        let suite_path = tests.map(Path::to_path_buf).or_else(|| auto.exists().then_some(auto));
        let tests = suite_path.map(|p| run_suite(&defs, &p));
        Checked { defs, issues, tests }
    });
    FileReport { path, result }
}

fn run_suite(defs: &Definitions, path: &Path) -> SuiteReport {
    let shown = path.display().to_string();
    let result = suite::load(path).and_then(|s| {
        let Some((decision, t)) = suite::find_table(defs, &s.decision) else {
            return Err(format!("{shown}: decision table \"{}\" not found", s.decision));
        };
        let results: Vec<_> = s.cases.iter().map(|c| suite::run_case(t, c)).collect();
        let coverage = Coverage::new(t.rules.len(), &results);
        Ok(SuiteRun { decision, suite: s, results, coverage })
    });
    SuiteReport { path: shown, result }
}

/// "1,4,5": 1-based rule numbers as the text report lists fired rules.
fn fired_list(rules: &[usize]) -> String {
    let mut s = String::with_capacity(rules.len() * 3);
    for (k, r) in rules.iter().enumerate() {
        let _ = write!(s, "{}{}", if k > 0 { "," } else { "" }, r + 1);
    }
    s
}

/// Escapes text for XML content and attribute values, dropping characters XML 1.0 cannot hold.
fn xml(s: &str) -> Cow<'_, str> {
    if !s.contains(|c: char| matches!(c, '&' | '<' | '>' | '"' | '\'') || (c < ' ' && !matches!(c, '\t' | '\n' | '\r'))) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c < ' ' && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// A relative path as a relative URI reference; an absolute one as a `file://` URI.
fn uri(path: &str) -> String {
    let p = path.replace('\\', "/");
    let p = p.strip_prefix("./").unwrap_or(&p);
    let drive = p.len() >= 2 && p.as_bytes()[1] == b':' && p.as_bytes()[0].is_ascii_alphabetic();
    let mut s = String::with_capacity(p.len() + 8);
    if drive {
        s.push_str("file:///");
    } else if p.starts_with('/') {
        s.push_str("file://");
    }
    for (k, b) in p.bytes().enumerate() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) || (drive && k == 1) {
            s.push(b as char);
        } else {
            let _ = write!(s, "%{b:02X}");
        }
    }
    s
}

/// Where an issue is, in terms a reader can find without a line number.
struct Located<'a> {
    decision: Option<&'a str>,
    rules: Vec<usize>,
    column: Option<JColumn<'a>>,
    line: Option<u32>,
}

impl<'a> Located<'a> {
    fn new(defs: &'a Definitions, i: &Issue) -> Located<'a> {
        let dec = i.decision.and_then(|d| defs.decisions.get(d));
        let table = i.decision.and_then(|d| defs.table(d));
        let mut rules = i.rules.clone();
        rules.sort_unstable();
        rules.dedup();
        let column = match (i.column, table) {
            (Some(Column::Input(c)), Some(t)) => t.inputs.get(c).map(|x| JColumn { kind: "input", index: c + 1, name: x.title() }),
            (Some(Column::Output(c)), Some(t)) => t.outputs.get(c).map(|x| JColumn { kind: "output", index: c + 1, name: x.title() }),
            _ => None,
        };
        let line = rules.first().and_then(|&r| table?.rules.get(r)).map(|r| r.line).filter(|&l| l > 0);
        Located { decision: dec.map(decision_name), rules, column, line }
    }

    /// "decision Shipping Fee, rules 3-4, input column Weight", or `None` when there is nothing to say.
    fn describe(&self) -> Option<String> {
        let mut parts = Vec::with_capacity(3);
        if let Some(d) = self.decision {
            parts.push(format!("decision {d}"));
        }
        if !self.rules.is_empty() {
            parts.push(format!("{} {}", if self.rules.len() == 1 { "rule" } else { "rules" }, lint::rules_label(&self.rules)));
        }
        if let Some(c) = &self.column {
            parts.push(format!("{} column {}", c.kind, c.name));
        }
        (!parts.is_empty()).then(|| parts.join(", "))
    }
}

// JSON: borrowed views of the report, serialised without copying the messages.

#[derive(Serialize)]
struct JReport<'a> {
    tool: &'static str,
    version: &'static str,
    failed: bool,
    files: Vec<JFile<'a>>,
}

#[derive(Serialize)]
struct JFile<'a> {
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<JCounts>,
    issues: Vec<JIssue<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tests: Option<JSuite<'a>>,
}

#[derive(Serialize)]
struct JCounts {
    errors: usize,
    warnings: usize,
    info: usize,
}

#[derive(Serialize)]
struct JIssue<'a> {
    severity: &'static str,
    code: &'static str,
    title: &'static str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<&'a str>,
    /// 1-based.
    rules: Vec<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<JColumn<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
}

impl<'a> JIssue<'a> {
    fn new(defs: &'a Definitions, i: &'a Issue) -> JIssue<'a> {
        let at = Located::new(defs, i);
        let severity = match i.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        JIssue { severity, code: i.code, title: i.title(), message: &i.message, decision: at.decision, rules: at.rules.iter().map(|r| r + 1).collect(), column: at.column, line: at.line }
    }
}

#[derive(Serialize)]
struct JColumn<'a> {
    kind: &'static str,
    /// 1-based among the table's inputs or outputs.
    index: usize,
    name: &'a str,
}

#[derive(Serialize)]
struct JSuite<'a> {
    path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<&'a str>,
    passed: usize,
    failed: usize,
    cases: Vec<JCase<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coverage: Option<JCoverage>,
}

impl<'a> JSuite<'a> {
    fn new(defs: &'a Definitions, t: &'a SuiteReport) -> JSuite<'a> {
        let mut js = JSuite { path: &t.path, error: None, decision: None, passed: 0, failed: 0, cases: vec![], coverage: None };
        match &t.result {
            Err(e) => js.error = Some(e),
            Ok(run) => {
                js.decision = Some(decision_name(&defs.decisions[run.decision]));
                js.failed = run.failed();
                js.passed = run.results.len() - js.failed;
                js.cases = run
                    .suite
                    .cases
                    .iter()
                    .zip(&run.results)
                    .map(|(c, r)| JCase { name: &c.name, pass: r.pass, fired: r.outcome.fired.iter().map(|x| x + 1).collect(), problems: &r.problems })
                    .collect();
                let cov = &run.coverage;
                let one_based = |v: Vec<usize>| v.into_iter().map(|r| r + 1).collect();
                js.coverage = Some(JCoverage { rules: cov.total(), covered: cov.covered(), never_fired: one_based(cov.never_fired()), only_failing: one_based(cov.only_failing()) });
            }
        }
        js
    }
}

#[derive(Serialize)]
struct JCase<'a> {
    name: &'a str,
    pass: bool,
    fired: Vec<usize>,
    problems: &'a [String],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JCoverage {
    rules: usize,
    covered: usize,
    never_fired: Vec<usize>,
    only_failing: Vec<usize>,
}

// SARIF 2.1.0: the subset the GitHub and Azure DevOps viewers read.

#[derive(Serialize)]
struct Sarif<'a> {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: [SRun<'a>; 1],
}

#[derive(Serialize)]
struct SRun<'a> {
    tool: STool,
    results: Vec<SResult<'a>>,
}

#[derive(Serialize)]
struct STool {
    driver: SDriver,
}

#[derive(Serialize)]
struct SDriver {
    name: &'static str,
    version: &'static str,
    rules: Vec<SRule>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SRule {
    id: &'static str,
    short_description: SText<'static>,
}

#[derive(Serialize)]
struct SText<'a> {
    text: Cow<'a, str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SResult<'a> {
    rule_id: &'static str,
    rule_index: usize,
    level: &'static str,
    message: SText<'a>,
    locations: [SLocation; 1],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SLocation {
    physical_location: SPhysical,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SPhysical {
    artifact_location: SArtifact,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<SRegion>,
}

#[derive(Serialize)]
struct SArtifact {
    uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SRegion {
    start_line: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris() {
        assert_eq!(uri("examples\\shipping fees.dmn"), "examples/shipping%20fees.dmn");
        assert_eq!(uri("./a/b.dmn"), "a/b.dmn");
        assert_eq!(uri("C:\\x\\y.dmn"), "file:///C:/x/y.dmn");
        assert_eq!(uri("/srv/ä.dmn"), "file:///srv/%C3%A4.dmn");
    }

    #[test]
    fn xml_escaping() {
        assert_eq!(xml("plain"), "plain");
        assert!(matches!(xml("plain"), Cow::Borrowed(_)));
        assert_eq!(xml("a<b & \"c\" 'd'>\u{1}\n"), "a&lt;b &amp; &quot;c&quot; &apos;d&apos;&gt;\n");
    }
}
