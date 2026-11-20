use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Memo {
    pub id: Arc<str>,
    pub text: String,
}

pub struct File {
    pub key: PathBuf,
    pub path: PathBuf,
    pub memos: Vec<Memo>,
}

impl File {
    pub fn get(&self, id: &str) -> Option<&str> {
        self.memos.iter().find(|m| &*m.id == id).map(|m| m.text.as_str())
    }
}

#[derive(Default)]
pub struct Memos {
    files: Vec<File>,
    total: usize,
    revision: u64,
}

impl Memos {
    pub fn total(&self) -> usize {
        self.total
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn files(&self) -> &[File] {
        &self.files
    }

    pub fn file(&self, key: &Path) -> Option<&File> {
        self.files.iter().find(|f| f.key == key)
    }

    pub fn count(&self, key: &Path) -> usize {
        self.file(key).map_or(0, |f| f.memos.len())
    }

    pub fn get(&self, key: &Path, id: &str) -> Option<&str> {
        self.file(key)?.get(id)
    }

    pub fn set(&mut self, key: &Path, path: &Path, id: &str, text: &str) {
        self.revision += 1;
        let at = self.files.iter().position(|f| f.key == key);
        if text.trim().is_empty() {
            let Some(fi) = at else { return };
            let file = &mut self.files[fi];
            if let Some(mi) = file.memos.iter().position(|m| &*m.id == id) {
                file.memos.remove(mi);
                self.total -= 1;
                if file.memos.is_empty() {
                    self.files.remove(fi);
                }
            }
            return;
        }
        let fi = at.unwrap_or_else(|| {
            self.files.push(File { key: key.to_path_buf(), path: path.to_path_buf(), memos: Vec::new() });
            self.files.len() - 1
        });
        let file = &mut self.files[fi];
        match file.memos.iter_mut().find(|m| &*m.id == id) {
            Some(m) => {
                m.text.clear();
                m.text.push_str(text);
            }
            None => {
                file.memos.push(Memo { id: id.into(), text: text.to_string() });
                self.total += 1;
            }
        }
    }

    pub fn rekey(&mut self, from: &Path, to: &Path, path: &Path) {
        if from == to {
            return;
        }
        let Some(fi) = self.files.iter().position(|f| f.key == from) else { return };
        self.revision += 1;
        let moved = self.files.remove(fi);
        match self.files.iter_mut().find(|f| f.key == to) {
            Some(file) => {
                for m in moved.memos {
                    match file.memos.iter_mut().find(|x| x.id == m.id) {
                        Some(x) => {
                            x.text.push_str("\n\n");
                            x.text.push_str(&m.text);
                            self.total -= 1;
                        }
                        None => file.memos.push(m),
                    }
                }
            }
            None => self.files.insert(fi.min(self.files.len()), File { key: to.to_path_buf(), path: path.to_path_buf(), memos: moved.memos }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn counts_follow_adds_and_removals() {
        let mut m = Memos::default();
        m.set(&p("a"), &p("a"), "d1", "first");
        m.set(&p("a"), &p("a"), "d2", "second");
        m.set(&p("b"), &p("b"), "d1", "other");
        assert_eq!((m.total(), m.count(&p("a")), m.count(&p("b"))), (3, 2, 1));
        m.set(&p("a"), &p("a"), "d1", "edited");
        assert_eq!(m.get(&p("a"), "d1"), Some("edited"));
        assert_eq!(m.total(), 3);
        m.set(&p("b"), &p("b"), "d1", "  \n");
        assert_eq!((m.total(), m.count(&p("b"))), (2, 0));
        assert!(m.file(&p("b")).is_none());
        m.set(&p("zzz"), &p("zzz"), "none", "");
        assert_eq!(m.total(), 2);
    }

    #[test]
    fn notes_keep_the_order_they_were_written_in() {
        let mut m = Memos::default();
        m.set(&p("b"), &p("b"), "x", "1");
        m.set(&p("a"), &p("a"), "y", "2");
        m.set(&p("b"), &p("b"), "w", "3");
        let order: Vec<(&Path, Vec<&str>)> = m.files().iter().map(|f| (f.key.as_path(), f.memos.iter().map(|x| &*x.id).collect())).collect();
        assert_eq!(order, vec![(Path::new("b"), vec!["x", "w"]), (Path::new("a"), vec!["y"])]);
    }

    #[test]
    fn rekeying_moves_notes_and_merges_clashes() {
        let mut m = Memos::default();
        m.set(&p("old"), &p("old"), "d1", "kept");
        m.set(&p("old"), &p("old"), "d2", "clash");
        m.set(&p("new"), &p("new"), "d2", "there");
        m.rekey(&p("old"), &p("new"), &p("new"));
        assert!(m.file(&p("old")).is_none());
        assert_eq!(m.get(&p("new"), "d1"), Some("kept"));
        assert_eq!(m.get(&p("new"), "d2"), Some("there\n\nclash"));
        assert_eq!(m.total(), 2);
        m.rekey(&p("new"), &p("moved"), &p("shown"));
        assert_eq!(m.file(&p("moved")).map(|f| f.path.as_path()), Some(Path::new("shown")));
        assert_eq!((m.count(&p("moved")), m.total()), (2, 2));
    }

    #[test]
    fn every_change_moves_the_revision() {
        let mut m = Memos::default();
        let r = m.revision();
        m.set(&p("a"), &p("a"), "d", "x");
        assert!(m.revision() > r);
    }
}
