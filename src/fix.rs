//! Automatic fixes for lint issues. A fixable issue offers one or more fixes; each fix is a list
//! of edits that the user confirms before it is applied. Nothing here reads or writes files.

use crate::edit::{self, Column, Edit};
use crate::lint::{self, Issue};
use crate::model::*;
use crate::suite::{self, Suite};
use crate::unary::{is_quoted, literal_items};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct Fix {
    pub label: String,
    /// What else the user should know before confirming, e.g. how results change.
    pub hint: Option<String>,
    /// Deletes a rule or a column. Never pre-selected; the label names what goes.
    pub destructive: bool,
    pub edits: Vec<Edit>,
}

/// A fixable issue with the ways it can be fixed.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub issue: Issue,
    /// Identifies the issue across edits that renumber rules, so a skip sticks to it.
    pub key: String,
    pub fixes: Vec<Fix>,
}

impl Candidate {
    pub fn destructive(&self) -> bool {
        self.fixes.iter().all(|f| f.destructive)
    }

    /// The fix selected when the step opens: the first one that deletes nothing.
    pub fn default_fix(&self) -> Option<usize> {
        self.fixes.iter().position(|f| !f.destructive)
    }
}

/// One line of the from → to overview.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub place: String,
    pub from: String,
    pub to: String,
}

/// How a fix changes the loaded test suite's results.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TestEffect {
    pub total: usize,
    pub passed_before: usize,
    pub passed_after: usize,
    /// Passing cases that would fail.
    pub broken: Vec<String>,
    /// Failing cases that would pass.
    pub fixed: Vec<String>,
    /// Cases that fail before and after, but for a different reason.
    pub changed: Vec<String>,
}

pub fn queue(defs: &Definitions, issues: &[Issue], skipped: &HashSet<String>, front: Option<&str>, only: Option<&str>) -> Vec<Candidate> {
    queue_with(&Keys::new(defs), defs, issues, skipped, front, only)
}

pub fn queue_with(keys: &Keys, defs: &Definitions, issues: &[Issue], skipped: &HashSet<String>, front: Option<&str>, only: Option<&str>) -> Vec<Candidate> {
    let mut q: Vec<Candidate> = issues
        .iter()
        .filter_map(|is| {
            let key = keys.key(defs, is);
            if skipped.contains(&key) || only.is_some_and(|o| o != key) {
                return None;
            }
            let fixes = fixes(defs, is);
            (!fixes.is_empty()).then(|| Candidate { issue: is.clone(), key, fixes })
        })
        .collect();
    let column = |c: Option<Column>| match c {
        None => 0,
        Some(Column::Input(i)) => 1 + i,
        Some(Column::Output(o)) => usize::MAX / 2 + o,
    };
    q.sort_by_key(|c| {
        let is = &c.issue;
        (front != Some(c.key.as_str()), is.severity, c.destructive(), is.decision, is.rules.iter().min().copied().unwrap_or(usize::MAX), column(is.column))
    });
    q
}

pub struct Keys {
    ambiguous: Vec<Vec<bool>>,
}

impl Keys {
    pub fn new(defs: &Definitions) -> Keys {
        let ambiguous = (0..defs.decisions.len())
            .map(|d| {
                let Some(t) = defs.table(d) else { return Vec::new() };
                let mut count: HashMap<&str, usize> = HashMap::new();
                for r in &t.rules {
                    *count.entry(r.id.as_str()).or_default() += 1;
                }
                t.rules.iter().map(|r| r.id.is_empty() || count[r.id.as_str()] > 1).collect()
            })
            .collect();
        Keys { ambiguous }
    }

    pub fn key(&self, defs: &Definitions, is: &Issue) -> String {
        let mut k = is.code.to_string();
        let dec = is.decision.and_then(|d| defs.decisions.get(d));
        k.push('|');
        k.push_str(dec.map_or("", |d| d.id.as_str()));
        let table = is.decision.and_then(|d| defs.table(d));
        if let (Some(t), Some(d)) = (table, is.decision) {
            for &r in &is.rules {
                let id = t.rules.get(r).map_or("", |x| x.id.as_str());
                k.push('|');
                k.push_str(id);
                if self.ambiguous[d].get(r).copied().unwrap_or(true) {
                    k.push_str(&format!("#{r}"));
                }
            }
            match is.column {
                Some(Column::Input(i)) => k.push_str(&format!("|in:{}", t.inputs.get(i).map_or(i.to_string(), |c| clause_key(&c.id, i)))),
                Some(Column::Output(o)) => k.push_str(&format!("|out:{}", t.outputs.get(o).map_or(o.to_string(), |c| clause_key(&c.id, o)))),
                None => {}
            }
        }
        if is.rules.is_empty() && is.column.is_none() {
            k.push('|');
            k.push_str(&is.message);
        }
        k
    }
}

