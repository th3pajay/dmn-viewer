use crate::eval::{self, Context, Outcome, Value};
use crate::model::*;
use crate::suite::{self, CaseResult, Coverage, Suite, binding_key, bound, case_from_outcome, value_to_json};
use serde_json::Value as J;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Expect {
    #[default]
    None,
    Rule(usize),
    Case(usize),
    Snapshot(Vec<Value>),
}

#[derive(Debug, PartialEq)]
pub struct Expected {
    pub label: String,
    pub values: Vec<Value>,
}

type ExpectedKey = (usize, Expect, Vec<String>, u64);

#[derive(Default)]
pub struct TestSession {
    expect: Expect,
    target: Option<usize>,
    expected: Option<(ExpectedKey, Arc<Option<Expected>>)>,
    fields: Vec<String>,
    suite: Suite,
    suite_path: Option<PathBuf>,
    live: Option<(usize, Vec<String>, Arc<Outcome>)>,
    results: Option<Run>,
    revision: u64,
    suite_gen: u64,
}

struct Run {
    decision: usize,
    suite_gen: u64,
    results: Arc<Vec<CaseResult>>,
    coverage: Arc<Coverage>,
}

fn table(defs: &Definitions, decision: usize) -> Option<(&DecisionTable, &str)> {
    Some((defs.table(decision)?, defs.decisions[decision].name.as_str()))
}

impl TestSession {
    pub fn suite(&self) -> &Suite {
        &self.suite
    }

    pub fn suite_path(&self) -> Option<&Path> {
        self.suite_path.as_deref()
    }

    pub fn fields(&self) -> &[String] {
        &self.fields
    }

    pub fn fields_mut(&mut self) -> &mut Vec<String> {
        &mut self.fields
    }

    pub fn clear_fields(&mut self) {
        self.fields.clear();
    }

    pub fn is_active(&self) -> bool {
        self.fields.iter().any(|f| !f.trim().is_empty())
    }

    pub fn target(&self) -> Option<usize> {
        self.target
    }

    pub fn clear_target(&mut self) {
        self.target = None;
        if matches!(self.expect, Expect::Rule(_)) {
            self.expect = Expect::None;
        }
    }

    pub fn clear_expected(&mut self) {
        self.expect = Expect::None;
    }

    pub fn snapshot(&mut self, defs: &Definitions, decision: usize) {
        let Some(o) = self.outcome(defs, decision) else { return };
        let Some((t, _)) = table(defs, decision) else { return };
        if o.error.is_none() {
            self.expect = Expect::Snapshot((0..t.outputs.len()).map(|i| o.output(i)).collect());
        }
    }

    pub fn expected(&mut self, defs: &Definitions, decision: usize) -> Arc<Option<Expected>> {
        let key = (decision, self.expect.clone(), self.fields.clone(), self.suite_gen);
        if let Some((k, e)) = &self.expected
            && *k == key
        {
            return e.clone();
        }
        let e = Arc::new(self.build_expected(defs, decision));
        self.expected = Some((key, e.clone()));
        e
    }

    fn build_expected(&self, defs: &Definitions, decision: usize) -> Option<Expected> {
        let (t, _) = table(defs, decision)?;
        match &self.expect {
            Expect::None => None,
            Expect::Snapshot(values) => Some(Expected { label: "the saved result".into(), values: values.clone() }),
            Expect::Case(i) => {
                let c = self.suite.cases.get(*i).filter(|c| !c.expect_error)?;
                let values = t
                    .outputs
                    .iter()
                    .map(|o| c.expected.get(&o.name).or_else(|| c.expected.get(&o.label)).map(|j| suite::json_to_value(j, o.type_ref.as_deref())).unwrap_or(Value::Null))
                    .collect();
                Some(Expected { label: format!("case \"{}\"", c.name), values })
            }
            Expect::Rule(r) => {
                let values = (*r < t.rules.len()).then(|| eval::rule_outputs(t, *r, &self.context(t)).ok()).flatten()?;
                Some(Expected { label: format!("rule {} outputs", r + 1), values })
            }
        }
    }

    fn shift_expect(&mut self, deleted: usize) {
        self.expect = match self.expect {
            Expect::Case(i) if i == deleted => Expect::None,
            Expect::Case(i) if i > deleted => Expect::Case(i - 1),
            ref other => other.clone(),
        };
    }

    fn forget_case_expect(&mut self) {
        if matches!(self.expect, Expect::Case(_)) {
            self.expect = Expect::None;
        }
    }

    pub fn invalidate(&mut self) {
        self.expected = None;
        self.live = None;
        self.results = None;
    }

    pub(crate) fn sync(&mut self, revision: u64) {
        if self.revision != revision {
            self.revision = revision;
            self.invalidate();
        }
    }

    pub fn clear_inputs(&mut self) {
        self.fields.iter_mut().for_each(|f| f.clear());
    }

    pub fn outcome(&mut self, defs: &Definitions, decision: usize) -> Option<Arc<Outcome>> {
        if let Some((d, fields, o)) = &self.live
            && *d == decision
            && *fields == self.fields
        {
            return Some(o.clone());
        }
        let (t, _) = table(defs, decision)?;
        let o = Arc::new(eval::evaluate(t, &self.context(t)));
        self.live = Some((decision, self.fields.clone(), o.clone()));
        Some(o)
    }

    pub fn results(&mut self, defs: &Definitions, decision: usize) -> Option<Arc<Vec<CaseResult>>> {
        self.run(defs, decision).map(|r| r.results.clone())
    }

