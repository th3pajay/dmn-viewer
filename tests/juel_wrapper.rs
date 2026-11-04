use dmn_viewer::eval::{Context, RuleStatus, Value, evaluate};
use dmn_viewer::parse;

const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="defs" name="defs" namespace="http://camunda.org/schema/1.0/dmn">
  <decision id="d" name="d">
    <decisionTable id="t" hitPolicy="FIRST">
      <input id="i1" label="stamp">
        <inputExpression id="ie1" typeRef="string" expressionLanguage="juel"><text>${Stamp}</text></inputExpression>
      </input>
      <output id="o1" name="result" typeRef="string" />
      <rule id="r1">
        <inputEntry id="r1i" expressionLanguage="juel"><text>${cellInput.substring(8,12).compareTo("1200") &gt; 0}</text></inputEntry>
        <outputEntry id="r1o" expressionLanguage="javascript"><text>"late-" + Stamp.substring(2,8)</text></outputEntry>
      </rule>
      <rule id="r2">
        <inputEntry id="r2i" expressionLanguage="juel"><text>${cellInput.substring(8,12).compareTo("1200") &lt;= 0}</text></inputEntry>
        <outputEntry id="r2o" expressionLanguage="javascript"><text>"early-" + Stamp.substring(2,8)</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#;

fn run(value: &str) -> dmn_viewer::eval::Outcome {
    let defs = parse::parse(XML).unwrap();
    let t = defs.table(0).unwrap();
    let mut ctx = Context::new();
    ctx.insert("Stamp".into(), Value::Str(value.into()));
    evaluate(t, &ctx)
}

#[test]
fn wrapped_juel_cells_evaluate() {
    let o = run("202612011113");
    assert!(o.error.is_none(), "{:?}", o.error);
    assert_eq!(o.status, vec![RuleStatus::Failed(0), RuleStatus::Matched]);
    assert_eq!(o.output(0), Value::Str("early-261201".into()));
}

#[test]
fn wrapped_juel_cells_pick_the_other_rule() {
    let o = run("202612011300");
    assert!(o.error.is_none(), "{:?}", o.error);
    assert_eq!(o.fired, vec![0]);
}

const DATE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="defs" name="defs" namespace="http://camunda.org/schema/1.0/dmn">
  <decision id="d" name="d">
    <decisionTable id="t" hitPolicy="FIRST">
      <input id="i1" label="stamp"><inputExpression id="ie1" typeRef="string"><text>Stamp</text></inputExpression></input>
      <output id="o1" name="result" typeRef="string" />
      <rule id="r1">
        <inputEntry id="r1i"><text></text></inputEntry>
        <outputEntry id="r1o" expressionLanguage="javascript"><text>var s = Stamp; var d = new Date(Date.UTC(+s.substring(0,4), +s.substring(4,6)-1, +s.substring(6,8)+1)); "X-" + d.toISOString().substring(2,10).replace(/-/g,"")</text></outputEntry>
      </rule>
    </decisionTable>
  </decision>
</definitions>"#;

#[test]
fn javascript_dates_and_literal_regex_replace() {
    let defs = parse::parse(DATE_XML).unwrap();
    let t = defs.table(0).unwrap();
    for (stamp, want) in [("202612121411", "X-261213"), ("202612311411", "X-270101"), ("202402281411", "X-240229"), ("202402291411", "X-240301")] {
        let mut ctx = Context::new();
        ctx.insert("Stamp".into(), Value::Str(stamp.into()));
        let o = evaluate(t, &ctx);
        assert!(o.error.is_none(), "{:?}", o.error);
        assert_eq!(o.output(0), Value::Str(want.into()), "{stamp}");
    }
}