pub fn key(defs: &Definitions, is: &Issue) -> String {
    Keys::new(defs).key(defs, is)
}

fn clause_key(id: &str, i: usize) -> String {
    if id.is_empty() { format!("#{i}") } else { id.to_string() }
}

/// The ways an issue can be fixed; empty when it needs a person (a placeholder, a missing default rule, …).
pub fn fixes(defs: &Definitions, is: &Issue) -> Vec<Fix> {
    if is.code == "DUPLICATE_ID" {
        return duplicate_id(is).into_iter().collect();
    }
    let Some(d) = is.decision else { return vec![] };
    let Some(t) = defs.table(d) else { return vec![] };
    let shaped = is.rules.iter().all(|&r| t.rules.get(r).is_some_and(|x| x.inputs.len() == t.inputs.len() && x.outputs.len() == t.outputs.len()));
    if !shaped {
        return vec![];
    }
    match (is.code, is.column, is.rules.as_slice()) {
        ("NON_FEEL_SYNTAX", Some(Column::Output(o)), [r]) => non_feel_output(d, t, *r, o),
        ("NON_FEEL_SYNTAX", Some(c @ Column::Input(_)), [r]) => vec![javascript_cell(d, *r, c)],
        ("NON_FEEL_SYNTAX", Some(Column::Input(i)), []) => t.inputs.get(i).map(|inp| javascript_input(d, i, inp)).into_iter().collect(),
        ("TYPE_MISMATCH", Some(Column::Input(i)), rules) => quote_values(d, t, i, rules).into_iter().collect(),
        ("NOT_IN_ALLOWED_VALUES", Some(Column::Input(i)), rules) => not_allowed_input(d, t, i, rules),
        ("NOT_IN_ALLOWED_VALUES", Some(Column::Output(o)), rules) => not_allowed_output(d, t, o, rules).into_iter().collect(),
        ("UNREACHABLE_RULE", None, [j, i]) => unreachable(d, t, *j, *i),
        ("DUPLICATE_RULE", None, [i, j]) => vec![Fix {
            label: format!("Delete rule {}", j + 1),
            hint: Some(format!(
                "Rule {} is an exact copy of rule {}. With hit policy {} both fire today, so a COLLECT result (a count or a sum) changes.",
                j + 1,
                i + 1,
                t.hit_policy.to_ascii_uppercase()
            )),
            destructive: true,
            edits: vec![Edit::DeleteRule { decision: d, rule: *j }],
        }],
        ("UNUSED_INPUT", Some(Column::Input(i)), []) => t
            .inputs
            .get(i)
            .map(|inp| Fix {
                label: format!("Delete input \"{}\"", inp.title()),
                hint: Some("Every rule has \"-\" in this column, so no result changes. Callers may still send the value.".into()),
                destructive: true,
                edits: vec![Edit::DeleteInput { decision: d, input: i }],
            })
            .into_iter()
            .collect(),
        _ => vec![],
    }
}

fn duplicate_id(is: &Issue) -> Option<Fix> {
    // The message is `Element id "<id>" is used more than once; …`.
    let id = is.message.split('"').nth(1).filter(|id| !id.is_empty())?;
    Some(Fix {
        label: format!("Keep the first \"{id}\" and give the later elements new ids ({id}_2, …)"),
        hint: Some("References to this id, such as the diagram, keep pointing at the first element.".into()),
        destructive: false,
        edits: vec![Edit::RenameDuplicateId { id: id.to_string() }],
    })
}

fn non_feel_output(d: usize, t: &DecisionTable, r: usize, o: usize) -> Vec<Fix> {
    let col = Column::Output(o);
    let mut out = Vec::new();
    if let Some(feel) = lint::feel_suggestion(&t.rules[r].outputs[o].text) {
        out.push(Fix { label: format!("Rewrite it in FEEL: {feel}"), hint: None, destructive: false, edits: vec![Edit::Cell { decision: d, rule: r, col, text: feel }] });
    }
    out.push(javascript_cell(d, r, col));
    out
}

