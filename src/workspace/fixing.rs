use super::document::Document;
use crate::diff::{self, Hunk, Spans};
use crate::fix::{self, Candidate, Change, TestEffect};
use crate::suite::Suite;
use std::collections::HashSet;

pub struct Applied {
    pub title: &'static str,
    pub label: String,
    pub changes: Vec<Change>,
    key: String,
}

pub struct Preview {
    for_step: (String, usize, usize),
    pub changes: Vec<Change>,
    pub tests: Option<TestEffect>,
    pub lines: Vec<(usize, String, String)>,
    pub error: Option<String>,
}

pub struct FixSession {
    start_depth: usize,
    only: Option<String>,
    front: Option<String>,
    queue: Vec<Candidate>,
    pub choice: Option<usize>,
    pub applied: Vec<Applied>,
    pub skipped_now: Vec<&'static str>,
    pub summary: bool,
    pub undone_all: bool,
    preview: Option<Preview>,
}

const PREVIEW_HUNKS: usize = 12;
const PREVIEW_LINES: usize = 12;

impl FixSession {
    pub fn start(doc: &Document, skipped: &HashSet<String>, only: Option<String>) -> FixSession {
        let mut s =
            FixSession { start_depth: doc.undo_depth(), only, front: None, queue: vec![], choice: None, applied: vec![], skipped_now: vec![], summary: false, undone_all: false, preview: None };
        s.settle(doc, skipped);
        s
    }

    pub fn current(&self) -> Option<&Candidate> {
        if self.summary { None } else { self.queue.first() }
    }

    pub fn progress(&self) -> (usize, usize) {
        let done = self.applied.len() + self.skipped_now.len();
        (done + 1, done + self.queue.len())
    }

    pub fn can_undo(&self, doc: &Document) -> bool {
        doc.undo_depth() > self.start_depth
    }

    pub fn choose(&mut self, i: usize) {
        if self.current().is_some_and(|c| i < c.fixes.len()) {
            self.choice = Some(i);
        }
    }

    pub fn move_choice(&mut self, down: bool) {
        let Some(n) = self.current().map(|c| c.fixes.len()) else { return };
        self.choice = Some(match (self.choice, down) {
            (None, _) => 0,
            (Some(i), true) => (i + 1).min(n - 1),
            (Some(i), false) => i.saturating_sub(1),
        });
    }

    pub fn apply(&mut self, doc: &mut Document, skipped: &HashSet<String>) -> Result<(), String> {
        let Some(c) = self.current() else { return Err("There is no fix to apply.".into()) };
        let Some(f) = self.choice.and_then(|i| c.fixes.get(i)) else { return Err("This fix deletes something, so pick it yourself first.".into()) };
        let changes = f.edits.iter().flat_map(|e| fix::describe(&doc.defs, e)).collect();
        let applied = Applied { title: c.issue.title(), label: f.label.clone(), changes, key: c.key.clone() };
        doc.commit(&f.edits)?;
        self.applied.push(applied);
        self.front = None;
        self.settle(doc, skipped);
        Ok(())
    }

    pub fn skip(&mut self, doc: &Document, skipped: &mut HashSet<String>) {
        let Some((title, key)) = self.current().map(|c| (c.issue.title(), c.key.clone())) else { return };
        self.skipped_now.push(title);
        skipped.insert(key);
        self.front = None;
        self.settle(doc, skipped);
    }

    pub fn stop(&mut self) {
        self.summary = true;
    }

    pub fn undo(&mut self, doc: &mut Document, skipped: &HashSet<String>) -> Result<bool, String> {
        if !self.can_undo(doc) {
            return Ok(false);
        }
        doc.undo()?;
        let a = self.applied.pop();
        self.front = a.map(|a| a.key);
        self.summary = false;
        self.settle(doc, skipped);
        Ok(true)
    }

    pub fn undo_all(&mut self, doc: &mut Document, skipped: &HashSet<String>) -> Result<(), String> {
        while self.can_undo(doc) {
            doc.undo()?;
        }
        self.applied.clear();
        self.undone_all = true;
        self.front = None;
        self.settle(doc, skipped);
        self.summary = true;
        Ok(())
    }

    pub fn settle(&mut self, doc: &Document, skipped: &HashSet<String>) {
        let before = self.queue.first().map(|c| c.key.clone());
        self.queue = fix::queue(&doc.defs, &doc.issues, skipped, self.front.as_deref(), self.only.as_deref());
        self.preview = None;
        match self.queue.first() {
            None => {
                self.summary = true;
                self.choice = None;
            }
            Some(c) if before.as_deref() != Some(c.key.as_str()) || self.choice.is_none_or(|i| i >= c.fixes.len()) => self.choice = c.default_fix(),
            Some(_) => {}
        }
    }

    pub fn preview(&mut self, doc: &Document, suite: &Suite) -> Option<&Preview> {
        let c = self.current()?;
        let i = self.choice?;
        let f = c.fixes.get(i)?;
        let for_step = (c.key.clone(), i, doc.undo_depth());
        if self.preview.as_ref().is_none_or(|p| p.for_step != for_step) {
            let changes = f.edits.iter().flat_map(|e| fix::describe(&doc.defs, e)).collect();
            let decision = c.issue.decision;
            let mut p = Preview { for_step, changes, tests: None, lines: vec![], error: None };
            match doc.trial(&f.edits) {
                Ok(trial) => {
                    let after = trial.defs.as_ref().unwrap_or(&doc.defs);
                    p.tests = decision.and_then(|d| fix::test_effect(&doc.defs, after, d, suite));
                    p.lines = lines(&doc.xml, &trial.xml, &diff::hunks(&doc.xml, &trial.xml, &Spans::default()));
                }
                Err(e) => p.error = Some(e),
            }
            self.preview = Some(p);
        }
        self.preview.as_ref()
    }
}

