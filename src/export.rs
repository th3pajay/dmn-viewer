
use crate::model::DecisionTable;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Csv,
    Markdown,
    Json,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Csv, Format::Markdown, Format::Json];

    pub fn label(self) -> &'static str {
        match self {
            Format::Csv => "CSV",
            Format::Markdown => "Markdown",
            Format::Json => "JSON",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Csv => "csv",
            Format::Markdown => "md",
            Format::Json => "json",
        }
    }
}

pub fn render(table: &DecisionTable, format: Format) -> String {
    match format {
        Format::Csv => csv(table),
        Format::Markdown => markdown(table),
        Format::Json => json_text(table),
    }
}

fn header(table: &DecisionTable) -> Vec<String> {
    let mut h = vec!["#".to_string()];
    h.extend(table.inputs.iter().map(|i| i.title().to_string()));
    h.extend(table.outputs.iter().map(|o| o.title().to_string()));
    h.push("Note".into());
    h
}

fn rows(table: &DecisionTable) -> Vec<Vec<String>> {
    table
        .rules
        .iter()
        .enumerate()
        .map(|(n, r)| {
            let mut row = vec![(n + 1).to_string()];
            row.extend(r.inputs.iter().map(|e| e.text.trim().to_string()));
            row.extend(r.outputs.iter().map(|e| e.text.trim().to_string()));
            row.push(table.note(r).to_string());
            row
        })
        .collect()
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

fn csv(table: &DecisionTable) -> String {
    let line = |cells: &[String]| cells.iter().map(|c| csv_field(c)).collect::<Vec<_>>().join(",");
    let mut out = line(&header(table));
    for r in rows(table) {
        out.push_str("\r\n");
        out.push_str(&line(&r));
    }
    out.push_str("\r\n");
    out
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn markdown(table: &DecisionTable) -> String {
    let line = |cells: &[String]| format!("| {} |\n", cells.iter().map(|c| md_cell(c)).collect::<Vec<_>>().join(" | "));
    let h = header(table);
    let mut out = line(&h);
    out.push_str(&format!("|{}|\n", vec![" --- "; h.len()].join("|")));
    for r in rows(table) {
        out.push_str(&line(&r));
    }
    out
}

fn json_text(table: &DecisionTable) -> String {
    let rules: Vec<Value> = table
        .rules
        .iter()
        .map(|r| {
            let inputs: Vec<Value> = table.inputs.iter().zip(&r.inputs).map(|(c, e)| json!({ "input": c.title(), "when": e.text.trim() })).collect();
            let outputs: Vec<Value> = table.outputs.iter().zip(&r.outputs).map(|(c, e)| json!({ "output": c.title(), "then": e.text.trim() })).collect();
            json!({ "id": r.id, "inputs": inputs, "outputs": outputs, "note": table.note(r) })
        })
        .collect();
    let doc = json!({ "hitPolicy": table.hit_policy, "aggregation": table.aggregation, "rules": rules });
    serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Entry, InputClause, InputEntry, OutputClause, Rule};

    fn table() -> DecisionTable {
        DecisionTable {
            hit_policy: "UNIQUE".into(),
            inputs: vec![InputClause { label: "Age".into(), ..Default::default() }],
            outputs: vec![OutputClause { name: "tier".into(), ..Default::default() }],
            rules: vec![Rule {
                id: "r1".into(),
                description: "say \"hi\", ok".into(),
                inputs: vec![InputEntry { text: ">= 18".into(), ..Default::default() }],
                outputs: vec![Entry { text: "\"adult\"".into(), ..Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn csv_quotes_fields_that_need_it() {
        let out = render(&table(), Format::Csv);
        assert_eq!(out, "#,Age,tier,Note\r\n1,>= 18,\"\"\"adult\"\"\",\"say \"\"hi\"\", ok\"\r\n");
    }

    #[test]
    fn markdown_escapes_pipes() {
        let mut t = table();
        t.rules[0].inputs[0].text = "a|b".into();
        let out = render(&t, Format::Markdown);
        assert!(out.contains("a\\|b"));
        assert!(out.starts_with("| # | Age | tier | Note |\n| --- | --- | --- | --- |\n"));
    }

    #[test]
    fn json_round_trips() {
        let v: Value = serde_json::from_str(&render(&table(), Format::Json)).unwrap();
        assert_eq!(v["hitPolicy"], "UNIQUE");
        assert_eq!(v["rules"][0]["inputs"][0]["when"], ">= 18");
    }
}