const JS_HINT: &str = "Runs on Camunda 7 with a JavaScript engine only; Camunda 8 supports FEEL only.";

fn javascript_cell(d: usize, r: usize, col: Column) -> Fix {
    Fix {
        label: "Keep the expression and set the cell's language to JavaScript".into(),
        hint: Some(JS_HINT.into()),
        destructive: false,
        edits: vec![Edit::CellLanguage { decision: d, rule: r, col, lang: Some("javascript".into()) }],
    }
}

fn javascript_input(d: usize, i: usize, inp: &InputClause) -> Fix {
    Fix {
        label: "Keep the input expression and set its language to JavaScript".into(),
        hint: Some(JS_HINT.into()),
        destructive: false,
        edits: vec![input_header(d, i, inp, Some("javascript".into()), inp.input_values.clone())],
    }
}

/// An input header edit that changes only the language and the allowed values.
fn input_header(d: usize, i: usize, inp: &InputClause, lang: Option<String>, allowed: Option<String>) -> Edit {
    Edit::InputHeader { decision: d, input: i, label: inp.label.clone(), expression: inp.expression.clone(), type_ref: inp.type_ref.clone(), lang, allowed }
}

/// The separator the cell already uses between list items.
fn separator(text: &str) -> &'static str {
    if text.contains(", ") { ", " } else { "," }
}

fn list_text(items: &[String], negated: bool, sep: &str) -> String {
    let list = items.join(sep);
    if negated { format!("not({list})") } else { list }
}

fn quote_values(d: usize, t: &DecisionTable, i: usize, rules: &[usize]) -> Option<Fix> {
    let mut edits = Vec::new();
    for &r in rules {
        let e = &t.rules[r].inputs[i];
        let l = e.tests.literals()?;
        let items: Vec<String> = l.items.iter().map(|v| if is_quoted(v) { v.clone() } else { format!("\"{v}\"") }).collect();
        edits.push(Edit::Cell { decision: d, rule: r, col: Column::Input(i), text: list_text(&items, l.negated, separator(&e.text)) });
    }
    Some(Fix {
        label: format!("Put quotes around the unquoted values in {}", rules_phrase(rules)),
        hint: Some(format!("\"{}\" is typed string, so FEEL compares text with text: 5 and \"5\" are different values.", t.inputs[i].title())),
        destructive: false,
        edits,
    })
}

fn rules_phrase(rules: &[usize]) -> String {
    let label = lint::rules_label(rules);
    if rules.len() == 1 { format!("rule {label}") } else { format!("rules {label}") }
}

/// Values in `items` missing from `allowed`, in written order, without repeats.
fn missing(items: &[String], allowed: &[String], out: &mut Vec<String>) {
    for v in items {
        if !allowed.contains(v) && !out.contains(v) {
            out.push(v.clone());
        }
    }
}

fn extended(allowed_text: &str, values: &[String]) -> String {
    let t = allowed_text.trim();
    let sep = separator(t);
    format!("{t}{sep}{}", values.join(sep))
}

fn not_allowed_input(d: usize, t: &DecisionTable, i: usize, rules: &[usize]) -> Vec<Fix> {
    let inp = &t.inputs[i];
    let Some(allowed_text) = inp.input_values.as_deref() else { return vec![] };
    let Some(allowed) = literal_items(allowed_text) else { return vec![] };
    let mut values = Vec::new();
    for &r in rules {
        if let Some(l) = t.rules[r].inputs[i].tests.literals() {
            missing(&l.items, &allowed, &mut values);
        }
    }
    if values.is_empty() {
        return vec![];
    }
    let shown = values.join(", ");
    let mut out = vec![Fix {
        label: format!("Add {shown} to the allowed values of \"{}\"", inp.title()),
        hint: None,
        destructive: false,
        edits: vec![input_header(d, i, inp, inp.lang.declared.clone().filter(|_| inp.lang.is_explicit()), Some(extended(allowed_text, &values)))],
    }];
    // Removing the values from the rules instead: only where each rule keeps at least one value.
    let mut edits = Vec::new();
    for &r in rules {
        let e = &t.rules[r].inputs[i];
        let Some(l) = e.tests.literals() else { return out };
        let kept: Vec<String> = l.items.iter().filter(|v| !values.contains(v)).cloned().collect();
        if kept.is_empty() || l.negated {
            return out;
        }
        edits.push(Edit::Cell { decision: d, rule: r, col: Column::Input(i), text: list_text(&kept, false, separator(&e.text)) });
    }
    out.push(Fix { label: format!("Remove {shown} from {}", rules_phrase(rules)), hint: Some("Inputs with those values then fall through to later rules.".into()), destructive: false, edits });
    out
}

