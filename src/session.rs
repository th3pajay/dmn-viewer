use crate::eval::{self, Context, Outcome, Value};
use crate::model::*;
use crate::suite::{self, Case, CaseResult, Coverage, Suite, binding_key, bound, case_from_outcome, value_to_json};
use serde_json::Value as J;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The tester's working state for one decision table at a time: the typed inputs, the loaded
/// test suite, and their evaluated results. Results are cached until the decision, the inputs or
/// the cases change; call `invalidate` when the definitions themselves are reloaded.
#[derive(Default)]
pub struct TestSession {
    /// One text per table input. Empty means null; `""` is the empty string.
    pub fields: Vec<String>,
    suite: Suite,
    suite_path: Option<PathBuf>,
    live: Option<(usize, Vec<String>, Arc<Outcome>)>,
    results: Option<Run>,
}

/// One run of the suite against a decision, kept until the decision or the cases change.
struct Run {
    decision: usize,
    cases: Vec<Case>,
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

    pub fn is_active(&self) -> bool {
        self.fields.iter().any(|f| !f.trim().is_empty())
    }

    pub fn invalidate(&mut self) {
        self.live = None;
        self.results = None;
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

    /// Which rules the suite's cases fire, from the same cached run as `results`.
    pub fn coverage(&mut self, defs: &Definitions, decision: usize) -> Option<Arc<Coverage>> {
        self.run(defs, decision).map(|r| r.coverage.clone())
    }

    fn run(&mut self, defs: &Definitions, decision: usize) -> Option<&Run> {
        let fresh = self.results.as_ref().is_some_and(|r| r.decision == decision && r.cases == self.suite.cases);
        if !fresh {
            let (t, _) = table(defs, decision)?;
            let results: Vec<_> = self.suite.cases.iter().map(|c| suite::run_case(t, c)).collect();
            let coverage = Arc::new(Coverage::new(t.rules.len(), &results));
            self.results = Some(Run { decision, cases: self.suite.cases.clone(), results: Arc::new(results), coverage });
        }
        self.results.as_ref()
    }

    /// Saves the current inputs and their current result as a new case. A blank name becomes "Case N".
    pub fn add_case(&mut self, defs: &Definitions, decision: usize, name: &str) {
        let Some(outcome) = self.outcome(defs, decision) else { return };
        let (t, decision_name) = table(defs, decision).expect("outcome implies a table");
        let name = if name.trim().is_empty() { format!("Case {}", self.suite.cases.len() + 1) } else { name.trim().to_string() };
        let case = case_from_outcome(name, t, self.inputs_json(t), &outcome);
        self.claim_suite(decision_name);
        self.suite.cases.push(case);
    }

    /// Replaces case `i`'s expectations with what the decision currently returns for it.
    pub fn accept(&mut self, defs: &Definitions, decision: usize, i: usize) {
        let Some(results) = self.results(defs, decision) else { return };
        let (t, _) = table(defs, decision).expect("results imply a table");
        let (Some(c), Some(r)) = (self.suite.cases.get(i), results.get(i)) else { return };
        self.suite.cases[i] = case_from_outcome(c.name.clone(), t, c.inputs.clone(), &r.outcome);
    }

    pub fn delete(&mut self, i: usize) {
        if i < self.suite.cases.len() {
            self.suite.cases.remove(i);
        }
    }

    pub fn fill_from_case(&mut self, defs: &Definitions, decision: usize, i: usize) {
        let (Some((t, _)), Some(case)) = (table(defs, decision), self.suite.cases.get(i)) else { return };
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

    /// Fills in, per input, a value the rule's condition accepts; inputs without an obvious one are left alone.
    pub fn fill_from_rule(&mut self, rule: &Rule) {
        for (e, f) in rule.inputs.iter().zip(self.fields.iter_mut()) {
            if let Some(v) = e.tests.sample() {
                *f = v;
            }
        }
    }

    pub fn new_suite(&mut self, decision_name: &str) {
        self.suite = Suite { decision: decision_name.to_string(), cases: vec![] };
        self.suite_path = None;
    }

    /// Returns the number of cases loaded. On error the session is unchanged.
    pub fn load(&mut self, path: &Path) -> Result<usize, String> {
        let s = suite::load(path)?;
        let n = s.cases.len();
        self.suite = s;
        self.suite_path = Some(path.to_path_buf());
        Ok(n)
    }

    /// Returns the number of cases saved. The path becomes the suite's path even if writing fails.
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
