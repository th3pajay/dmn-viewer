use criterion::{Criterion, criterion_group, criterion_main};
use dmn_viewer::edit::{Column, Edit};
use dmn_viewer::session::TestSession;
use dmn_viewer::workspace::document::Document;
use dmn_viewer::{lint, parse};
use std::fmt::Write;
use std::hint::black_box;
use std::path::PathBuf;

const RULES: usize = 5000;
const CASES: usize = 300;
const REGIONS: [&str; 4] = ["EU", "NA", "APAC", "LATAM"];

fn dmn(rules: usize) -> String {
    let mut x = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="big" name="Big" namespace="http://camunda.org/schema/1.0/dmn">
  <decision id="Fee" name="Fee">
    <decisionTable id="FeeTable" hitPolicy="FIRST">
      <input id="InRegion" label="Region"><inputExpression id="InRegionE" typeRef="string"><text>Region</text></inputExpression><inputValues id="InRegionV"><text>"EU","NA","APAC","LATAM"</text></inputValues></input>
      <input id="InWeight" label="Weight"><inputExpression id="InWeightE" typeRef="double"><text>Weight</text></inputExpression></input>
      <input id="InExpress" label="Express"><inputExpression id="InExpressE" typeRef="boolean"><text>Express</text></inputExpression></input>
      <input id="InDate" label="OrderDate"><inputExpression id="InDateE" typeRef="string"><text>OrderDate</text></inputExpression></input>
      <output id="OutFee" label="Fee" name="Fee" typeRef="double" />
      <output id="OutCarrier" label="Carrier" name="Carrier" typeRef="string" />
      <output id="OutLabel" label="Label" name="Label" typeRef="string" />
"#,
    );
    for i in 0..rules {
        let region = REGIONS[i % REGIONS.len()];
        let express = if i.is_multiple_of(2) { "false" } else { "true" };
        write!(
            x,
            r#"      <rule id="R{i}">
        <description>Rule number {i}</description>
        <inputEntry id="R{i}a"><text>"{region}"</text></inputEntry>
        <inputEntry id="R{i}b"><text>[{i}..{n}]</text></inputEntry>
        <inputEntry id="R{i}c"><text>{express}</text></inputEntry>
        <inputEntry id="R{i}d"><text></text></inputEntry>
        <outputEntry id="R{i}e"><text>{fee}.5</text></outputEntry>
        <outputEntry id="R{i}f"><text>"CARRIER-{i}"</text></outputEntry>
        <outputEntry id="R{i}g"><text>"L-" + substring(OrderDate, 3, 6)</text></outputEntry>
      </rule>
"#,
            n = i + 1,
            fee = i % 97,
        )
        .unwrap();
    }
    x.push_str("    </decisionTable>\n  </decision>\n</definitions>\n");
    x
}

fn suite(cases: usize) -> String {
    let mut s = String::from("{\"decision\":\"Fee\",\"cases\":[");
    for c in 0..cases {
        let i = c * (RULES / cases);
        if c > 0 {
            s.push(',');
        }
        write!(
            s,
            r#"{{"name":"case {c}","inputs":{{"Region":"{region}","Weight":{w},"Express":{express},"OrderDate":"20260314"}},"expected":{{"Carrier":"CARRIER-{i}"}}}}"#,
            region = REGIONS[i % REGIONS.len()],
            w = i as f64 + 0.5,
            express = !i.is_multiple_of(2),
        )
        .unwrap();
    }
    s.push_str("]}");
    s
}

fn scratch(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dmn-viewer-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

fn cell(rule: usize, text: String) -> Edit {
    Edit::Cell { decision: 0, rule, col: Column::Output(0), text }
}

fn document(c: &mut Criterion) {
    let xml = dmn(RULES);
    let path = scratch("big.dmn", &xml);
    let mut g = c.benchmark_group("document");
    g.sample_size(30);
    g.bench_function("parse", |b| b.iter(|| black_box(parse::parse(black_box(&xml)).unwrap())));
    let defs = parse::parse(&xml).unwrap();
    g.bench_function("lint", |b| b.iter(|| black_box(lint::lint(black_box(&defs)))));
    g.bench_function("open", |b| b.iter(|| black_box(Document::open(path.clone()).unwrap())));

    let mut doc = Document::open(path.clone()).unwrap();
    let mut n = 0u64;
    g.bench_function("commit_cell_middle", |b| {
        b.iter(|| {
            n += 1;
            black_box(doc.commit(&[cell(RULES / 2, n.to_string())]).unwrap())
        })
    });

    let mut doc = Document::open(path.clone()).unwrap();
    let mut flip = false;
    g.bench_function("commit_cell_first_shifting_lines", |b| {
        b.iter(|| {
            flip = !flip;
            let text = if flip { "1\n2" } else { "1" };
            black_box(doc.commit(&[cell(0, text.to_string())]).unwrap())
        })
    });

    let mut doc = Document::open(path.clone()).unwrap();
    g.bench_function("insert_rule_then_undo", |b| {
        b.iter(|| {
            doc.commit(&[Edit::InsertRule { decision: 0, at: RULES / 2, copy_of: None }]).unwrap();
            black_box(doc.undo().unwrap())
        })
    });

    let mut doc = Document::open(path).unwrap();
    for i in 0..200 {
        doc.commit(&[cell(i * 7, format!("{i}"))]).unwrap();
    }
    g.bench_function("undo_redo_cell", |b| {
        b.iter(|| {
            doc.undo().unwrap();
            black_box(doc.redo().unwrap())
        })
    });
    g.finish();
}

fn session(c: &mut Criterion) {
    let xml = dmn(RULES);
    let path = scratch("sess.dmn", &xml);
    let suite_path = scratch("sess.tests.json", &suite(CASES));
    let doc = Document::open(path).unwrap();
    let mut s = TestSession::default();
    s.load(&suite_path).unwrap();
    s.fields = vec!["EU".into(), "100.5".into(), "false".into(), "20260314".into()];
    let mut g = c.benchmark_group("session");
    g.sample_size(30);
    let rev = doc.revision();
    s.sync(rev);
    s.results(&doc.defs, 0);
    s.outcome(&doc.defs, 0);

    g.bench_function("frame_cache_hit", |b| {
        b.iter(|| {
            s.sync(rev);
            black_box(s.outcome(&doc.defs, 0));
            black_box(s.results(&doc.defs, 0));
            black_box(s.coverage(&doc.defs, 0));
            black_box(s.coverage(&doc.defs, 0));
            black_box(s.results(&doc.defs, 0));
        })
    });

    let mut r = rev;
    g.bench_function("frame_after_revision_change", |b| {
        b.iter(|| {
            r += 1;
            s.sync(r);
            black_box(s.outcome(&doc.defs, 0));
            black_box(s.results(&doc.defs, 0));
            black_box(s.coverage(&doc.defs, 0));
        })
    });
    g.finish();
}

criterion_group!(benches, document, session);
criterion_main!(benches);
