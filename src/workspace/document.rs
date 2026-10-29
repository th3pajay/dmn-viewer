use crate::diff::{self, Hunk, Spans};
use crate::edit::{Column, Edit};
use crate::fix;
use crate::lint::{self, Issue};
use crate::model::{DecisionLogic, Definitions};
use crate::parse;
use crate::unary::UnaryTests;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static REVISIONS: AtomicU64 = AtomicU64::new(1);

fn next_revision() -> u64 {
    REVISIONS.fetch_add(1, Ordering::Relaxed)
}

pub const READ_ONLY: &str = "This file is not valid UTF-8, so it is open read-only: saving it could change bytes outside the edits.";

#[derive(Clone, Debug)]
struct CellText {
    decision: usize,
    rule: usize,
    col: Column,
    text: String,
}

struct Change {
    at: usize,
    len: usize,
    text: String,
    cell: Option<CellText>,
}

pub(super) struct Trial {
    pub(super) xml: String,
    pub(super) defs: Option<Definitions>,
}

pub struct Document {
    pub(super) path: PathBuf,
    pub(super) xml: String,
    pub(super) defs: Definitions,
    pub(super) issues: Vec<Issue>,
    pub(super) editable: bool,
    saved: String,
    dirty: bool,
    undo: Vec<Change>,
    redo: Vec<Change>,
    spans: Spans,
    revision: u64,
}

fn newlines(s: &str) -> i64 {
    s.bytes().filter(|&b| b == b'\n').count() as i64
}

impl Document {
    pub fn open(path: PathBuf) -> Result<Document, String> {
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let (xml, editable) = match String::from_utf8(bytes) {
            Ok(s) => (s, true),
            Err(e) => (String::from_utf8_lossy(e.as_bytes()).into_owned(), false),
        };
        let defs = parse::parse(&xml)?;
        let issues = lint::lint(&defs);
        Ok(Document { path, saved: xml.clone(), xml, defs, issues, editable, dirty: false, undo: vec![], redo: vec![], spans: Spans::default(), revision: next_revision() })
    }

    pub fn defs(&self) -> &Definitions {
        &self.defs
    }

    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    pub fn xml(&self) -> &str {
        &self.xml
    }

