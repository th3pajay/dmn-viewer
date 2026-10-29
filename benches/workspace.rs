use criterion::{Criterion, criterion_group, criterion_main};
use dmn_viewer::edit::{Column, Edit};
use dmn_viewer::suite;
use dmn_viewer::workspace::Workspace;
use dmn_viewer::{lint, parse};
use std::fmt::Write;
use std::hint::black_box;
use std::path::PathBuf;

const RULES: usize = 5000;
const CASES: usize = 300;
const REGIONS: [&str; 4] = ["EU", "NA", "APAC", "LATAM"];

fn dmn(rules: usize) -> String {
    dmn_with(rules, false)
}

fn dmn_with(rules: usize, catch_all: bool) -> String {
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
        let any = catch_all && i == 0;
        let region = if any { String::new() } else { format!("\"{}\"", REGIONS[i % REGIONS.len()]) };
        let weight = if any { String::new() } else { format!("[{i}..{}]", i + 1) };
        let express = if any || !i.is_multiple_of(2) { "" } else { "false" };
        write!(
            x,
            r#"      <rule id="R{i}">
        <description>Rule number {i}</description>
        <inputEntry id="R{i}a"><text>{region}</text></inputEntry>
        <inputEntry id="R{i}b"><text>{weight}</text></inputEntry>
        <inputEntry id="R{i}c"><text>{express}</text></inputEntry>
        <inputEntry id="R{i}d"><text></text></inputEntry>
        <outputEntry id="R{i}e"><text>{fee}.5</text></outputEntry>
        <outputEntry id="R{i}f"><text>"CARRIER-{i}"</text></outputEntry>
        <outputEntry id="R{i}g"><text>"L-" + substring(OrderDate, 3, 6)</text></outputEntry>
      </rule>
"#,
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
    g.bench_function("open", |b| b.iter(|| black_box(Workspace::open(path.clone()).unwrap())));

    let mut ws = Workspace::open(path.clone()).unwrap();
    let mut n = 0u64;
    g.bench_function("commit_cell_middle", |b| {
        b.iter(|| {
            n += 1;
            black_box(ws.commit_group(&[cell(RULES / 2, n.to_string())], None))
        })
    });

    let mut ws = Workspace::open(path.clone()).unwrap();
    let mut flip = false;
    g.bench_function("commit_cell_first_shifting_lines", |b| {
        b.iter(|| {
            flip = !flip;
            let text = if flip { "1\n2" } else { "1" };
            black_box(ws.commit_group(&[cell(0, text.to_string())], None))
        })
    });

    let mut ws = Workspace::open(path.clone()).unwrap();
    g.bench_function("insert_rule_then_undo", |b| {
        b.iter(|| {
            ws.commit_group(&[Edit::InsertRule { decision: 0, at: RULES / 2, copy_of: None }], None);
            ws.undo();
        })
    });

    let mut ws = Workspace::open(path).unwrap();
    for i in 0..200 {
        ws.commit_group(&[cell(i * 7, format!("{i}"))], None);
    }
    g.bench_function("undo_redo_cell", |b| {
        b.iter(|| {
            ws.undo();
            ws.redo();
        })
    });
    g.finish();
}

fn session(c: &mut Criterion) {
    let xml = dmn(RULES);
    let path = scratch("sess.dmn", &xml);
    scratch("sess.tests.json", &suite(CASES));
    let mut ws = Workspace::open(path).unwrap();
    *ws.tests().fields_mut() = vec!["EU".into(), "100.5".into(), "false".into(), "20260314".into()];
    let mut g = c.benchmark_group("session");
    g.sample_size(30);
    ws.tests().results();
    ws.tests().outcome();

    g.bench_function("frame_cache_hit", |b| {
        b.iter(|| {
            black_box(ws.tests().outcome());
            black_box(ws.tests().results());
            black_box(ws.tests().coverage());
            black_box(ws.tests().coverage());
            black_box(ws.tests().results());
        })
    });

    let mut n = 0u64;
    g.bench_function("edit_then_frame", |b| {
        b.iter(|| {
            n += 1;
            ws.commit_group(&[cell(RULES / 2, n.to_string())], None);
            black_box(ws.tests().outcome());
            black_box(ws.tests().results());
            black_box(ws.tests().coverage());
        })
    });
    g.finish();
}

fn fixing(c: &mut Criterion) {
    let path = scratch("fix.dmn", &dmn_with(3000, true));
    let mut g = c.benchmark_group("fixing");
    g.sample_size(20);
    let mut ws = Workspace::open(path).unwrap();
    let mut n = 0u64;
    g.bench_function("edit", |b| {
        b.iter(|| {
            n += 1;
            ws.commit_group(&[cell(1500, n.to_string())], None)
        })
    });
    g.bench_function("edit_then_fixable_count", |b| {
        b.iter(|| {
            n += 1;
            ws.commit_group(&[cell(1500, n.to_string())], None);
            black_box(ws.fixable_count())
        })
    });
    g.bench_function("start_and_close", |b| {
        b.iter(|| {
            ws.fix_start(None);
            ws.fix_close();
        })
    });
    ws.fix_start(None);
    g.bench_function("reset_skipped_settle", |b| b.iter(|| ws.fix_reset_skipped()));
    g.bench_function("settle_then_preview", |b| {
        b.iter(|| {
            ws.fix_reset_skipped();
            ws.fix_move(true);
            black_box(ws.fix_view().is_some_and(|v| v.preview.is_some()));
        })
    });
    g.finish();
}

fn suite_run(c: &mut Criterion) {
    let big = parse::parse(&dmn(RULES)).unwrap();
    let small = parse::parse(&dmn(50)).unwrap();
    let big_suite = suite::load(&scratch("run.tests.json", &suite(CASES))).unwrap();
    let small_suite = suite::load(&scratch("run-small.tests.json", &suite(9))).unwrap();
    let mut g = c.benchmark_group("suite_run");
    g.sample_size(30);
    for (name, defs, s) in [("300_cases_5000_rules", &big, &big_suite), ("9_cases_50_rules", &small, &small_suite)] {
        let t = defs.table(0).unwrap();
        g.bench_function(name, |b| b.iter(|| black_box(s.run(t))));
    }
    g.finish();
}

criterion_group!(benches, document, session, fixing, suite_run);
criterion_main!(benches);
