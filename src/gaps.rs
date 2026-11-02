use crate::eval::{Context, Mode, Value, fmt_num, literal};
use crate::model::DecisionTable;
use crate::unary::literal_items;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Gap {
    pub cells: Vec<Option<String>>,
}

const BUDGET: usize = 200_000;
const OTHER: &str = "\u{0}other";

enum Domain {
    Num(Vec<f64>),
    Disc { values: Vec<Value>, other: bool },
}

struct Col {
    index: usize,
    domain: Domain,
    w: usize,
    masks: Vec<u64>,
    spans: Vec<(usize, usize)>,
    members: Vec<Vec<usize>>,
}

impl Col {
    fn mask(&self, g: usize) -> &[u64] {
        &self.masks[g * self.w..(g + 1) * self.w]
    }
}

pub fn gaps(t: &DecisionTable, limit: usize) -> Option<Vec<Gap>> {
    let ni = t.inputs.len();
    if ni == 0 || t.rules.is_empty() || t.rules.iter().any(|r| r.inputs.len() != ni) {
        return None;
    }
    if t.rules.iter().any(|r| r.inputs.iter().all(|e| e.tests.is_any())) {
        return Some(Vec::new());
    }
    let w = t.rules.len().div_ceil(64);
    let ctx = Context::new();
    let mut cols = Vec::with_capacity(ni);
    for c in 0..ni {
        let col = column(t, c, w, &ctx)?;
        if !(col.members.len() == 1 && full(col.mask(0), t.rules.len())) {
            cols.push(col);
        }
    }
    cols.sort_by_key(|c| std::cmp::Reverse(c.members.len()));
    let mut s = Search {
        cols: &cols,
        w,
        buf: vec![0; (cols.len() + 1) * w],
        ranges: vec![(0, w); cols.len() + 1],
        path: Vec::with_capacity(cols.len()),
        empties: vec![Vec::new(); cols.len()],
        nodes: 0,
        limit,
        found: Vec::new(),
    };
    for r in 0..t.rules.len() {
        s.buf[r / 64] |= 1 << (r % 64);
    }
    if cols.is_empty() {
        return Some(Vec::new());
    }
    s.dfs(0).ok()?;
    Some(
        s.found
            .into_iter()
            .map(|(path, last)| {
                let mut cells = vec![None; ni];
                for (d, g) in path.iter().enumerate() {
                    cells[cols[d].index] = label(&cols[d].domain, cols[d].members[*g].clone());
                }
                let d = path.len();
                let members = last.iter().flat_map(|&g| cols[d].members[g].iter().copied()).collect();
                cells[cols[d].index] = label(&cols[d].domain, members);
                Gap { cells }
            })
            .collect(),
    )
}

fn full(mask: &[u64], n: usize) -> bool {
    mask.iter().enumerate().all(|(i, &m)| {
        let bits = (n - i * 64).min(64);
        m == if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 }
    })
}

