//! Edits to a DMN file, applied as byte-range patches to its XML text so everything outside
//! the edited range (diagram, namespaces, vendor attributes, comments, formatting) is kept.

pub use crate::lint::Column;
use crate::parse::{child, kids};
use roxmltree::{Document, Node};
use std::collections::HashSet;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    Cell {
        decision: usize,
        rule: usize,
        col: Column,
        text: String,
    },
    /// `None` removes the cell's `expressionLanguage`, so it falls back to the file default.
    CellLanguage {
        decision: usize,
        rule: usize,
        col: Column,
        lang: Option<String>,
    },
    /// See `DecisionTable::note` for which element this changes.
    Note {
        decision: usize,
        rule: usize,
        text: String,
    },
    InputHeader {
        decision: usize,
        input: usize,
        label: String,
        expression: String,
        type_ref: Option<String>,
        lang: Option<String>,
        allowed: Option<String>,
    },
    OutputHeader {
        decision: usize,
        output: usize,
        label: String,
        name: String,
        type_ref: Option<String>,
        allowed: Option<String>,
    },
    HitPolicy {
        decision: usize,
        hit_policy: String,
        aggregation: Option<String>,
    },
    /// `copy_of: None` inserts an empty rule shaped like its neighbour.
    InsertRule {
        decision: usize,
        at: usize,
        copy_of: Option<usize>,
    },
    DeleteRule {
        decision: usize,
        rule: usize,
    },
    MoveRule {
        decision: usize,
        from: usize,
        to: usize,
    },
    InsertInput {
        decision: usize,
        at: usize,
    },
    DeleteInput {
        decision: usize,
        input: usize,
    },
    InsertOutput {
        decision: usize,
        at: usize,
    },
    DeleteOutput {
        decision: usize,
        output: usize,
    },
    /// Keeps the first element with this id and gives every later one `<id>_2`, `<id>_3`, …
    /// (skipping ids already in use). References to the id keep pointing at the first element.
    RenameDuplicateId {
        id: String,
    },
}

/// Returns the edited XML. Text before and after every changed range is kept byte for byte.
pub fn apply(xml: &str, edit: &Edit) -> Result<String, String> {
    if let Some(rest) = xml.strip_prefix('\u{feff}') {
        return apply(rest, edit).map(|s| format!("\u{feff}{s}"));
    }
    let out = match edit {
        Edit::Cell { decision, rule, col, text } => step(xml, |x, doc| {
            let e = entry(doc, *decision, *rule, *col)?;
            Ok(set_text(x, e, text).into_iter().collect())
        })?,
        Edit::CellLanguage { decision, rule, col, lang } => step(xml, |x, doc| {
            let e = entry(doc, *decision, *rule, *col)?;
            Ok(set_attr(x, e, "expressionLanguage", lang.as_deref()).into_iter().collect())
        })?,
        Edit::Note { decision, rule, text } => step(xml, |x, doc| note(x, doc, *decision, *rule, text))?,
        Edit::HitPolicy { decision, hit_policy, aggregation } => step(xml, |x, doc| {
            let t = table(doc, *decision)?;
            let keep_default = hit_policy == "UNIQUE" && t.attribute("hitPolicy").is_none();
            let hp = if keep_default { None } else { set_attr(x, t, "hitPolicy", Some(hit_policy)) };
            let agg = set_attr(x, t, "aggregation", aggregation.as_deref().filter(|_| hit_policy == "COLLECT"));
            Ok(hp.into_iter().chain(agg).collect())
        })?,
        // One parse for all of a header's changes; only overlapping changes (e.g. growing a
        // self-closing <output/> while also editing its attributes) need one parse per change.
        Edit::InputHeader { .. } | Edit::OutputHeader { .. } => match step(xml, |x, doc| header_patches(x, doc, edit)) {
            Err(e) if e == OVERLAP => header_steps(xml, edit)?,
            r => r?,
        },
        Edit::InsertRule { decision, at, copy_of } => step(xml, |x, doc| insert_rule(x, doc, *decision, *at, *copy_of))?,
        Edit::DeleteRule { decision, rule } => step(xml, |x, doc| Ok(vec![remove(x, nth(table(doc, *decision)?, "rule", *rule)?)]))?,
        Edit::MoveRule { decision, from, to } => step(xml, |x, doc| move_rule(x, doc, *decision, *from, *to))?,
        Edit::InsertInput { decision, at } => step(xml, |x, doc| insert_column(x, doc, *decision, *at, true))?,
        Edit::InsertOutput { decision, at } => step(xml, |x, doc| insert_column(x, doc, *decision, *at, false))?,
        Edit::DeleteInput { decision, input } => step(xml, |x, doc| delete_column(x, doc, *decision, *input, true))?,
        Edit::DeleteOutput { decision, output } => step(xml, |x, doc| delete_column(x, doc, *decision, *output, false))?,
        Edit::RenameDuplicateId { id } => step(xml, |_, doc| Ok(rename_duplicates(doc, id)))?,
    };
    // Unchanged output is the input, which already parsed, so it needs no second check.
    if out != xml {
        Document::parse(&out).map_err(|e| format!("the edit would produce invalid XML: {e}"))?;
    }
    Ok(out)
}

