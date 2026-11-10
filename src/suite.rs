use crate::eval::{Context, Outcome, Value, evaluate};
use crate::model::*;
use crate::verdict::{Expectation, check};
use serde::{Deserialize, Serialize};
use serde_json::Value as J;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Suite {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub decision: String,
    #[serde(default)]
    pub cases: Vec<Case>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Case {
    pub name: String,
    #[serde(default)]
    pub inputs: BTreeMap<String, J>,
    #[serde(flatten)]
    pub expect: Expectation,
}

pub struct CaseResult {
    pub pass: bool,
    pub problems: Vec<String>,
    pub outcome: Outcome,
}

pub fn json_to_value(j: &J, type_ref: Option<&str>) -> Value {
    match j {
        J::Null => Value::Null,
        J::String(s) => Value::Str(s.clone()),
        J::Bool(b) => Value::Bool(*b),
        J::Number(n) => {
            let f = n.as_f64().unwrap_or(f64::NAN);
            if type_ref.is_some_and(|t| t.eq_ignore_ascii_case("string")) { Value::Str(n.to_string()) } else { Value::Num(f) }
        }
        other => Value::Str(other.to_string()),
    }
}

pub fn value_to_json(v: &Value) -> J {
    match v {
        Value::Null => J::Null,
        Value::Str(s) => J::String(s.clone()),
        Value::Bool(b) => J::Bool(*b),
        Value::Num(n) => serde_json::Number::from_f64(*n).map(J::Number).unwrap_or(J::Null),
    }
}

pub fn bound<'a, V>(inp: &InputClause, values: &'a BTreeMap<String, V>) -> Option<&'a V> {
    values.get(inp.binding.as_str()).or_else(|| values.get(&inp.label))
}

pub fn context(t: &DecisionTable, case: &Case) -> Context {
    let mut ctx = Context::new();
    for (k, v) in &case.inputs {
        ctx.insert(k.clone(), json_to_value(v, None));
    }
    for inp in &t.inputs {
        let v = bound(inp, &case.inputs).map(|j| json_to_value(j, inp.type_ref.as_deref())).unwrap_or(Value::Null);
        ctx.insert(inp.binding.as_str().to_string(), v);
    }
    ctx
}

pub fn find_table<'a>(defs: &'a Definitions, name: &str) -> Option<(usize, &'a DecisionTable)> {
    defs.decisions.iter().enumerate().find_map(|(i, d)| match &d.logic {
        DecisionLogic::Table(t) if targets(name, d) => Some((i, t)),
        _ => None,
    })
}

pub struct SuiteRun {
    pub results: Vec<CaseResult>,
    pub coverage: Coverage,
}

pub const PARALLEL_WORK: usize = 50_000;

fn workers() -> usize {
    static WORKERS: OnceLock<usize> = OnceLock::new();
    *WORKERS.get_or_init(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
}

impl Suite {
    pub fn run(&self, t: &DecisionTable) -> SuiteRun {
        let n = self.cases.len();
        let threads = if n.saturating_mul(t.rules.len()) >= PARALLEL_WORK { workers().min(n) } else { 1 };
        let results: Vec<CaseResult> = if threads > 1 {
            std::thread::scope(|scope| {
                let handles: Vec<_> = self.cases.chunks(n.div_ceil(threads)).map(|cases| scope.spawn(move || cases.iter().map(|c| run_case(t, c)).collect::<Vec<_>>())).collect();
                handles.into_iter().flat_map(|h| h.join().expect("a case run panicked")).collect()
            })
        } else {
            self.cases.iter().map(|c| run_case(t, c)).collect()
        };
        let coverage = Coverage::new(t.rules.len(), &results);
        SuiteRun { results, coverage }
    }
}

pub fn run_case(t: &DecisionTable, case: &Case) -> CaseResult {
    let outcome = evaluate(t, &context(t, case));
    let v = check(t, &outcome, &case.expect);
    CaseResult { pass: v.pass(), problems: v.problems, outcome }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Coverage {
    pub fired_by: Vec<(u32, u32)>,
}

impl Coverage {
    pub fn new(rules: usize, results: &[CaseResult]) -> Coverage {
        let mut fired_by = vec![(0, 0); rules];
        for r in results {
            for &ri in &r.outcome.fired {
                if let Some(c) = fired_by.get_mut(ri) {
                    if r.pass { c.0 += 1 } else { c.1 += 1 }
                }
            }
        }
        Coverage { fired_by }
    }

    pub fn total(&self) -> usize {
        self.fired_by.len()
    }

    pub fn is_covered(&self, rule: usize) -> bool {
        self.fired_by.get(rule).is_some_and(|&(p, f)| p + f > 0)
    }

    pub fn covered(&self) -> usize {
        self.fired_by.iter().filter(|&&(p, f)| p + f > 0).count()
    }

    pub fn never_fired(&self) -> Vec<usize> {
        self.fired_by.iter().enumerate().filter(|(_, c)| c.0 + c.1 == 0).map(|(i, _)| i).collect()
    }

    pub fn only_failing(&self) -> Vec<usize> {
        self.fired_by.iter().enumerate().filter(|(_, c)| c.0 == 0 && c.1 > 0).map(|(i, _)| i).collect()
    }

    pub fn summary(&self) -> String {
        let (covered, total) = (self.covered(), self.total());
        let pct = (covered * 100).checked_div(total).unwrap_or(100);
        let failing = self.only_failing().len();
        let tail = if failing == 0 { String::new() } else { format!(", {failing} only by failing cases") };
        format!("{covered}/{total} rules covered ({pct}%){tail}")
    }

    pub fn never_fired_label(&self) -> String {
        let never = self.never_fired();
        if never.is_empty() { "none".into() } else { crate::lint::rules_label(&never) }
    }
}

pub fn targets(decision: &str, d: &Decision) -> bool {
    decision.is_empty() || d.id == decision || d.name == decision
}

pub fn case_from_outcome(name: String, t: &DecisionTable, inputs: BTreeMap<String, J>, outcome: &Outcome) -> Case {
    Case { name, inputs, expect: Expectation::of_outcome(t, outcome) }
}

pub fn load(path: &Path) -> Result<Suite, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn save(path: &Path, suite: &Suite) -> Result<(), String> {
    let text = serde_json::to_string_pretty(suite).map_err(|e| e.to_string())?;
    std::fs::write(path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

pub fn default_path(dmn: &Path) -> PathBuf {
    let stem = dmn.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    dmn.with_file_name(format!("{stem}.tests.json"))
}