    pub fn coverage(&mut self, defs: &Definitions, decision: usize) -> Option<Arc<Coverage>> {
        self.run(defs, decision).map(|r| r.coverage.clone())
    }

    fn run(&mut self, defs: &Definitions, decision: usize) -> Option<&Run> {
        let fresh = self.results.as_ref().is_some_and(|r| r.decision == decision && r.suite_gen == self.suite_gen);
        if !fresh {
            let (t, _) = table(defs, decision)?;
            let run = self.suite.run(t);
            self.results = Some(Run { decision, suite_gen: self.suite_gen, results: Arc::new(run.results), coverage: Arc::new(run.coverage) });
        }
        self.results.as_ref()
    }

    pub fn add_case(&mut self, defs: &Definitions, decision: usize, name: &str) {
        let Some(outcome) = self.outcome(defs, decision) else { return };
        let (t, decision_name) = table(defs, decision).expect("outcome implies a table");
        let name = if name.trim().is_empty() { format!("Case {}", self.suite.cases.len() + 1) } else { name.trim().to_string() };
        let case = case_from_outcome(name, t, self.inputs_json(t), &outcome);
        self.claim_suite(decision_name);
        self.suite.cases.push(case);
        self.expect = Expect::Case(self.suite.cases.len() - 1);
        self.suite_gen += 1;
    }

    pub fn accept(&mut self, defs: &Definitions, decision: usize, i: usize) {
        let Some(results) = self.results(defs, decision) else { return };
        let (t, _) = table(defs, decision).expect("results imply a table");
        let (Some(c), Some(r)) = (self.suite.cases.get(i), results.get(i)) else { return };
        self.suite.cases[i] = case_from_outcome(c.name.clone(), t, c.inputs.clone(), &r.outcome);
        self.suite_gen += 1;
    }

    pub fn delete(&mut self, i: usize) {
        if i < self.suite.cases.len() {
            self.suite.cases.remove(i);
            self.shift_expect(i);
            self.suite_gen += 1;
        }
    }

    pub fn fill_from_case(&mut self, defs: &Definitions, decision: usize, i: usize) {
        let (Some((t, _)), Some(case)) = (table(defs, decision), self.suite.cases.get(i)) else { return };
        self.expect = Expect::Case(i);
        self.target = None;
        self.fields = t
            .inputs
            .iter()
            .map(|inp| match bound(inp, &case.inputs) {
                Some(J::String(s)) => s.clone(),
                Some(J::Null) | None => String::new(),
                Some(other) => other.to_string(),
            })
            .collect();
    }

    pub fn fill_from_rule(&mut self, rule: &Rule) {
        for (e, f) in rule.inputs.iter().zip(self.fields.iter_mut()) {
            if let Some(v) = e.tests.sample() {
                *f = v;
            }
        }
    }

    pub fn fill_to_fire(&mut self, defs: &Definitions, decision: usize, rule: usize) -> bool {
        let Some(t) = defs.table(decision) else { return false };
        let Some(values) = crate::gaps::witness(t, rule) else { return false };
        let free = |i: usize| t.rules.iter().all(|r| r.inputs[i].tests.is_any());
        let from_suite = |i: usize| {
            self.suite.cases.iter().find_map(|c| match bound(&t.inputs[i], &c.inputs)? {
                J::Null => None,
                J::String(s) => Some(s.clone()),
                other => Some(other.to_string()),
            })
        };
        self.target = Some(rule);
        self.expect = Expect::Rule(rule);
        self.fields = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| match v {
                Some(Value::Str(s)) => s,
                Some(Value::Num(n)) => eval::fmt_num(n),
                Some(Value::Bool(b)) => b.to_string(),
                Some(Value::Null) | None if free(i) => from_suite(i).unwrap_or_default(),
                Some(Value::Null) | None => String::new(),
            })
            .collect();
        true
    }

    pub fn new_suite(&mut self, decision_name: &str) {
        self.suite = Suite { decision: decision_name.to_string(), cases: vec![] };
        self.forget_case_expect();
        self.suite_gen += 1;
        self.suite_path = None;
    }

    pub fn load(&mut self, path: &Path) -> Result<usize, String> {
        let s = suite::load(path)?;
        let n = s.cases.len();
        self.suite = s;
        self.forget_case_expect();
        self.suite_gen += 1;
        self.suite_path = Some(path.to_path_buf());
        Ok(n)
    }

    pub fn save(&mut self, path: &Path, decision_name: &str) -> Result<usize, String> {
        self.claim_suite(decision_name);
        let r = suite::save(path, &self.suite).map(|()| self.suite.cases.len());
        self.suite_path = Some(path.to_path_buf());
        r
    }

    fn claim_suite(&mut self, decision_name: &str) {
        if self.suite.decision.is_empty() {
            self.suite.decision = decision_name.to_string();
        }
    }

    fn value(&self, t: &DecisionTable, i: usize) -> Option<Value> {
        Some(Value::from_input(self.fields.get(i)?, t.inputs[i].type_ref.as_deref()))
    }

    fn context(&self, t: &DecisionTable) -> Context {
        t.inputs.iter().enumerate().map(|(i, inp)| (binding_key(inp).to_string(), self.value(t, i).unwrap_or(Value::Null))).collect()
    }

    fn inputs_json(&self, t: &DecisionTable) -> BTreeMap<String, J> {
        t.inputs
            .iter()
            .enumerate()
            .filter_map(|(i, inp)| {
                let v = self.value(t, i)?;
                (v != Value::Null).then(|| (binding_key(inp).to_string(), value_to_json(&v)))
            })
            .collect()
    }
}