fn column(t: &DecisionTable, c: usize, w: usize, ctx: &Context) -> Option<Col> {
    let mut ops = Vec::new();
    let mut spans = Vec::with_capacity(t.rules.len());
    let mut ordered = false;
    for r in &t.rules {
        let e = &r.inputs[c];
        let before = ops.len();
        ordered |= e.tests.operands(&mut ops)?;
        if ops.len() == before && !e.tests.is_any() {
            return None;
        }
        spans.push(before..ops.len());
    }
    let allowed: Option<Vec<Value>> = t.inputs[c].input_values.as_deref().and_then(literal_items).map(|items| items.iter().filter_map(|i| literal(i, Mode::Feel)).collect());
    let domain = if !ops.is_empty() && ops.iter().all(|v| matches!(v, Value::Num(_))) {
        let mut b: Vec<f64> = ops.iter().filter_map(|v| if let Value::Num(n) = v { Some(*n) } else { None }).collect();
        b.sort_by(f64::total_cmp);
        b.dedup();
        Domain::Num(b)
    } else if ordered {
        return None;
    } else if !ops.is_empty() && ops.iter().all(|v| matches!(v, Value::Bool(_))) {
        Domain::Disc { values: vec![Value::Bool(true), Value::Bool(false)], other: false }
    } else if ops.iter().all(|v| matches!(v, Value::Str(_))) {
        let closed = allowed.filter(|a| !a.is_empty() && a.iter().all(|v| matches!(v, Value::Str(_))));
        let other = closed.is_none();
        let mut seen = HashMap::new();
        let values = closed.into_iter().flatten().chain(ops.iter().map(|v| (*v).clone())).filter(|v| seen.insert(text(v), ()).is_none()).collect();
        Domain::Disc { values, other }
    } else {
        return None;
    };
    let reps = reps(&domain);
    let n = reps.len();
    let index: HashMap<String, usize> = match &domain {
        Domain::Disc { values, .. } => values.iter().enumerate().map(|(k, v)| (text(v), k)).collect(),
        Domain::Num(_) => HashMap::new(),
    };
    let mut events: Vec<(usize, usize)> = Vec::with_capacity(t.rules.len() * 2);
    let mut pos: Vec<usize> = Vec::new();
    for (ri, r) in t.rules.iter().enumerate() {
        let tests = &r.inputs[c].tests;
        if tests.is_any() {
            events.push((0, ri));
            continue;
        }
        pos.clear();
        match &domain {
            Domain::Num(b) => pos.extend(ops[spans[ri].clone()].iter().filter_map(|v| if let Value::Num(x) = v { b.binary_search_by(|p| p.total_cmp(x)).ok() } else { None }).map(|j| 2 * j + 1)),
            Domain::Disc { .. } => pos.extend(ops[spans[ri].clone()].iter().filter_map(|v| index.get(&text(v)).copied())),
        }
        pos.sort_unstable();
        pos.dedup();
        let hit = |k: usize| tests.matches(&reps[k], ctx).ok();
        match &domain {
            Domain::Num(_) => {
                let mut on = false;
                let mut start = 0;
                for p in pos.iter().copied().chain([n]) {
                    for s in [(start < p).then_some(start), (p < n).then_some(p)].into_iter().flatten() {
                        let m = hit(s)?;
                        if m != on {
                            events.push((s, ri));
                            on = m;
                        }
                    }
                    start = p + 1;
                }
            }
            Domain::Disc { .. } => {
                let outside = (0..n).find(|k| pos.binary_search(k).is_err());
                let out_hit = match outside {
                    Some(k) => hit(k)?,
                    None => false,
                };
                if out_hit {
                    events.push((0, ri));
                }
                for &k in &pos {
                    if hit(k)? != out_hit {
                        events.push((k, ri));
                        if k + 1 < n {
                            events.push((k + 1, ri));
                        }
                    }
                }
            }
        }
    }
    events.sort_unstable_by_key(|e| e.0);
    let mut heads: HashMap<u64, usize> = HashMap::new();
    let mut chain: Vec<usize> = Vec::new();
    let mut masks: Vec<u64> = Vec::new();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut members: Vec<Vec<usize>> = Vec::new();
    let mut cur = vec![0u64; w];
    let mut h = 0u64;
    let mut ei = 0;
    let mut last = 0;
    for k in 0..n {
        let before = ei;
        while ei < events.len() && events[ei].0 == k {
            let ri = events[ei].1;
            cur[ri / 64] ^= 1 << (ri % 64);
            h ^= key(ri);
            ei += 1;
        }
        if k > 0 && ei == before {
            members[last].push(k);
            continue;
        }
        let mut g = heads.get(&h).copied().unwrap_or(usize::MAX);
        while g != usize::MAX && masks[g * w..(g + 1) * w] != cur[..] {
            g = chain[g];
        }
        if g == usize::MAX {
            g = members.len();
            chain.push(heads.insert(h, g).unwrap_or(usize::MAX));
            masks.extend_from_slice(&cur);
            let first = cur.iter().position(|&x| x != 0).unwrap_or(0);
            spans.push((first, cur.iter().rposition(|&x| x != 0).map_or(first, |p| p + 1)));
            members.push(Vec::new());
        }
        last = g;
        members[g].push(k);
    }
    Some(Col { index: c, domain, w, masks, spans, members })
}

