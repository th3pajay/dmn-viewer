use crate::eval::{Context, Mode, Program, Value, binop, feel_cmp, literal, mode_of};
use crate::model::ExpressionLanguage;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// An input entry's condition, parsed once when the file loads.
#[derive(Clone, Debug, Default)]
pub struct UnaryTests {
    kind: Kind,
}

/// The cell as a plain list of literals, e.g. `"EU","NA"` or `not(1, 2)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Literals {
    pub negated: bool,
    pub items: Vec<String>,
    pub set: BTreeSet<String>,
}

#[derive(Clone, Debug, Default)]
enum Kind {
    #[default]
    Any,
    Unsupported(String),
    Script(Operand),
    Feel {
        negated: bool,
        items: Vec<Item>,
        literals: Option<Literals>,
    },
}

#[derive(Clone, Debug)]
enum Item {
    Any,
    Null,
    Compare(&'static str, Operand),
    Range { lo: Operand, hi: Operand, lo_closed: bool, hi_closed: bool },
    Expr { value: Operand, uses_input: bool },
    Error(String),
}

#[derive(Clone, Debug)]
enum Operand {
    Value(Value),
    Program(Result<Program, String>),
}

impl Operand {
    fn compile(src: &str, mode: Mode) -> Operand {
        match literal(src, mode) {
            Some(v) => Operand::Value(v),
            None => Operand::Program(Program::compile(src, mode)),
        }
    }

    fn eval(&self, vars: &Context, local: Option<(&str, &Value)>) -> Result<Value, String> {
        match self {
            Operand::Value(v) => Ok(v.clone()),
            Operand::Program(Ok(p)) => p.exec(vars, local.map(|(k, v)| BTreeMap::from([(k.to_string(), v.clone())])).unwrap_or_default()),
            Operand::Program(Err(e)) => Err(e.clone()),
        }
    }
}

impl UnaryTests {
    pub fn parse(text: &str, lang: &ExpressionLanguage) -> UnaryTests {
        let t = text.trim();
        let kind = if t.is_empty() || t == "-" {
            Kind::Any
        } else {
            match mode_of(lang) {
                Err(e) => Kind::Unsupported(e),
                Ok(Mode::Feel) => feel(t),
                Ok(mode) => Kind::Script(Operand::compile(t, mode)),
            }
        };
        UnaryTests { kind }
    }

    /// Errors are the parse or evaluation errors of the first item that fails, as the cell is evaluated.
    pub fn matches(&self, input: &Value, vars: &Context) -> Result<bool, String> {
        match &self.kind {
            Kind::Any => Ok(true),
            Kind::Unsupported(e) => Err(e.clone()),
            Kind::Script(op) => Ok(op.eval(vars, Some(("cellInput", input)))?.truthy()),
            Kind::Feel { negated, items, .. } => {
                for it in items {
                    if it.matches(input, vars)? {
                        return Ok(!negated);
                    }
                }
                Ok(*negated)
            }
        }
    }

    pub fn is_any(&self) -> bool {
        matches!(self.kind, Kind::Any)
    }

    /// Only FEEL cells that are nothing but literals, so lint can compare cells as sets.
    pub fn literals(&self) -> Option<&Literals> {
        match &self.kind {
            Kind::Feel { literals, .. } => literals.as_ref(),
            _ => None,
        }
    }