fn not_allowed_output(d: usize, t: &DecisionTable, o: usize, rules: &[usize]) -> Option<Fix> {
    let out = &t.outputs[o];
    let allowed_text = out.output_values.as_deref()?;
    let allowed = literal_items(allowed_text)?;
    let mut values = Vec::new();
    for &r in rules {
        missing(&[t.rules[r].outputs[o].text.trim().to_string()], &allowed, &mut values);
    }
    (!values.is_empty()).then(|| Fix {
        label: format!("Add {} to the allowed values of \"{}\"", values.join(", "), out.title()),
        hint: None,
        destructive: false,
        edits: vec![Edit::OutputHeader { decision: d, output: o, label: out.label.clone(), name: out.name.clone(), type_ref: out.type_ref.clone(), allowed: Some(extended(allowed_text, &values)) }],
    })
}

/// Rule `j` can never fire because the earlier rule `i` matches everything it matches.
fn unreachable(d: usize, t: &DecisionTable, j: usize, i: usize) -> Vec<Fix> {
    let same = t.rules[i].inputs.iter().zip(&t.rules[j].inputs).all(|(a, b)| a.text.trim() == b.text.trim());
    let mut out = Vec::new();
    // With identical conditions, moving only makes the other rule unreachable.
    if !same {
        out.push(Fix {
            label: format!("Move rule {} above rule {}", j + 1, i + 1),
            hint: Some(format!("Inputs that match both rules then get rule {}'s outputs instead of rule {}'s.", j + 1, i + 1)),
            destructive: false,
            edits: vec![Edit::MoveRule { decision: d, from: j, to: i }],
        });
    }
    out.push(Fix { label: format!("Delete rule {}", j + 1), hint: Some("It can never fire, so no result changes.".into()), destructive: true, edits: vec![Edit::DeleteRule { decision: d, rule: j }] });
    out
}

/// Applies the edits in order; on error nothing is returned.
pub fn apply(xml: &str, edits: &[Edit]) -> Result<String, String> {
    let mut out = xml.to_string();
    for e in edits {
        out = edit::apply(&out, e)?;
    }
    Ok(out)
}

