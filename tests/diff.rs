use dmn_viewer::diff::{Hunk, Spans, changed, hunks, line_count, line_diff};

/// An editor in miniature: applies replacements to the text and records them in `Spans`.
struct Doc {
    saved: String,
    text: String,
    spans: Spans,
}

impl Doc {
    fn new(s: &str) -> Doc {
        Doc { saved: s.into(), text: s.into(), spans: Spans::default() }
    }

    fn edit(&mut self, at: usize, removed: usize, with: &str) -> &mut Doc {
        self.text.replace_range(at..at + removed, with);
        self.spans.splice(at, removed, with.len());
        self
    }

    fn replace(&mut self, old: &str, new: &str) -> &mut Doc {
        let at = self.text.find(old).unwrap();
        self.edit(at, old.len(), new)
    }

    fn hunks(&self) -> Vec<Hunk> {
        assert!(self.spans.describe(&self.saved, &self.text), "spans out of step: {:?}", self.spans);
        hunks(&self.saved, &self.text, &self.spans)
    }

    /// (old line, new line, old text, new text) per hunk.
    fn shown(&self) -> Vec<(usize, usize, &str, &str)> {
        self.hunks().into_iter().map(|h| (h.old_line, h.new_line, &self.saved[h.old], &self.text[h.new])).collect()
    }
}

const TEXT: &str = "one\ntwo\nthree\nfour\nfive\nsix\n";

#[test]
fn changed_respects_char_boundaries() {
    assert_eq!(changed("aé", "aè"), (1, 3, 3));
    assert_eq!(changed("abc", "abc"), (3, 3, 3));
    assert_eq!(changed("aaa", "aa"), (2, 3, 2));
    assert_eq!(changed("x✓y", "x✗y"), (1, 4, 4));
}

#[test]
fn hunks_are_whole_lines() {
    let mut d = Doc::new(TEXT);
    d.replace("two", "2");
    assert_eq!(d.shown(), vec![(2, 2, "two\n", "2\n")]);
    d.replace("five", "FIVE");
    assert_eq!(d.shown(), vec![(2, 2, "two\n", "2\n"), (5, 5, "five\n", "FIVE\n")]);
}

#[test]
fn inserted_and_deleted_lines_shift_the_numbers() {
    let mut d = Doc::new(TEXT);
    d.replace("two\n", "");
    d.replace("five\n", "five\nfive and a half\n");
    assert_eq!(d.shown(), vec![(2, 2, "two\n", ""), (6, 5, "", "five and a half\n")]);
    let mut d = Doc::new("no newline at the end");
    d.replace("end", "finish");
    assert_eq!(d.shown(), vec![(1, 1, "no newline at the end", "no newline at the finish")]);
}

#[test]
fn edits_on_one_line_share_a_hunk_and_undone_edits_vanish() {
    let mut d = Doc::new(TEXT);
    d.replace("thr", "THR").replace("ee", "EE");
    assert_eq!(d.shown(), vec![(3, 3, "three\n", "THREE\n")]);
    d.replace("THREE", "three");
    assert!(d.hunks().is_empty(), "back to the saved text");
    d.replace("one", "uno").replace("uno", "one");
    assert!(d.hunks().is_empty());
}

#[test]
fn adjacent_and_overlapping_edits_merge() {
    let mut d = Doc::new(TEXT);
    d.replace("two\nthree", "2\n3").replace("3\nfour", "3\n4");
    assert_eq!(d.spans.as_slice().len(), 1);
    assert_eq!(d.shown(), vec![(2, 2, "two\nthree\nfour\n", "2\n3\n4\n")]);
}

#[test]
fn stale_spans_fall_back_to_a_full_comparison() {
    let saved = "a\nb\nc\n";
    let h = hunks(saved, "a\nB\nc\n", &Spans::default());
    assert_eq!(h, vec![Hunk { old: 2..4, new: 2..4, old_line: 2, new_line: 2 }]);
    assert!(hunks(saved, saved, &Spans::default()).is_empty());
}

