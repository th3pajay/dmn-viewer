use crate::model::*;
use crate::unary::{is_literal, is_quoted, literal_items};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN",
            Severity::Info => "INFO",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    Input(usize),
    Output(usize),
}

#[derive(Clone, Debug)]
pub struct Issue {
    pub severity: Severity,
    pub code: &'static str,
    pub decision: Option<usize>,
    pub rules: Vec<usize>,
    pub column: Option<Column>,
    pub message: String,
}

impl Issue {
    pub fn title(&self) -> &'static str {
        match self.code {
            "NON_FEEL_SYNTAX" => "Invalid FEEL expression",
            "UNREACHABLE_RULE" => "Unreachable rule",
            "OVERLAPPING_RULES" => "Overlapping rules",
            "NOT_IN_ALLOWED_VALUES" => "Value not allowed",
            "RULE_SHAPE" => "Malformed rule",
            "OUTPUT_NAME" => "Output name problem",
            "DUPLICATE_ID" => "Duplicate id",
            "DANGLING_REQUIREMENT" => "Missing requirement target",
            "NO_LOGIC" => "Decision has no logic",
            "JAVASCRIPT" => "JavaScript expressions",
            "NULL_UNSAFE" => "No null checks",
            "INCONSISTENT_SLICING" => "Inconsistent slicing",
            "PLACEHOLDER" => "Placeholder output",
            "EMPTY_OUTPUT" => "Empty output",
            "UNUSED_INPUT" => "Unused input",
            "TYPE_MISMATCH" => "Type mismatch",
            "DUPLICATE_RULE" => "Duplicate rules",
            "MERGEABLE_RULES" => "Rules can be merged",
            "MIXED_LANGUAGES" => "Mixed expression languages",
            "INPUT_ONLY_IN_OUTPUT" => "Input only read in outputs",
            "NO_DEFAULT_RULE" => "No catch-all rule",
            _ => "Issue",
        }
    }
}

