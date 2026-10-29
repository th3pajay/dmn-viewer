use dmn_viewer::edit::{Column, Edit};
use dmn_viewer::workspace::{Cursor, READ_ONLY, RuleOp, View, Workspace};
use std::path::PathBuf;

const RULE: &str = r#"<rule id="R{n}"><inputEntry id="I{n}"><text>{in}</text></inputEntry><outputEntry id="O{n}"><text>{out}</text></outputEntry></rule>"#;

fn scratch(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dmn-viewer-workspace-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn table_xml(rules: &[(&str, &str)]) -> String {
    let body: String = rules.iter().enumerate().map(|(n, (i, o))| RULE.replace("{n}", &n.to_string()).replace("{in}", i).replace("{out}", o)).collect();
    format!(
        r#"<definitions xmlns="https://www.omg.org/spec/DMN/20191111/MODEL/" id="D" name="D" namespace="x"><decision id="Dec" name="Dec"><decisionTable id="T" hitPolicy="FIRST"><input id="In"><inputExpression id="IE" typeRef="string"><text>Region</text></inputExpression><inputValues id="IV"><text>"EU","NA"</text></inputValues></input><output id="Out" name="Carrier" typeRef="string"/>{body}</decisionTable></decision><decision id="Lit" name="Lit"><literalExpression><text>1</text></literalExpression></decision></definitions>"#
    )
}

fn open(name: &str, rules: &[(&str, &str)]) -> Workspace {
    Workspace::open(scratch(name, table_xml(rules).as_bytes())).unwrap()
}

fn clean(name: &str) -> Workspace {
    open(name, &[(r#""EU""#, r#""A""#), (r#""NA""#, r#""B""#), ("-", r#""C""#)])
}

fn messy(name: &str) -> Workspace {
    open(name, &[(r#""LATAM""#, r#""A""#), ("-", r#""B""#), (r#""EU""#, r#""C""#)])
}

fn rules(ws: &Workspace) -> usize {
    ws.defs().table(0).unwrap().rules.len()
}

fn output(ws: &Workspace, rule: usize) -> String {
    ws.defs().table(0).unwrap().rules[rule].outputs[0].text.clone()
}

fn cell(rule: usize, text: &str) -> Edit {
    Edit::Cell { decision: 0, rule, col: Column::Output(0), text: text.into() }
}

#[test]
fn inserting_below_the_selection_selects_and_scrolls_to_the_new_rule() {
    let mut ws = clean("insert.dmn");
    ws.toggle_select(1);
    ws.rule_op(RuleOp::InsertBelow);
    assert_eq!((rules(&ws), ws.selected(), ws.take_scroll()), (4, Some(2), Some(2)));
    assert_eq!(ws.take_scroll(), None, "the scroll request is consumed once");
}

#[test]
fn inserting_without_a_selection_appends() {
    let mut ws = clean("append.dmn");
    ws.rule_op(RuleOp::InsertBelow);
    assert_eq!((rules(&ws), ws.selected()), (4, Some(3)));
}

#[test]
fn rule_operations_that_do_not_apply_do_nothing() {
    let mut ws = clean("noop.dmn");
    let before = ws.xml().to_string();
    ws.rule_op(RuleOp::InsertAbove);
    ws.rule_op(RuleOp::Delete);
    ws.toggle_select(0);
    ws.rule_op(RuleOp::MoveUp);
    ws.toggle_select(0);
    ws.toggle_select(2);
    ws.rule_op(RuleOp::MoveDown);
    assert_eq!(ws.xml(), before);
    assert_eq!(ws.error(), None);
}

#[test]
fn moving_a_rule_keeps_the_selection_on_it() {
    let mut ws = clean("move.dmn");
    ws.toggle_select(2);
    ws.rule_op(RuleOp::MoveUp);
    assert_eq!(ws.selected(), Some(1));
    assert_eq!(output(&ws, 1), "\"C\"");
}

#[test]
fn duplicating_selects_the_copy() {
    let mut ws = clean("duplicate.dmn");
    ws.toggle_select(0);
    ws.rule_op(RuleOp::Duplicate);
    assert_eq!((rules(&ws), ws.selected()), (4, Some(1)));
    assert_eq!(output(&ws, 1), output(&ws, 0));
}

#[test]
fn deleting_shifts_or_clears_the_selection() {
    let mut ws = clean("delete.dmn");
    ws.toggle_select(2);
    ws.commit(Edit::DeleteRule { decision: 0, rule: 0 });
    assert_eq!(ws.selected(), Some(1), "rules after the deleted one move up");
    ws.commit(Edit::DeleteRule { decision: 0, rule: 1 });
    assert_eq!(ws.selected(), None, "deleting the selected rule clears it");
}

#[test]
fn undo_clamps_a_selection_that_no_longer_exists() {
    let mut ws = clean("clamp.dmn");
    ws.rule_op(RuleOp::InsertBelow);
    assert_eq!(ws.selected(), Some(3));
    ws.undo();
    assert_eq!((rules(&ws), ws.selected()), (3, None));
}

#[test]
fn jump_selects_scrolls_and_shows_the_rules() {
    let mut ws = clean("jump.dmn");
    ws.set_view(View::Issues);
    ws.jump(2);
    assert_eq!((ws.view(), ws.selected(), ws.take_scroll()), (View::Rules, Some(2), Some(2)));
    ws.toggle_select(2);
    assert_eq!(ws.selected(), None, "selecting the selected rule again clears it");
}

#[test]
fn changing_the_decision_clears_the_selection_and_the_tester_inputs() {
    let mut ws = clean("decision.dmn");
    ws.toggle_select(1);
    *ws.tests().fields_mut() = vec!["\"EU\"".into()];
    assert!(!ws.set_decision(0), "the same decision changes nothing");
    assert_eq!(ws.selected(), Some(1));
    assert!(ws.set_decision(1));
    assert_eq!((ws.decision(), ws.selected()), (1, None));
    assert!(ws.tests().fields().is_empty());
}

#[test]
fn an_edit_in_progress_is_committed_by_the_next_action() {
    let mut ws = clean("editing.dmn");
    let at = Cursor::Cell(0, Column::Output(0));
    ws.begin_edit(at, output(&ws, 0));
    let mut ed = ws.take_editing().unwrap();
    ed.text = "\"Z\"".into();
    ws.restore_editing(Some(ed));
    assert_eq!(ws.editing_at(), Some(at));
    ws.begin_edit(Cursor::Note(1), String::new());
    assert_eq!(output(&ws, 0), "\"Z\"", "starting another edit commits the first");

    let mut ed = ws.take_editing().unwrap();
    ed.text = "a note".into();
    ws.restore_editing(Some(ed));
    ws.flush_editing();
    assert!(ws.editing_at().is_none());
    assert!(ws.xml().contains("a note"));
}

#[test]
fn ending_an_edit_without_committing_discards_it_and_unchanged_text_is_not_an_edit() {
    let mut ws = clean("end.dmn");
    let at = Cursor::Cell(0, Column::Output(0));
    ws.begin_edit(at, output(&ws, 0));
    let mut ed = ws.take_editing().unwrap();
    ed.text = "\"Z\"".into();
    ws.restore_editing(Some(ed));
    ws.end_edit(Cursor::Cell(1, Column::Output(0)), true);
    assert_eq!(ws.editing_at(), Some(at), "another cell's end does not touch this edit");
    ws.end_edit(at, false);
    assert!(!ws.dirty());

    ws.begin_edit(at, output(&ws, 0));
    ws.end_edit(at, true);
    assert!(!ws.dirty(), "committing the original text changes nothing");
}

#[test]
fn undo_and_discard_drop_the_edit_in_progress_without_committing_it() {
    let mut ws = clean("drop.dmn");
    ws.commit(cell(1, "\"Q\""));
    let at = Cursor::Cell(0, Column::Output(0));
    ws.begin_edit(at, output(&ws, 0));
    let mut ed = ws.take_editing().unwrap();
    ed.text = "\"Z\"".into();
    ws.restore_editing(Some(ed));
    ws.undo();
    assert!(ws.editing_at().is_none());
    assert_eq!(output(&ws, 0), "\"A\"");
    ws.redo();
    ws.begin_edit(at, output(&ws, 0));
    ws.discard();
    assert!(ws.editing_at().is_none() && !ws.dirty());
}

#[test]
fn a_file_that_is_not_utf8_is_read_only() {
    let mut bytes = table_xml(&[("-", r#""A""#)]).into_bytes();
    bytes.extend_from_slice(b"<!-- \xff -->");
    let mut ws = Workspace::open(scratch("bad.dmn", &bytes)).unwrap();
    assert_eq!(ws.error(), Some(READ_ONLY));
    ws.clear_error();
    let before = ws.xml().to_string();
    ws.commit(cell(0, "\"Z\""));
    assert_eq!(ws.error(), Some(READ_ONLY));
    assert_eq!(ws.xml(), before);
    assert!(!ws.fix_start(None));
    assert!(ws.error().is_some_and(|e| e.starts_with("Fix not applied")));
}

#[test]
fn the_table_is_locked_while_a_fix_session_is_open() {
    let mut ws = messy("fixing.dmn");
    assert!(ws.fixable_count() > 0);
    ws.set_view(View::Issues);
    assert!(ws.fix_start(None));
    assert_eq!(ws.view(), View::Rules);
    assert!(!ws.fix_start(None), "a second start changes nothing");
    assert!(ws.selected().is_some(), "the session shows the first issue's rule");

    let before = ws.xml().to_string();
    ws.commit(cell(0, "\"Z\""));
    assert!(ws.error().is_some_and(|e| e.contains("read-only while fixes")));
    assert_eq!(ws.xml(), before);
    ws.redo();
    assert_eq!(ws.xml(), before, "redo does nothing during a session");

    ws.fix_apply();
    assert_ne!(ws.xml(), before);
    assert_eq!(ws.error().filter(|e| e.starts_with("Fix not applied")), None);
    ws.undo();
    assert_eq!(ws.xml(), before, "undo takes back the session's own fix");

    ws.fix_close();
    assert!(!ws.is_fixing());
    ws.clear_error();
    ws.commit(cell(0, "\"Z\""));
    assert_eq!(output(&ws, 0), "\"Z\"");
}

#[test]
fn skipping_lowers_the_fixable_count_until_the_skips_are_reset() {
    let mut ws = messy("skip.dmn");
    let all = ws.fixable_count();
    assert!(ws.fix_start(None));
    ws.fix_skip();
    assert_eq!(ws.skipped_count(), 1);
    assert_eq!(ws.fixable_count(), all - 1);
    ws.fix_reset_skipped();
    assert_eq!((ws.skipped_count(), ws.fixable_count()), (0, all));
}

#[test]
fn stepping_through_issues_wraps_and_shows_the_rules() {
    let mut ws = messy("issues.dmn");
    ws.set_view(View::Xml);
    ws.step_issue(true);
    assert_eq!((ws.view(), ws.selected(), ws.take_scroll()), (View::Rules, Some(0), Some(0)));
    ws.step_issue(true);
    ws.step_issue(true);
    assert_eq!(ws.selected(), Some(2));
    ws.step_issue(true);
    assert_eq!(ws.selected(), Some(0), "wraps forward");
    ws.step_issue(false);
    assert_eq!(ws.selected(), Some(2), "wraps backward");

    let mut none = clean("no-issues.dmn");
    none.set_decision(1);
    none.step_issue(true);
    assert_eq!(none.notice(), Some("No issues on a rule in this decision"));
    none.clear_notice();
    assert_eq!(none.notice(), None);
}

#[test]
fn copy_and_paste_insert_the_copy_after_the_selection_and_select_it() {
    let mut ws = clean("paste.dmn");
    ws.paste_rule();
    assert_eq!(rules(&ws), 3, "nothing copied yet");
    ws.toggle_select(2);
    ws.copy_rule();
    assert!(ws.has_clip());
    assert_eq!(ws.notice(), Some("Rule copied"));
    ws.toggle_select(2);
    ws.toggle_select(0);
    ws.paste_rule();
    assert_eq!((rules(&ws), ws.selected(), ws.take_scroll()), (4, Some(1), Some(1)));
    assert_eq!(output(&ws, 1), "\"C\"");
    ws.undo();
    assert_eq!(rules(&ws), 3, "a paste is one undo step");
}

#[test]
fn replace_all_is_one_undo_step_and_reports_what_it_did() {
    let mut ws = clean("replace.dmn");
    let before = ws.xml().to_string();
    assert_eq!(ws.replace_count("\"EU\"", "\"XX\"", true), 1);
    assert_eq!(ws.replace_count("\"eu\"", "\"XX\"", true), 0);
    assert!(!ws.replace_all("missing", "x", true));
    assert!(ws.replace_all("\"", "'", true));
    assert!(ws.notice().is_some_and(|n| n.starts_with("Replaced")));
    assert_ne!(ws.xml(), before);
    ws.undo();
    assert_eq!(ws.xml(), before);
}

#[test]
fn reloading_resets_the_transient_state_but_keeps_the_decision_and_view() {
    let mut ws = clean("reload.dmn");
    ws.set_decision(1);
    ws.set_decision(0);
    ws.toggle_select(1);
    ws.set_view(View::Test);
    ws.begin_edit(Cursor::Note(0), String::new());
    ws.reload().unwrap();
    assert_eq!((ws.selected(), ws.take_scroll(), ws.editing_at()), (None, None, None));
    assert_eq!((ws.decision(), ws.view()), (0, View::Test));
}

#[test]
fn a_workspace_adopts_the_view_of_the_one_it_replaces() {
    let mut old = clean("adopt-a.dmn");
    old.set_view(View::Xml);
    let mut new = clean("adopt-b.dmn");
    assert_eq!(new.view(), View::Rules);
    new.adopt_view(&old);
    assert_eq!(new.view(), View::Xml);
}

#[test]
fn saving_clears_the_dirty_flag_and_a_failed_save_lands_in_the_error() {
    let mut ws = clean("save.dmn");
    ws.commit(cell(0, "\"Z\""));
    assert!(ws.dirty());
    assert!(ws.save());
    assert!(!ws.dirty());
    let dir = std::env::temp_dir().join(format!("dmn-viewer-workspace-{}", std::process::id()));
    assert!(!ws.save_as(dir.join("missing-dir").join("x.dmn")));
    assert!(ws.error().is_some_and(|e| e.starts_with("Could not save")));
}

#[test]
fn a_fix_session_applies_skips_and_ends_in_a_summary() {
    let mut ws = messy("session.dmn");
    assert!(ws.fix_view().is_none(), "no session, no view");
    assert!(ws.fix_start(None));
    let v = ws.fix_view().unwrap();
    assert_eq!(v.progress, (1, 2));
    assert_eq!(v.current.unwrap().issue.code, "NOT_IN_ALLOWED_VALUES");
    assert_eq!(v.choice, Some(0));
    assert!(!v.can_undo);

    ws.fix_apply();
    assert!(ws.xml().contains(r#""EU","NA","LATAM""#));
    let v = ws.fix_view().unwrap();
    assert_eq!(v.current.unwrap().issue.code, "UNREACHABLE_RULE");
    assert_eq!(v.progress, (2, 2));
    assert!(v.can_undo && v.dirty);

    ws.fix_skip();
    let v = ws.fix_view().unwrap();
    assert!(v.current.is_none(), "an empty queue is the summary");
    assert_eq!((v.applied.len(), v.skipped_now.to_vec(), v.skipped), (1, vec!["Unreachable rule"], 1));

    ws.fix_close();
    assert!(ws.fix_start(None));
    assert!(ws.fix_view().unwrap().current.is_none(), "a skipped issue is not offered again");
}

#[test]
fn a_fix_that_deletes_needs_a_choice_first() {
    let mut ws = open("delete-fix.dmn", &[("-", r#""B""#), ("-", r#""B""#)]);
    assert!(ws.fix_start(None));
    let v = ws.fix_view().unwrap();
    assert_eq!(v.current.unwrap().issue.code, "UNREACHABLE_RULE");
    assert_eq!(v.choice, None);
    assert!(v.preview.is_none());

    ws.fix_apply();
    assert!(ws.error().is_some_and(|e| e.starts_with("Fix not applied") && e.contains("pick it yourself")));
    assert_eq!(ws.undo_depth(), 0);

    ws.fix_move(true);
    assert!(ws.fix_view().unwrap().preview.is_some());
    ws.fix_apply();
    assert_eq!((rules(&ws), ws.error()), (1, None), "a successful step clears the earlier fix error");
}

#[test]
fn undo_brings_the_issue_back_first_and_stops_at_the_session_start() {
    let mut ws = messy("fix-undo.dmn");
    ws.commit(cell(2, "\"D\""));
    let start = ws.xml().to_string();
    assert!(ws.fix_start(None));
    let first = ws.fix_view().unwrap().current.unwrap().key.clone();
    ws.fix_apply();
    ws.fix_apply();
    assert!(ws.fix_view().unwrap().current.is_none());

    ws.fix_undo();
    let v = ws.fix_view().unwrap();
    assert_eq!(v.current.unwrap().issue.code, "UNREACHABLE_RULE", "the last fix's issue comes back first");
    ws.fix_undo();
    assert_eq!(ws.fix_view().unwrap().current.unwrap().key, first);
    assert_eq!(ws.xml(), start);
    ws.fix_undo();
    assert!(output(&ws, 2).contains('D'), "the edit from before the session stays");
    assert!(!ws.fix_view().unwrap().can_undo);

    ws.fix_apply();
    ws.fix_undo_all();
    let v = ws.fix_view().unwrap();
    assert!(v.current.is_none() && v.undone_all && v.applied.is_empty());
    assert_eq!(ws.xml(), start);
}

#[test]
fn a_one_issue_session_previews_its_change_and_ends_after_it() {
    let mut ws = messy("one-issue.dmn");
    let key = ws.issues().iter().find(|i| i.code == "UNREACHABLE_RULE").and_then(|i| ws.fixable_key(i)).unwrap();
    assert!(ws.fix_start(Some(key)));
    let v = ws.fix_view().unwrap();
    assert_eq!(v.progress, (1, 1));
    let p = v.preview.unwrap();
    assert!(p.error.is_none() && p.tests.is_none());
    assert_eq!(p.changes[0].place, "Rule 3");
    assert!(!p.lines.is_empty());
    let first = p as *const _;
    let again = ws.fix_view().unwrap().preview.unwrap() as *const _;
    assert_eq!(first, again, "an unchanged step keeps its preview");

    ws.fix_apply();
    assert!(ws.fix_view().unwrap().current.is_none(), "one issue, then the summary");
}