fn key(r: usize) -> u64 {
    let mut z = (r as u64).wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn reps(d: &Domain) -> Vec<Value> {
    match d {
        Domain::Num(b) => {
            let k = b.len();
            (0..=2 * k)
                .map(|i| {
                    let j = i / 2;
                    Value::Num(match (i % 2, j) {
                        (1, _) => b[j],
                        (_, 0) => b[0] - 1.0,
                        (_, j) if j == k => b[k - 1] + 1.0,
                        _ => (b[j - 1] + b[j]) / 2.0,
                    })
                })
                .collect()
        }
        Domain::Disc { values, other } => values.iter().cloned().chain(other.then(|| Value::Str(OTHER.into()))).collect(),
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::Str(s) => format!("\"{s}\""),
        Value::Num(n) => fmt_num(*n),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
    }
}

fn label(d: &Domain, mut members: Vec<usize>) -> Option<String> {
    members.sort_unstable();
    match d {
        Domain::Num(b) => {
            let last = 2 * b.len();
            if members.len() == last + 1 {
                return None;
            }
            let mut parts = Vec::new();
            let mut i = 0;
            while i < members.len() {
                let s = members[i];
                let mut e = s;
                while i + 1 < members.len() && members[i + 1] == e + 1 {
                    i += 1;
                    e += 1;
                }
                parts.push(interval(b, s, e, last));
                i += 1;
            }
            Some(parts.join(", "))
        }
        Domain::Disc { values, other } => {
            let n = values.len() + *other as usize;
            if members.len() == n {
                return None;
            }
            let has_other = *other && members.last() == Some(&values.len());
            let list = |pick: &dyn Fn(usize) -> bool| values.iter().enumerate().filter(|(k, _)| pick(*k)).map(|(_, v)| text(v)).collect::<Vec<_>>().join(", ");
            Some(if has_other { format!("not({})", list(&|k| members.binary_search(&k).is_err())) } else { list(&|k| members.binary_search(&k).is_ok()) })
        }
    }
}

fn interval(b: &[f64], s: usize, e: usize, last: usize) -> String {
    let lo = (s > 0).then(|| (fmt_num(b[if s % 2 == 1 { s / 2 } else { s / 2 - 1 }]), s % 2 == 1));
    let hi = (e < last).then(|| (fmt_num(b[e / 2]), e % 2 == 1));
    match (lo, hi) {
        (None, Some((h, true))) => format!("<= {h}"),
        (None, Some((h, false))) => format!("< {h}"),
        (Some((l, true)), None) => format!(">= {l}"),
        (Some((l, false)), None) => format!("> {l}"),
        (Some((l, _)), Some(_)) if s == e && s % 2 == 1 => l,
        (Some((l, lc)), Some((h, hc))) => format!("{}{l}..{h}{}", if lc { '[' } else { '(' }, if hc { ']' } else { ')' }),
        (None, None) => "-".into(),
    }
}

type Found = (Vec<usize>, Vec<usize>);

struct Search<'a> {
    cols: &'a [Col],
    w: usize,
    buf: Vec<u64>,
    ranges: Vec<(usize, usize)>,
    path: Vec<usize>,
    empties: Vec<Vec<usize>>,
    nodes: usize,
    limit: usize,
    found: Vec<Found>,
}

impl Search<'_> {
    fn dfs(&mut self, d: usize) -> Result<bool, ()> {
        let w = self.w;
        let col = &self.cols[d];
        let groups = col.members.len();
        self.nodes += groups;
        if self.nodes > BUDGET {
            return Err(());
        }
        let (lo, hi) = self.ranges[d];
        let mut empties = std::mem::take(&mut self.empties[d]);
        empties.clear();
        let base = &self.buf[d * w..(d + 1) * w];
        empties.extend((0..groups).filter(|&g| {
            let (a, b) = col.spans[g];
            let (s, e) = (a.max(lo), b.min(hi));
            s >= e || !base[s..e].iter().zip(&col.mask(g)[s..e]).any(|(x, y)| x & y != 0)
        }));
        if !empties.is_empty() {
            self.found.push((self.path.clone(), empties.clone()));
            if self.found.len() >= self.limit {
                self.empties[d] = empties;
                return Ok(false);
            }
        }
        let deeper = d + 1 < self.cols.len();
        if deeper {
            for g in 0..groups {
                if empties.binary_search(&g).is_ok() {
                    continue;
                }
                let (a, b) = col.spans[g];
                let (s, e) = (a.max(lo), b.min(hi));
                let (cur, rest) = self.buf.split_at_mut((d + 1) * w);
                let live = &cur[d * w + s..d * w + e];
                let next = &mut rest[s..e];
                for ((n, x), y) in next.iter_mut().zip(live).zip(&col.mask(g)[s..e]) {
                    *n = x & y;
                }
                let first = next.iter().position(|&x| x != 0).unwrap_or(0);
                let end = next.iter().rposition(|&x| x != 0).map_or(first, |p| p + 1);
                self.ranges[d + 1] = (s + first, s + end);
                self.path.push(g);
                let more = self.dfs(d + 1)?;
                self.path.pop();
                if !more {
                    self.empties[d] = empties;
                    return Ok(false);
                }
            }
        }
        self.empties[d] = empties;
        Ok(true)
    }
}
