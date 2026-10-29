use crate::model::*;
use crate::unary::split_top;
use std::borrow::Cow;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Str(String),
    Num(f64),
    Bool(bool),
}

pub type Context = BTreeMap<String, Value>;

pub fn fmt_num(n: f64) -> String {
    if n.is_nan() {
        "NaN".into()
    } else if n.fract() == 0.0 && n.abs() < 1e21 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

impl Value {
    pub fn display(&self) -> String {
        match self {
            Value::Null => "null".into(),
            Value::Str(s) => format!("\"{s}\""),
            Value::Num(n) => fmt_num(*n),
            Value::Bool(b) => b.to_string(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Str(_) => "string",
            Value::Num(_) => "number",
            Value::Bool(_) => "boolean",
        }
    }

    fn js_string(&self) -> String {
        match self {
            Value::Null => "null".into(),
            Value::Str(s) => s.clone(),
            Value::Num(n) => fmt_num(*n),
            Value::Bool(b) => b.to_string(),
        }
    }

    fn js_number(&self) -> f64 {
        match self {
            Value::Null => 0.0,
            Value::Str(s) if s.trim().is_empty() => 0.0,
            Value::Str(s) => s.trim().parse().unwrap_or(f64::NAN),
            Value::Num(n) => *n,
            Value::Bool(b) => *b as i32 as f64,
        }
    }

    pub(crate) fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Str(s) => !s.is_empty(),
            Value::Num(n) => *n != 0.0 && !n.is_nan(),
            Value::Bool(b) => *b,
        }
    }

    pub fn from_input(text: &str, type_ref: Option<&str>) -> Value {
        let t = text.trim();
        if t.is_empty() || t == "null" {
            return Value::Null;
        }
        let quoted = t.len() >= 2 && t.starts_with('"') && t.ends_with('"');
        match type_ref.map(|x| x.to_ascii_lowercase()).as_deref() {
            Some("integer" | "long" | "double" | "number") => t.parse().map(Value::Num).unwrap_or_else(|_| Value::Str(text.to_string())),
            Some("boolean") if t == "true" || t == "false" => Value::Bool(t == "true"),
            _ if quoted => Value::Str(t[1..t.len() - 1].to_string()),
            _ => Value::Str(text.to_string()),
        }
    }

    pub fn coerce(self, type_ref: Option<&str>) -> Result<Value, String> {
        let Some(tr) = type_ref.map(|x| x.to_ascii_lowercase()) else { return Ok(self) };
        Ok(match (tr.as_str(), self) {
            (_, Value::Null) => Value::Null,
            ("string", v) => Value::Str(v.js_string()),
            ("boolean", Value::Bool(b)) => Value::Bool(b),
            ("boolean", Value::Str(s)) if s == "true" || s == "false" => Value::Bool(s == "true"),
            ("boolean", v) => return Err(format!("cannot convert {} to boolean", v.display())),
            ("integer" | "long" | "double" | "number", Value::Num(n)) => Value::Num(n),
            ("integer" | "long" | "double" | "number", Value::Str(s)) => Value::Num(s.trim().parse().map_err(|_| format!("cannot convert \"{s}\" to {tr}"))?),
            ("integer" | "long" | "double" | "number", v) => return Err(format!("cannot convert {} to {tr}", v.display())),
            (_, v) => v,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Feel,
    Js,
    Juel,
}

pub fn mode_of(lang: &ExpressionLanguage) -> Result<Mode, String> {
    match lang.language {
        Language::Feel => Ok(Mode::Feel),
        Language::Js => Ok(Mode::Js),
        Language::Juel => Ok(Mode::Juel),
        Language::Unsupported => {
            let l = lang.declared.as_deref().unwrap_or_default().to_ascii_lowercase();
            Err(format!("expression language \"{l}\" is not supported by the tester"))
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Str(String),
    Num(f64),
    Id(String),
    Op(&'static str),
    Eof,
}

const FEEL_FUNCS: [(&str, &str); 7] = [
    ("string length", "string_length"),
    ("upper case", "upper_case"),
    ("lower case", "lower_case"),
    ("starts with", "starts_with"),
    ("ends with", "ends_with"),
    ("substring before", "substring_before"),
    ("substring after", "substring_after"),
];

const OPS: [&str; 29] = ["===", "!==", "==", "!=", "<=", ">=", "&&", "||", "..", "+", "-", "*", "/", "%", "<", ">", "=", "!", "?", ":", "(", ")", ",", ".", ";", "[", "]", "{", "}"];

fn lex(src: &str, mode: Mode) -> Result<Vec<Tok>, String> {
    let mut src = Cow::Borrowed(src);
    if mode == Mode::Feel {
        for (a, b) in FEEL_FUNCS {
            if src.contains(a) {
                src = Cow::Owned(src.replace(a, b));
            }
        }
    }
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        if ch == '"' || ch == '\'' {
            let mut s = String::new();
            i += 1;
            loop {
                let Some(&x) = c.get(i) else { return Err("unterminated string literal".into()) };
                i += 1;
                if x == ch {
                    break;
                }
                if x == '\\' {
                    let Some(&e) = c.get(i) else { return Err("unterminated string literal".into()) };
                    i += 1;
                    s.push(match e {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        other => other,
                    });
                } else {
                    s.push(x);
                }
            }
            out.push(Tok::Str(s));
            continue;
        }
        if ch.is_ascii_digit() || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            while i < c.len() && c[i].is_ascii_digit() {
                i += 1;
            }
            if i < c.len() && c[i] == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit()) {
                i += 1;
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let s: String = c[start..i].iter().collect();
            out.push(Tok::Num(s.parse().map_err(|_| format!("bad number {s}"))?));
            continue;
        }
        if mode == Mode::Feel && ch == '?' {
            out.push(Tok::Id("?".into()));
            i += 1;
            continue;
        }
        if ch.is_alphabetic() || ch == '_' || ch == '$' {
            let start = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '$') {
                i += 1;
            }
            out.push(Tok::Id(c[start..i].iter().collect()));
            continue;
        }
        let rest = &c[i..];
        if let Some(op) = OPS.iter().find(|op| op.len() <= rest.len() && op.chars().zip(rest).all(|(a, &b)| a == b)) {
            out.push(Tok::Op(op));
            i += op.len();
            continue;
        }
        return Err(format!("unexpected character '{ch}'"));
    }
    out.push(Tok::Eof);
    Ok(out)
}

#[derive(Clone, Debug)]
enum Expr {
    Lit(Value),
    Var(String),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(&'static str, Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Method(Box<Expr>, String, Vec<Expr>),
    Prop(Box<Expr>, String),
}

#[derive(Clone, Debug)]
enum Stmt {
    Let(String, Expr),
    Expr(Expr),
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
    mode: Mode,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.t[self.i.min(self.t.len() - 1)]
    }

    fn next(&mut self) -> Tok {
        if self.i + 1 < self.t.len() {
            self.i += 1;
            std::mem::replace(&mut self.t[self.i - 1], Tok::Eof)
        } else {
            Tok::Eof
        }
    }

    fn is_op(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Op(o) if *o == s)
    }

    fn eat_op(&mut self, s: &str) -> bool {
        if self.is_op(s) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn is_kw(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Id(o) if o == s)
    }

    fn eat_kw(&mut self, s: &str) -> bool {
        if self.is_kw(s) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect_op(&mut self, s: &str) -> Result<(), String> {
        if self.eat_op(s) { Ok(()) } else { Err(format!("expected '{s}' but found {}", describe(self.peek()))) }
    }

    fn ident(&mut self) -> Result<String, String> {
        match self.next() {
            Tok::Id(s) => Ok(s),
            t => Err(format!("expected a name but found {}", describe(&t))),
        }
    }

    fn program(&mut self) -> Result<Vec<Stmt>, String> {
        let mut v = Vec::new();
        loop {
            while self.eat_op(";") {}
            if *self.peek() == Tok::Eof {
                break;
            }
            if self.mode != Mode::Feel && (self.eat_kw("var") || self.eat_kw("let") || self.eat_kw("const")) {
                let name = self.ident()?;
                self.expect_op("=")?;
                v.push(Stmt::Let(name, self.expr()?));
            } else {
                v.push(Stmt::Expr(self.expr()?));
            }
            if !self.eat_op(";") && *self.peek() != Tok::Eof {
                return Err(format!("unexpected {}", describe(self.peek())));
            }
        }
        Ok(v)
    }

    fn expr(&mut self) -> Result<Expr, String> {
        if self.mode == Mode::Feel && self.eat_kw("if") {
            let c = self.expr()?;
            if !self.eat_kw("then") {
                return Err("expected 'then'".into());
            }
            let a = self.expr()?;
            if !self.eat_kw("else") {
                return Err("expected 'else'".into());
            }
            let b = self.expr()?;
            return Ok(Expr::Cond(Box::new(c), Box::new(a), Box::new(b)));
        }
        let c = self.or()?;
        if self.mode != Mode::Feel && self.eat_op("?") {
            let a = self.expr()?;
            self.expect_op(":")?;
            let b = self.expr()?;
            return Ok(Expr::Cond(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
    }

    fn binary(&mut self, ops: &[&'static str], kws: &[(&str, &'static str)], next: fn(&mut Self) -> Result<Expr, String>) -> Result<Expr, String> {
        let mut l = next(self)?;
        loop {
            let op = if let Some(o) = ops.iter().find(|o| self.is_op(o)) {
                self.i += 1;
                *o
            } else if let Some((_, o)) = kws.iter().find(|(k, _)| (self.mode == Mode::Juel || (self.mode == Mode::Feel && matches!(*k, "and" | "or"))) && self.is_kw(k)) {
                self.i += 1;
                *o
            } else {
                return Ok(l);
            };
            let r = next(self)?;
            let op = match op {
                "===" | "=" => "==",
                "!==" => "!=",
                _ => op,
            };
            l = Expr::Bin(op, Box::new(l), Box::new(r));
        }
    }

    fn or(&mut self) -> Result<Expr, String> {
        self.binary(&["||"], &[("or", "||")], Self::and)
    }

    fn and(&mut self) -> Result<Expr, String> {
        self.binary(&["&&"], &[("and", "&&")], Self::equality)
    }

    fn equality(&mut self) -> Result<Expr, String> {
        let ops: &[&'static str] = if self.mode == Mode::Feel { &["!=", "="] } else { &["===", "!==", "==", "!="] };
        self.binary(ops, &[("eq", "=="), ("ne", "!=")], Self::comparison)
    }

    fn comparison(&mut self) -> Result<Expr, String> {
        self.binary(&["<=", ">=", "<", ">"], &[("lt", "<"), ("gt", ">"), ("le", "<="), ("ge", ">=")], Self::additive)
    }

    fn additive(&mut self) -> Result<Expr, String> {
        self.binary(&["+", "-"], &[], Self::multiplicative)
    }

    fn multiplicative(&mut self) -> Result<Expr, String> {
        self.binary(&["*", "/", "%"], &[], Self::unary)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat_op("-") {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.mode != Mode::Feel && (self.eat_op("!") || (self.mode == Mode::Juel && self.eat_kw("not"))) {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.primary()?;
        while self.eat_op(".") {
            let name = self.ident()?;
            if self.is_op("(") {
                e = Expr::Method(Box::new(e), name, self.args()?);
            } else {
                e = Expr::Prop(Box::new(e), name);
            }
        }
        Ok(e)
    }

    fn args(&mut self) -> Result<Vec<Expr>, String> {
        self.expect_op("(")?;
        let mut v = Vec::new();
        if self.eat_op(")") {
            return Ok(v);
        }
        loop {
            v.push(self.expr()?);
            if self.eat_op(")") {
                return Ok(v);
            }
            self.expect_op(",")?;
        }
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Tok::Num(n) => Ok(Expr::Lit(Value::Num(n))),
            Tok::Str(s) => Ok(Expr::Lit(Value::Str(s))),
            Tok::Id(s) => match s.as_str() {
                "true" => Ok(Expr::Lit(Value::Bool(true))),
                "false" => Ok(Expr::Lit(Value::Bool(false))),
                "null" | "undefined" => Ok(Expr::Lit(Value::Null)),
                _ if self.is_op("(") => Ok(Expr::Call(s, self.args()?)),
                _ => Ok(Expr::Var(s)),
            },
            Tok::Op("(") => {
                let e = self.expr()?;
                self.expect_op(")")?;
                Ok(e)
            }
            t => Err(format!("unexpected {}", describe(&t))),
        }
    }
}

fn describe(t: &Tok) -> String {
    match t {
        Tok::Str(s) => format!("string \"{s}\""),
        Tok::Num(n) => format!("number {}", fmt_num(*n)),
        Tok::Id(s) => format!("'{s}'"),
        Tok::Op(o) => format!("'{o}'"),
        Tok::Eof => "end of expression".into(),
    }
}

struct Env<'a> {
    vars: &'a Context,
    locals: BTreeMap<String, Value>,
    mode: Mode,
}

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn js_index(v: Option<&Value>, len: usize, default: usize) -> usize {
    match v {
        None | Some(Value::Null) => default,
        Some(v) => {
            let n = v.js_number();
            if n.is_nan() { 0 } else { n.trunc().clamp(0.0, len as f64) as usize }
        }
    }
}

fn rel_index(v: Option<&Value>, len: usize, default: usize) -> usize {
    match v {
        None | Some(Value::Null) => default,
        Some(v) => {
            let n = v.js_number();
            let n = if n.is_nan() { 0.0 } else { n.trunc() };
            let n = if n < 0.0 { (len as f64 + n).max(0.0) } else { n.min(len as f64) };
            n as usize
        }
    }
}

fn arg_str(args: &[Value], i: usize) -> String {
    args.get(i).map(|v| v.js_string()).unwrap_or_else(|| "undefined".into())
}

fn string_method(s: &str, m: &str, args: &[Value], mode: Mode) -> Result<Value, String> {
    let c = chars(s);
    let len = c.len();
    let sub = |a: usize, b: usize| Value::Str(c[a.min(b)..a.max(b)].iter().collect());
    Ok(match m {
        "substring" => {
            let a = js_index(args.first(), len, 0);
            let b = js_index(args.get(1), len, len);
            if mode == Mode::Juel && (args.first().map(|v| v.js_number()).unwrap_or(0.0) > len as f64 || args.get(1).map(|v| v.js_number()).unwrap_or(0.0) > len as f64) {
                return Err(format!("StringIndexOutOfBoundsException: \"{s}\" has length {len}"));
            }
            sub(a, b)
        }
        "substr" => {
            let a = rel_index(args.first(), len, 0);
            let n = js_index(args.get(1), len, len);
            sub(a, (a + n).min(len))
        }
        "slice" => {
            let a = rel_index(args.first(), len, 0);
            let b = rel_index(args.get(1), len, len);
            if a >= b { Value::Str(String::new()) } else { sub(a, b) }
        }
        "charAt" => {
            let i = js_index(args.first(), len, 0);
            Value::Str(c.get(i).map(|x| x.to_string()).unwrap_or_default())
        }
        "toUpperCase" => Value::Str(s.to_uppercase()),
        "toLowerCase" => Value::Str(s.to_lowercase()),
        "trim" => Value::Str(s.trim().to_string()),
        "toString" => Value::Str(s.to_string()),
        "length" if mode == Mode::Juel => Value::Num(len as f64),
        "indexOf" => Value::Num(s.find(&arg_str(args, 0)).map(|b| s[..b].chars().count() as f64).unwrap_or(-1.0)),
        "includes" | "contains" => Value::Bool(s.contains(&arg_str(args, 0))),
        "startsWith" => Value::Bool(s.starts_with(&arg_str(args, 0))),
        "endsWith" => Value::Bool(s.ends_with(&arg_str(args, 0))),
        "equals" => Value::Bool(args.first().is_some_and(|v| *v == Value::Str(s.to_string()))),
        "replace" => Value::Str(s.replacen(&arg_str(args, 0), &arg_str(args, 1), 1)),
        "replaceAll" => Value::Str(s.replace(&arg_str(args, 0), &arg_str(args, 1))),
        "concat" => Value::Str(format!("{s}{}", args.iter().map(|a| a.js_string()).collect::<String>())),
        "padStart" | "padEnd" => {
            let n = js_index(args.first(), usize::MAX >> 1, 0);
            let pad = args.get(1).map(|v| v.js_string()).unwrap_or_else(|| " ".into());
            if n <= len || pad.is_empty() {
                Value::Str(s.to_string())
            } else {
                let fill: String = pad.chars().cycle().take(n - len).collect();
                Value::Str(if m == "padStart" { format!("{fill}{s}") } else { format!("{s}{fill}") })
            }
        }
        _ => return Err(format!("unsupported string method {m}()")),
    })
}

pub(crate) fn feel_cmp(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.partial_cmp(y),
        (Value::Str(x), Value::Str(y)) => Some(x.cmp(y)),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

fn loose_eq(a: &Value, b: &Value, mode: Mode) -> bool {
    match (a, b) {
        (Value::Num(_), Value::Str(_)) | (Value::Str(_), Value::Num(_)) if mode != Mode::Feel => a.js_number() == b.js_number(),
        _ => a == b,
    }
}

fn eval(e: &Expr, env: &mut Env) -> Result<Value, String> {
    let mode = env.mode;
    match e {
        Expr::Lit(v) => Ok(v.clone()),
        Expr::Var(n) => match env.locals.get(n).or_else(|| env.vars.get(n)) {
            Some(v) => Ok(v.clone()),
            None => match mode {
                Mode::Feel => Ok(Value::Null),
                Mode::Js => Err(format!("ReferenceError: \"{n}\" is not defined")),
                Mode::Juel => Err(format!("Cannot resolve identifier '{n}'")),
            },
        },
        Expr::Neg(x) => match eval(x, env)? {
            Value::Null if mode == Mode::Feel => Ok(Value::Null),
            v => Ok(Value::Num(-v.js_number())),
        },
        Expr::Not(x) => Ok(Value::Bool(!eval(x, env)?.truthy())),
        Expr::Cond(c, a, b) => {
            let c = eval(c, env)?;
            let yes = if mode == Mode::Feel { c == Value::Bool(true) } else { c.truthy() };
            if yes { eval(a, env) } else { eval(b, env) }
        }
        Expr::Bin(op, l, r) if *op == "&&" || *op == "||" => {
            let a = eval(l, env)?;
            if mode == Mode::Feel {
                let b = eval(r, env)?;
                return Ok(match (*op, &a, &b) {
                    ("&&", Value::Bool(false), _) | ("&&", _, Value::Bool(false)) => Value::Bool(false),
                    ("&&", Value::Bool(true), Value::Bool(true)) => Value::Bool(true),
                    ("||", Value::Bool(true), _) | ("||", _, Value::Bool(true)) => Value::Bool(true),
                    ("||", Value::Bool(false), Value::Bool(false)) => Value::Bool(false),
                    _ => Value::Null,
                });
            }
            let short = if *op == "&&" { !a.truthy() } else { a.truthy() };
            if short {
                return Ok(if mode == Mode::Juel { Value::Bool(a.truthy()) } else { a });
            }
            let b = eval(r, env)?;
            Ok(if mode == Mode::Juel { Value::Bool(b.truthy()) } else { b })
        }
        Expr::Bin(op, l, r) => {
            let a = eval(l, env)?;
            let b = eval(r, env)?;
            binop(op, &a, &b, mode)
        }
        Expr::Call(name, args) => {
            let args = args.iter().map(|a| eval(a, env)).collect::<Result<Vec<_>, _>>()?;
            call(name, &args, mode)
        }
        Expr::Method(recv, m, args) => {
            if mode == Mode::Feel {
                return Err(format!(
                    "FEEL has no method calls: '.{m}(...)' is JavaScript/Java syntax. Set expressionLanguage=\"javascript\" on this cell or use FEEL functions such as substring(x, start, length)"
                ));
            }
            let r = eval(recv, env)?;
            let args = args.iter().map(|a| eval(a, env)).collect::<Result<Vec<_>, _>>()?;
            match r {
                Value::Null => Err(match mode {
                    Mode::Juel => format!("NullPointerException: cannot call {m}() on null"),
                    _ => format!("TypeError: Cannot read property \"{m}\" from null"),
                }),
                Value::Str(s) => string_method(&s, m, &args, mode),
                Value::Num(n) => match m.as_str() {
                    "toString" => Ok(Value::Str(fmt_num(n))),
                    "toFixed" => Ok(Value::Str(format!("{:.*}", js_index(args.first(), 20, 0), n))),
                    _ => Err(format!("unsupported number method {m}()")),
                },
                Value::Bool(b) if m == "toString" => Ok(Value::Str(b.to_string())),
                v => Err(format!("TypeError: {}.{m} is not a function", v.type_name())),
            }
        }
        Expr::Prop(recv, p) => {
            let r = eval(recv, env)?;
            match (&r, p.as_str()) {
                (Value::Null, _) => Err(match mode {
                    Mode::Feel => return Ok(Value::Null),
                    Mode::Juel => format!("PropertyNotFoundException: '{p}' on null"),
                    Mode::Js => format!("TypeError: Cannot read property \"{p}\" from null"),
                }),
                (Value::Str(s), "length") if mode == Mode::Js => Ok(Value::Num(s.chars().count() as f64)),
                _ if mode == Mode::Feel => Ok(Value::Null),
                _ => Err(format!("unknown property {p} on {}", r.type_name())),
            }
        }
    }
}

pub(crate) fn binop(op: &str, a: &Value, b: &Value, mode: Mode) -> Result<Value, String> {
    use std::cmp::Ordering::*;
    if mode == Mode::Feel && (*a == Value::Null || *b == Value::Null) {
        return Ok(match op {
            "==" => Value::Bool(a == b),
            "!=" => Value::Bool(a != b),
            _ => Value::Null,
        });
    }
    match op {
        "+" => match mode {
            Mode::Js => match (a, b) {
                (Value::Str(_), _) | (_, Value::Str(_)) => Ok(Value::Str(a.js_string() + &b.js_string())),
                _ => Ok(Value::Num(a.js_number() + b.js_number())),
            },
            Mode::Feel => match (a, b) {
                (Value::Str(x), Value::Str(y)) => Ok(Value::Str(format!("{x}{y}"))),
                (Value::Num(x), Value::Num(y)) => Ok(Value::Num(x + y)),
                _ => Ok(Value::Null),
            },
            Mode::Juel => {
                let n = |v: &Value| match v {
                    Value::Str(s) => s.trim().parse::<f64>().map_err(|_| format!("Cannot coerce \"{s}\" to a number: in JUEL '+' is numeric addition, not string concatenation")),
                    other => Ok(other.js_number()),
                };
                Ok(Value::Num(n(a)? + n(b)?))
            }
        },
        "-" | "*" | "/" | "%" => {
            if mode == Mode::Feel && !(matches!(a, Value::Num(_)) && matches!(b, Value::Num(_))) {
                return Ok(Value::Null);
            }
            let (x, y) = (a.js_number(), b.js_number());
            Ok(Value::Num(match op {
                "-" => x - y,
                "*" => x * y,
                "/" => x / y,
                _ => x % y,
            }))
        }
        "==" => Ok(Value::Bool(loose_eq(a, b, mode))),
        "!=" => Ok(Value::Bool(!loose_eq(a, b, mode))),
        "<" | "<=" | ">" | ">=" => {
            let ord = match (a, b) {
                (Value::Str(x), Value::Str(y)) => Some(x.cmp(y)),
                _ if mode == Mode::Feel => feel_cmp(a, b),
                _ => a.js_number().partial_cmp(&b.js_number()),
            };
            let Some(o) = ord else { return Ok(if mode == Mode::Feel { Value::Null } else { Value::Bool(false) }) };
            Ok(Value::Bool(match op {
                "<" => o == Less,
                "<=" => o != Greater,
                ">" => o == Greater,
                _ => o != Less,
            }))
        }
        _ => Err(format!("unsupported operator {op}")),
    }
}

fn call(name: &str, a: &[Value], mode: Mode) -> Result<Value, String> {
    let s = |i: usize| match a.get(i) {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    };
    match (mode, name) {
        (Mode::Feel, "substring") => {
            let (Some(text), Some(Value::Num(start))) = (s(0), a.get(1)) else { return Ok(Value::Null) };
            let c = chars(&text);
            let len = c.len() as i64;
            let st = *start as i64;
            let from = if st > 0 { st - 1 } else { (len + st).max(0) };
            let from = from.clamp(0, len) as usize;
            let to = match a.get(2) {
                Some(Value::Num(n)) => (from as i64 + *n as i64).clamp(from as i64, len) as usize,
                _ => len as usize,
            };
            Ok(Value::Str(c[from..to].iter().collect()))
        }
        (Mode::Feel, "string_length") => Ok(s(0).map(|x| Value::Num(x.chars().count() as f64)).unwrap_or(Value::Null)),
        (Mode::Feel, "upper_case") => Ok(s(0).map(|x| Value::Str(x.to_uppercase())).unwrap_or(Value::Null)),
        (Mode::Feel, "lower_case") => Ok(s(0).map(|x| Value::Str(x.to_lowercase())).unwrap_or(Value::Null)),
        (Mode::Feel, "contains") => Ok(match (s(0), s(1)) {
            (Some(x), Some(y)) => Value::Bool(x.contains(&y)),
            _ => Value::Null,
        }),
        (Mode::Feel, "starts_with") => Ok(match (s(0), s(1)) {
            (Some(x), Some(y)) => Value::Bool(x.starts_with(&y)),
            _ => Value::Null,
        }),
        (Mode::Feel, "ends_with") => Ok(match (s(0), s(1)) {
            (Some(x), Some(y)) => Value::Bool(x.ends_with(&y)),
            _ => Value::Null,
        }),
        (Mode::Feel, "substring_before") => Ok(match (s(0), s(1)) {
            (Some(x), Some(y)) => Value::Str(x.find(&y).map(|i| x[..i].to_string()).unwrap_or_default()),
            _ => Value::Null,
        }),
        (Mode::Feel, "substring_after") => Ok(match (s(0), s(1)) {
            (Some(x), Some(y)) => Value::Str(x.find(&y).map(|i| x[i + y.len()..].to_string()).unwrap_or_default()),
            _ => Value::Null,
        }),
        (Mode::Feel, "string") => Ok(match a.first() {
            None | Some(Value::Null) => Value::Null,
            Some(v) => Value::Str(v.js_string()),
        }),
        (Mode::Feel, "number") => Ok(s(0).and_then(|x| x.trim().parse().ok()).map(Value::Num).unwrap_or(Value::Null)),
        (Mode::Feel, "not") => Ok(match a.first() {
            Some(Value::Bool(b)) => Value::Bool(!b),
            _ => Value::Null,
        }),
        (Mode::Js, "String") => Ok(Value::Str(a.first().map(|v| v.js_string()).unwrap_or_default())),
        (Mode::Js, "Number") => Ok(Value::Num(a.first().map(|v| v.js_number()).unwrap_or(0.0))),
        (Mode::Js, "parseInt") => Ok(Value::Num(a.first().map(|v| v.js_number().trunc()).unwrap_or(f64::NAN))),
        (Mode::Js, "parseFloat") => Ok(Value::Num(a.first().map(|v| v.js_number()).unwrap_or(f64::NAN))),
        _ => Err(format!("unknown function {name}()")),
    }
}

// Plain literals evaluate the same in every mode; skipping lex/parse for them covers most table cells.
pub(crate) fn literal(src: &str, mode: Mode) -> Option<Value> {
    let s = src.trim();
    match s {
        "true" => return Some(Value::Bool(true)),
        "false" => return Some(Value::Bool(false)),
        "null" => return Some(Value::Null),
        _ => {}
    }
    if let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
        let rewritten = mode == Mode::Feel && FEEL_FUNCS.iter().any(|(a, _)| inner.contains(a));
        return (!rewritten && !inner.contains(['"', '\\'])).then(|| Value::Str(inner.to_string()));
    }
    let digits = s.strip_prefix('-').unwrap_or(s);
    let all_digits = |x: &str| x.bytes().all(|b| b.is_ascii_digit());
    let valid = match digits.split_once('.') {
        Some((a, b)) => all_digits(a) && !b.is_empty() && all_digits(b),
        None => !digits.is_empty() && all_digits(digits),
    };
    let n: f64 = digits.parse().ok().filter(|_| valid)?;
    Some(Value::Num(if digits.len() < s.len() { -n } else { n }))
}

pub fn run(src: &str, mode: Mode, vars: &Context, locals: BTreeMap<String, Value>) -> Result<Value, String> {
    match literal(src, mode) {
        Some(v) => Ok(v),
        None => run_program(src, mode, vars, locals),
    }
}

fn run_program(src: &str, mode: Mode, vars: &Context, locals: BTreeMap<String, Value>) -> Result<Value, String> {
    Program::compile(src, mode)?.exec(vars, locals)
}

#[derive(Clone, Debug)]
pub(crate) struct Program {
    stmts: Vec<Stmt>,
    mode: Mode,
}

impl Program {
    pub(crate) fn compile(src: &str, mode: Mode) -> Result<Program, String> {
        let mut p = Parser { t: lex(src, mode)?, i: 0, mode };
        Ok(Program { stmts: p.program()?, mode })
    }

    pub(crate) fn exec(&self, vars: &Context, locals: BTreeMap<String, Value>) -> Result<Value, String> {
        let mut env = Env { vars, locals, mode: self.mode };
        let mut last = Value::Null;
        for s in &self.stmts {
            match s {
                Stmt::Let(n, e) => {
                    let v = eval(e, &mut env)?;
                    env.locals.insert(n.clone(), v);
                }
                Stmt::Expr(e) => last = eval(e, &mut env)?,
            }
        }
        Ok(last)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuleStatus {
    Matched,
    Failed(usize),
    Error(String),
}

#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub inputs: Vec<Value>,
    pub status: Vec<RuleStatus>,
    pub matched: Vec<usize>,
    pub fired: Vec<usize>,
    pub rows: Vec<Vec<Value>>,
    pub aggregate: Option<Value>,
    pub error: Option<String>,
}

impl Outcome {
    pub fn output(&self, idx: usize) -> Value {
        if let Some(a) = &self.aggregate {
            return a.clone();
        }
        self.rows.first().and_then(|r| r.get(idx)).cloned().unwrap_or(Value::Null)
    }
}

fn eval_input(inp: &InputClause, vars: &Context) -> Result<Value, String> {
    let e = inp.expression.trim();
    if e.is_empty() {
        return Ok(Value::Null);
    }
    if let Some(v) = vars.get(e) {
        return Ok(v.clone());
    }
    let mode = mode_of(&inp.lang)?;
    let v = run(e, mode, vars, BTreeMap::new())?;
    Ok(v.coerce(inp.type_ref.as_deref()).unwrap_or(Value::Null))
}

fn priority_key(t: &DecisionTable, row: &[Value]) -> Vec<usize> {
    t.outputs
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let list: Vec<String> = o.output_values.as_deref().map(split_top).unwrap_or_default();
            let shown = row.get(i).map(|v| v.display()).unwrap_or_default();
            list.iter().position(|x| x.trim() == shown).unwrap_or(usize::MAX)
        })
        .collect()
}

static NULL: Value = Value::Null;

pub fn evaluate(t: &DecisionTable, vars: &Context) -> Outcome {
    let mut o = Outcome::default();
    for inp in &t.inputs {
        match eval_input(inp, vars) {
            Ok(v) => o.inputs.push(v),
            Err(e) => {
                o.error.get_or_insert(format!("Input \"{}\": {e}", inp.title()));
                o.inputs.push(Value::Null);
            }
        }
    }
    if o.error.is_some() {
        return o;
    }
    o.status.reserve_exact(t.rules.len());
    for (ri, r) in t.rules.iter().enumerate() {
        let mut st = RuleStatus::Matched;
        for (ci, e) in r.inputs.iter().enumerate() {
            let input = o.inputs.get(ci).unwrap_or(&NULL);
            match e.tests.matches(input, vars) {
                Ok(true) => {}
                Ok(false) => {
                    st = RuleStatus::Failed(ci);
                    break;
                }
                Err(err) => {
                    let col = t.inputs.get(ci).map(|i| i.title()).unwrap_or("?");
                    o.error.get_or_insert(format!("Rule {} input \"{col}\" `{}`: {err}", ri + 1, e.text.trim()));
                    st = RuleStatus::Error(err);
                    break;
                }
            }
        }
        if st == RuleStatus::Matched {
            o.matched.push(ri);
        }
        o.status.push(st);
    }
    if o.error.is_some() {
        return o;
    }

    let hp = t.hit_policy.to_ascii_uppercase();
    let to_eval: Vec<usize> = match hp.as_str() {
        "FIRST" => o.matched.iter().take(1).copied().collect(),
        "UNIQUE" if o.matched.len() > 1 => {
            o.error = Some(format!("Hit policy UNIQUE violated: rules {} all matched", crate::lint::rules_label(&o.matched)));
            return o;
        }
        _ => o.matched.clone(),
    };

    let mut rows = Vec::new();
    for &ri in &to_eval {
        let r = &t.rules[ri];
        let mut row = Vec::new();
        for (oi, out) in t.outputs.iter().enumerate() {
            let Some(e) = r.outputs.get(oi) else {
                row.push(Value::Null);
                continue;
            };
            if e.is_any() {
                row.push(Value::Null);
                continue;
            }
            let res = mode_of(&e.lang).and_then(|m| run(&e.text, m, vars, BTreeMap::new())).and_then(|v| v.coerce(out.type_ref.as_deref()));
            match res {
                Ok(v) => row.push(v),
                Err(err) => {
                    o.error = Some(format!("Rule {} output \"{}\": {err}", ri + 1, out.title()));
                    return o;
                }
            }
        }
        rows.push((ri, row));
    }

    match hp.as_str() {
        "ANY" if rows.windows(2).any(|w| w[0].1 != w[1].1) => {
            o.error = Some(format!("Hit policy ANY violated: rules {} matched with different outputs", crate::lint::rules_label(&o.matched)));
            return o;
        }
        "ANY" => rows.truncate(1),
        "PRIORITY" => {
            rows.sort_by_cached_key(|(_, r)| priority_key(t, r));
            rows.truncate(1);
        }
        "OUTPUT ORDER" => rows.sort_by_cached_key(|(_, r)| priority_key(t, r)),
        _ => {}
    }
    o.fired = rows.iter().map(|(ri, _)| *ri).collect();
    o.rows = rows.into_iter().map(|(_, r)| r).collect();

    if hp == "COLLECT"
        && let Some(agg) = t.aggregation.as_deref().map(|a| a.to_ascii_uppercase())
    {
        let nums: Vec<f64> = o.rows.iter().filter_map(|r| r.first()).filter(|v| **v != Value::Null).map(|v| v.js_number()).collect();
        o.aggregate = Some(match agg.as_str() {
            "COUNT" => Value::Num(nums.len() as f64),
            "SUM" => Value::Num(nums.iter().sum()),
            "MIN" => nums.iter().copied().reduce(f64::min).map(Value::Num).unwrap_or(Value::Null),
            "MAX" => nums.iter().copied().reduce(f64::max).map(Value::Num).unwrap_or(Value::Null),
            _ => Value::Null,
        });
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_fast_path_matches_full_evaluation() {
        let srcs = [
            "true",
            "false",
            "null",
            " 42 ",
            "-7",
            "-0",
            "3.25",
            ".5",
            "007",
            "\"EU\"",
            "\"\"",
            "\"a b\"",
            "\"upper case\"",
            "\"a\\\"b\"",
            "5.",
            "1.2.3",
            "1e5",
            "+5",
            "- 5",
            "-",
            "--5",
            "\"x",
            "x",
            "'q'",
            "5 // c",
        ];
        for mode in [Mode::Feel, Mode::Js, Mode::Juel] {
            for src in srcs {
                if let Some(v) = literal(src, mode) {
                    assert_eq!(Ok(v), run_program(src, mode, &Context::new(), BTreeMap::new()), "{src:?} in {mode:?}");
                }
            }
        }
        assert_eq!(literal("\"upper case\"", Mode::Feel), None);
        assert_eq!(literal("-5", Mode::Js), Some(Value::Num(-5.0)));
    }
}
