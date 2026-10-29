pub mod document;
pub mod fixing;

use crate::diff::Hunk;
use crate::edit::Edit;
use crate::eval::Outcome;
use crate::fix;
use crate::lint::Issue;
use crate::model::{Definitions, Rule};
use crate::session::TestSession;
use crate::suite::{self, CaseResult, Coverage, Suite};
use document::Document;
use fixing::FixSession;
use std::cell::{Ref, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use document::READ_ONLY;

const FIXING: &str = "The table is read-only while fixes are being reviewed. Stop the fix session to edit it.";

#[derive(Default)]
struct Fixable {
    revision: u64,
    skip_gen: u64,
    keys: HashSet<String>,
    open: usize,
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
}

pub struct Tests<'a> {
    session: &'a mut TestSession,
    pub defs: &'a Definitions,
    pub path: &'a Path,
    decision: usize,
}

pub struct FixParts<'a> {
    pub session: &'a mut FixSession,
    pub doc: &'a Document,
    pub suite: &'a Suite,
    pub skipped: usize,
}

impl Workspace {
    pub fn open(path: PathBuf) -> Result<Workspace, String> {
        let doc = Document::open(path)?;
        let mut session = TestSession::default();
        let suite_file = suite::default_path(doc.path());
        let suite_load = suite_file.exists().then(|| session.load(&suite_file));
        Ok(Workspace { doc, decision: 0, session, fixing: None, skipped: HashSet::new(), skip_gen: 0, fixable: RefCell::new(None), suite_load })
    }

    pub fn reload(&mut self) -> Result<(), String> {
        self.doc = Document::open(self.doc.path().to_path_buf())?;
        self.fixing = None;
        self.skipped.clear();
        self.skip_gen += 1;
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

    pub fn decision(&self) -> usize {
        self.decision
    }

    pub fn set_decision(&mut self, decision: usize) -> bool {
        if self.decision == decision {
            return false;
        }
        self.decision = decision;
        self.session.clear_fields();
        true
    }

    pub fn commit(&mut self, edits: &[Edit]) -> Result<bool, String> {
        if !self.doc.editable {
            return Err(READ_ONLY.into());
        }
        if self.fixing.is_some() {
            return Err(FIXING.into());
        }
        self.doc.commit(edits).map_err(|e| format!("Edit not applied: {e}"))
    }

    pub fn undo(&mut self) -> Result<bool, String> {
        self.doc.undo()
    }

    pub fn redo(&mut self) -> Result<bool, String> {
        self.doc.redo()
    }

    pub fn discard(&mut self) -> Result<(), String> {
        self.doc.discard()
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.doc.save()
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<(), String> {
        self.doc.save_as(path)
    }

    pub fn tests(&mut self) -> Tests<'_> {
        self.session.sync(self.doc.revision());
        Tests { session: &mut self.session, defs: &self.doc.defs, path: self.doc.path(), decision: self.decision }
    }

    pub fn undo_depth(&self) -> usize {
        self.doc.undo_depth()
    }

    pub fn can_redo(&self) -> bool {
        self.doc.can_redo()
    }

    pub fn is_fixing(&self) -> bool {
        self.fixing.is_some()
    }

    pub fn fixing(&self) -> Option<&FixSession> {
        self.fixing.as_ref()
    }

    pub fn fix_parts(&mut self) -> Option<FixParts<'_>> {
        let session = self.fixing.as_mut()?;
        Some(FixParts { session, doc: &self.doc, suite: self.session.suite(), skipped: self.skipped.len() })
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
            let keys: HashSet<String> = if self.doc.editable { fix::queue(&self.doc.defs, &self.doc.issues, &HashSet::new(), None, None).into_iter().map(|c| c.key).collect() } else { HashSet::new() };
            let open = keys.iter().filter(|k| !self.skipped.contains(*k)).count();
            *self.fixable.borrow_mut() = Some(Fixable { revision: self.doc.revision(), skip_gen: self.skip_gen, keys, open });
        }
        Ref::map(self.fixable.borrow(), |f| f.as_ref().expect("filled above"))
    }

    pub fn fixable_count(&self) -> usize {
        self.fresh().open
    }

    pub fn fixable_key(&self, issue: &Issue) -> Option<String> {
        let key = fix::key(&self.doc.defs, issue);
        self.fresh().keys.contains(&key).then_some(key)
    }

    pub fn fix_start(&mut self, only: Option<String>) -> Result<bool, String> {
        if self.fixing.is_some() {
            return Ok(false);
        }
        if !self.doc.editable {
            return Err(READ_ONLY.into());
        }
        self.fixing = Some(FixSession::start(&self.doc, &self.skipped, only));
        Ok(true)
    }

    pub fn fix_choose(&mut self, i: usize) {
        if let Some(s) = self.fixing.as_mut() {
            s.choose(i);
        }
    }

    pub fn fix_move(&mut self, down: bool) {
        if let Some(s) = self.fixing.as_mut() {
            s.move_choice(down);
        }
    }

    pub fn fix_apply(&mut self) -> Result<(), String> {
        match self.fixing.as_mut() {
            Some(s) => s.apply(&mut self.doc, &self.skipped),
            None => Ok(()),
        }
    }

    pub fn fix_skip(&mut self) {
        if let Some(s) = self.fixing.as_mut() {
            s.skip(&self.doc, &mut self.skipped);
            self.skip_gen += 1;
        }
    }

    pub fn fix_stop(&mut self) {
        if let Some(s) = self.fixing.as_mut() {
            s.stop();
        }
    }

    pub fn fix_undo(&mut self) -> Result<(), String> {
        match self.fixing.as_mut() {
            Some(s) => s.undo(&mut self.doc, &self.skipped).map(|_| ()),
            None => Ok(()),
        }
    }

    pub fn fix_undo_all(&mut self) -> Result<(), String> {
        match self.fixing.as_mut() {
            Some(s) => s.undo_all(&mut self.doc, &self.skipped),
            None => Ok(()),
        }
    }

    pub fn fix_reset_skipped(&mut self) {
        self.skipped.clear();
        self.skip_gen += 1;
        if let Some(s) = self.fixing.as_mut() {
            s.settle(&self.doc, &self.skipped);
        }
    }

    pub fn fix_close(&mut self) {
        self.fixing = None;
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
        self.session.coverage(self.defs, self.decision)
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
