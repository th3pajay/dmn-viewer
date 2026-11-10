#![allow(dead_code)]

use dmn_viewer::model::{DecisionTable, Definitions};
use std::fmt::Write;

pub struct Out<'a> {
    pub name: &'a str,
    pub type_ref: &'a str,
    pub values: Option<&'a str>,
}

pub const LEVEL: Out = Out { name: "level", type_ref: "string", values: Some(r#""high","mid","low""#) };
pub const AMOUNT: Out = Out { name: "amount", type_ref: "double", values: None };

pub fn dmn(table_attrs: &str, outputs: &[Out], rules: &[(&str, &[&str])]) -> String {
    let mut x = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="d" name="d" namespace="t"><decision id="dec" name="Dec"><decisionTable id="tbl" "#,
    );
    x.push_str(table_attrs);
    x.push_str(r#"><input id="in_x" label="x"><inputExpression id="ie_x" typeRef="double"><text>x</text></inputExpression></input>"#);
    for (i, o) in outputs.iter().enumerate() {
        let _ = write!(x, r#"<output id="out{i}" name="{}" typeRef="{}">"#, o.name, o.type_ref);
        if let Some(v) = o.values {
            let _ = write!(x, "<outputValues><text>{}</text></outputValues>", esc(v));
        }
        x.push_str("</output>");
    }
    for (ri, (test, outs)) in rules.iter().enumerate() {
        let _ = write!(x, r#"<rule id="r{ri}"><inputEntry id="r{ri}i"><text>{}</text></inputEntry>"#, esc(test));
        for (oi, o) in outs.iter().enumerate() {
            let _ = write!(x, r#"<outputEntry id="r{ri}o{oi}"><text>{}</text></outputEntry>"#, esc(o));
        }
        x.push_str("</rule>");
    }
    x.push_str("</decisionTable></decision></definitions>");
    x
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn defs(xml: &str) -> Definitions {
    dmn_viewer::parse::parse(xml).unwrap()
}

pub fn table(xml: &str) -> DecisionTable {
    defs(xml).table(0).unwrap().clone()
}

pub const LEVELS: [(&str, &[&str]); 3] = [(">= 0", &[r#""low""#]), (">= 5", &[r#""mid""#]), (">= 8", &[r#""high""#])];
pub const AMOUNTS: [(&str, &[&str]); 3] = [(">= 0", &["1"]), (">= 5", &["2"]), (">= 8", &["3"])];

pub fn workspace(name: &str, xml: &str) -> dmn_viewer::workspace::Workspace {
    let dir = std::env::temp_dir().join(format!("dmn-viewer-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("table.dmn");
    std::fs::write(&path, xml).unwrap();
    let _ = std::fs::remove_file(dir.join("table.tests.json"));
    dmn_viewer::workspace::Workspace::open(path).unwrap()
}
