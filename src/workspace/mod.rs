mod document;
mod fixing;
mod rules;

use crate::diff::Hunk;
use crate::edit::Edit;
use crate::eval::Outcome;
use crate::export::{self, Format};
use crate::lint::{Column, Issue};
use crate::model::{Definitions, Rule};
use crate::session::TestSession;
use crate::suite::{self, CaseResult, Coverage, Suite};
use document::Document;
use fixing::{FixSession, Fixable};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use document::READ_ONLY;
pub use fixing::{Applied, FixView, Preview};
pub use rules::RuleClip;

const FIXING: &str = "The table is read-only while fixes are being reviewed. Stop the fix session to edit it.";
const FIX_FAILED: &str = "Fix not applied";

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum View {
    #[default]
    Rules,
    Issues,
    Test,
    Xml,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cursor {
    Cell(usize, Column),
    Note(usize),
}

pub struct Editing {
    pub at: Cursor,
    pub original: String,
    pub text: String,
    pub focus: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RuleOp {
    InsertAbove,
    InsertBelow,
    Duplicate,
    Delete,
    MoveUp,
    MoveDown,
}

pub struct Workspace {
    doc: Document,
    decision: usize,
    session: TestSession,
    fixing: Option<FixSession>,
    skipped: HashSet<String>,
    skip_gen: u64,
    fixable: RefCell<Option<Fixable>>,
    suite_load: Option<Result<usize, String>>,
    selected: Option<usize>,
    scroll_to: Option<usize>,
    view: View,
    editing: Option<Editing>,
    error: Option<String>,
    notice: Option<String>,
    clip: Option<RuleClip>,
}

pub struct Tests<'a> {
    session: &'a mut TestSession,
    pub defs: &'a Definitions,
    pub path: &'a Path,
    decision: usize,
}

impl Workspace {
    pub fn open(path: PathBuf) -> Result<Workspace, String> {
        let doc = Document::open(path)?;
        let mut session = TestSession::default();
        let suite_file = suite::default_path(doc.path());
        let suite_load = suite_file.exists().then(|| session.load(&suite_file));
        let error = (!doc.editable).then(|| READ_ONLY.to_string());
        Ok(Workspace {
            doc,
            decision: 0,
            session,
            fixing: None,
            skipped: HashSet::new(),
            skip_gen: 0,
            fixable: RefCell::new(None),
            suite_load,
            selected: None,
            scroll_to: None,
            view: View::default(),
            editing: None,
            error,
            notice: None,
            clip: None,
        })
    }

    pub fn adopt_view(&mut self, from: &Workspace) {
        self.view = from.view;
    }

    pub fn reload(&mut self) -> Result<(), String> {
        self.doc = Document::open(self.doc.path().to_path_buf())?;
        self.fixing = None;
        self.skipped.clear();
        self.skip_gen += 1;
        self.editing = None;
        self.selected = None;
        self.scroll_to = None;
        self.error = (!self.doc.editable).then(|| READ_ONLY.to_string());
        Ok(())
    }

    pub fn suite_load(&self) -> Option<&Result<usize, String>> {
        self.suite_load.as_ref()
    }

    pub fn defs(&self) -> &Definitions {
        &self.doc.defs
    }

    pub fn issues(&self) -> &[Issue] {
        &self.doc.issues
    }

    pub fn xml(&self) -> &str {
        &self.doc.xml
    }

    pub fn saved(&self) -> &str {
        self.doc.saved()
    }

    pub fn path(&self) -> &Path {
        self.doc.path()
    }

    pub fn editable(&self) -> bool {
        self.doc.editable
    }

    pub fn dirty(&self) -> bool {
        self.doc.dirty()
    }

    pub fn revision(&self) -> u64 {
        self.doc.revision()
    }

    pub fn changes(&self) -> Vec<Hunk> {
        self.doc.changes()
    }

    pub fn undo_depth(&self) -> usize {
        self.doc.undo_depth()
    }

    pub fn can_redo(&self) -> bool {
        self.doc.can_redo()
    }

    pub fn decision(&self) -> usize {
        self.decision
    }

    pub fn set_decision(&mut self, decision: usize) -> bool {
        self.flush_editing();
        if self.decision == decision {
            return false;
        }
        self.decision = decision;
        self.session.clear_fields();
        self.selected = None;
        true
    }

    fn write(&mut self, edits: &[Edit]) -> Result<bool, String> {
        if !self.doc.editable {
            return Err(READ_ONLY.into());
        }
        if self.fixing.is_some() {
            return Err(FIXING.into());
        }
        self.doc.commit(edits).map_err(|e| format!("Edit not applied: {e}"))
    }

    fn clamp(&mut self) {
        let n = self.doc.defs.table(self.decision).map_or(0, |t| t.rules.len());
        self.selected = self.selected.filter(|&s| s < n);
    }

    fn reveal(&mut self, rule: Option<usize>) {
        self.selected = rule;
        self.scroll_to = rule;
    }

    pub fn commit(&mut self, edit: Edit) {
        match self.write(std::slice::from_ref(&edit)) {
            Ok(false) => {}
            Ok(true) => match edit {
                Edit::InsertRule { at, .. } | Edit::MoveRule { to: at, .. } => self.reveal(Some(at)),
                Edit::DeleteRule { rule, .. } => self.selected = self.selected.filter(|&s| s != rule).map(|s| if s > rule { s - 1 } else { s }),
                _ => {}
            },
            Err(e) => self.error = Some(e),
        }
        self.clamp();
    }

    pub fn commit_group(&mut self, edits: &[Edit], select: Option<usize>) -> bool {
        let changed = match self.write(edits) {
            Ok(changed) => {
                if let (true, Some(r)) = (changed, select) {
                    self.reveal(Some(r));
                }
                changed
            }
            Err(e) => {
                self.error = Some(e);
                false
            }
        };
        self.clamp();
        changed
    }

    pub fn rule_op(&mut self, op: RuleOp) {
        let decision = self.decision;
        let nrules = self.doc.defs.table(decision).map_or(0, |t| t.rules.len());
        let edit = match (op, self.selected) {
            (RuleOp::InsertAbove, Some(r)) => Edit::InsertRule { decision, at: r, copy_of: None },
            (RuleOp::InsertBelow, sel) => Edit::InsertRule { decision, at: sel.map_or(nrules, |r| r + 1), copy_of: None },
            (RuleOp::Duplicate, Some(r)) => Edit::InsertRule { decision, at: r + 1, copy_of: Some(r) },
            (RuleOp::Delete, Some(r)) => Edit::DeleteRule { decision, rule: r },
            (RuleOp::MoveUp, Some(r)) if r > 0 => Edit::MoveRule { decision, from: r, to: r - 1 },
            (RuleOp::MoveDown, Some(r)) if r + 1 < nrules => Edit::MoveRule { decision, from: r, to: r + 1 },
            _ => return,
        };
        self.commit(edit);
    }

    pub fn copy_rule(&mut self) {
        let table = self.doc.defs.table(self.decision);
        let Some(clip) = table.zip(self.selected).and_then(|(t, r)| rules::copy(t, r)) else { return };
        self.clip = Some(clip);
        self.notice = Some("Rule copied".into());
    }

    pub fn paste_rule(&mut self) {
        let table = self.doc.defs.table(self.decision);
        let Some((edits, at)) = table.zip(self.clip.as_ref()).map(|(t, clip)| rules::paste(t, self.decision, self.selected, clip)) else { return };
        self.commit_group(&edits, Some(at));
    }

    pub fn has_clip(&self) -> bool {
        self.clip.is_some()
    }

    pub fn clip(&self) -> Option<&RuleClip> {
        self.clip.as_ref()
    }

    fn replace_edits(&self, find: &str, with: &str, case: bool) -> Vec<Edit> {
        self.doc.defs.table(self.decision).map_or_else(Vec::new, |t| rules::replace(t, self.decision, find, with, case))
    }

    pub fn replace_count(&self, find: &str, with: &str, case: bool) -> usize {
        self.replace_edits(find, with, case).len()
    }

    pub fn replace_all(&mut self, find: &str, with: &str, case: bool) -> bool {
        let edits = self.replace_edits(find, with, case);
        if edits.is_empty() || !self.commit_group(&edits, None) {
            return false;
        }
        self.notice = Some("Replaced in this decision. Undo reverts all of it at once".into());
        true
    }

    pub fn step_issue(&mut self, forward: bool) {
        match rules::next_issue(&self.doc.issues, self.decision, self.selected, forward) {
            Some(r) => {
                self.reveal(Some(r));
                self.view = View::Rules;
            }
            None => self.notice = Some("No issues on a rule in this decision".into()),
        }
    }

    pub fn export(&self, format: Format) -> Option<(String, String)> {
        let d = self.doc.defs.decisions.get(self.decision)?;
        let name = if d.name.is_empty() { d.id.clone() } else { d.name.clone() };
        Some((name, export::render(self.doc.defs.table(self.decision)?, format)))
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn toggle_select(&mut self, rule: usize) {
        self.selected = if self.selected == Some(rule) { None } else { Some(rule) };
    }

    pub fn deselect(&mut self) {
        self.selected = None;
    }

    pub fn select_and_reveal(&mut self, rule: usize) {
        self.reveal(Some(rule));
    }

    pub fn jump(&mut self, rule: usize) {
        self.reveal(Some(rule));
        self.view = View::Rules;
    }

    pub fn take_scroll(&mut self) -> Option<usize> {
        self.scroll_to.take()
    }

    pub fn restore_scroll(&mut self, rule: Option<usize>) {
        if self.scroll_to.is_none() {
            self.scroll_to = rule;
        }
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn set_view(&mut self, view: View) {
        self.view = view;
    }

    pub fn editing_at(&self) -> Option<Cursor> {
        self.editing.as_ref().map(|e| e.at)
    }

    pub fn take_editing(&mut self) -> Option<Editing> {
        self.editing.take()
    }

    pub fn restore_editing(&mut self, editing: Option<Editing>) {
        if self.editing.is_none() {
            self.editing = editing;
        }
    }

    pub fn flush_editing(&mut self) {
        if let Some(ed) = self.editing.take() {
            self.commit_editing(ed);
        }
    }

    fn commit_editing(&mut self, ed: Editing) {
        if ed.text == ed.original {
            return;
        }
        let decision = self.decision;
        self.commit(match ed.at {
            Cursor::Cell(rule, col) => Edit::Cell { decision, rule, col, text: ed.text },
            Cursor::Note(rule) => Edit::Note { decision, rule, text: ed.text },
        });
    }

    pub fn begin_edit(&mut self, at: Cursor, text: String) {
        self.flush_editing();
        self.editing = Some(Editing { at, original: text.clone(), text, focus: true });
    }

    pub fn end_edit(&mut self, at: Cursor, commit: bool) {
        if self.editing.as_ref().is_some_and(|ed| ed.at == at) {
            let ed = self.editing.take().expect("checked");
            if commit {
                self.commit_editing(ed);
            }
        }
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn clear_notice(&mut self) {
        self.notice = None;
    }

    fn history(&mut self, redo: bool) {
        self.editing = None;
        if let Err(e) = if redo { self.doc.redo() } else { self.doc.undo() } {
            self.error = Some(e);
        }
        self.clamp();
    }

    pub fn undo(&mut self) {
        if self.fixing.is_some() {
            self.fix_undo();
        } else {
            self.history(false);
        }
    }

    pub fn redo(&mut self) {
        if self.fixing.is_none() {
            self.history(true);
        }
    }

    pub fn discard(&mut self) {
        self.editing = None;
        if !self.doc.dirty() {
            return;
        }
        if let Err(e) = self.doc.discard() {
            self.error = Some(e);
        }
        self.clamp();
    }

    pub fn save(&mut self) -> bool {
        match self.doc.save() {
            Ok(()) => true,
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }

    pub fn save_as(&mut self, path: PathBuf) -> bool {
        match self.doc.save_as(path) {
            Ok(()) => true,
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }

    pub fn tests(&mut self) -> Tests<'_> {
        self.session.sync(self.doc.revision());
        Tests { session: &mut self.session, defs: &self.doc.defs, path: self.doc.path(), decision: self.decision }
    }
}

impl Tests<'_> {
    pub fn fields(&self) -> &[String] {
        self.session.fields()
    }

    pub fn fields_mut(&mut self) -> &mut Vec<String> {
        self.session.fields_mut()
    }

    pub fn is_active(&self) -> bool {
        self.session.is_active()
    }

    pub fn clear_inputs(&mut self) {
        self.session.clear_inputs();
    }

    pub fn suite(&self) -> &Suite {
        self.session.suite()
    }

    pub fn suite_path(&self) -> Option<&Path> {
        self.session.suite_path()
    }

    pub fn outcome(&mut self) -> Option<Arc<Outcome>> {
        self.session.outcome(self.defs, self.decision)
    }

    pub fn results(&mut self) -> Option<Arc<Vec<CaseResult>>> {
        self.session.results(self.defs, self.decision)
    }

    pub fn coverage(&mut self) -> Option<Arc<Coverage>> {
        let defs = self.defs;
        let suite = self.session.suite();
        if suite.cases.is_empty() || !defs.decisions.get(self.decision).is_some_and(|d| suite::targets(&suite.decision, d)) {
            return None;
        }
        self.session.coverage(defs, self.decision)
    }

    pub fn add_case(&mut self, name: &str) {
        self.session.add_case(self.defs, self.decision, name);
    }

    pub fn accept(&mut self, i: usize) {
        self.session.accept(self.defs, self.decision, i);
    }

    pub fn delete(&mut self, i: usize) {
        self.session.delete(i);
    }

    pub fn fill_from_case(&mut self, i: usize) {
        self.session.fill_from_case(self.defs, self.decision, i);
    }

    pub fn fill_from_rule(&mut self, rule: &Rule) {
        self.session.fill_from_rule(rule);
    }

    pub fn new_suite(&mut self) {
        let defs = self.defs;
        let name = defs.decisions.get(self.decision).map_or("", |d| d.name.as_str());
        self.session.new_suite(name);
    }

    pub fn load(&mut self, path: &Path) -> Result<usize, String> {
        self.session.load(path)
    }

    pub fn save(&mut self, path: &Path) -> Result<usize, String> {
        let defs = self.defs;
        let name = defs.decisions.get(self.decision).map_or("", |d| d.name.as_str());
        self.session.save(path, name)
    }
}