fn lines(old: &str, new: &str, hunks: &[Hunk]) -> Vec<(usize, String, String)> {
    let cut = |s: &str| {
        let mut out: Vec<&str> = s.lines().take(PREVIEW_LINES).collect();
        let more = s.lines().count().saturating_sub(PREVIEW_LINES);
        if more > 0 {
            out.push("");
            return format!("{}… {more} more lines", out.join("\n"));
        }
        out.join("\n")
    };
    hunks.iter().take(PREVIEW_HUNKS).map(|h| (h.new_line, cut(&old[h.old.clone()]), cut(&new[h.new.clone()]))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const RULE: &str = r#"<rule id="R{n}"><inputEntry id="I{n}"><text>{in}</text></inputEntry><outputEntry id="O{n}"><text>{out}</text></outputEntry></rule>"#;

    fn doc(name: &str, rules: &[(&str, &str)]) -> Document {
        let body: String = rules.iter().enumerate().map(|(n, (i, o))| RULE.replace("{n}", &n.to_string()).replace("{in}", i).replace("{out}", o)).collect();
        let xml = format!(
            r#"<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="D" name="D" namespace="x"><decision id="Dec" name="Dec"><decisionTable id="T" hitPolicy="FIRST"><input id="In"><inputExpression id="IE" typeRef="string"><text>Region</text></inputExpression><inputValues id="IV"><text>"EU","NA"</text></inputValues></input><output id="Out" name="Carrier" typeRef="string"/>{body}</decisionTable></decision></definitions>"#
        );
        let dir = std::env::temp_dir().join(format!("dmn-viewer-fixing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p: PathBuf = dir.join(name);
        std::fs::write(&p, xml).unwrap();
        Document::open(p).unwrap()
    }

    fn messy(name: &str) -> Document {
        doc(name, &[(r#""LATAM""#, r#""A""#), ("-", r#""B""#), (r#""EU""#, r#""C""#)])
    }

    #[test]
    fn apply_skip_and_summary() {
        let mut d = messy("apply.dmn");
        let mut skipped = HashSet::new();
        let mut s = FixSession::start(&d, &skipped, None);
        assert_eq!(s.progress(), (1, 2));
        assert_eq!(s.current().unwrap().issue.code, "NOT_IN_ALLOWED_VALUES");
        assert_eq!(s.choice, Some(0));
        s.apply(&mut d, &skipped).unwrap();
        assert!(d.xml.contains(r#""EU","NA","LATAM""#));
        assert_eq!(s.current().unwrap().issue.code, "UNREACHABLE_RULE");
        assert_eq!(s.progress(), (2, 2));
        s.skip(&d, &mut skipped);
        assert!(s.summary && s.current().is_none());
        assert_eq!((s.applied.len(), s.skipped_now.clone()), (1, vec!["Unreachable rule"]));
        let s2 = FixSession::start(&d, &skipped, None);
        assert!(s2.summary);
    }

    #[test]
    fn deletions_need_a_choice() {
        let mut d = doc("delete.dmn", &[("-", r#""B""#), ("-", r#""B""#)]);
        let skipped = HashSet::new();
        let mut s = FixSession::start(&d, &skipped, None);
        assert_eq!(s.current().unwrap().issue.code, "UNREACHABLE_RULE");
        assert_eq!(s.choice, None);
        assert!(s.apply(&mut d, &skipped).is_err());
        assert_eq!(d.undo_depth(), 0);
        s.move_choice(true);
        s.apply(&mut d, &skipped).unwrap();
        assert_eq!(d.defs.table(0).unwrap().rules.len(), 1);
    }

    #[test]
    fn undo_brings_the_issue_back_first_and_stops_at_the_session_start() {
        let mut d = messy("undo.dmn");
        let skipped = HashSet::new();
        d.commit(&[crate::edit::Edit::Cell { decision: 0, rule: 2, col: crate::edit::Column::Output(0), text: r#""D""#.into() }]).unwrap();
        let start = d.xml.clone();
        let mut s = FixSession::start(&d, &skipped, None);
        let first = s.current().unwrap().key.clone();
        s.apply(&mut d, &skipped).unwrap();
        s.apply(&mut d, &skipped).unwrap();
        assert!(s.summary);
        assert!(s.undo(&mut d, &skipped).unwrap());
        assert!(!s.summary);
        assert_eq!(s.current().unwrap().issue.code, "UNREACHABLE_RULE");
        s.undo(&mut d, &skipped).unwrap();
        assert_eq!(s.current().unwrap().key, first);
        assert_eq!(d.xml, start);
        assert!(!s.undo(&mut d, &skipped).unwrap(), "the edit before the session stays");
        assert!(d.xml.contains(r#""D""#));

        s.apply(&mut d, &skipped).unwrap();
        s.undo_all(&mut d, &skipped).unwrap();
        assert!(s.summary && s.undone_all && s.applied.is_empty());
        assert_eq!(d.xml, start);
    }

    #[test]
    fn one_issue_sessions_and_previews() {
        let mut d = messy("one.dmn");
        let skipped = HashSet::new();
        let all = FixSession::start(&d, &skipped, None);
        let last = all.queue.last().unwrap().key.clone();
        let mut s = FixSession::start(&d, &skipped, Some(last));
        assert_eq!(s.progress(), (1, 1));
        let suite = Suite::default();
        let p = s.preview(&d, &suite).unwrap();
        assert!(p.error.is_none() && p.tests.is_none());
        assert_eq!(p.changes[0].place, "Rule 3");
        assert!(!p.lines.is_empty());
        s.apply(&mut d, &skipped).unwrap();
        assert!(s.summary, "one issue, then the summary");
    }
}
