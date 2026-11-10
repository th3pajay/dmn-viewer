use crate::eval::{self, Context, Outcome, Value};
use crate::model::DecisionTable;
use crate::suite::{json_to_value, value_to_json};
use serde::{Deserialize, Serialize};
use serde_json::Value as J;
use std::collections::BTreeMap;

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Expectation {
    #[serde(default)]
    pub expected: BTreeMap<String, J>,
    #[serde(default, rename = "expectError", skip_serializing_if = "is_false")]
    pub expect_error: bool,
    #[serde(default, rename = "expectRules", skip_serializing_if = "Option::is_none")]
    pub expect_rules: Option<Vec<usize>>,
    #[serde(skip)]
    pub rule: Option<usize>,
}

fn output_key(o: &crate::model::OutputClause) -> String {
    if o.name.is_empty() { o.label.clone() } else { o.name.clone() }
}

impl Expectation {
    pub fn of_outcome(t: &DecisionTable, o: &Outcome) -> Self {
        let mut expected = BTreeMap::new();
        if o.error.is_none() {
            let rows = t.hit_policy.returns_rows();
            for (i, out) in t.outputs.iter().enumerate() {
                let v = if rows { J::Array(o.rows.iter().map(|r| r.get(i).map_or(J::Null, value_to_json)).collect()) } else { value_to_json(&o.output(i)) };
                expected.insert(output_key(out), v);
            }
        }
        Expectation { expected, expect_error: o.error.is_some(), expect_rules: Some(o.fired.iter().map(|r| r + 1).collect()), rule: None }
    }

    pub fn of_rule(t: &DecisionTable, rule: usize, vars: &Context) -> Option<Self> {
        let values = eval::rule_outputs(t, rule, vars).ok()?;
        let expected = t.outputs.iter().zip(&values).map(|(o, v)| (output_key(o), value_to_json(v))).collect();
        Some(Expectation { expected, expect_error: false, expect_rules: None, rule: Some(rule) })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub output: usize,
    pub row: usize,
    pub checked: bool,
    pub expected: Option<Value>,
    pub actual: Option<Value>,
    pub type_mismatch: bool,
}

impl Cell {
    pub fn matches(&self) -> bool {
        !self.checked || self.expected == self.actual
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Verdict {
    pub rows: usize,
    pub cells: Vec<Cell>,
    pub rules: Option<(Vec<usize>, Vec<usize>)>,
    pub expect_error: bool,
    pub checks: usize,
    pub failed: usize,
    pub problems: Vec<String>,
    pub against: Option<String>,
}

impl Verdict {
    pub fn pass(&self) -> bool {
        self.problems.is_empty()
    }
}

fn shown(v: Option<&Value>) -> String {
    v.map_or_else(|| "nothing".into(), Value::display)
}

pub fn check(t: &DecisionTable, o: &Outcome, e: &Expectation) -> Verdict {
    judge(t, o, e, true)
}

pub fn problems(t: &DecisionTable, o: &Outcome, e: &Expectation) -> Vec<String> {
    judge(t, o, e, false).problems
}

static NULL: Value = Value::Null;

fn judge(t: &DecisionTable, o: &Outcome, e: &Expectation, cells: bool) -> Verdict {
    let mut v = Verdict { expect_error: e.expect_error, ..Verdict::default() };
    let no = t.outputs.len();
    let rows_policy = t.hit_policy.returns_rows() && e.rule.is_none() && o.aggregate.is_none();
    let rule_row = e.rule.map(|r| o.fired.iter().position(|&f| f == r).unwrap_or(0));
    let actual_rows = if rows_policy { o.rows.len() } else { 1 };
    let actual = |row: usize, oi: usize| -> Option<&Value> {
        if rows_policy {
            return o.rows.get(row).and_then(|r| r.get(oi));
        }
        if row > 0 {
            return None;
        }
        if let Some(pos) = rule_row {
            return Some(o.rows.get(pos).and_then(|r| r.get(oi)).unwrap_or(&NULL));
        }
        Some(o.aggregate.as_ref().or_else(|| o.rows.first().and_then(|r| r.get(oi))).unwrap_or(&NULL))
    };

    let mut expected: Vec<Option<Vec<Value>>> = vec![None; no];
    if e.expect_error {
        v.checks += 1;
        if o.error.is_none() {
            v.failed += 1;
            v.problems.push("expected an evaluation error, but the decision returned a result".into());
        }
    } else if let Some(err) = &o.error {
        v.checks += 1;
        v.failed += 1;
        v.problems.push(format!("evaluation failed: {err}"));
    } else {
        for (name, j) in &e.expected {
            let Some(idx) = t.outputs.iter().position(|out| out.name == *name || out.label == *name) else {
                v.checks += 1;
                v.failed += 1;
                v.problems.push(format!("unknown output \"{name}\""));
                continue;
            };
            let ty = t.outputs[idx].type_ref.as_deref();
            let want = match j {
                J::Array(items) if rows_policy => items.iter().map(|x| json_to_value(x, ty)).collect(),
                J::Null if rows_policy => Vec::new(),
                J::Array(_) => {
                    v.checks += 1;
                    v.failed += 1;
                    v.problems.push(format!("{name}: expected one value for hit policy {}, got a list", t.hit_policy));
                    continue;
                }
                other => vec![json_to_value(other, ty)],
            };
            if rows_policy && want.len() != actual_rows {
                v.problems.push(format!("{name}: expected {} rows, got {actual_rows}", want.len()));
            } else {
                for (row, w) in want.iter().enumerate() {
                    let got = actual(row, idx);
                    if got != Some(w) {
                        let at = if rows_policy { format!("{name} #{}", row + 1) } else { name.clone() };
                        v.problems.push(format!("{at}: expected {}, got {}", w.display(), shown(got)));
                    }
                }
            }
            expected[idx] = Some(want);
        }
    }

    let rows = expected.iter().flatten().map(Vec::len).max().unwrap_or(0).max(actual_rows).max(1);
    v.rows = rows;
    v.cells.reserve_exact(if cells { rows * no } else { 0 });
    for row in (0..rows).filter(|_| cells) {
        for (oi, out) in t.outputs.iter().enumerate() {
            let actual = actual(row, oi).cloned();
            let type_mismatch = out.type_ref.as_deref().is_some_and(|x| x.eq_ignore_ascii_case("string")) && actual.as_ref().is_some_and(|a| !matches!(a, Value::Str(_) | Value::Null));
            let checked = expected[oi].is_some();
            let cell = Cell { output: oi, row, checked, expected: expected[oi].as_ref().and_then(|w| w.get(row).cloned()), actual, type_mismatch };
            if checked {
                v.checks += 1;
                v.failed += usize::from(!cell.matches());
            }
            v.cells.push(cell);
        }
    }

    if let Some(rules) = &e.expect_rules {
        let fired: Vec<usize> = o.fired.iter().map(|r| r + 1).collect();
        v.checks += 1;
        if *rules != fired {
            v.failed += 1;
            v.problems.push(format!("expected rules {rules:?} to fire, got {fired:?}"));
        }
        v.rules = Some((rules.clone(), fired));
    }
    if let Some(r) = e.rule
        && o.error.is_none()
    {
        v.checks += 1;
        if !o.fired.contains(&r) {
            v.failed += 1;
            v.problems.push(format!("rule {} did not fire", r + 1));
        }
    }
    v
}