const OVERLAP: &str = "internal error: overlapping edits";

fn header_patches(x: &str, doc: &Document, edit: &Edit) -> Result<Vec<Patch>, String> {
    Ok(match edit {
        Edit::InputHeader { decision, input, label, expression, type_ref, lang, allowed } => {
            let i = clause(doc, *decision, "input", *input)?;
            let Some(ie) = child(i, "inputExpression") else { return Err(OVERLAP.into()) };
            [
                set_attr(x, i, "label", nonempty(label)),
                set_attr(x, ie, "typeRef", type_ref.as_deref()),
                set_attr(x, ie, "expressionLanguage", lang.as_deref()),
                set_text(x, ie, expression),
                optional_text(x, doc, i, "inputValues", allowed.as_deref(), Place::After("inputExpression")),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
        Edit::OutputHeader { decision, output, label, name, type_ref, allowed } => {
            let o = clause(doc, *decision, "output", *output)?;
            [
                set_attr(x, o, "label", nonempty(label)),
                set_attr(x, o, "name", nonempty(name)),
                set_attr(x, o, "typeRef", type_ref.as_deref()),
                optional_text(x, doc, o, "outputValues", allowed.as_deref(), Place::First),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
        _ => vec![],
    })
}

/// The same header changes applied one at a time, re-parsing in between.
fn header_steps(xml: &str, edit: &Edit) -> Result<String, String> {
    Ok(match edit {
        Edit::InputHeader { decision, input, label, expression, type_ref, lang, allowed } => {
            let d = *decision;
            let mut x = step(xml, |x, doc| Ok(set_attr(x, clause(doc, d, "input", *input)?, "label", nonempty(label)).into_iter().collect()))?;
            x = step(&x, |x, doc| {
                let i = clause(doc, d, "input", *input)?;
                Ok(match child(i, "inputExpression") {
                    Some(ie) => set_attr(x, ie, "typeRef", type_ref.as_deref()).into_iter().chain(set_attr(x, ie, "expressionLanguage", lang.as_deref())).collect(),
                    None => {
                        let p = prefix(x, i);
                        let mut ids = Ids::new(doc);
                        let mut attrs = format!(r#" id="{}""#, ids.fresh("LiteralExpression"));
                        for (k, v) in [("typeRef", type_ref), ("expressionLanguage", lang)] {
                            if let Some(v) = v {
                                attrs.push_str(&format!(r#" {k}="{}""#, esc_attr(v)));
                            }
                        }
                        vec![insert_child(x, i, &format!("<{p}inputExpression{attrs}><{p}text></{p}text></{p}inputExpression>"), true, false)]
                    }
                })
            })?;
            x = step(&x, |x, doc| Ok(child(clause(doc, d, "input", *input)?, "inputExpression").and_then(|ie| set_text(x, ie, expression)).into_iter().collect()))?;
            step(&x, |x, doc| {
                let i = clause(doc, d, "input", *input)?;
                Ok(optional_text(x, doc, i, "inputValues", allowed.as_deref(), Place::After("inputExpression")).into_iter().collect())
            })?
        }
        Edit::OutputHeader { decision, output, label, name, type_ref, allowed } => {
            let d = *decision;
            let x = step(xml, |x, doc| {
                let o = clause(doc, d, "output", *output)?;
                Ok([set_attr(x, o, "label", nonempty(label)), set_attr(x, o, "name", nonempty(name)), set_attr(x, o, "typeRef", type_ref.as_deref())].into_iter().flatten().collect())
            })?;
            step(&x, |x, doc| Ok(optional_text(x, doc, clause(doc, d, "output", *output)?, "outputValues", allowed.as_deref(), Place::First).into_iter().collect()))?
        }
        _ => xml.to_string(),
    })
}

struct Patch {
    at: Range<usize>,
    with: String,
}

impl Patch {
    fn new(at: Range<usize>, with: impl Into<String>) -> Patch {
        Patch { at, with: with.into() }
    }

    fn insert(at: usize, with: impl Into<String>) -> Patch {
        Patch::new(at..at, with)
    }
}

fn step(xml: &str, f: impl FnOnce(&str, &Document) -> Result<Vec<Patch>, String>) -> Result<String, String> {
    let doc = Document::parse(xml).map_err(|e| format!("XML error: {e}"))?;
    let mut patches = f(xml, &doc)?;
    patches.sort_by_key(|p| p.at.start);
    let mut out = String::with_capacity(xml.len() + patches.iter().map(|p| p.with.len()).sum::<usize>());
    let mut pos = 0;
    for p in patches {
        if p.at.start < pos {
            return Err(OVERLAP.into());
        }
        out.push_str(&xml[pos..p.at.start]);
        out.push_str(&p.with);
        pos = p.at.end;
    }
    out.push_str(&xml[pos..]);
    Ok(out)
}

fn table<'a, 'i>(doc: &'a Document<'i>, decision: usize) -> Result<Node<'a, 'i>, String> {
    let d = kids(doc.root_element(), "decision").nth(decision).ok_or("no such decision")?;
    child(d, "decisionTable").ok_or_else(|| "this decision has no decision table".into())
}

fn nth<'a, 'i>(n: Node<'a, 'i>, name: &'static str, i: usize) -> Result<Node<'a, 'i>, String> {
    kids(n, name).nth(i).ok_or_else(|| format!("no {name} #{}", i + 1))
}

fn clause<'a, 'i>(doc: &'a Document<'i>, decision: usize, name: &'static str, i: usize) -> Result<Node<'a, 'i>, String> {
    nth(table(doc, decision)?, name, i)
}

fn entry<'a, 'i>(doc: &'a Document<'i>, decision: usize, rule: usize, col: Column) -> Result<Node<'a, 'i>, String> {
    let r = nth(table(doc, decision)?, "rule", rule)?;
    match col {
        Column::Input(i) => nth(r, "inputEntry", i),
        Column::Output(i) => nth(r, "outputEntry", i),
    }
}

fn nonempty(s: &str) -> Option<&str> {
    Some(s).filter(|s| !s.is_empty())
}

fn escape(s: &str, attr: bool) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            '\'' if attr => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

fn esc_text(s: &str) -> String {
    escape(s, false)
}

fn esc_attr(s: &str) -> String {
    escape(s, true)
}

// Raw-source helpers. Offsets are into `xml`, which is the exact text the Document was parsed from.

fn start(xml: &str, n: Node) -> usize {
    let s = n.range().start;
    if xml.as_bytes().get(s) == Some(&b'<') { s } else { s - 1 }
}

/// Just past the `>` that closes the start tag.
fn open_end(xml: &str, n: Node) -> usize {
    let b = xml.as_bytes();
    let mut quote = 0u8;
    for (i, &c) in b.iter().enumerate().skip(start(xml, n)) {
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            return i + 1;
        }
    }
    b.len()
}

fn self_closing(xml: &str, n: Node) -> bool {
    xml.as_bytes()[open_end(xml, n) - 2] == b'/'
}

fn close_start(xml: &str, n: Node) -> usize {
    xml[..n.range().end].rfind("</").unwrap_or(n.range().end)
}

fn qname<'x>(xml: &'x str, n: Node) -> &'x str {
    let s = &xml[start(xml, n) + 1..];
    &s[..s.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(s.len())]
}

fn prefix<'x>(xml: &'x str, n: Node) -> &'x str {
    let q = qname(xml, n);
    q.rfind(':').map_or("", |i| &q[..=i])
}

fn ws_start(xml: &str, n: Node) -> usize {
    let b = xml.as_bytes();
    let mut i = start(xml, n);
    while i > 0 && b[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    i
}

/// The whitespace (newline + indentation) in front of an element.
fn lead<'x>(xml: &'x str, n: Node) -> &'x str {
    &xml[ws_start(xml, n)..start(xml, n)]
}

fn elements<'a, 'i>(n: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(|c| c.is_element())
}

fn remove(xml: &str, n: Node) -> Patch {
    Patch::new(ws_start(xml, n)..n.range().end, "")
}

/// Replaces an element's content (keeping its tags and attributes) with already-escaped `content`.
fn set_inner(xml: &str, n: Node, content: &str) -> Patch {
    if self_closing(xml, n) {
        let s = start(xml, n);
        let open = xml[s..open_end(xml, n) - 2].trim_end();
        Patch::new(s..n.range().end, format!("{open}>{content}</{}>", qname(xml, n)))
    } else {
        Patch::new(open_end(xml, n)..close_start(xml, n), content)
    }
}

/// Inserts `src` as the first or last child element of `parent`, indented like its siblings.
/// `inline` keeps a child added to an element without children on the same line.
fn insert_child(xml: &str, parent: Node, src: &str, first: bool, inline: bool) -> Patch {
    let sibling = if first { elements(parent).next() } else { elements(parent).last() };
    match sibling {
        Some(s) if first => Patch::insert(ws_start(xml, s), format!("{}{src}", lead(xml, s))),
        Some(s) => Patch::insert(s.range().end, format!("{}{src}", lead(xml, s))),
        None if self_closing(xml, parent) => {
            let s = start(xml, parent);
            let open = xml[s..open_end(xml, parent) - 2].trim_end();
            let q = qname(xml, parent);
            if inline {
                return Patch::new(s..parent.range().end, format!("{open}>{src}</{q}>"));
            }
            let outer = lead(xml, parent);
            let nl = if outer.contains("\r\n") { "\r\n" } else { "\n" };
            let indent = outer.rsplit('\n').next().unwrap_or("");
            Patch::new(s..parent.range().end, format!("{open}>{nl}{indent}  {src}{nl}{indent}</{q}>"))
        }
        None => Patch::insert(close_start(xml, parent), src),
    }
}

fn set_text(xml: &str, holder: Node, text: &str) -> Option<Patch> {
    match child(holder, "text") {
        Some(t) if t.text().unwrap_or("") == text => None,
        Some(t) => Some(set_inner(xml, t, &esc_text(text))),
        None if text.is_empty() => None,
        None => {
            let p = prefix(xml, holder);
            Some(insert_child(xml, holder, &format!("<{p}text>{}</{p}text>", esc_text(text)), true, true))
        }
    }
}

fn set_attr(xml: &str, el: Node, name: &str, value: Option<&str>) -> Option<Patch> {
    let cur = el.attributes().find(|a| a.name() == name && a.namespace().is_none());
    match (cur, value) {
        (Some(a), Some(v)) if a.value() == v => None,
        (Some(a), Some(v)) => Some(Patch::new(a.range_value(), esc_attr(v))),
        (Some(a), None) => {
            let b = xml.as_bytes();
            let mut s = a.range().start;
            while s > 0 && b[s - 1].is_ascii_whitespace() {
                s -= 1;
            }
            Some(Patch::new(s..a.range().end, ""))
        }
        (None, Some(v)) => {
            let at = start(xml, el) + 1 + qname(xml, el).len();
            Some(Patch::insert(at, format!(r#" {name}="{}""#, esc_attr(v))))
        }
        (None, None) => None,
    }
}

enum Place {
    First,
    After(&'static str),
}

/// An optional `<name><text>…</text></name>` child: updated, created, or removed when `text` is empty.
fn optional_text(xml: &str, doc: &Document, parent: Node, name: &'static str, text: Option<&str>, place: Place) -> Option<Patch> {
    let text = text.filter(|t| !t.trim().is_empty());
    match (child(parent, name), text) {
        (Some(n), None) => Some(remove(xml, n)),
        (Some(n), Some(t)) => set_text(xml, n, t),
        (None, None) => None,
        (None, Some(t)) => {
            let p = prefix(xml, parent);
            let src = format!(r#"<{p}{name} id="{}"><{p}text>{}</{p}text></{p}{name}>"#, Ids::new(doc).fresh("UnaryTests"), esc_text(t));
            Some(match place {
                Place::After(sib) => match child(parent, sib) {
                    Some(s) => Patch::insert(s.range().end, format!("{}{src}", lead(xml, s))),
                    None => insert_child(xml, parent, &src, false, false),
                },
                Place::First => insert_child(xml, parent, &src, true, false),
            })
        }
    }
}

fn note(xml: &str, doc: &Document, decision: usize, rule: usize, text: &str) -> Result<Vec<Patch>, String> {
    let t = table(doc, decision)?;
    let r = nth(t, "rule", rule)?;
    let p = prefix(xml, r);
    let patch = if kids(t, "annotation").next().is_some() {
        match kids(r, "annotationEntry").next() {
            Some(a) => set_text(xml, a, text),
            None if text.is_empty() => None,
            None => Some(insert_child(xml, r, &format!("<{p}annotationEntry><{p}text>{}</{p}text></{p}annotationEntry>", esc_text(text)), false, false)),
        }
    } else {
        match child(r, "description") {
            Some(d) if text.is_empty() => Some(remove(xml, d)),
            Some(d) if d.text().unwrap_or("") == text => None,
            Some(d) => Some(set_inner(xml, d, &esc_text(text))),
            None if text.is_empty() => None,
            None => Some(insert_child(xml, r, &format!("<{p}description>{}</{p}description>", esc_text(text)), true, false)),
        }
    };
    Ok(patch.into_iter().collect())
}

/// Ids already in the document, and fresh ones in the style Camunda Modeler generates.
struct Ids<'a> {
    used: HashSet<&'a str>,
    made: HashSet<String>,
    seed: u64,
}

impl<'a> Ids<'a> {
    fn new(doc: &'a Document) -> Ids<'a> {
        let used: HashSet<&str> = doc.descendants().filter_map(|n| n.attribute("id")).collect();
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        Ids { seed: nanos ^ (used.len() as u64).rotate_left(32) | 1, used, made: HashSet::new() }
    }

    fn fresh(&mut self, prefix: &str) -> String {
        loop {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 7;
            self.seed ^= self.seed << 17;
            let mut n = self.seed;
            let suffix: String = (0..7)
                .map(|_| {
                    let c = b"0123456789abcdefghijklmnopqrstuvwxyz"[(n % 36) as usize] as char;
                    n /= 36;
                    c
                })
                .collect();
            let id = format!("{prefix}_{suffix}");
            if !self.used.contains(id.as_str()) && self.made.insert(id.clone()) {
                return id;
            }
        }
    }
}

fn id_attr<'a, 'i>(n: Node<'a, 'i>) -> Option<roxmltree::Attribute<'a, 'i>> {
    n.attributes().find(|a| a.name() == "id" && a.namespace().is_none())
}

fn rename_duplicates(doc: &Document, id: &str) -> Vec<Patch> {
    let used: HashSet<&str> = doc.descendants().filter_map(|n| n.attribute("id")).collect();
    // The counter only grows, so the new ids are distinct from each other as well.
    let mut n = 2;
    let mut fresh = || loop {
        let c = format!("{id}_{n}");
        n += 1;
        if !used.contains(c.as_str()) {
            break c;
        }
    };
    doc.descendants().filter(|n| n.is_element()).filter_map(id_attr).filter(|a| a.value() == id).skip(1).map(|a| Patch::new(a.range_value(), esc_attr(&fresh()))).collect()
}

fn id_prefix(element: &str) -> &'static str {
    match element {
        "rule" => "DecisionRule",
        "inputEntry" | "inputValues" | "outputValues" => "UnaryTests",
        "outputEntry" | "inputExpression" => "LiteralExpression",
        "input" => "InputClause",
        "output" => "OutputClause",
        _ => "Element",
    }
}

fn insert_rule(xml: &str, doc: &Document, decision: usize, at: usize, copy_of: Option<usize>) -> Result<Vec<Patch>, String> {
    let t = table(doc, decision)?;
    let rules: Vec<Node> = kids(t, "rule").collect();
    let mut ids = Ids::new(doc);
    let at = at.min(rules.len());
    let Some(&template) = copy_of.map_or_else(|| rules.get(at.min(rules.len().saturating_sub(1))), |i| rules.get(i)) else {
        if copy_of.is_some() {
            return Err("no such rule to copy".into());
        }
        let p = prefix(xml, t);
        let mut src = format!(r#"<{p}rule id="{}">"#, ids.fresh("DecisionRule"));
        for (name, idp) in [("input", "UnaryTests"), ("output", "LiteralExpression")] {
            for _ in kids(t, name) {
                src.push_str(&format!(r#"<{p}{name}Entry id="{}"><{p}text></{p}text></{p}{name}Entry>"#, ids.fresh(idp)));
            }
        }
        src.push_str(&format!("</{p}rule>"));
        return Ok(vec![insert_child(xml, t, &src, false, false)]);
    };

    let base = start(xml, template);
    let mut inner = Vec::new();
    for n in template.descendants().filter(|n| n.is_element()) {
        if let Some(a) = n.attributes().find(|a| a.name() == "id" && a.namespace().is_none()) {
            inner.push(Patch::new(a.range_value(), ids.fresh(id_prefix(n.tag_name().name()))));
        }
        if copy_of.is_none() {
            match n.tag_name().name() {
                "description" => inner.push(remove(xml, n)),
                "text" if !self_closing(xml, n) => inner.push(Patch::new(open_end(xml, n)..close_start(xml, n), "")),
                _ => {}
            }
        }
    }
    // A removed <description> holds plain text (no ids, no <text>), so these patches never overlap.
    inner.sort_by_key(|p| p.at.start);
    let mut src = String::new();
    let mut pos = base;
    for p in inner {
        src.push_str(&xml[pos..p.at.start]);
        src.push_str(&p.with);
        pos = p.at.end;
    }
    src.push_str(&xml[pos..template.range().end]);

    Ok(vec![match rules.get(at) {
        Some(&next) => Patch::insert(ws_start(xml, next), format!("{}{src}", lead(xml, next))),
        None => Patch::insert(template.range().end, format!("{}{src}", lead(xml, template))),
    }])
}

fn move_rule(xml: &str, doc: &Document, decision: usize, from: usize, to: usize) -> Result<Vec<Patch>, String> {
    let rules: Vec<Node> = kids(table(doc, decision)?, "rule").collect();
    let (Some(&r), Some(&dest)) = (rules.get(from), rules.get(to)) else { return Err("no such rule".into()) };
    if from == to {
        return Ok(vec![]);
    }
    let src = format!("{}{}", lead(xml, r), &xml[start(xml, r)..r.range().end]);
    let insert = if to < from { Patch::insert(ws_start(xml, dest), src) } else { Patch::insert(dest.range().end, src) };
    Ok(vec![remove(xml, r), insert])
}

fn insert_column(xml: &str, doc: &Document, decision: usize, at: usize, input: bool) -> Result<Vec<Patch>, String> {
    let t = table(doc, decision)?;
    let p = prefix(xml, t);
    let mut ids = Ids::new(doc);
    let (clause, entry, after_entry) = if input { ("input", "inputEntry", "") } else { ("output", "outputEntry", "inputEntry") };
    let src = if input {
        format!(r#"<{p}input id="{}"><{p}inputExpression id="{}" typeRef="string"><{p}text></{p}text></{p}inputExpression></{p}input>"#, ids.fresh("InputClause"), ids.fresh("LiteralExpression"))
    } else {
        let names: HashSet<&str> = kids(t, "output").filter_map(|o| o.attribute("name")).collect();
        let name = (1..).map(|n| format!("output{n}")).find(|n| !names.contains(n.as_str())).unwrap_or_default();
        format!(r#"<{p}output id="{}" name="{name}" typeRef="string" />"#, ids.fresh("OutputClause"))
    };
    let mut patches = vec![insert_among(xml, t, clause, if input { "" } else { "input" }, at, &src)];
    for r in kids(t, "rule") {
        let src = format!(r#"<{p}{entry} id="{}"><{p}text></{p}text></{p}{entry}>"#, ids.fresh(id_prefix(entry)));
        patches.push(insert_among(xml, r, entry, after_entry, at, &src));
    }
    Ok(patches)
}

/// Inserts `src` as the `at`-th `name` child of `parent`. With no such children yet it goes after
/// the last `after` child, else before the first element that has to follow it, else last.
fn insert_among(xml: &str, parent: Node, name: &'static str, after: &'static str, at: usize, src: &str) -> Patch {
    if let Some(next) = kids(parent, name).nth(at) {
        return Patch::insert(ws_start(xml, next), format!("{}{src}", lead(xml, next)));
    }
    if let Some(last) = kids(parent, name).last() {
        return Patch::insert(last.range().end, format!("{}{src}", lead(xml, last)));
    }
    if let Some(prev) = (!after.is_empty()).then(|| kids(parent, after).last()).flatten() {
        return Patch::insert(prev.range().end, format!("{}{src}", lead(xml, prev)));
    }
    const ORDER: [&str; 7] = ["input", "output", "annotation", "rule", "inputEntry", "outputEntry", "annotationEntry"];
    let rank = |n: &str| ORDER.iter().position(|o| *o == n);
    let mine = rank(name);
    match elements(parent).find(|e| rank(e.tag_name().name()).is_some_and(|r| Some(r) > mine)) {
        Some(next) => Patch::insert(ws_start(xml, next), format!("{}{src}", lead(xml, next))),
        None => insert_child(xml, parent, src, false, false),
    }
}

fn delete_column(xml: &str, doc: &Document, decision: usize, i: usize, input: bool) -> Result<Vec<Patch>, String> {
    let t = table(doc, decision)?;
    let (clause, entry) = if input { ("input", "inputEntry") } else { ("output", "outputEntry") };
    if !input && kids(t, "output").count() <= 1 {
        return Err("a decision table needs at least one output".into());
    }
    let mut patches = vec![remove(xml, nth(t, clause, i)?)];
    patches.extend(kids(t, "rule").filter_map(|r| kids(r, entry).nth(i)).map(|e| remove(xml, e)));
    Ok(patches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_matches_the_chained_replacements() {
        for s in ["", "plain", "a < b && c > d", r#"say "hi" & 'bye'"#, "ünï <cødé> ✓", "&amp; already"] {
            let text = s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            assert_eq!(esc_text(s), text);
            assert_eq!(esc_attr(s), text.replace('"', "&quot;").replace('\'', "&apos;"));
        }
    }

    /// Fresh ids are random, so mask the ones header edits create before comparing.
    fn mask_ids(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(i) = rest.find("id=\"UnaryTests_") {
            let cut = i + "id=\"UnaryTests_".len();
            out.push_str(&rest[..cut]);
            out.push_str("#######");
            rest = &rest[cut + 7..];
        }
        out.push_str(rest);
        out
    }

    #[test]
    fn one_parse_header_edits_match_step_by_step() {
        let example = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/shipping-fees.dmn")).unwrap();
        let bare = r#"<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="d"><decision id="x"><decisionTable id="t"><input id="i" label="A"><inputExpression id="ie" typeRef="string"/></input><output id="o" name="b"/></decisionTable></decision></definitions>"#;
        let input = |input: usize, label: &str, expression: &str, type_ref: Option<&str>, lang: Option<&str>, allowed: Option<&str>| Edit::InputHeader {
            decision: 0,
            input,
            label: label.into(),
            expression: expression.into(),
            type_ref: type_ref.map(Into::into),
            lang: lang.map(Into::into),
            allowed: allowed.map(Into::into),
        };
        let output = |output: usize, label: &str, name: &str, type_ref: Option<&str>, allowed: Option<&str>| Edit::OutputHeader {
            decision: 0,
            output,
            label: label.into(),
            name: name.into(),
            type_ref: type_ref.map(Into::into),
            allowed: allowed.map(Into::into),
        };
        let cases = [
            (&example[..], input(0, "Zone", "Region", Some("string"), Some("juel"), Some(r#""EU","NA","APAC","LATAM""#))),
            (&example[..], input(1, "Weight (kg)", "weight", Some("integer"), None, Some("[0..100]"))),
            (&example[..], input(0, "Region", "Region", Some("string"), Some("juel"), None)),
            (&example[..], input(3, "", "x < y & z", None, Some("feel"), None)),
            (&example[..], output(0, "Fee", "Fee", Some("double"), Some("1,2"))),
            (&example[..], output(0, "Cost", "Fee", Some("double"), Some("1,2"))),
            (&example[..], output(2, "Label", "Label", None, None)),
            (bare, input(0, "A", "expr", Some("string"), None, None)),
            (bare, input(0, "B", "expr", Some("number"), None, Some("1"))),
            (bare, output(0, "", "b", None, Some(r#""y""#))),
        ];
        for (xml, edit) in cases {
            let fast = apply(xml, &edit).unwrap();
            let slow = header_steps(xml, &edit).unwrap();
            Document::parse(&slow).unwrap();
            assert_eq!(mask_ids(&fast), mask_ids(&slow), "{edit:?}");
            assert_ne!(fast, xml, "{edit:?} changed nothing");
        }
    }
}
