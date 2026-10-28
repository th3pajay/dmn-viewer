use crate::edit::{Column, Edit};
use crate::lint::Issue;
use crate::model::DecisionTable;

#[derive(Clone, Debug, PartialEq)]
pub struct RuleClip {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub note: String,
}

pub fn copy(table: &DecisionTable, rule: usize) -> Option<RuleClip> {
    let r = table.rules.get(rule)?;
    Some(RuleClip { inputs: r.inputs.iter().map(|e| e.text.clone()).collect(), outputs: r.outputs.iter().map(|e| e.text.clone()).collect(), note: table.note(r).to_string() })
}

pub fn paste(table: &DecisionTable, decision: usize, after: Option<usize>, clip: &RuleClip) -> (Vec<Edit>, usize) {
    let at = after.map_or(table.rules.len(), |r| r + 1);
    let mut edits = vec![Edit::InsertRule { decision, at, copy_of: None }];
    let inputs = clip.inputs.iter().take(table.inputs.len()).enumerate().map(|(i, x)| (Column::Input(i), x));
    let outputs = clip.outputs.iter().take(table.outputs.len()).enumerate().map(|(i, x)| (Column::Output(i), x));
    edits.extend(inputs.chain(outputs).filter(|(_, x)| !x.trim().is_empty()).map(|(col, x)| Edit::Cell { decision, rule: at, col, text: x.clone() }));
    if !clip.note.is_empty() {
        edits.push(Edit::Note { decision, rule: at, text: clip.note.clone() });
    }
    (edits, at)
}

pub fn replace_text(text: &str, find: &str, with: &str, case: bool) -> Option<String> {
    if find.is_empty() {
        return None;
    }
    let (hay, needle) = if case { (text.to_string(), find.to_string()) } else { (text.to_ascii_lowercase(), find.to_ascii_lowercase()) };
    if !hay.contains(&needle) {
        return None;
    }
    let (mut out, mut from) = (String::new(), 0);
    while let Some(i) = hay[from..].find(&needle) {
        out.push_str(&text[from..from + i]);
        out.push_str(with);
        from += i + needle.len();
    }
    out.push_str(&text[from..]);
    Some(out)
}

pub fn replace(table: &DecisionTable, decision: usize, find: &str, with: &str, case: bool) -> Vec<Edit> {
    let mut edits = Vec::new();
    for (rule, r) in table.rules.iter().enumerate() {
        let inputs = r.inputs.iter().enumerate().map(|(i, e)| (Column::Input(i), &e.text));
        let outputs = r.outputs.iter().enumerate().map(|(i, e)| (Column::Output(i), &e.text));
        for (col, text) in inputs.chain(outputs) {
            if let Some(text) = replace_text(text, find, with, case) {
                edits.push(Edit::Cell { decision, rule, col, text });
            }
        }
    }
    edits
}

pub fn next_issue(issues: &[Issue], decision: usize, selected: Option<usize>, forward: bool) -> Option<usize> {
    let mut rules: Vec<usize> = issues.iter().filter(|i| i.decision == Some(decision)).flat_map(|i| i.rules.iter().copied()).collect();
    rules.sort_unstable();
    rules.dedup();
    let pick = match (selected, forward) {
        (Some(s), true) => rules.iter().find(|&&r| r > s).or(rules.first()),
        (Some(s), false) => rules.iter().rev().find(|&&r| r < s).or(rules.last()),
        (None, true) => rules.first(),
        (None, false) => rules.last(),
    };
    pick.copied()
}
