use crate::eval::{self, Context, Outcome, Value};
use crate::model::*;
use crate::suite::{self, CaseResult, Coverage, Suite, bound, case_from_outcome, value_to_json};
use crate::verdict::{Expectation, Verdict, check};
use serde_json::Value as J;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuleRef {
    id: String,
    index: usize,
    count: usize,
}

impl RuleRef {
    fn new(t: &DecisionTable, index: usize) -> Self {
        let id = &t.rules[index].id;
        let unique = !id.is_empty() && t.rules.iter().filter(|r| r.id == *id).count() == 1;
        RuleRef { id: if unique { id.clone() } else { String::new() }, index, count: t.rules.len() }
    }

    fn resolve(&self, t: &DecisionTable) -> Option<usize> {
        if self.id.is_empty() {
            return (t.rules.len() == self.count && self.index < self.count).then_some(self.index);
        }
        let mut hits = t.rules.iter().enumerate().filter(|(_, r)| r.id == self.id);
        let (i, _) = hits.next()?;
        hits.next().is_none().then_some(i)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum Expect {
    #[default]
    None,
    Rule(RuleRef),
    Case(usize),
    Snapshot(Expectation),
}

type VerdictKey = (usize, Expect, Vec<String>, u64);

#[derive(Default)]
pub(crate) struct TestSession {
    expect: Expect,
    target: Option<RuleRef>,
    verdict: Option<(VerdictKey, Arc<Verdict>)>,
    values: HashMap<String, String>,
    cols: Vec<String>,
    cols_at: Option<(usize, u64)>,
    suite: Suite,
    suite_path: Option<PathBuf>,
    status: String,
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

fn input_keys(t: &DecisionTable) -> Vec<String> {
    let unique = |k: &str, f: fn(&InputClause) -> &str| !k.is_empty() && t.inputs.iter().filter(|i| f(i) == k).count() == 1;
    t.inputs
        .iter()
        .enumerate()
        .map(|(c, inp)| {
            if unique(&inp.id, |i| &i.id) {
                format!("id:{}", inp.id)
            } else if unique(&inp.binding, |i| &i.binding) {
                format!("in:{}", inp.binding)
            } else {
                format!("#{c}")
            }
        })
        .collect()
}

fn text_of(v: &J) -> Option<String> {
    match v {
        J::Null => None,
        J::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

impl TestSession {
    pub fn suite(&self) -> &Suite {
        &self.suite
    }

    pub fn suite_path(&self) -> Option<&Path> {
        self.suite_path.as_deref()
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub(crate) fn sync(&mut self, revision: u64, defs: &Definitions, decision: usize) {
        if self.revision != revision {
            self.revision = revision;
            self.invalidate();
        }
        if self.cols_at != Some((decision, revision)) {
            self.cols = defs.table(decision).map(input_keys).unwrap_or_default();
            self.cols_at = Some((decision, revision));
        }
    }

    pub(crate) fn change_decision(&mut self) {
        self.values.clear();
        self.expect = Expect::None;
        self.target = None;
        self.cols_at = None;
    }

    fn invalidate(&mut self) {
        self.verdict = None;
        self.live = None;
        self.results = None;
    }

    pub fn input(&self, col: usize) -> &str {
        self.cols.get(col).and_then(|k| self.values.get(k)).map_or("", String::as_str)
    }

    pub fn input_mut(&mut self, col: usize) -> Option<&mut String> {
        let k = self.cols.get(col)?;
        if !self.values.contains_key(k) {
            self.values.insert(k.clone(), String::new());
        }
        self.values.get_mut(k)
    }

    pub fn set_input(&mut self, col: usize, text: String) {
        if let Some(k) = self.cols.get(col) {
            self.values.insert(k.clone(), text);
        }
    }

    pub fn inputs(&self) -> Vec<&str> {
        (0..self.cols.len()).map(|c| self.input(c)).collect()
    }

    fn inputs_owned(&self) -> Vec<String> {
        (0..self.cols.len()).map(|c| self.input(c).to_string()).collect()
    }

    fn same_inputs(&self, snap: &[String]) -> bool {
        snap.len() == self.cols.len() && snap.iter().enumerate().all(|(c, s)| self.input(c) == s)
    }

    pub fn is_active(&self) -> bool {
        (0..self.cols.len()).any(|c| !self.input(c).trim().is_empty())
    }

    pub fn clear_inputs(&mut self) {
        self.values.clear();
    }

    pub fn target(&self, defs: &Definitions, decision: usize) -> Option<usize> {
        self.target.as_ref()?.resolve(defs.table(decision)?)
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
        self.expect = Expect::Snapshot(Expectation::of_outcome(t, &o));
    }

    pub fn verdict(&mut self, defs: &Definitions, decision: usize) -> Option<Arc<Verdict>> {
        if let Some(((d, expect, inputs, gen_), v)) = &self.verdict
            && *d == decision
            && *expect == self.expect
            && *gen_ == self.suite_gen
            && self.same_inputs(inputs)
        {
            return Some(v.clone());
        }
        let v = Arc::new(self.build_verdict(defs, decision)?);
        self.verdict = Some(((decision, self.expect.clone(), self.inputs_owned(), self.suite_gen), v.clone()));
        Some(v)
    }

    fn build_verdict(&mut self, defs: &Definitions, decision: usize) -> Option<Verdict> {
        let o = self.outcome(defs, decision)?;
        let (t, _) = table(defs, decision)?;
        let rule;
        let (against, e) = match &self.expect {
            Expect::None => (None, None),
            Expect::Snapshot(x) => (Some("the saved result".to_string()), Some(x)),
            Expect::Case(i) => match self.suite.cases.get(*i) {
                Some(c) => (Some(format!("case \"{}\"", c.name)), Some(&c.expect)),
                None => (None, None),
            },
            Expect::Rule(r) => {
                let at = r.resolve(t);
                rule = at.and_then(|r| Expectation::of_rule(t, r, &self.context(t)));
                match (&rule, at) {
                    (Some(x), Some(r)) => (Some(format!("rule {} outputs", r + 1)), Some(x)),
                    _ => (None, None),
                }
            }
        };
        let mut v = check(t, &o, e.unwrap_or(&Expectation::default()));
        v.against = against;
        Some(v)
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

    pub fn outcome(&mut self, defs: &Definitions, decision: usize) -> Option<Arc<Outcome>> {
        if let Some((d, inputs, o)) = &self.live
            && *d == decision
            && self.same_inputs(inputs)
        {
            return Some(o.clone());
        }
        let (t, _) = table(defs, decision)?;
        let o = Arc::new(eval::evaluate(t, &self.context(t)));
        self.live = Some((decision, self.inputs_owned(), o.clone()));
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
        let texts: Vec<String> = t.inputs.iter().map(|inp| bound(inp, &case.inputs).and_then(text_of).unwrap_or_default()).collect();
        for (c, text) in texts.into_iter().enumerate() {
            self.set_input(c, text);
        }
        self.expect = Expect::Case(i);
        self.target = None;
    }

    pub fn fill_from_rule(&mut self, defs: &Definitions, decision: usize, rule: usize) {
        if self.fill_for_rule(defs, decision, rule) {
            return;
        }
        let Some(r) = defs.table(decision).and_then(|t| t.rules.get(rule)) else { return };
        let samples: Vec<Option<String>> = r.inputs.iter().map(|e| e.tests.sample()).collect();
        for (c, s) in samples.into_iter().enumerate() {
            if let Some(s) = s {
                self.set_input(c, s);
            }
        }
    }

    pub fn fill_to_fire(&mut self, defs: &Definitions, decision: usize, rule: usize) -> bool {
        if !self.fill_for_rule(defs, decision, rule) {
            return false;
        }
        let t = defs.table(decision).expect("filled implies a table");
        self.target = Some(RuleRef::new(t, rule));
        self.expect = Expect::Rule(RuleRef::new(t, rule));
        true
    }

    fn fill_for_rule(&mut self, defs: &Definitions, decision: usize, rule: usize) -> bool {
        let Some(t) = defs.table(decision) else { return false };
        let Some(values) = crate::gaps::witness(t, rule) else { return false };
        let free = |i: usize| t.rules.iter().all(|r| r.inputs.get(i).is_none_or(InputEntry::is_any));
        let from_suite = |i: usize| self.suite.cases.iter().find_map(|c| bound(&t.inputs[i], &c.inputs).and_then(text_of));
        let texts: Vec<String> = values
            .into_iter()
            .enumerate()
            .map(|(i, v)| match v {
                Some(Value::Str(s)) => s,
                Some(Value::Num(n)) => eval::fmt_num(n),
                Some(Value::Bool(b)) => b.to_string(),
                Some(Value::Null) if !free(i) => String::new(),
                _ if !self.input(i).is_empty() => self.input(i).to_string(),
                _ if free(i) => from_suite(i).unwrap_or_default(),
                _ => String::new(),
            })
            .collect();
        for (c, text) in texts.into_iter().enumerate() {
            self.set_input(c, text);
        }
        true
    }

    pub fn new_suite(&mut self, decision_name: &str) {
        self.suite = Suite { decision: decision_name.to_string(), cases: vec![] };
        self.forget_case_expect();
        self.suite_gen += 1;
        self.suite_path = None;
        self.status = "New empty suite".into();
    }

    pub fn load(&mut self, path: &Path) -> Result<usize, String> {
        let loaded = suite::load(path);
        self.status = match &loaded {
            Ok(s) => format!("Loaded {} cases from {}", s.cases.len(), path.file_name().map(|f| f.to_string_lossy()).unwrap_or_default()),
            Err(e) => e.clone(),
        };
        let s = loaded?;
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
        self.status = match &r {
            Ok(n) => format!("Saved {n} cases"),
            Err(e) => e.clone(),
        };
        self.suite_path = Some(path.to_path_buf());
        r
    }

    fn claim_suite(&mut self, decision_name: &str) {
        if self.suite.decision.is_empty() {
            self.suite.decision = decision_name.to_string();
        }
    }

    fn value(&self, t: &DecisionTable, i: usize) -> Value {
        Value::from_input(self.input(i), t.inputs[i].type_ref.as_deref())
    }

    fn context(&self, t: &DecisionTable) -> Context {
        t.inputs.iter().enumerate().map(|(i, inp)| (inp.binding.clone(), self.value(t, i))).collect()
    }

    fn inputs_json(&self, t: &DecisionTable) -> BTreeMap<String, J> {
        t.inputs
            .iter()
            .enumerate()
            .filter_map(|(i, inp)| {
                let v = self.value(t, i);
                (v != Value::Null).then(|| (inp.binding.clone(), value_to_json(&v)))
            })
            .collect()
    }
}