    /// A value to type into the tester so the cell matches: the first listed literal, or empty (null) for `not(...)`.
    pub fn sample(&self) -> Option<String> {
        let l = self.literals()?;
        Some(if l.negated { String::new() } else { l.items.first().map(|v| v.trim_matches('"').to_string()).unwrap_or_default() })
    }
}

fn feel(t: &str) -> Kind {
    let (negated, list) = match strip_not(t) {
        Some(inner) => (true, inner),
        None => (false, t),
    };
    let items = if !list.contains(',') { if list.trim().is_empty() { vec![] } else { vec![item(list)] } } else { split_top(list).iter().map(|i| item(i)).collect() };
    let literals = literal_items(list).map(|items| Literals { negated, set: items.iter().cloned().collect(), items });
    Kind::Feel { negated, items, literals }
}

fn item(src: &str) -> Item {
    let it = src.trim();
    if it == "-" || it.is_empty() {
        return Item::Any;
    }
    if it == "null" {
        return Item::Null;
    }
    for (op, bin) in [("<=", "<="), (">=", ">="), ("!=", "!="), ("<", "<"), (">", ">"), ("=", "==")] {
        if let Some(rest) = it.strip_prefix(op) {
            return Item::Compare(bin, Operand::compile(rest, Mode::Feel));
        }
    }
    let first = it.chars().next().unwrap_or(' ');
    let last = it.chars().last().unwrap_or(' ');
    if matches!(first, '[' | '(' | ']') && matches!(last, ']' | ')' | '[') && it.contains("..") {
        let Some((lo, hi)) = it[1..it.len() - 1].split_once("..") else { return Item::Error("bad range".into()) };
        return Item::Range { lo: Operand::compile(lo, Mode::Feel), hi: Operand::compile(hi, Mode::Feel), lo_closed: first == '[', hi_closed: last == ']' };
    }
    Item::Expr { value: Operand::compile(it, Mode::Feel), uses_input: it.contains('?') }
}

impl Item {
    fn matches(&self, input: &Value, vars: &Context) -> Result<bool, String> {
        Ok(match self {
            Item::Any => true,
            Item::Null => *input == Value::Null,
            Item::Compare(op, v) => binop(op, input, &v.eval(vars, None)?, Mode::Feel)? == Value::Bool(true),
            Item::Range { lo, hi, lo_closed, hi_closed } => {
                let lo = lo.eval(vars, None)?;
                let hi = hi.eval(vars, None)?;
                let (Some(a), Some(b)) = (feel_cmp(input, &lo), feel_cmp(input, &hi)) else { return Ok(false) };
                let lo_ok = if *lo_closed { a != Ordering::Less } else { a == Ordering::Greater };
                let hi_ok = if *hi_closed { b != Ordering::Greater } else { b == Ordering::Less };
                lo_ok && hi_ok
            }
            Item::Expr { value, uses_input } => {
                let v = value.eval(vars, Some(("?", input)))?;
                match (&v, input) {
                    (Value::Bool(b), inp) if !matches!(inp, Value::Bool(_)) || *uses_input => *b,
                    _ => v == *input,
                }
            }
            Item::Error(e) => return Err(e.clone()),
        })
    }
}

pub(crate) fn split_top(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut prev = '\0';
    for c in s.chars() {
        if in_str {
            cur.push(c);
            if c == '"' && prev != '\\' {
                in_str = false;
            }
        } else {
            match c {
                '"' => {
                    in_str = true;
                    cur.push(c);
                }
                '(' | '[' | '{' => {
                    depth += 1;
                    cur.push(c);
                }
                ')' | ']' | '}' => {
                    depth -= 1;
                    cur.push(c);
                }
                ',' if depth == 0 => {
                    out.push(cur.trim().to_string());
                    cur.clear();
                }
                _ => cur.push(c),
            }
        }
        prev = c;
    }
    if !cur.trim().is_empty() || !out.is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

pub(crate) fn strip_not(t: &str) -> Option<&str> {
    let inner = t.strip_prefix("not")?.trim_start().strip_prefix('(')?.strip_suffix(')')?;
    let mut depth = 0i32;
    let mut in_str = false;
    for c in inner.chars() {
        match c {
            '"' => in_str = !in_str,
            '(' if !in_str => depth += 1,
            ')' if !in_str => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    Some(inner)
}

pub(crate) fn is_quoted(s: &str) -> bool {
    s.len() >= 2 && s.starts_with('"') && s.ends_with('"') && !s[1..s.len() - 1].contains('"')
}

pub(crate) fn is_literal(s: &str) -> bool {
    is_quoted(s) || s.parse::<f64>().is_ok() || s == "true" || s == "false"
}

/// The items of a comma-separated list, in written order, if every one is a literal.
pub(crate) fn literal_items(s: &str) -> Option<Vec<String>> {
    let items = split_top(s);
    (!items.is_empty() && items.iter().all(|i| is_literal(i))).then_some(items)
}
