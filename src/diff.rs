use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub old: Range<usize>,
    pub new: Range<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spans(Vec<Span>);

impl Spans {
    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn as_slice(&self) -> &[Span] {
        &self.0
    }

    pub fn splice(&mut self, at: usize, removed: usize, inserted: usize) {
        let end = at + removed;
        let v = &mut self.0;
        let first = v.partition_point(|s| s.new.end < at);
        let last = v.partition_point(|s| s.new.start <= end);
        let growth = |s: &Span| s.new.len() as isize - s.old.len() as isize;
        let before: isize = v[..first].iter().map(growth).sum();
        let through = before + v[first..last].iter().map(growth).sum::<isize>();
        let (start, stop) = if first < last { (v[first].new.start.min(at), v[last - 1].new.end.max(end)) } else { (at, end) };
        let shift = inserted as isize - removed as isize;
        let moved = |p: usize, by: isize| (p as isize + by) as usize;
        let merged = Span { old: moved(start, -before)..moved(stop, -through), new: start..moved(stop, shift) };
        for s in &mut v[last..] {
            s.new = moved(s.new.start, shift)..moved(s.new.end, shift);
        }
        v.splice(first..last, [merged]);
    }

    pub fn describe(&self, saved: &str, current: &str) -> bool {
        consistent(saved, current, &self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub old: Range<usize>,
    pub new: Range<usize>,
    pub old_line: usize,
    pub new_line: usize,
}

pub fn changed(a: &str, b: &str) -> (usize, usize, usize) {
    let (x, y) = (a.as_bytes(), b.as_bytes());
    let mut p = x.iter().zip(y).take_while(|(c, d)| c == d).count();
    while !a.is_char_boundary(p) {
        p -= 1;
    }
    let room = x.len().min(y.len()) - p;
    let mut s = x.iter().rev().zip(y.iter().rev()).take(room).take_while(|(c, d)| c == d).count();
    while !a.is_char_boundary(x.len() - s) || !b.is_char_boundary(y.len() - s) {
        s -= 1;
    }
    (p, x.len() - s, y.len() - s)
}

fn consistent(saved: &str, current: &str, spans: &[Span]) -> bool {
    let (mut o, mut n) = (0, 0);
    for s in spans {
        let ok = o <= s.old.start && s.old.start <= s.old.end && s.old.end <= saved.len() && n <= s.new.start && s.new.start <= s.new.end && s.new.end <= current.len();
        if !ok || ![s.old.start, s.old.end].iter().all(|&i| saved.is_char_boundary(i)) || ![s.new.start, s.new.end].iter().all(|&i| current.is_char_boundary(i)) {
            return false;
        }
        if saved[o..s.old.start] != current[n..s.new.start] {
            return false;
        }
        (o, n) = (s.old.end, s.new.end);
    }
    saved[o..] == current[n..]
}

fn newlines(s: &str) -> usize {
    s.bytes().filter(|&b| b == b'\n').count()
}

fn at_line_start(t: &str, p: usize) -> bool {
    p == 0 || t.as_bytes()[p - 1] == b'\n'
}

pub fn hunks(saved: &str, current: &str, spans: &Spans) -> Vec<Hunk> {
    let fallback;
    let spans = if consistent(saved, current, &spans.0) {
        &spans.0[..]
    } else {
        let (p, ea, eb) = changed(saved, current);
        fallback = [Span { old: p..ea, new: p..eb }];
        &fallback[..]
    };
    let narrowed: Vec<Span> = spans
        .iter()
        .filter_map(|s| {
            let (a, b) = (&saved[s.old.clone()], &current[s.new.clone()]);
            (a != b).then(|| {
                let (p, ea, eb) = changed(a, b);
                Span { old: s.old.start + p..s.old.start + ea, new: s.new.start + p..s.new.start + eb }
            })
        })
        .collect();

    let mut out = Vec::new();
    let (mut counted, mut line, mut growth) = (0, 1, 0isize);
    let mut i = 0;
    while i < narrowed.len() {
        let mut g = narrowed[i].clone();
        i += 1;
        while let Some(next) = narrowed.get(i).filter(|s| !current[g.new.end..s.new.start].contains('\n')) {
            (g.old.end, g.new.end) = (next.old.end, next.new.end);
            i += 1;
        }
        let left = current[..g.new.start].rfind('\n').map_or(0, |p| p + 1);
        let right = if at_line_start(current, g.new.end) && at_line_start(saved, g.old.end) { g.new.end } else { current[g.new.end..].find('\n').map_or(current.len(), |p| g.new.end + p + 1) };
        let old = g.old.start - (g.new.start - left)..g.old.end + (right - g.new.end);
        let new = left..right;
        line += newlines(&current[counted..new.start]);
        counted = new.start;
        let new_line = line;
        let old_line = (new_line as isize - growth) as usize;
        growth += newlines(&current[new.clone()]) as isize - newlines(&saved[old.clone()]) as isize;
        refine(saved, current, Hunk { old, new, old_line, new_line }, &mut out);
    }
    out
}

fn refine(saved: &str, current: &str, h: Hunk, out: &mut Vec<Hunk>) {
    let a: Vec<&str> = saved[h.old.clone()].split_inclusive('\n').collect();
    let b: Vec<&str> = current[h.new.clone()].split_inclusive('\n').collect();
    if a.len() < 2 && b.len() < 2 {
        out.push(h);
        return;
    }
    let offsets = |start: usize, lines: &[&str]| {
        let mut v = Vec::with_capacity(lines.len() + 1);
        v.push(start);
        v.extend(lines.iter().scan(start, |p, l| {
            *p += l.len();
            Some(*p)
        }));
        v
    };
    let (oa, ob) = (offsets(h.old.start, &a), offsets(h.new.start, &b));
    for (ra, rb) in line_diff(&a, &b) {
        out.push(Hunk { old: oa[ra.start]..oa[ra.end], new: ob[rb.start]..ob[rb.end], old_line: h.old_line + ra.start, new_line: h.new_line + rb.start });
    }
}

pub fn line_diff(a: &[&str], b: &[&str]) -> Vec<(Range<usize>, Range<usize>)> {
    let mut out = Vec::new();
    gap(a, b, 0..a.len(), 0..b.len(), 0, &mut out);
    out
}

fn emit(out: &mut Vec<(Range<usize>, Range<usize>)>, ra: Range<usize>, rb: Range<usize>) {
    if ra.is_empty() && rb.is_empty() {
        return;
    }
    match out.last_mut() {
        Some((pa, pb)) if pa.end == ra.start && pb.end == rb.start => (pa.end, pb.end) = (ra.end, rb.end),
        _ => out.push((ra, rb)),
    }
}

const LCS_CELLS: usize = 1 << 18;

fn gap(a: &[&str], b: &[&str], mut ra: Range<usize>, mut rb: Range<usize>, depth: u32, out: &mut Vec<(Range<usize>, Range<usize>)>) {
    while ra.start < ra.end && rb.start < rb.end && a[ra.start] == b[rb.start] {
        (ra.start, rb.start) = (ra.start + 1, rb.start + 1);
    }
    while ra.start < ra.end && rb.start < rb.end && a[ra.end - 1] == b[rb.end - 1] {
        (ra.end, rb.end) = (ra.end - 1, rb.end - 1);
    }
    if ra.is_empty() || rb.is_empty() {
        return emit(out, ra, rb);
    }
    let mut seen: std::collections::HashMap<&str, (u32, u32, usize, usize)> = std::collections::HashMap::with_capacity(ra.len() + rb.len());
    for i in ra.clone() {
        let e = seen.entry(a[i]).or_default();
        (e.0, e.2) = (e.0 + 1, i);
    }
    for j in rb.clone() {
        if let Some(e) = seen.get_mut(b[j]) {
            (e.1, e.3) = (e.1 + 1, j);
        }
    }
    let mut anchors: Vec<(usize, usize)> = seen.into_values().filter(|e| e.0 == 1 && e.1 == 1).map(|e| (e.2, e.3)).collect();
    if anchors.is_empty() || depth > 64 {
        return lcs(a, b, ra, rb, out);
    }
    anchors.sort_unstable();
    let mut pa = ra.start;
    let mut pb = rb.start;
    for (x, y) in increasing(&anchors) {
        gap(a, b, pa..x, pb..y, depth + 1, out);
        (pa, pb) = (x + 1, y + 1);
    }
    gap(a, b, pa..ra.end, pb..rb.end, depth + 1, out);
}

fn increasing(anchors: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut tails: Vec<usize> = Vec::new();
    let mut prev = vec![usize::MAX; anchors.len()];
    for (k, &(_, y)) in anchors.iter().enumerate() {
        let len = tails.partition_point(|&t| anchors[t].1 < y);
        if len > 0 {
            prev[k] = tails[len - 1];
        }
        if len == tails.len() {
            tails.push(k);
        } else {
            tails[len] = k;
        }
    }
    let mut run = Vec::with_capacity(tails.len());
    let mut k = tails.last().copied().unwrap_or(usize::MAX);
    while k != usize::MAX {
        run.push(anchors[k]);
        k = prev[k];
    }
    run.reverse();
    run
}

fn lcs(a: &[&str], b: &[&str], ra: Range<usize>, rb: Range<usize>, out: &mut Vec<(Range<usize>, Range<usize>)>) {
    let (n, m) = (ra.len(), rb.len());
    if n * m > LCS_CELLS {
        return emit(out, ra, rb);
    }
    let w = m + 1;
    let mut t = vec![0u32; (n + 1) * w];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i * w + j] = if a[ra.start + i] == b[rb.start + j] { t[(i + 1) * w + j + 1] + 1 } else { t[(i + 1) * w + j].max(t[i * w + j + 1]) };
        }
    }
    let (mut i, mut j, mut si, mut sj) = (0, 0, 0, 0);
    while i < n && j < m {
        if a[ra.start + i] == b[rb.start + j] {
            emit(out, ra.start + si..ra.start + i, rb.start + sj..rb.start + j);
            (i, j) = (i + 1, j + 1);
            (si, sj) = (i, j);
        } else if t[(i + 1) * w + j] >= t[i * w + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    emit(out, ra.start + si..ra.end, rb.start + sj..rb.end);
}

pub fn line_count(s: &str) -> usize {
    newlines(s) + usize::from(!s.is_empty() && !s.ends_with('\n'))
}
