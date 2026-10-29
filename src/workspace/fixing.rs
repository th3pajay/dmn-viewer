use super::{FIX_FAILED, READ_ONLY, View, Workspace};
use crate::diff::{self, Hunk, Spans};
use crate::fix::{self, Candidate, Change, TestEffect};
use crate::lint::Issue;
use std::cell::Ref;
use std::collections::HashSet;

pub struct Applied {
    pub title: &'static str,
    pub label: String,
    pub changes: Vec<Change>,
    key: String,
}

pub struct Preview {
    key: String,
    choice: usize,
    depth: usize,
    pub changes: Vec<Change>,
    pub tests: Option<TestEffect>,
    pub lines: Vec<(usize, String, String)>,
    pub error: Option<String>,
}

pub(super) struct FixSession {
    start_depth: usize,
    only: Option<String>,
    front: Option<String>,
    queue: Vec<Candidate>,
    choice: Option<usize>,
    applied: Vec<Applied>,
    skipped_now: Vec<&'static str>,
    summary: bool,
    undone_all: bool,
    preview: Option<Preview>,
}

pub(super) struct Fixable {
    pub(super) revision: u64,
    pub(super) skip_gen: u64,
    pub(super) keys: HashSet<String>,
    pub(super) ids: fix::Keys,
    pub(super) open: usize,
}

pub struct FixView<'a> {
    pub current: Option<&'a Candidate>,
    pub progress: (usize, usize),
    pub choice: Option<usize>,
    pub preview: Option<&'a Preview>,
    pub can_undo: bool,
    pub applied: &'a [Applied],
    pub skipped_now: &'a [&'static str],
    pub undone_all: bool,
    pub skipped: usize,
    pub dirty: bool,
}

const PREVIEW_HUNKS: usize = 12;
const PREVIEW_LINES: usize = 12;

impl FixSession {
    fn new(start_depth: usize, only: Option<String>) -> FixSession {
        FixSession { start_depth, only, front: None, queue: vec![], choice: None, applied: vec![], skipped_now: vec![], summary: false, undone_all: false, preview: None }
    }

    fn current(&self) -> Option<&Candidate> {
        if self.summary { None } else { self.queue.first() }
    }

    fn progress(&self) -> (usize, usize) {
        let done = self.applied.len() + self.skipped_now.len();
        (done + 1, done + self.queue.len())
    }

    fn choose(&mut self, i: usize) {
        if self.current().is_some_and(|c| i < c.fixes.len()) {
            self.choice = Some(i);
        }
    }

    fn move_choice(&mut self, down: bool) {
        let Some(n) = self.current().map(|c| c.fixes.len()) else { return };
        self.choice = Some(match (self.choice, down) {
            (None, _) => 0,
            (Some(i), true) => (i + 1).min(n - 1),
            (Some(i), false) => i.saturating_sub(1),
        });
    }
}

impl Workspace {
    pub fn is_fixing(&self) -> bool {
        self.fixing.is_some()
    }

    pub fn fix_offering(&self) -> bool {
        self.fixing.as_ref().is_some_and(|s| s.current().is_some())
    }

    pub fn skipped_count(&self) -> usize {
        self.skipped.len()
    }

    pub fn is_skipped(&self, key: &str) -> bool {
        self.skipped.contains(key)
    }