    pub fn editable(&self) -> bool {
        self.editable
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn commit(&mut self, edits: &[Edit]) -> Result<bool, String> {
        if let [Edit::Cell { decision, rule, col, text }] = edits {
            let new = fix::apply(&self.xml, edits)?;
            if new == self.xml {
                return Ok(false);
            }
            let cell = CellText { decision: *decision, rule: *rule, col: *col, text: text.clone() };
            let before = CellText { text: self.cell_text(&cell), ..cell.clone() };
            let delta = self.record(new, Some(before));
            self.set_cell(&cell, delta);
        } else {
            let trial = self.trial(edits)?;
            let Some(defs) = trial.defs else { return Ok(false) };
            self.record(trial.xml, None);
            self.defs = defs;
        }
        self.issues = lint::lint(&self.defs);
        Ok(true)
    }

    pub(super) fn trial(&self, edits: &[Edit]) -> Result<Trial, String> {
        let xml = fix::apply_unchecked(&self.xml, edits)?;
        if xml == self.xml {
            return Ok(Trial { xml, defs: None });
        }
        let defs = parse::parse(&xml).map_err(|e| match e.strip_prefix("XML error: ") {
            Some(reason) => format!("the edit would produce invalid XML: {reason}"),
            None => e,
        })?;
        Ok(Trial { xml, defs: Some(defs) })
    }

    fn record(&mut self, new: String, cell: Option<CellText>) -> i64 {
        let (at, old_end, new_end) = diff::changed(&self.xml, &new);
        let delta = newlines(&new[at..new_end]) - newlines(&self.xml[at..old_end]);
        self.undo.push(Change { at, len: new_end - at, text: self.xml[at..old_end].to_string(), cell });
        self.redo.clear();
        self.spans.splice(at, old_end - at, new_end - at);
        self.xml = new;
        self.refresh_dirty();
        delta
    }

    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    pub fn undo(&mut self) -> Result<bool, String> {
        self.travel(false)
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn redo(&mut self) -> Result<bool, String> {
        self.travel(true)
    }

    pub fn discard(&mut self) -> Result<(), String> {
        if !self.dirty {
            return Ok(());
        }
        self.record(self.saved.clone(), None);
        self.reparse()
    }

    pub fn save(&mut self) -> Result<(), String> {
        std::fs::write(&self.path, &self.xml).map_err(|e| format!("Could not save {}: {e}", self.path.display()))?;
        self.saved.clone_from(&self.xml);
        self.dirty = false;
        self.spans.clear();
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<(), String> {
        let old = std::mem::replace(&mut self.path, path);
        self.save().inspect_err(|_| self.path = old)
    }

    pub fn changes(&self) -> Vec<Hunk> {
        diff::hunks(&self.saved, &self.xml, &self.spans)
    }

    pub fn saved(&self) -> &str {
        &self.saved
    }

    fn travel(&mut self, redo: bool) -> Result<bool, String> {
        let Some(c) = (if redo { self.redo.pop() } else { self.undo.pop() }) else { return Ok(false) };
        let end = c.at + c.len;
        let current = self.xml[c.at..end].to_string();
        let delta = newlines(&c.text) - newlines(&current);
        let inverse = Change { at: c.at, len: c.text.len(), cell: c.cell.as_ref().map(|h| CellText { text: self.cell_text(h), ..h.clone() }), text: current };
        if redo { &mut self.undo } else { &mut self.redo }.push(inverse);
        self.spans.splice(c.at, c.len, c.text.len());
        self.xml.replace_range(c.at..end, &c.text);
        self.refresh_dirty();
        match &c.cell {
            Some(h) => {
                self.set_cell(h, delta);
                self.issues = lint::lint(&self.defs);
            }
            None => self.reparse()?,
        }
        Ok(true)
    }

    fn reparse(&mut self) -> Result<(), String> {
        let defs = parse::parse(&self.xml)?;
        self.issues = lint::lint(&defs);
        self.defs = defs;
        Ok(())
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn refresh_dirty(&mut self) {
        self.revision = next_revision();
        self.dirty = self.xml != self.saved;
        if !self.dirty {
            self.spans.clear();
        }
    }

    fn cell_text(&self, c: &CellText) -> String {
        let rule = self.defs.table(c.decision).and_then(|t| t.rules.get(c.rule));
        let text = match c.col {
            Column::Input(i) => rule.and_then(|r| r.inputs.get(i)).map(|e| &e.text),
            Column::Output(i) => rule.and_then(|r| r.outputs.get(i)).map(|e| &e.text),
        };
        text.cloned().unwrap_or_default()
    }

    fn set_cell(&mut self, c: &CellText, delta: i64) {
        let text = c.text.replace("\r\n", "\n").replace('\r', "\n");
        for (d, dec) in self.defs.decisions.iter_mut().enumerate() {
            let DecisionLogic::Table(t) = &mut dec.logic else { continue };
            if delta != 0 {
                for (ri, r) in t.rules.iter_mut().enumerate() {
                    if (d, ri) > (c.decision, c.rule) {
                        r.line = (i64::from(r.line) + delta) as u32;
                    }
                }
            }
            if d != c.decision {
                continue;
            }
            let Some(r) = t.rules.get_mut(c.rule) else { continue };
            match c.col {
                Column::Input(i) => {
                    if let Some(e) = r.inputs.get_mut(i) {
                        e.tests = UnaryTests::parse(&text, &e.lang);
                        e.text.clone_from(&text);
                    }
                }
                Column::Output(i) => {
                    if let Some(e) = r.outputs.get_mut(i) {
                        e.text.clone_from(&text);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str, xml: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dmn-viewer-document-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, xml).unwrap();
        p
    }

    fn example() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/shipping-fees.dmn")).unwrap()
    }

    fn assert_in_sync(doc: &Document) {
        let defs = parse::parse(&doc.xml).unwrap();
        assert_eq!(format!("{:?}", doc.defs), format!("{defs:?}"));
        assert_eq!(format!("{:?}", doc.issues), format!("{:?}", lint::lint(&defs)));
    }

    fn cell(rule: usize, col: Column, text: &str) -> Edit {
        Edit::Cell { decision: 0, rule, col, text: text.into() }
    }

    #[test]
    fn every_open_and_every_change_gets_a_fresh_revision() {
        let path = scratch("revisions.dmn", &example());
        let mut doc = Document::open(path.clone()).unwrap();
        let mut seen = vec![doc.revision(), Document::open(path).unwrap().revision()];
        doc.commit(&[cell(0, Column::Output(0), "1")]).unwrap();
        seen.push(doc.revision());
        doc.commit(&[cell(0, Column::Output(0), "2"), cell(1, Column::Output(0), "3")]).unwrap();
        seen.push(doc.revision());
        doc.undo().unwrap();
        seen.push(doc.revision());
        doc.redo().unwrap();
        seen.push(doc.revision());
        doc.discard().unwrap();
        seen.push(doc.revision());
        let mut unique = seen.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), seen.len(), "{seen:?}");
    }

    #[test]
    fn cell_edits_update_the_model_like_a_full_parse() {
        for (name, xml) in [("lf.dmn", example()), ("crlf.dmn", example().replace('\n', "\r\n"))] {
            let mut doc = Document::open(scratch(name, &xml)).unwrap();
            let edits = [
                cell(0, Column::Input(1), "<= 6"),
                cell(0, Column::Output(2), "\"multi\nline\r\nlabel\""),
                cell(2, Column::Input(0), "not(\"EU\", \"NA\")"),
                cell(1, Column::Output(0), ""),
                cell(3, Column::Input(3), "a & b < c"),
                cell(4, Column::Output(1), "\"x\"\r\n+\r\n\"y\""),
                cell(0, Column::Output(2), "single"),
                cell(6, Column::Input(0), "-"),
            ];
            for e in &edits {
                assert!(doc.commit(std::slice::from_ref(e)).unwrap(), "{e:?}");
                assert_in_sync(&doc);
            }
            assert!(!doc.commit(&[cell(0, Column::Input(1), "<= 6")]).unwrap(), "same text again is a no-op");
        }
    }

    #[test]
    fn undo_and_redo_walk_back_and_forth_exactly() {
        let original = example();
        let mut doc = Document::open(scratch("history.dmn", &original)).unwrap();
        let edits = [
            cell(0, Column::Input(1), "<= 6\n"),
            Edit::InsertRule { decision: 0, at: 1, copy_of: Some(0) },
            cell(1, Column::Output(0), "7.5"),
            Edit::InsertInput { decision: 0, at: 2 },
            cell(3, Column::Input(2), "\"z\""),
            Edit::MoveRule { decision: 0, from: 0, to: 4 },
            Edit::Note { decision: 0, rule: 2, text: "note\nwith lines".into() },
        ];
        let mut states = vec![doc.xml.clone()];
        for e in &edits {
            doc.commit(std::slice::from_ref(e)).unwrap();
            states.push(doc.xml.clone());
        }
        for want in states.iter().rev().skip(1) {
            assert!(doc.undo().unwrap());
            assert_eq!(&doc.xml, want);
            assert_in_sync(&doc);
        }
        assert!(!doc.undo().unwrap(), "nothing left to undo");
        assert!(!doc.dirty());
        for want in states.iter().skip(1) {
            assert!(doc.redo().unwrap());
            assert_eq!(&doc.xml, want);
            assert_in_sync(&doc);
        }
        assert!(!doc.redo().unwrap());
        doc.undo().unwrap();
        doc.commit(&[cell(0, Column::Output(1), "\"NEW\"")]).unwrap();
        assert!(!doc.redo().unwrap(), "a new edit clears redo");
    }

    #[test]
    fn undo_keeps_only_the_changed_spans() {
        let mut doc = Document::open(scratch("memory.dmn", &example())).unwrap();
        for i in 0..50 {
            doc.commit(&[cell(i % 7, Column::Output(0), &i.to_string())]).unwrap();
        }
        let kept: usize = doc.undo.iter().map(|c| c.text.len()).sum();
        assert!(kept < 200, "undo keeps {kept} bytes for 50 one-number edits");
    }

    #[test]
    fn dirty_follows_the_content_and_discard_and_save() {
        let original = example();
        let path = scratch("dirty.dmn", &original);
        let mut doc = Document::open(path.clone()).unwrap();
        assert!(!doc.dirty());
        let old = doc.defs.table(0).unwrap().rules[0].outputs[0].text.clone();
        doc.commit(&[cell(0, Column::Output(0), "1")]).unwrap();
        assert!(doc.dirty());
        doc.commit(&[cell(0, Column::Output(0), &old)]).unwrap();
        assert!(!doc.dirty(), "editing back to the saved text is clean again");

        doc.commit(&[Edit::DeleteRule { decision: 0, rule: 0 }]).unwrap();
        let edited = doc.xml.clone();
        doc.discard().unwrap();
        assert_eq!((doc.xml.as_str(), doc.dirty()), (original.as_str(), false));
        assert_in_sync(&doc);
        doc.undo().unwrap();
        assert_eq!(doc.xml, edited, "undo brings discarded edits back");

        doc.save().unwrap();
        assert!(!doc.dirty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), edited);
    }

    fn replay(doc: &Document) -> String {
        let (saved, mut out, mut pos) = (doc.saved(), String::new(), 0);
        for h in doc.changes() {
            out.push_str(&saved[pos..h.old.start]);
            out.push_str(&doc.xml[h.new.clone()]);
            pos = h.old.end;
        }
        out + &saved[pos..]
    }

    #[test]
    fn changes_follow_edits_undo_and_save() {
        let path = scratch("review.dmn", &example());
        let mut doc = Document::open(path).unwrap();
        assert!(doc.changes().is_empty());
        doc.commit(&[cell(0, Column::Output(0), "1.25")]).unwrap();
        doc.commit(&[cell(5, Column::Input(1), "> 99")]).unwrap();
        doc.commit(&[Edit::InsertRule { decision: 0, at: 2, copy_of: Some(1) }]).unwrap();
        doc.commit(&[cell(0, Column::Output(0), "1.5")]).unwrap();
        let hunks = doc.changes();
        assert_eq!(hunks.len(), 3, "{hunks:#?}");
        assert_eq!(replay(&doc), doc.xml);
        let first = &hunks[0];
        assert!(doc.saved()[first.old.clone()].contains(">4.5<") && doc.xml[first.new.clone()].contains(">1.5<"));
        assert_eq!(first.old_line, first.new_line);
        assert!(hunks[2].new_line > hunks[2].old_line, "lines after the inserted rule moved down");

        doc.undo().unwrap();
        assert_eq!(replay(&doc), doc.xml);
        doc.undo().unwrap();
        doc.undo().unwrap();
        assert_eq!(doc.changes().len(), 1);
        doc.undo().unwrap();
        assert!(doc.changes().is_empty() && !doc.dirty());
        doc.redo().unwrap();
        assert_eq!(doc.changes().len(), 1);
        doc.save().unwrap();
        assert!(doc.changes().is_empty());
        doc.commit(&[cell(1, Column::Output(0), "2")]).unwrap();
        doc.discard().unwrap();
        assert!(doc.changes().is_empty());
    }
}