#[test]
fn a_structural_edit_splits_into_the_lines_it_changed() {
    // One edit spanning every rule that adds a line to each, like inserting an input column.
    let rule =
        |i: usize, extra: bool| format!("  <rule id=\"R{i}\">\n    <inputEntry><text>a</text></inputEntry>\n{}  </rule>\n", if extra { "    <inputEntry><text></text></inputEntry>\n" } else { "" });
    let saved: String = (0..500).map(|i| rule(i, false)).collect();
    let current: String = (0..500).map(|i| rule(i, true)).collect();
    let mut d = Doc::new(&saved);
    d.edit(0, saved.len(), &current);
    let shown = d.shown();
    assert_eq!(shown.len(), 500);
    for (k, &(old_line, new_line, old, new)) in shown.iter().enumerate() {
        assert_eq!((old_line, new_line), (k * 3 + 3, k * 4 + 3));
        assert_eq!((old, new), ("", "    <inputEntry><text></text></inputEntry>\n"));
    }
}

#[test]
fn repeated_lines_without_anchors_still_diff_exactly() {
    let mut d = Doc::new("x\nx\nx\nx\n");
    d.edit(0, 8, "x\ny\nx\nx\nz\n");
    // Three of the four x lines are kept: one becomes y, and z is added at the end.
    assert_eq!(d.shown(), vec![(2, 2, "x\n", "y\n"), (5, 5, "", "z\n")]);
}

#[test]
fn line_diff_runs() {
    let a = ["a\n", "b\n", "c\n", "d\n", "e\n"];
    let b = ["a\n", "B\n", "c\n", "d\n", "new\n", "e\n"];
    assert_eq!(line_diff(&a, &b), vec![(1..2, 1..2), (4..4, 4..5)]);
    assert_eq!(line_diff(&a, &a), vec![]);
    assert_eq!(line_diff(&a, &[]), vec![(0..5, 0..0)]);
    // A moved line: kept where the longer run of anchors agrees, reported as removed and added.
    assert_eq!(line_diff(&["1", "2", "3"], &["2", "3", "1"]), vec![(0..1, 0..0), (3..3, 2..3)]);
}

#[test]
fn line_counts() {
    assert_eq!((line_count(""), line_count("a"), line_count("a\n"), line_count("a\nb")), (0, 1, 1, 2));
}

#[test]
fn random_edits_replay_exactly() {
    let base: String = (0..400).map(|i| format!("line {i} with ünïcode ✓\n")).collect();
    let mut seed = 0x9e3779b97f4a7c15u64;
    let mut rnd = |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n.max(1) as u64) as usize
    };
    for round in 0..40 {
        let mut d = Doc::new(&base);
        for _ in 0..1 + round % 12 {
            let mut at = rnd(d.text.len());
            while !d.text.is_char_boundary(at) {
                at -= 1;
            }
            // Now and then a long edit across many lines, so hunks get split line by line.
            let len = if rnd(4) == 0 { rnd(900) } else { rnd(40) };
            let mut end = (at + len).min(d.text.len());
            while !d.text.is_char_boundary(end) {
                end -= 1;
            }
            let kept = d.text[at..end].replace("line 1", "LINE 1");
            let with = ["", "x", "\n", "new line\n", "é", "line 7 with", &kept][rnd(7)];
            let with = with.to_string();
            d.edit(at, end - at, &with);
        }
        let hs = d.hunks();
        let (mut out, mut pos) = (String::new(), 0);
        for (k, h) in hs.iter().enumerate() {
            assert!(h.old.start >= pos, "round {round}: hunks overlap");
            assert_ne!(&d.saved[h.old.clone()], &d.text[h.new.clone()]);
            assert!(h.new.start == 0 || d.text.as_bytes()[h.new.start - 1] == b'\n', "starts at a line start");
            assert_eq!(h.new_line, 1 + d.text[..h.new.start].matches('\n').count());
            assert_eq!(h.old_line, 1 + d.saved[..h.old.start].matches('\n').count());
            if k > 0 {
                assert!(h.new.start >= hs[k - 1].new.end);
            }
            out.push_str(&d.saved[pos..h.old.start]);
            out.push_str(&d.text[h.new.clone()]);
            pos = h.old.end;
        }
        out.push_str(&d.saved[pos..]);
        assert_eq!(out, d.text, "round {round}");
    }
}