    fn fresh(&self) -> Ref<'_, Fixable> {
        let stale = self.fixable.borrow().as_ref().is_none_or(|f| f.revision != self.doc.revision() || f.skip_gen != self.skip_gen);
        if stale {
            let ids = fix::Keys::new(&self.doc.defs);
            let keys: HashSet<String> =
                if self.doc.editable { fix::queue_with(&ids, &self.doc.defs, &self.doc.issues, &HashSet::new(), None, None).into_iter().map(|c| c.key).collect() } else { HashSet::new() };
            let open = keys.iter().filter(|k| !self.skipped.contains(*k)).count();
            *self.fixable.borrow_mut() = Some(Fixable { revision: self.doc.revision(), skip_gen: self.skip_gen, keys, ids, open });
        }
        Ref::map(self.fixable.borrow(), |f| f.as_ref().expect("filled above"))
    }

    pub fn fixable_count(&self) -> usize {
        self.fresh().open
    }

    pub fn fixable_key(&self, issue: &Issue) -> Option<String> {
        let fresh = self.fresh();
        let key = fresh.ids.key(&self.doc.defs, issue);
        fresh.keys.contains(&key).then_some(key)
    }

    pub fn fix_view(&mut self) -> Option<FixView<'_>> {
        self.ensure_preview();
        let s = self.fixing.as_ref()?;
        let current = s.current();
        Some(FixView {
            current,
            progress: s.progress(),
            choice: s.choice,
            preview: s.preview.as_ref().filter(|_| current.is_some() && s.choice.is_some()),
            can_undo: self.doc.undo_depth() > s.start_depth,
            applied: &s.applied,
            skipped_now: &s.skipped_now,
            undone_all: s.undone_all,
            skipped: self.skipped.len(),
            dirty: self.doc.dirty(),
        })
    }

    fn settle(&mut self) {
        let Some(s) = self.fixing.as_mut() else { return };
        let before = s.queue.first().map(|c| c.key.clone());
        s.queue = fix::queue(&self.doc.defs, &self.doc.issues, &self.skipped, s.front.as_deref(), s.only.as_deref());
        s.preview = None;
        match s.queue.first() {
            None => {
                s.summary = true;
                s.choice = None;
            }
            Some(c) if before.as_deref() != Some(c.key.as_str()) || s.choice.is_none_or(|i| i >= c.fixes.len()) => s.choice = c.default_fix(),
            Some(_) => {}
        }
    }

    fn ensure_preview(&mut self) {
        let Some(s) = self.fixing.as_ref() else { return };
        let (Some(c), Some(i)) = (s.current(), s.choice) else { return };
        let Some(f) = c.fixes.get(i) else { return };
        let depth = self.doc.undo_depth();
        if s.preview.as_ref().is_some_and(|p| p.choice == i && p.depth == depth && p.key == c.key) {
            return;
        }
        let changes = f.edits.iter().flat_map(|e| fix::describe(&self.doc.defs, e)).collect();
        let mut p = Preview { key: c.key.clone(), choice: i, depth, changes, tests: None, lines: vec![], error: None };
        match self.doc.trial(&f.edits) {
            Ok(trial) => {
                let after = trial.defs.as_ref().unwrap_or(&self.doc.defs);
                p.tests = c.issue.decision.and_then(|d| fix::test_effect(&self.doc.defs, after, d, self.session.suite()));
                p.lines = lines(&self.doc.xml, &trial.xml, &diff::hunks(&self.doc.xml, &trial.xml, &Spans::default()));
            }
            Err(e) => p.error = Some(e),
        }
        if let Some(s) = self.fixing.as_mut() {
            s.preview = Some(p);
        }
    }

    fn follow_fix(&mut self) {
        let Some((decision, rule)) = self.fixing.as_ref().and_then(|s| s.current()).map(|c| (c.issue.decision, c.issue.rules.iter().min().copied())) else { return };
        if let Some(d) = decision {
            self.set_decision(d);
        }
        self.reveal(rule);
    }

    fn fix_step(&mut self, step: impl FnOnce(&mut Self) -> Result<(), String>) {
        self.flush_editing();
        match step(self) {
            Ok(()) => {
                if self.error.as_deref().is_some_and(|e| e.starts_with(FIX_FAILED)) {
                    self.error = None;
                }
            }
            Err(e) => self.error = Some(format!("{FIX_FAILED}: {e}")),
        }
        self.follow_fix();
        self.clamp();
    }

    pub fn fix_start(&mut self, only: Option<String>) -> bool {
        let mut started = false;
        self.fix_step(|ws| {
            if ws.fixing.is_some() {
                return Ok(());
            }
            if !ws.doc.editable {
                return Err(READ_ONLY.into());
            }
            ws.fixing = Some(FixSession::new(ws.doc.undo_depth(), only));
            ws.settle();
            ws.view = View::Rules;
            started = true;
            Ok(())
        });
        started
    }

    pub fn fix_choose(&mut self, i: usize) {
        self.fix_step(|ws| {
            if let Some(s) = ws.fixing.as_mut() {
                s.choose(i);
            }
            Ok(())
        });
    }

    pub fn fix_move(&mut self, down: bool) {
        self.fix_step(|ws| {
            if let Some(s) = ws.fixing.as_mut() {
                s.move_choice(down);
            }
            Ok(())
        });
    }

    pub fn fix_apply(&mut self) {
        self.fix_step(|ws| {
            let Some(s) = ws.fixing.as_mut() else { return Ok(()) };
            let Some(c) = s.current() else { return Err("There is no fix to apply.".into()) };
            let Some(f) = s.choice.and_then(|i| c.fixes.get(i)) else { return Err("This fix deletes something, so pick it yourself first.".into()) };
            let changes = f.edits.iter().flat_map(|e| fix::describe(&ws.doc.defs, e)).collect();
            let applied = Applied { title: c.issue.title(), label: f.label.clone(), changes, key: c.key.clone() };
            ws.doc.commit(&f.edits)?;
            s.applied.push(applied);
            s.front = None;
            ws.settle();
            Ok(())
        });
    }

    pub fn fix_skip(&mut self) {
        self.fix_step(|ws| {
            let Some(s) = ws.fixing.as_mut() else { return Ok(()) };
            let Some((title, key)) = s.current().map(|c| (c.issue.title(), c.key.clone())) else { return Ok(()) };
            s.skipped_now.push(title);
            s.front = None;
            ws.skipped.insert(key);
            ws.skip_gen += 1;
            ws.settle();
            Ok(())
        });
    }

    pub fn fix_stop(&mut self) {
        self.fix_step(|ws| {
            if let Some(s) = ws.fixing.as_mut() {
                s.summary = true;
            }
            Ok(())
        });
    }

    pub fn fix_undo(&mut self) {
        self.fix_step(|ws| {
            let Some(s) = ws.fixing.as_mut() else { return Ok(()) };
            if ws.doc.undo_depth() <= s.start_depth {
                return Ok(());
            }
            ws.doc.undo()?;
            s.front = s.applied.pop().map(|a| a.key);
            s.summary = false;
            ws.settle();
            Ok(())
        });
    }

    pub fn fix_undo_all(&mut self) {
        self.fix_step(|ws| {
            let Some(s) = ws.fixing.as_mut() else { return Ok(()) };
            while ws.doc.undo_depth() > s.start_depth {
                ws.doc.undo()?;
            }
            s.applied.clear();
            s.undone_all = true;
            s.front = None;
            ws.settle();
            if let Some(s) = ws.fixing.as_mut() {
                s.summary = true;
            }
            Ok(())
        });
    }

    pub fn fix_reset_skipped(&mut self) {
        self.fix_step(|ws| {
            ws.skipped.clear();
            ws.skip_gen += 1;
            ws.settle();
            Ok(())
        });
    }

    pub fn fix_close(&mut self) {
        self.fix_step(|ws| {
            ws.fixing = None;
            Ok(())
        });
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