/// The from → to lines for one edit, read against the definitions before it is applied.
pub fn describe(defs: &Definitions, e: &Edit) -> Vec<Change> {
    let ch = |place: String, from: &str, to: &str| Change { place, from: from.to_string(), to: to.to_string() };
    let table = |d: usize| defs.table(d);
    let col_title = |t: &DecisionTable, c: Column| match c {
        Column::Input(i) => t.inputs.get(i).map_or("?", |x| x.title()).to_string(),
        Column::Output(o) => t.outputs.get(o).map_or("?", |x| x.title()).to_string(),
    };
    let any = |s: Option<&str>| s.filter(|s| !s.trim().is_empty()).unwrap_or("(any value)").to_string();
    match e {
        Edit::Cell { decision, rule, col, text } => {
            let Some(t) = table(*decision) else { return vec![] };
            let r = t.rules.get(*rule);
            let old = match col {
                Column::Input(i) => r.and_then(|r| r.inputs.get(*i)).map(|x| x.text.as_str()),
                Column::Output(o) => r.and_then(|r| r.outputs.get(*o)).map(|x| x.text.as_str()),
            };
            vec![ch(format!("Rule {} · {}", rule + 1, col_title(t, *col)), old.unwrap_or(""), text)]
        }
        Edit::CellLanguage { decision, rule, col, lang } => {
            let Some(t) = table(*decision) else { return vec![] };
            let r = t.rules.get(*rule);
            let old = match col {
                Column::Input(i) => r.and_then(|r| r.inputs.get(*i)).map(|x| &x.lang),
                Column::Output(o) => r.and_then(|r| r.outputs.get(*o)).map(|x| &x.lang),
            };
            let new = ExpressionLanguage::resolve(lang.as_deref(), defs.expression_language.as_deref());
            vec![ch(format!("Rule {} · {} · language", rule + 1, col_title(t, *col)), &old.map(language_name).unwrap_or_default(), &language_name(&new))]
        }
        Edit::InputHeader { decision, input, lang, allowed, .. } => {
            let Some(inp) = table(*decision).and_then(|t| t.inputs.get(*input)) else { return vec![] };
            let mut out = Vec::new();
            let explicit = inp.lang.declared.as_deref().filter(|_| inp.lang.is_explicit());
            if explicit != lang.as_deref() {
                let new = ExpressionLanguage::resolve(lang.as_deref(), defs.expression_language.as_deref());
                out.push(ch(format!("Input \"{}\" · language", inp.title()), &language_name(&inp.lang), &language_name(&new)));
            }
            if inp.input_values.as_deref() != allowed.as_deref() {
                out.push(ch(format!("Input \"{}\" · allowed values", inp.title()), &any(inp.input_values.as_deref()), &any(allowed.as_deref())));
            }
            out
        }
        Edit::OutputHeader { decision, output, allowed, .. } => {
            let Some(o) = table(*decision).and_then(|t| t.outputs.get(*output)) else { return vec![] };
            vec![ch(format!("Output \"{}\" · allowed values", o.title()), &any(o.output_values.as_deref()), &any(allowed.as_deref()))]
        }
        Edit::MoveRule { from, to, .. } => vec![ch(format!("Rule {}", from + 1), &format!("position {}", from + 1), &format!("position {}, above the current rule {}", to + 1, to + 1))],
        Edit::DeleteRule { decision, rule } => {
            let summary = table(*decision).and_then(|t| t.rules.get(*rule)).map(|r| {
                let side = |v: Vec<&str>| v.iter().map(|s| if s.trim().is_empty() { "-" } else { s.trim() }).collect::<Vec<_>>().join(" | ");
                format!("{}  →  {}", side(r.inputs.iter().map(|x| x.text.as_str()).collect()), side(r.outputs.iter().map(|x| x.text.as_str()).collect()))
            });
            vec![ch(format!("Rule {}", rule + 1), &summary.unwrap_or_default(), "(deleted; the rules after it move up one)")]
        }
        Edit::DeleteInput { decision, input } => {
            let Some(t) = table(*decision) else { return vec![] };
            let title = t.inputs.get(*input).map_or("?", |x| x.title());
            vec![ch(format!("Input \"{title}\""), &format!("\"-\" in all {} rules", t.rules.len()), "(deleted with its cell in every rule)")]
        }
        Edit::RenameDuplicateId { id } => vec![ch("Later elements with this id".into(), id, &format!("{id}_2, {id}_3, …"))],
        other => vec![ch("Edit".into(), "", &format!("{other:?}"))],
    }
}

fn language_name(l: &ExpressionLanguage) -> String {
    let name = match l.language {
        Language::Feel => "FEEL",
        Language::Js => "JavaScript",
        Language::Juel => "JUEL",
        Language::Unsupported => return l.short_name(),
    };
    if l.is_explicit() { name.to_string() } else { format!("{name} (default)") }
}

/// How the suite's results for `decision` change between two versions of the definitions.
/// `None` when the suite is empty or is not for this decision.
pub fn test_effect(before: &Definitions, after: &Definitions, decision: usize, s: &Suite) -> Option<TestEffect> {
    if s.cases.is_empty() || !suite::targets(&s.decision, before.decisions.get(decision)?) {
        return None;
    }
    let (tb, ta) = (before.table(decision)?, after.table(decision)?);
    let mut fx = TestEffect { total: s.cases.len(), ..TestEffect::default() };
    for c in &s.cases {
        let (b, a) = (suite::run_case(tb, c), suite::run_case(ta, c));
        fx.passed_before += usize::from(b.pass);
        fx.passed_after += usize::from(a.pass);
        match (b.pass, a.pass) {
            (true, false) => fx.broken.push(c.name.clone()),
            (false, true) => fx.fixed.push(c.name.clone()),
            (false, false) if b.problems != a.problems => fx.changed.push(c.name.clone()),
            _ => {}
        }
    }
    Some(fx)
}