// A cell's condition as lint compares it: literal sets by value, anything else by its text.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Cond<'a> {
    Any,
    In(&'a BTreeSet<String>),
    NotIn(&'a BTreeSet<String>),
    Other(&'a str),
}

fn cond(e: &InputEntry) -> Cond<'_> {
    if e.tests.is_any() {
        return Cond::Any;
    }
    match e.tests.literals() {
        Some(l) if l.negated => Cond::NotIn(&l.set),
        Some(l) => Cond::In(&l.set),
        None => Cond::Other(e.text.trim()),
    }
}

fn literal_set(s: &str) -> Option<BTreeSet<String>> {
    literal_items(s).map(|items| items.into_iter().collect())
}

fn covers(a: &Cond, b: &Cond) -> bool {
    match (a, b) {
        (Cond::Any, _) => true,
        (_, Cond::Any) => false,
        (Cond::In(x), Cond::In(y)) => y.is_subset(x),
        (Cond::NotIn(x), Cond::In(y)) => y.is_disjoint(x),
        (Cond::NotIn(x), Cond::NotIn(y)) => x.is_subset(y),
        (Cond::Other(x), Cond::Other(y)) => x == y,
        _ => false,
    }
}

fn overlaps(a: &Cond, b: &Cond) -> Option<bool> {
    match (a, b) {
        (Cond::Any, _) | (_, Cond::Any) => Some(true),
        (Cond::In(x), Cond::In(y)) => Some(!x.is_disjoint(y)),
        (Cond::In(x), Cond::NotIn(y)) | (Cond::NotIn(y), Cond::In(x)) => Some(!x.is_subset(y)),
        (Cond::NotIn(_), Cond::NotIn(_)) => Some(true),
        (Cond::Other(x), Cond::Other(y)) if x == y => Some(true),
        _ => None,
    }
}

fn masked(s: &str) -> Vec<char> {
    let mut out = Vec::with_capacity(s.len());
    let mut in_str = false;
    let mut prev = '\0';
    for c in s.chars() {
        if in_str {
            if c == '"' && prev != '\\' {
                in_str = false;
                out.push('"');
            } else {
                out.push(' ');
            }
        } else {
            if c == '"' {
                in_str = true;
            }
            out.push(c);
        }
        prev = c;
    }
    out
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

#[derive(Clone, Debug)]
pub struct MethodCall {
    pub receiver: String,
    pub method: String,
    pub start: usize,
    pub args_start: usize,
    pub end: usize,
}

pub fn method_calls(s: &str) -> Vec<MethodCall> {
    let m = masked(s);
    let mut out = Vec::new();
    for i in 0..m.len() {
        if m[i] != '.' {
            continue;
        }
        let mut j = i;
        while j > 0 && is_ident(m[j - 1]) {
            j -= 1;
        }
        let receiver: String = m[j..i].iter().collect();
        let paren_receiver = receiver.is_empty() && i > 0 && m[i - 1] == ')';
        if !paren_receiver && (receiver.is_empty() || receiver.chars().next().is_some_and(|c| c.is_ascii_digit())) {
            continue;
        }
        let mut k = i + 1;
        while k < m.len() && is_ident(m[k]) {
            k += 1;
        }
        if k == i + 1 {
            continue;
        }
        let method: String = m[i + 1..k].iter().collect();
        let mut p = k;
        while p < m.len() && m[p].is_whitespace() {
            p += 1;
        }
        if p >= m.len() || m[p] != '(' {
            continue;
        }
        let mut depth = 0;
        let mut e = p;
        while e < m.len() {
            match m[e] {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            e += 1;
        }
        out.push(MethodCall { receiver: if paren_receiver { ")".into() } else { receiver }, method, start: j, args_start: p + 1, end: e.min(m.len()) });
    }
    out
}

fn substring_args(s: &[char], call: &MethodCall) -> Option<(i64, Option<i64>)> {
    if call.method != "substring" || call.end > s.len() {
        return None;
    }
    let args: String = s[call.args_start..call.end].iter().collect();
    let parts: Vec<&str> = args.split(',').map(|x| x.trim()).collect();
    match parts.as_slice() {
        [a] => Some((a.parse().ok()?, None)),
        [a, b] => Some((a.parse().ok()?, Some(b.parse().ok()?))),
        _ => None,
    }
}

pub fn feel_suggestion(text: &str) -> Option<String> {
    let calls = method_calls(text);
    if calls.is_empty() {
        return None;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut pos = 0;
    for c in &calls {
        if c.receiver == ")" || c.start < pos {
            return None;
        }
        let (a, b) = substring_args(&chars, c)?;
        out.extend(&chars[pos..c.start]);
        match b {
            Some(b) => write!(out, "substring({}, {}, {})", c.receiver, a + 1, (b - a).max(0)).ok()?,
            None => write!(out, "substring({}, {})", c.receiver, a + 1).ok()?,
        }
        pos = c.end + 1;
    }
    out.extend(&chars[pos.min(chars.len())..]);
    Some(out.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn aliases(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for stmt in text.split([';', '\n']) {
        let s = stmt.trim();
        let rest = ["var ", "let ", "const "].iter().find_map(|k| s.strip_prefix(k));
        if let Some((name, value)) = rest.and_then(|r| r.split_once('=')) {
            let (name, value) = (name.trim(), value.trim());
            if !name.is_empty() && !value.is_empty() && name.chars().all(is_ident) && value.chars().all(is_ident) {
                out.insert(name.to_string(), value.to_string());
            }
        }
    }
    out
}

fn rule_list(rules: &[usize]) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i < rules.len() {
        let start = rules[i];
        let mut end = start;
        while i + 1 < rules.len() && rules[i + 1] == end + 1 {
            i += 1;
            end = rules[i];
        }
        if !s.is_empty() {
            s.push_str(", ");
        }
        if end > start {
            let _ = write!(s, "{}-{}", start + 1, end + 1);
        } else {
            let _ = write!(s, "{}", start + 1);
        }
        i += 1;
    }
    s
}

fn word_in(hay: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let m: String = masked(hay).into_iter().collect();
    m.match_indices(word).any(|(i, _)| {
        let before = m[..i].chars().next_back();
        let after = m[i + word.len()..].chars().next();
        !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
    })
}

pub fn lint(defs: &Definitions) -> Vec<Issue> {
    let mut v = Vec::new();
    for id in &defs.duplicate_ids {
        v.push(Issue {
            severity: Severity::Error,
            code: "DUPLICATE_ID",
            decision: None,
            rules: vec![],
            column: None,
            message: format!("Element id \"{id}\" is used more than once; ids must be unique."),
        });
    }
    let known: BTreeSet<&str> = defs
        .decisions
        .iter()
        .map(|d| d.id.as_str())
        .chain(defs.input_data.iter().map(|x| x.id.as_str()))
        .chain(defs.knowledge_sources.iter().map(|x| x.id.as_str()))
        .chain(defs.bkms.iter().map(|x| x.id.as_str()))
        .collect();

    for (di, d) in defs.decisions.iter().enumerate() {
        for r in &d.requirements {
            if !r.href.is_empty() && !r.href.contains('/') && !known.contains(r.href.as_str()) {
                v.push(Issue {
                    severity: Severity::Error,
                    code: "DANGLING_REQUIREMENT",
                    decision: Some(di),
                    rules: vec![],
                    column: None,
                    message: format!("{} points to \"{}\", which does not exist in this file.", r.kind, r.href),
                });
            }
        }
        match &d.logic {
            DecisionLogic::Table(t) => lint_table(di, t, &mut v),
            DecisionLogic::Literal { text, lang, .. } => {
                if lang.language == Language::Feel && !method_calls(text).is_empty() {
                    v.push(Issue {
                        severity: Severity::Error,
                        code: "NON_FEEL_SYNTAX",
                        decision: Some(di),
                        rules: vec![],
                        column: None,
                        message: "Literal expression uses method-call syntax but is evaluated as FEEL.".into(),
                    });
                }
            }
            DecisionLogic::None => v.push(Issue {
                severity: Severity::Warning,
                code: "NO_LOGIC",
                decision: Some(di),
                rules: vec![],
                column: None,
                message: "Decision has no decision table or literal expression.".into(),
            }),
        }
    }
    v.sort_by_key(|i| (i.decision, i.severity, i.rules.first().copied().unwrap_or(usize::MAX)));
    v
}

fn overlapping(a: &Cond, b: &Cond) -> bool {
    overlaps(a, b) == Some(true)
}

const MEMO_LIMIT: usize = 128;

struct Interned<'a> {
    ids: Vec<u32>,
    reps: Vec<Cond<'a>>,
    table: Option<Vec<bool>>,
}

impl<'a> Interned<'a> {
    fn new(tests: &[Vec<Cond<'a>>], live: &[usize], c: usize, relation: Option<fn(&Cond, &Cond) -> bool>) -> Self {
        let mut seen: HashMap<Cond<'a>, u32> = HashMap::new();
        let mut reps = Vec::new();
        let mut ids = vec![0; tests.len()];
        for &r in live {
            let cond = tests[r][c];
            let next = reps.len() as u32;
            let id = *seen.entry(cond).or_insert(next);
            if id == next {
                reps.push(cond);
            }
            ids[r] = id;
        }
        let d = reps.len();
        let table = relation.filter(|_| d <= MEMO_LIMIT).map(|f| (0..d * d).map(|k| f(&reps[k / d], &reps[k % d])).collect());
        Interned { ids, reps, table }
    }

    fn holds(&self, relation: fn(&Cond, &Cond) -> bool, i: usize, j: usize) -> bool {
        let (a, b) = (self.ids[i] as usize, self.ids[j] as usize);
        match &self.table {
            Some(t) => t[a * self.reps.len() + b],
            None => relation(&self.reps[a], &self.reps[b]),
        }
    }
}

fn lint_table(di: usize, t: &DecisionTable, v: &mut Vec<Issue>) {
    let mut push = |severity, code, rules: Vec<usize>, column, message: String| {
        v.push(Issue { severity, code, decision: Some(di), rules, column, message });
    };
    let hp = t.hit_policy.to_ascii_uppercase();
    let ni = t.inputs.len();
    let no = t.outputs.len();

    for (ri, r) in t.rules.iter().enumerate() {
        if r.inputs.len() != ni || r.outputs.len() != no {
            push(Severity::Error, "RULE_SHAPE", vec![ri], None, format!("Rule has {} input / {} output entries but the table has {ni} inputs / {no} outputs.", r.inputs.len(), r.outputs.len()));
        }
    }

    if no > 1 {
        let mut names = BTreeMap::<&str, usize>::new();
        for (oi, o) in t.outputs.iter().enumerate() {
            if o.name.is_empty() {
                push(Severity::Error, "OUTPUT_NAME", vec![], Some(Column::Output(oi)), format!("Output \"{}\" has no name; a table with several outputs needs a name on every output.", o.title()));
            } else {
                *names.entry(&o.name).or_default() += 1;
            }
        }
        for (n, c) in names {
            if c > 1 {
                push(Severity::Error, "OUTPUT_NAME", vec![], None, format!("Output name \"{n}\" is used {c} times."));
            }
        }
    }

    let tests: Vec<Vec<Cond>> = t.rules.iter().map(|r| r.inputs.iter().map(cond).collect()).collect();
    let outs: Vec<Vec<String>> = t.rules.iter().map(|r| r.outputs.iter().map(|e| e.text.split_whitespace().collect::<Vec<_>>().join(" ")).collect()).collect();
    let shaped = |ri: usize| tests[ri].len() == ni;

    let live: Vec<usize> = (0..t.rules.len()).filter(|&r| shaped(r)).collect();
    let relation: Option<fn(&Cond, &Cond) -> bool> = match hp.as_str() {
        "FIRST" => Some(covers),
        "UNIQUE" | "ANY" => Some(overlapping),
        _ => None,
    };
    let cols: Vec<Interned> = (0..ni).map(|c| Interned::new(&tests, &live, c, relation)).collect();
    let mut seen_outs: HashMap<&[String], u32> = HashMap::new();
    let out_ids: Vec<u32> = outs
        .iter()
        .map(|o| {
            let next = seen_outs.len() as u32;
            *seen_outs.entry(o.as_slice()).or_insert(next)
        })
        .collect();
    let same_inputs = |i: usize, j: usize| cols.iter().all(|col| col.ids[i] == col.ids[j]);

    match relation {
        Some(rel) => {
            let mut order: Vec<usize> = (0..ni).collect();
            order.sort_by_key(|&c| std::cmp::Reverse(cols[c].reps.len()));
            for (jn, &j) in live.iter().enumerate() {
                for &i in &live[..jn] {
                    if !order.iter().all(|&c| cols[c].holds(rel, i, j)) {
                        continue;
                    }
                    if hp == "FIRST" {
                        let what = if same_inputs(i, j) { "has exactly the same conditions as" } else { "is shadowed by" };
                        push(Severity::Error, "UNREACHABLE_RULE", vec![j, i], None, format!("Rule {} {} rule {}: with hit policy FIRST it can never fire.", j + 1, what, i + 1));
                        break;
                    }
                    if hp == "UNIQUE" || out_ids[i] != out_ids[j] {
                        push(Severity::Error, "OVERLAPPING_RULES", vec![i, j], None, format!("Rules {} and {} can match the same input, which violates hit policy {hp}.", i + 1, j + 1));
                    }
                }
            }
        }
        None => {
            let mut groups: HashMap<(Vec<u32>, u32), Vec<usize>> = HashMap::new();
            for &j in &live {
                let key = (cols.iter().map(|c| c.ids[j]).collect::<Vec<_>>(), out_ids[j]);
                let members = groups.entry(key).or_default();
                for &i in members.iter() {
                    push(Severity::Warning, "DUPLICATE_RULE", vec![i, j], None, format!("Rules {} and {} are identical.", i + 1, j + 1));
                }
                members.push(j);
            }
        }
    }

    let mut start = 0;
    while start < t.rules.len() {
        let mut end = start;
        let mut col: Option<usize> = None;
        while end + 1 < t.rules.len() && shaped(end) && shaped(end + 1) && outs[end] == outs[end + 1] {
            let diff: Vec<usize> = (0..ni).filter(|&c| tests[end][c] != tests[end + 1][c]).collect();
            if diff.len() != 1 || col.is_some_and(|c| c != diff[0]) {
                break;
            }
            let c = diff[0];
            if matches!(tests[end][c], Cond::Any | Cond::Other(_)) || matches!(tests[end + 1][c], Cond::Any | Cond::Other(_)) {
                break;
            }
            col = Some(c);
            end += 1;
        }
        if let Some(c) = col {
            let mut union = BTreeSet::new();
            let mut negs: Vec<&BTreeSet<String>> = vec![];
            for row in &tests[start..=end] {
                match &row[c] {
                    Cond::In(x) => union.extend(x.iter().cloned()),
                    Cond::NotIn(x) => negs.push(*x),
                    _ => {}
                }
            }
            let rules: Vec<usize> = (start..=end).collect();
            let name = t.inputs[c].title();
            let msg = if !negs.is_empty() && negs.iter().all(|n| n.is_subset(&union)) {
                format!(
                    "Rules {} have identical outputs and differ only in \"{name}\", where together they match every value. Replace them with a single rule that leaves \"{name}\" empty (-).",
                    rule_list(&rules)
                )
            } else if negs.is_empty() {
                format!("Rules {} have identical outputs and differ only in \"{name}\". They can be one rule with {name} = {}.", rule_list(&rules), union.into_iter().collect::<Vec<_>>().join(","))
            } else {
                String::new()
            };
            if !msg.is_empty() {
                push(Severity::Info, "MERGEABLE_RULES", rules, Some(Column::Input(c)), msg);
            }
        }
        start = end + 1;
    }

    let input_vars: Vec<&str> = t.inputs.iter().map(|i| i.expression.trim()).collect();
    let mut js_rules = BTreeSet::new();
    let mut null_rules = BTreeSet::new();
    let mut null_vars = BTreeSet::new();
    let mut slices: BTreeMap<String, BTreeMap<String, BTreeSet<usize>>> = BTreeMap::new();

    for (ii, inp) in t.inputs.iter().enumerate() {
        if inp.lang.language == Language::Feel && !method_calls(&inp.expression).is_empty() {
            push(Severity::Error, "NON_FEEL_SYNTAX", vec![], Some(Column::Input(ii)), format!("Input expression \"{}\" uses method-call syntax but is evaluated as FEEL.", inp.expression));
        }
    }

    for (ri, r) in t.rules.iter().enumerate() {
        for (ci, e) in r.inputs.iter().enumerate() {
            if e.is_any() {
                continue;
            }
            if e.lang.language == Language::Feel && !method_calls(&e.text).is_empty() {
                push(Severity::Error, "NON_FEEL_SYNTAX", vec![ri], Some(Column::Input(ci)), format!("Input entry `{}` uses method-call syntax but is evaluated as FEEL.", e.text.trim()));
            }
        }
        for (oi, e) in r.outputs.iter().enumerate() {
            if e.is_any() {
                continue;
            }
            let calls = method_calls(&e.text);
            let chars: Vec<char> = e.text.chars().collect();
            let alias = aliases(&e.text);
            for c in &calls {
                if let Some((a, b)) = substring_args(&chars, c)
                    && c.receiver != ")"
                {
                    let key = match b {
                        Some(b) => format!("({a},{b})"),
                        None => format!("({a})"),
                    };
                    let var = alias.get(&c.receiver).cloned().unwrap_or_else(|| c.receiver.clone());
                    slices.entry(var).or_default().entry(key).or_default().insert(ri);
                }
            }
            if e.lang.language == Language::Js {
                js_rules.insert(ri);
                for c in &calls {
                    if input_vars.contains(&c.receiver.as_str()) {
                        null_rules.insert(ri);
                        null_vars.insert(c.receiver.clone());
                    }
                }
            } else if e.lang.language == Language::Feel && !calls.is_empty() {
                let lang_note = if !e.lang.is_explicit() { "has no expressionLanguage, so Camunda evaluates it as FEEL (the default since 7.13)" } else { "is declared as FEEL" };
                let fix = match feel_suggestion(&e.text) {
                    Some(f) => format!(" FEEL equivalent: `{f}`. Or set expressionLanguage=\"javascript\" like the other rules."),
                    None => " Set expressionLanguage=\"javascript\" or rewrite it in FEEL.".into(),
                };
                push(
                    Severity::Error,
                    "NON_FEEL_SYNTAX",
                    vec![ri],
                    Some(Column::Output(oi)),
                    format!(
                        "Output \"{}\" `{}` calls .{}() but {}. FEEL has no method calls, so evaluation fails at runtime (on the legacy JUEL default, '+' string concatenation fails instead).{}",
                        t.outputs.get(oi).map(|o| o.title()).unwrap_or("?"),
                        e.text.trim(),
                        calls[0].method,
                        lang_note,
                        fix
                    ),
                );
            }
            let bare = e.text.trim().trim_matches('"').to_ascii_uppercase();
            if matches!(bare.as_str(), "TBC" | "TBD" | "TODO" | "FIXME" | "XXX" | "N/A" | "DUMMY" | "TEST") {
                push(
                    Severity::Warning,
                    "PLACEHOLDER",
                    vec![ri],
                    Some(Column::Output(oi)),
                    format!("Output \"{}\" is the placeholder value {}.", t.outputs.get(oi).map(|o| o.title()).unwrap_or("?"), e.text.trim()),
                );
            }
        }
        if !r.outputs.is_empty() && r.outputs.iter().all(|e| e.is_any()) {
            push(Severity::Warning, "EMPTY_OUTPUT", vec![ri], None, "Rule matches but every output is empty (null).".into());
        } else if no > 1 && r.outputs.first().is_some_and(|e| e.is_any()) {
            push(Severity::Warning, "EMPTY_OUTPUT", vec![ri], Some(Column::Output(0)), format!("Rule leaves the primary output \"{}\" empty, so callers get null for it.", t.outputs[0].title()));
        }
    }

    if !js_rules.is_empty() {
        let rules: Vec<usize> = js_rules.into_iter().collect();
        push(
            Severity::Warning,
            "JAVASCRIPT",
            rules.clone(),
            None,
            format!(
                "Rules {} use expressionLanguage=\"javascript\". This only works on Camunda 7 with a JSR-223 JavaScript engine (Nashorn was removed in JDK 15, add GraalJS) and is not supported at all in Camunda 8 / Zeebe (FEEL only).",
                rule_list(&rules)
            ),
        );
    }
    if !null_rules.is_empty() {
        let rules: Vec<usize> = null_rules.into_iter().collect();
        push(
            Severity::Warning,
            "NULL_UNSAFE",
            rules.clone(),
            None,
            format!(
                "Rules {} call methods on {} without a null check. If such a variable is missing the whole decision evaluation throws instead of returning a result.",
                rule_list(&rules),
                null_vars.into_iter().collect::<Vec<_>>().join(", ")
            ),
        );
    }
    for (var, ranges) in &slices {
        let mut per_rule: BTreeMap<usize, BTreeSet<&String>> = BTreeMap::new();
        for (k, rs) in ranges {
            for r in rs {
                per_rule.entry(*r).or_default().insert(k);
            }
        }
        let signatures: BTreeSet<&BTreeSet<&String>> = per_rule.values().collect();
        if signatures.len() > 1 {
            let lower = var.to_ascii_lowercase();
            let severity = if lower.contains("date") || lower.contains("time") { Severity::Warning } else { Severity::Info };
            let detail: Vec<String> = ranges.iter().map(|(k, rs)| format!("{k} in rule {}", rule_list(&rs.iter().copied().collect::<Vec<_>>()))).collect();
            let all: Vec<usize> = ranges.values().flatten().copied().collect::<BTreeSet<_>>().into_iter().collect();
            push(
                severity,
                "INCONSISTENT_SLICING",
                all,
                None,
                format!("{var} is sliced with different positions: {}. Check that every caller sends {var} in the same format, or the generated values will be wrong.", detail.join("; ")),
            );
        }
    }

    let langs: BTreeSet<String> = t.inputs.iter().map(|i| i.lang.declared.clone().unwrap_or_else(|| "default (FEEL)".into())).collect();
    if langs.len() > 1 {
        push(Severity::Info, "MIXED_LANGUAGES", vec![], None, format!("Input expressions mix expression languages: {}.", langs.into_iter().collect::<Vec<_>>().join(", ")));
    }

    for (ci, inp) in t.inputs.iter().enumerate() {
        let col: Vec<&Cond> = tests.iter().filter(|r| r.len() == ni).map(|r| &r[ci]).collect();
        if col.iter().all(|x| **x == Cond::Any) {
            let var = inp.expression.trim();
            let used = t.rules.iter().any(|r| r.outputs.iter().any(|e| word_in(&e.text, var)));
            if used {
                push(Severity::Info, "INPUT_ONLY_IN_OUTPUT", vec![], Some(Column::Input(ci)), format!("Input \"{}\" never constrains a rule; it is only read inside output expressions.", inp.title()));
            } else {
                push(Severity::Warning, "UNUSED_INPUT", vec![], Some(Column::Input(ci)), format!("Input \"{}\" is never used by any rule; remove it or add conditions.", inp.title()));
            }
        }
        if inp.type_ref.as_deref().is_some_and(|x| x.eq_ignore_ascii_case("string")) {
            let bad: Vec<usize> = col
                .iter()
                .enumerate()
                .filter(|(_, x)| match x {
                    Cond::In(s) | Cond::NotIn(s) => s.iter().any(|v| !is_quoted(v)),
                    _ => false,
                })
                .map(|(i, _)| i)
                .collect();
            if !bad.is_empty() {
                push(
                    Severity::Warning,
                    "TYPE_MISMATCH",
                    bad.clone(),
                    Some(Column::Input(ci)),
                    format!("Input \"{}\" is typed string but rules {} compare it with unquoted values.", inp.title(), rule_list(&bad)),
                );
            }
        }
        if let Some(allowed) = inp.input_values.as_deref().and_then(literal_set) {
            let mut bad_vals = BTreeSet::new();
            let mut bad_rules = Vec::new();
            for (ri, x) in col.iter().enumerate() {
                if let Cond::In(s) | Cond::NotIn(s) = x {
                    let missing: Vec<&String> = s.difference(&allowed).collect();
                    if !missing.is_empty() {
                        bad_rules.push(ri);
                        bad_vals.extend(missing.into_iter().cloned());
                    }
                }
            }
            if !bad_rules.is_empty() {
                push(
                    Severity::Error,
                    "NOT_IN_ALLOWED_VALUES",
                    bad_rules.clone(),
                    Some(Column::Input(ci)),
                    format!(
                        "Input \"{}\" restricts values to {} but rules {} use {}. The allowed-values list looks stale; Camunda Modeler flags these cells and they document the wrong domain.",
                        inp.title(),
                        inp.input_values.as_deref().unwrap_or("").trim(),
                        rule_list(&bad_rules),
                        bad_vals.into_iter().collect::<Vec<_>>().join(", ")
                    ),
                );
            }
        }
    }

    for (oi, out) in t.outputs.iter().enumerate() {
        if let Some(allowed) = out.output_values.as_deref().and_then(literal_set) {
            let bad: Vec<usize> =
                t.rules.iter().enumerate().filter(|(_, r)| r.outputs.get(oi).is_some_and(|e| is_literal(e.text.trim()) && !allowed.contains(e.text.trim()))).map(|(i, _)| i).collect();
            if !bad.is_empty() {
                push(
                    Severity::Error,
                    "NOT_IN_ALLOWED_VALUES",
                    bad.clone(),
                    Some(Column::Output(oi)),
                    format!("Output \"{}\" values in rules {} are not in its allowed list.", out.title(), rule_list(&bad)),
                );
            }
        }
    }

    if matches!(hp.as_str(), "FIRST" | "UNIQUE" | "ANY" | "PRIORITY") && !t.rules.is_empty() && !tests.iter().any(|r| r.iter().all(|x| *x == Cond::Any)) {
        push(
            Severity::Info,
            "NO_DEFAULT_RULE",
            vec![],
            None,
            "No catch-all rule: inputs that match nothing return an empty result (all outputs null). Add a final rule with every input set to '-' if callers expect a value.".into(),
        );
    }
}

pub fn report(defs: &Definitions, issues: &[Issue], source: &str) -> String {
    let mut s = String::new();
    let count = |sev| issues.iter().filter(|i| i.severity == sev).count();
    let _ = writeln!(s, "DMN check: {} ({source})", if defs.name.is_empty() { &defs.id } else { &defs.name });
    let _ = writeln!(s, "{} errors, {} warnings, {} info\n", count(Severity::Error), count(Severity::Warning), count(Severity::Info));
    let mut last = None;
    for i in issues {
        if i.decision != last {
            last = i.decision;
            let title = i.decision.and_then(|d| defs.decisions.get(d)).map(|d| d.name.as_str()).unwrap_or("File");
            let _ = writeln!(s, "== {title} ==");
        }
        let rules = if i.rules.is_empty() {
            String::new()
        } else {
            format!(
                " [rule {}]",
                rule_list(&{
                    let mut r = i.rules.clone();
                    r.sort();
                    r.dedup();
                    r
                })
            )
        };
        let _ = writeln!(s, "{:5} {}{}: {}", i.severity.label(), i.code, rules, i.message);
    }
    s
}

pub fn rules_label(rules: &[usize]) -> String {
    let mut r = rules.to_vec();
    r.sort();
    r.dedup();
    rule_list(&r)
}
