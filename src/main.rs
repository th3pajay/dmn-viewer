#![windows_subsystem = "windows"]

mod document;
mod fixing;

use dmn_viewer::diff::{self, Hunk};
use dmn_viewer::edit::Edit;
use dmn_viewer::eval::{RuleStatus, Value};
use dmn_viewer::fix;
use dmn_viewer::lint::{self, Column as Col, Issue, Severity};
use dmn_viewer::model::*;
use dmn_viewer::session::TestSession;
use dmn_viewer::suite::{self, Coverage};
use dmn_viewer::unary::UnaryTests;
use document::{Document, READ_ONLY};
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Layout, Margin, RichText, Sense, Stroke, StrokeKind, TextWrapMode, vec2};
use egui_extras::{Column, TableBuilder};
use fixing::FixSession;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod ic {
    pub const OPEN: &str = "\u{E256}";
    pub const RELOAD: &str = "\u{E036}";
    pub const MOON: &str = "\u{E330}";
    pub const SUN: &str = "\u{E472}";
    pub const TABLE: &str = "\u{E476}";
    pub const WARNING: &str = "\u{E4E0}";
    pub const PLAY: &str = "\u{E3D0}";
    pub const CODE: &str = "\u{E914}";
    pub const SEARCH: &str = "\u{E30C}";
    pub const COPY: &str = "\u{E1CA}";
    pub const CHECK: &str = "\u{E182}";
    pub const X: &str = "\u{E4F6}";
    pub const FLASK: &str = "\u{E79E}";
    pub const SAVE: &str = "\u{E248}";
    pub const PLUS: &str = "\u{E3D4}";
    pub const TRASH: &str = "\u{E4A6}";
    pub const NEW: &str = "\u{E236}";
    pub const IMPORT: &str = "\u{E5DC}";
    pub const ERASER: &str = "\u{E21E}";
    pub const DECISION: &str = "\u{E67C}";
    pub const GRID: &str = "\u{E464}";
}

#[derive(Clone, Copy)]
struct Theme {
    dark: bool,
    canvas: Color32,
    surface: Color32,
    line: Color32,
    line_soft: Color32,
    text: Color32,
    muted: Color32,
    faint: Color32,
    accent: Color32,
    accent_soft: Color32,
    on_accent: Color32,
    chip: Color32,
    chip_text: Color32,
    not_bg: Color32,
    not_text: Color32,
    warn: Color32,
    warn_text: Color32,
    info: Color32,
    info_text: Color32,
    error: Color32,
    error_text: Color32,
    sel_row: Color32,
    hover: Color32,
    in_label: Color32,
    out_label: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

impl Theme {
    fn light() -> Self {
        Theme {
            dark: false,
            canvas: hex(0xf6f7f9),
            surface: hex(0xffffff),
            line: hex(0xe6e8ec),
            line_soft: hex(0xf0f1f4),
            text: hex(0x19202b),
            muted: hex(0x5f6774),
            faint: hex(0x9aa1ab),
            accent: hex(0x13615a),
            accent_soft: hex(0xe4f1ef),
            on_accent: hex(0xffffff),
            chip: hex(0xf0f2f5),
            chip_text: hex(0x2b3340),
            not_bg: hex(0xfbeee8),
            not_text: hex(0x8b3b1d),
            warn: hex(0xe8a33d),
            warn_text: hex(0x9a5c05),
            info: hex(0x7ea3e0),
            info_text: hex(0x2f62b8),
            error: hex(0xd64545),
            error_text: hex(0xb73232),
            sel_row: hex(0xeef7f5),
            hover: hex(0xf8f9fb),
            in_label: hex(0x4f74b8),
            out_label: hex(0x2f8a5e),
        }
    }

    fn dark() -> Self {
        Theme {
            dark: true,
            canvas: hex(0x111418),
            surface: hex(0x181c22),
            line: hex(0x2a3039),
            line_soft: hex(0x21262d),
            text: hex(0xe6e9ee),
            muted: hex(0xa0a8b4),
            faint: hex(0x6b7480),
            accent: hex(0x4fb3a7),
            accent_soft: hex(0x17332f),
            on_accent: hex(0x0d1f1c),
            chip: hex(0x232932),
            chip_text: hex(0xd5dae2),
            not_bg: hex(0x3a2620),
            not_text: hex(0xf0b49a),
            warn: hex(0xe8a33d),
            warn_text: hex(0xeab45a),
            info: hex(0x7ea3e0),
            info_text: hex(0x95b8f0),
            error: hex(0xe06060),
            error_text: hex(0xf08585),
            sel_row: hex(0x19302d),
            hover: hex(0x1d2229),
            in_label: hex(0x86a8e6),
            out_label: hex(0x6cc59a),
        }
    }

    fn sev(&self, s: Severity) -> Color32 {
        match s {
            Severity::Error => self.error,
            Severity::Warning => self.warn,
            Severity::Info => self.info,
        }
    }

    fn sev_text(&self, s: Severity) -> Color32 {
        match s {
            Severity::Error => self.error_text,
            Severity::Warning => self.warn_text,
            Severity::Info => self.info_text,
        }
    }
}

fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}

fn bold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).size(size).family(bold_family())
}

fn setup_fonts(ctx: &egui::Context) {
    let mut f = egui::FontDefinitions::default();
    f.font_data.insert("manrope".into(), Arc::new(egui::FontData::from_static(include_bytes!("../assets/Manrope-Medium.ttf"))));
    f.font_data.insert("manrope-bold".into(), Arc::new(egui::FontData::from_static(include_bytes!("../assets/Manrope-Bold.ttf"))));
    f.font_data.insert("plex-mono".into(), Arc::new(egui::FontData::from_static(include_bytes!("../assets/IBMPlexMono-Regular.ttf"))));
    f.font_data.insert("phosphor".into(), Arc::new(egui::FontData::from_static(include_bytes!("../assets/Phosphor.ttf")).tweak(egui::FontTweak { y_offset_factor: 0.08, ..Default::default() })));
    if let Some(p) = f.families.get_mut(&FontFamily::Proportional) {
        p.insert(0, "manrope".into());
        p.push("phosphor".into());
    }
    if let Some(m) = f.families.get_mut(&FontFamily::Monospace) {
        m.insert(0, "plex-mono".into());
        m.push("phosphor".into());
    }
    let mut b = vec!["manrope-bold".to_string(), "phosphor".to_string()];
    if let Some(p) = f.families.get(&FontFamily::Proportional) {
        b.extend(p.iter().skip(1).cloned());
    }
    f.families.insert(bold_family(), b);
    ctx.set_fonts(f);
}

fn apply_theme(ctx: &egui::Context, t: &Theme) {
    let mut v = if t.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = t.canvas;
    v.window_fill = t.surface;
    v.extreme_bg_color = t.surface;
    v.faint_bg_color = t.hover;
    v.code_bg_color = t.chip;
    v.hyperlink_color = t.accent;
    v.selection.bg_fill = if t.dark { hex(0x24564f) } else { hex(0xc9e6e1) };
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v.window_stroke = Stroke::new(1.0, t.line);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.noninteractive.bg_fill = t.surface;
    v.widgets.noninteractive.weak_bg_fill = t.surface;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(7);
        w.expansion = 0.0;
    }
    v.widgets.inactive.bg_fill = t.surface;
    v.widgets.inactive.weak_bg_fill = t.surface;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, t.line);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.hovered.bg_fill = t.hover;
    v.widgets.hovered.weak_bg_fill = t.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.faint);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.active.bg_fill = t.accent_soft;
    v.widgets.active.weak_bg_fill = t.accent_soft;
    v.widgets.active.bg_stroke = Stroke::new(1.0, t.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.open = v.widgets.active;
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.button_padding = vec2(10.0, 5.0);
        s.spacing.interact_size.y = 28.0;
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(13.5));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(13.5));
        s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(11.0));
        s.text_styles.insert(egui::TextStyle::Monospace, FontId::monospace(12.5));
        s.text_styles.insert(egui::TextStyle::Heading, FontId::new(17.0, bold_family()));
    });
}

fn badge_shape(ui: &mut egui::Ui, text: String, font: FontId, bg: Color32, fg: Color32, pad: f32, height: f32, radius: u8, max_w: f32) -> egui::Response {
    let galley = ui.painter().layout(text, font, fg, (max_w - pad * 2.0).max(10.0));
    let galley = if galley.rows.len() > 1 {
        let first: String = galley.rows[0].text().to_string();
        ui.painter().layout_no_wrap(format!("{}…", first.trim_end()), galley.job.sections[0].format.font_id.clone(), fg)
    } else {
        galley
    };
    let size = vec2(galley.size().x + pad * 2.0, height);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(radius), bg);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg);
    resp
}

fn pill(ui: &mut egui::Ui, text: impl Into<String>, bg: Color32, fg: Color32) -> egui::Response {
    badge_shape(ui, text.into(), FontId::new(11.5, bold_family()), bg, fg, 9.0, 22.0, 11, 600.0)
}

fn chip(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32) -> egui::Response {
    let max_w = ui.available_width().max(40.0);
    badge_shape(ui, text.to_string(), FontId::monospace(11.5), bg, fg, 7.0, 22.0, 5, max_w)
}

/// How many test cases fired a rule: accent when a passing case did, red when only failing ones did.
fn coverage_badge(ui: &mut egui::Ui, t: &Theme, cov: &Coverage, ri: usize) {
    let (pass, fail) = cov.fired_by.get(ri).copied().unwrap_or_default();
    let (bg, fg, tip) = match (pass, fail) {
        (0, 0) => (t.chip, t.faint, "Not covered: no test case fires this rule".to_string()),
        (0, f) => (t.error, hex(0xffffff), format!("Covered only by {f} failing test case{}", if f == 1 { "" } else { "s" })),
        (p, f) => (t.accent_soft, t.accent, format!("Covered by {p} passing{} test case{}", if f > 0 { format!(" and {f} failing") } else { String::new() }, if p + f == 1 { "" } else { "s" })),
    };
    badge_shape(ui, (pass + fail).to_string(), FontId::monospace(10.0), bg, fg, 4.0, 16.0, 8, 40.0).on_hover_text(tip);
}

/// "Coverage 7/9 rules covered (77%) · never fired: 3, 8", truncated to the space available.
fn coverage_line(ui: &mut egui::Ui, t: &Theme, cov: &Coverage) {
    let full = cov.covered() == cov.total();
    let text = format!("Coverage {}  ·  never fired: {}", cov.summary(), cov.never_fired_label());
    ui.add(egui::Label::new(RichText::new(&text).size(12.0).color(if full { t.accent } else { t.muted })).wrap_mode(TextWrapMode::Truncate)).on_hover_text(text);
}

fn button(ui: &mut egui::Ui, t: &Theme, icon: &str, label: &str, primary: bool) -> egui::Response {
    let (fill, stroke, fg) = if primary { (t.accent, t.accent, t.on_accent) } else { (t.surface, t.line, t.text) };
    let text = match (icon.is_empty(), label.is_empty()) {
        (_, true) => RichText::new(icon).size(15.0),
        (true, false) => RichText::new(label),
        (false, false) => RichText::new(format!("{icon}  {label}")),
    };
    ui.add(egui::Button::new(text.color(fg)).fill(fill).stroke(Stroke::new(1.0, stroke)).corner_radius(CornerRadius::same(8)).min_size(vec2(0.0, 32.0)))
}

fn small_button(ui: &mut egui::Ui, t: &Theme, icon: &str, label: &str) -> egui::Response {
    let text = if label.is_empty() { RichText::new(icon).size(13.0) } else { RichText::new(format!("{icon} {label}")).size(12.5) };
    ui.add(egui::Button::new(text.color(t.text)).fill(t.surface).stroke(Stroke::new(1.0, t.line)).corner_radius(CornerRadius::same(6)).min_size(vec2(0.0, 24.0)))
}

fn dot(ui: &mut egui::Ui, color: Color32, size: f32) {
    let (r, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    ui.painter().circle_filled(r.center(), size / 2.0, color);
}

fn nav_item(ui: &mut egui::Ui, t: &Theme, icon: &str, label: &str, badge: Option<(String, Color32, Color32)>, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    let p = ui.painter();
    if active {
        p.rect(rect, CornerRadius::same(7), t.surface, Stroke::new(1.0, t.line), StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(7), if t.dark { t.hover } else { hex(0xeceef2) });
    }
    let fg = if active { t.text } else { t.muted };
    p.text(rect.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, icon, FontId::proportional(16.0), if active { t.accent } else { t.muted });
    p.text(rect.left_center() + vec2(34.0, 0.0), Align2::LEFT_CENTER, label, FontId::new(13.5, if active { bold_family() } else { FontFamily::Proportional }), fg);
    if let Some((text, bg, color)) = badge {
        let galley = p.layout_no_wrap(text, FontId::new(11.0, bold_family()), color);
        let w = galley.size().x + 14.0;
        let r = egui::Rect::from_center_size(egui::pos2(rect.right() - 8.0 - w / 2.0, rect.center().y), vec2(w, 18.0));
        p.rect_filled(r, CornerRadius::same(9), bg);
        p.galley(r.center() - galley.size() / 2.0, galley, color);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn group_label(ui: &mut egui::Ui, t: &Theme, text: &str) {
    ui.add_space(4.0);
    ui.label(bold(text.to_uppercase(), 10.5).color(t.faint));
}

fn card<R>(ui: &mut egui::Ui, t: &Theme, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    Frame::new().fill(t.surface).stroke(Stroke::new(1.0, t.line)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(14)).show(ui, add)
}

#[derive(Default)]
struct Tester {
    session: TestSession,
    case_name: String,
    status: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Cursor {
    Cell(usize, Col),
    Note(usize),
}

struct Editing {
    at: Cursor,
    original: String,
    text: String,
    focus: bool,
}

#[derive(Clone)]
enum Pending {
    Open,
    Reload,
    Load(PathBuf),
    Close,
}

enum HeaderForm {
    Input { input: usize, label: String, expression: String, type_ref: String, lang: String, allowed: String },
    Output { output: usize, label: String, name: String, type_ref: String, allowed: String },
}

fn nonblank(s: &str) -> Option<String> {
    Some(s.to_string()).filter(|s| !s.trim().is_empty())
}

impl HeaderForm {
    fn new(t: &DecisionTable, c: Col) -> Option<HeaderForm> {
        Some(match c {
            Col::Input(i) => {
                let inp = t.inputs.get(i)?;
                HeaderForm::Input {
                    input: i,
                    label: inp.label.clone(),
                    expression: inp.expression.clone(),
                    type_ref: inp.type_ref.clone().unwrap_or_default(),
                    lang: if inp.lang.is_explicit() { inp.lang.declared.clone().unwrap_or_default() } else { String::new() },
                    allowed: inp.input_values.clone().unwrap_or_default(),
                }
            }
            Col::Output(i) => {
                let o = t.outputs.get(i)?;
                HeaderForm::Output { output: i, label: o.label.clone(), name: o.name.clone(), type_ref: o.type_ref.clone().unwrap_or_default(), allowed: o.output_values.clone().unwrap_or_default() }
            }
        })
    }

    fn edit(&self, decision: usize) -> Edit {
        match self {
            HeaderForm::Input { input, label, expression, type_ref, lang, allowed } => {
                Edit::InputHeader { decision, input: *input, label: label.clone(), expression: expression.clone(), type_ref: nonblank(type_ref), lang: nonblank(lang), allowed: nonblank(allowed) }
            }
            HeaderForm::Output { output, label, name, type_ref, allowed } => {
                Edit::OutputHeader { decision, output: *output, label: label.clone(), name: name.clone(), type_ref: nonblank(type_ref), allowed: nonblank(allowed) }
            }
        }
    }
}

const HIT_POLICIES: [&str; 7] = ["UNIQUE", "FIRST", "PRIORITY", "ANY", "COLLECT", "RULE ORDER", "OUTPUT ORDER"];
const TYPES: [&str; 7] = ["string", "integer", "long", "double", "boolean", "date", "number"];
const CELL_LANGS: [(&str, Option<&str>); 4] = [("Default (file or FEEL)", None), ("FEEL", Some("feel")), ("JavaScript", Some("javascript")), ("JUEL", Some("juel"))];

fn rule_menu(ui: &mut egui::Ui, decision: usize, ri: usize, nrules: usize, acts: &mut Vec<Act>) {
    let items = [
        ("Insert rule above", true, Edit::InsertRule { decision, at: ri, copy_of: None }),
        ("Insert rule below", true, Edit::InsertRule { decision, at: ri + 1, copy_of: None }),
        ("Duplicate rule", true, Edit::InsertRule { decision, at: ri + 1, copy_of: Some(ri) }),
        ("Move rule up", ri > 0, Edit::MoveRule { decision, from: ri, to: ri.saturating_sub(1) }),
        ("Move rule down", ri + 1 < nrules, Edit::MoveRule { decision, from: ri, to: ri + 1 }),
        ("Delete rule", true, Edit::DeleteRule { decision, rule: ri }),
    ];
    for (label, enabled, edit) in items {
        if label == "Delete rule" {
            ui.separator();
        }
        if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
            acts.push(Act::Edit(edit));
            ui.close();
        }
    }
}

fn column_menu(ui: &mut egui::Ui, decision: usize, c: Col, outputs: usize, acts: &mut Vec<Act>) {
    let (at, input) = match c {
        Col::Input(i) => (i, true),
        Col::Output(i) => (i, false),
    };
    let what = if input { "input" } else { "output" };
    if ui.button(format!("Edit {what}…")).clicked() {
        acts.push(Act::EditHeader(c));
        ui.close();
    }
    ui.separator();
    for (label, pos) in [(format!("Insert {what} left"), at), (format!("Insert {what} right"), at + 1)] {
        if ui.button(label).clicked() {
            acts.push(Act::Edit(if input { Edit::InsertInput { decision, at: pos } } else { Edit::InsertOutput { decision, at: pos } }));
            ui.close();
        }
    }
    let delete = ui.add_enabled(input || outputs > 1, egui::Button::new(format!("Delete {what}")));
    if delete.on_disabled_hover_text("A decision table needs at least one output").clicked() {
        acts.push(Act::Edit(if input { Edit::DeleteInput { decision, input: at } } else { Edit::DeleteOutput { decision, output: at } }));
        ui.close();
    }
}

/// The inline editor in a table cell. Enter commits, Shift+Enter adds a line, Escape cancels, clicking away commits.
fn edit_box(ui: &mut egui::Ui, ed: &mut Editing, acts: &mut Vec<Act>) {
    let id = ui.id().with("inline_edit");
    if ui.memory(|m| m.has_focus(id)) {
        // consume_key ignores extra Shift, so check it here to leave Shift+Enter to the text box.
        if ui.input_mut(|i| !i.modifiers.shift && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)) {
            acts.push(Act::EndEdit { at: ed.at, commit: true });
        }
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            acts.push(Act::EndEdit { at: ed.at, commit: false });
        }
    }
    let resp = ui.add(egui::TextEdit::multiline(&mut ed.text).id(id).font(egui::TextStyle::Monospace).desired_rows(1).desired_width(f32::INFINITY).margin(Margin::symmetric(4, 2)));
    if ed.focus {
        resp.request_focus();
        ed.focus = false;
    } else if resp.lost_focus() {
        acts.push(Act::EndEdit { at: ed.at, commit: true });
    }
}

#[derive(PartialEq, Clone, Copy)]
enum View {
    Rules,
    Issues,
    Test,
    Xml,
}

enum Act {
    Select(usize),
    Jump(usize),
    View(View),
    Decision(usize),
    Open,
    Reload,
    ToggleDark,
    ToggleLangs,
    Deselect,
    SevFilter(Option<Severity>),
    Edit(Edit),
    BeginEdit(Cursor, String),
    EndEdit {
        at: Cursor,
        commit: bool,
    },
    EditHeader(Col),
    Save,
    Undo,
    Redo,
    Review,
    /// Starts a fix session: for every fixable issue, or for the one with this key.
    FixStart(Option<String>),
    FixChoose(usize),
    FixMove {
        down: bool,
    },
    FixApply,
    FixSkip,
    FixStop,
    FixUndo,
    FixUndoAll,
    FixClose,
    FixReview,
    FixResetSkipped,
}

/// Which issues have an automatic fix, recomputed when the document or the skip list changes.
#[derive(Default)]
struct Fixable {
    revision: u64,
    skipped: usize,
    keys: HashSet<String>,
}

/// The review-changes panel. Hunks are computed once when it opens; nothing can edit the
/// document while it is up, so their ranges stay valid.
struct Review {
    hunks: Vec<Hunk>,
    /// Per hunk: lines in its old and its new text.
    lines: Vec<(usize, usize)>,
}

/// Hunks and lines per side shown in the review panel; the rest is summarised.
const REVIEW_HUNKS: usize = 300;
const REVIEW_LINES: usize = 40;

struct App {
    loaded: Option<Document>,
    error: Option<String>,
    decision: usize,
    view: View,
    filter: String,
    selected: Option<usize>,
    scroll_to: Option<usize>,
    sev_filter: Option<Severity>,
    show_langs: bool,
    theme: Theme,
    tester: Tester,
    editing: Option<Editing>,
    header_form: Option<HeaderForm>,
    pending: Option<Pending>,
    allow_close: bool,
    review: Option<Review>,
    /// While a fix session is open the table is read-only; fixes change it one confirmed step at a time.
    fixing: Option<FixSession>,
    /// Issues skipped in a fix session, by key; forgotten when the file is opened or reloaded.
    fix_skipped: HashSet<String>,
    fixable: Option<Fixable>,
}

fn lang_long(short: &str) -> &str {
    match short {
        "JS" => "JavaScript",
        other => other,
    }
}

fn lang_colors(t: &Theme, short: &str) -> (Color32, Color32) {
    match short {
        "JS" => (t.warn.gamma_multiply(if t.dark { 0.28 } else { 0.2 }), t.warn_text),
        "JUEL" => (t.info.gamma_multiply(if t.dark { 0.28 } else { 0.2 }), t.info_text),
        _ => (t.chip, t.muted),
    }
}

fn lang_describe(lang: &ExpressionLanguage) -> String {
    let source = match lang.source {
        LanguageSource::Cell => "set on cell",
        LanguageSource::FileDefault => "file default",
        LanguageSource::Default => "default, not set",
    };
    format!("{} ({source})", lang_long(&lang.short_name()))
}

fn paint_lang_tag(ui: &mut egui::Ui, t: &Theme, short: &str, explicit: bool, mask: Color32) {
    let (bg, fg) = lang_colors(t, short);
    let (bg, fg) = if explicit { (bg, fg) } else { (t.chip.gamma_multiply(0.7), t.faint) };
    let galley = ui.painter().layout_no_wrap(short.to_string(), FontId::new(9.5, bold_family()), fg);
    let rect = ui.max_rect();
    let size = vec2(galley.size().x + 10.0, 16.0);
    let tag = egui::Rect::from_min_size(egui::pos2(rect.right() - size.x - 8.0, rect.center().y - size.y / 2.0), size);
    ui.painter().rect_filled(tag.expand2(vec2(6.0, 4.0)), CornerRadius::same(4), mask);
    ui.painter().rect_filled(tag, CornerRadius::same(4), bg);
    ui.painter().galley(tag.center() - galley.size() / 2.0, galley, fg);
}

fn row_specific(issue: &Issue, nrules: usize) -> bool {
    !issue.rules.is_empty() && issue.rules.len() <= (nrules / 3).max(3)
}

fn first_rule(range: &str) -> Option<usize> {
    range.split('-').next()?.trim().parse::<usize>().ok().map(|n| n.saturating_sub(1))
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn draw_value(ui: &mut egui::Ui, t: &Theme, text: &str, tests: Option<&UnaryTests>) {
    let tr = text.trim();
    if tr.is_empty() || tr == "-" {
        ui.label(RichText::new("·").color(t.faint).size(16.0));
        return;
    }
    let quoted = [tr.to_string()];
    let (items, negated) = match tests {
        Some(tests) => tests.literals().map_or((None, false), |l| (Some(&l.items[..]), l.negated)),
        None if tr.starts_with('"') && tr.ends_with('"') && tr.len() >= 2 && !tr[1..tr.len() - 1].contains('"') => (Some(&quoted[..]), false),
        None => (None, false),
    };
    match items {
        Some(list) => {
            ui.spacing_mut().item_spacing.x = 4.0;
            let (bg, fg) = if negated { (t.not_bg, t.not_text) } else { (t.chip, t.chip_text) };
            for it in list.iter().take(2) {
                let shown = it.trim().trim_matches('"');
                let shown = if negated { format!("not {shown}") } else { shown.to_string() };
                chip(ui, &shown, bg, fg);
            }
            if list.len() > 2 {
                chip(ui, &format!("+{}", list.len() - 2), t.chip, t.muted);
            }
        }
        None => {
            ui.add(egui::Label::new(RichText::new(one_line(tr)).font(FontId::monospace(11.5)).color(t.chip_text)).wrap_mode(TextWrapMode::Truncate));
        }
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
        setup_fonts(&cc.egui_ctx);
        let theme = Theme::light();
        apply_theme(&cc.egui_ctx, &theme);
        let mut app = App {
            loaded: None,
            error: None,
            decision: 0,
            view: View::Rules,
            filter: String::new(),
            selected: None,
            scroll_to: None,
            sev_filter: None,
            show_langs: false,
            theme,
            tester: Tester::default(),
            editing: None,
            header_form: None,
            pending: None,
            allow_close: false,
            review: None,
            fixing: None,
            fix_skipped: HashSet::new(),
            fixable: None,
        };
        if let Some(p) = args.path {
            app.load(p);
        }
        if let Some(v) = args.view {
            app.view = v;
        }
        if let Some(r) = args.rule.filter(|r| *r > 0) {
            app.selected = Some(r - 1);
            app.scroll_to = Some(r - 1);
        }
        if let (Some(n), Some(l)) = (args.case, app.loaded.as_ref()) {
            app.tester.session.fill_from_case(&l.defs, app.decision, n.saturating_sub(1));
        }
        app
    }

    fn load(&mut self, path: PathBuf) {
        match Document::open(path.clone()) {
            Ok(doc) => {
                let editable = doc.editable;
                let same_file = self.loaded.as_ref().is_some_and(|l| l.path == path);
                if !same_file {
                    self.decision = 0;
                    self.tester = Tester::default();
                    let suite_file = suite::default_path(&path);
                    if suite_file.exists() {
                        self.tester.status = match self.tester.session.load(&suite_file) {
                            Ok(n) => format!("Loaded {n} cases from {}", suite_file.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()),
                            Err(e) => e,
                        };
                    }
                }
                self.tester.session.invalidate();
                self.loaded = Some(doc);
                self.fixing = None;
                self.fix_skipped.clear();
                self.fixable = None;
                self.error = (!editable).then(|| READ_ONLY.to_string());
                self.editing = None;
                self.header_form = None;
                self.selected = None;
                self.scroll_to = None;
            }
            Err(e) => self.error = Some(format!("Could not open {}: {e}", path.display())),
        }
    }

    fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter("DMN", &["dmn", "xml"]).add_filter("All files", &["*"]).pick_file() {
            self.load(p);
        }
    }

    fn guard(&mut self, ctx: &egui::Context, p: Pending) {
        if self.loaded.as_ref().is_some_and(|l| l.dirty()) {
            self.pending = Some(p);
        } else {
            self.run(ctx, p);
        }
    }

    fn run(&mut self, ctx: &egui::Context, p: Pending) {
        match p {
            Pending::Open => self.open_dialog(),
            Pending::Reload => {
                if let Some(p) = self.loaded.as_ref().map(|l| l.path.clone()) {
                    self.load(p);
                }
            }
            Pending::Load(p) => self.load(p),
            Pending::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn commit(&mut self, edit: Edit) {
        let Some(l) = self.loaded.as_mut() else { return };
        if !l.editable {
            self.error = Some(READ_ONLY.into());
            return;
        }
        if self.fixing.is_some() {
            self.error = Some("The table is read-only while fixes are being reviewed. Stop the fix session to edit it.".into());
            return;
        }
        match l.commit(&edit) {
            Ok(false) => {}
            Ok(true) => {
                self.tester.session.invalidate();
                match edit {
                    Edit::InsertRule { at, .. } | Edit::MoveRule { to: at, .. } => {
                        self.selected = Some(at);
                        self.scroll_to = Some(at);
                    }
                    Edit::DeleteRule { rule, .. } => self.selected = self.selected.filter(|&s| s != rule).map(|s| if s > rule { s - 1 } else { s }),
                    _ => {}
                }
                self.clamp_selection();
            }
            Err(e) => self.error = Some(format!("Edit not applied: {e}")),
        }
    }

    fn commit_editing(&mut self, ed: Editing) {
        if ed.text == ed.original {
            return;
        }
        let decision = self.decision;
        self.commit(match ed.at {
            Cursor::Cell(rule, col) => Edit::Cell { decision, rule, col, text: ed.text },
            Cursor::Note(rule) => Edit::Note { decision, rule, text: ed.text },
        });
    }

    fn history(&mut self, redo: bool) {
        self.editing = None;
        let Some(l) = self.loaded.as_mut() else { return };
        match if redo { l.redo() } else { l.undo() } {
            Ok(false) => return,
            Ok(true) => {}
            Err(e) => self.error = Some(e),
        }
        self.tester.session.invalidate();
        self.clamp_selection();
    }

    /// Throws away unsaved edits (undo can still bring them back).
    fn discard(&mut self) {
        self.editing = None;
        let Some(l) = self.loaded.as_mut().filter(|l| l.dirty()) else { return };
        if let Err(e) = l.discard() {
            self.error = Some(e);
        }
        self.tester.session.invalidate();
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        let n = self.loaded.as_ref().and_then(|l| l.defs.table(self.decision)).map_or(0, |t| t.rules.len());
        self.selected = self.selected.filter(|&s| s < n);
    }

    fn save(&mut self) -> bool {
        let Some(l) = self.loaded.as_mut() else { return false };
        match l.save() {
            Ok(()) => true,
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }

    fn open_review(&mut self) {
        if let Some(ed) = self.editing.take() {
            self.commit_editing(ed);
        }
        let Some(l) = &self.loaded else { return };
        let hunks = l.changes();
        let lines = hunks.iter().map(|h| (diff::line_count(&l.saved()[h.old.clone()]), diff::line_count(&l.xml[h.new.clone()]))).collect();
        self.review = Some(Review { hunks, lines });
    }

    fn review_modal(&mut self, ctx: &egui::Context) {
        let (Some(rv), Some(l)) = (&self.review, &self.loaded) else { return };
        let t = self.theme;
        let mut choice = None;
        let screen = ctx.content_rect();
        let modal = egui::Modal::new(egui::Id::new("review_changes")).show(ctx, |ui| {
            ui.set_width((screen.width() * 0.7).clamp(420.0, 1000.0));
            let (removed, added) = rv.lines.iter().fold((0, 0), |(r, a), &(o, n)| (r + o, a + n));
            ui.horizontal(|ui| {
                ui.label(bold("Review changes", 15.0).color(t.text));
                if !rv.hunks.is_empty() {
                    let n = rv.hunks.len();
                    pill(ui, format!("{n} change{}  \u{b7}  -{removed}  +{added} lines", if n == 1 { "" } else { "s" }), t.chip, t.muted);
                }
            });
            ui.add_space(6.0);
            if rv.hunks.is_empty() {
                ui.label(RichText::new("No unsaved changes.").color(t.muted));
            } else {
                let removed_bg = if t.dark { hex(0x3a1f22) } else { hex(0xfdeeee) };
                let side = |ui: &mut egui::Ui, text: &str, lines: usize, mark: char, bg: Color32, fg: Color32| {
                    if text.is_empty() {
                        return;
                    }
                    let mut shown = String::with_capacity(text.len().min(4096) + 64);
                    for line in text.lines().take(REVIEW_LINES) {
                        shown.push(mark);
                        shown.push(' ');
                        shown.push_str(line);
                        shown.push('\n');
                    }
                    if lines > REVIEW_LINES {
                        shown.push_str(&format!("  \u{2026} {} more lines", lines - REVIEW_LINES));
                    } else {
                        shown.pop();
                    }
                    Frame::new().fill(bg).corner_radius(CornerRadius::same(6)).inner_margin(Margin::symmetric(8, 4)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(egui::Label::new(RichText::new(shown).font(FontId::monospace(12.0)).color(fg)).wrap_mode(TextWrapMode::Wrap));
                    });
                };
                egui::ScrollArea::vertical().max_height(screen.height() * 0.6).auto_shrink([false, true]).show(ui, |ui| {
                    for (h, &(old_lines, new_lines)) in rv.hunks.iter().zip(&rv.lines).take(REVIEW_HUNKS) {
                        let at = if h.old_line == h.new_line { format!("Line {}", h.new_line) } else { format!("Line {} (was {})", h.new_line, h.old_line) };
                        ui.label(RichText::new(at).size(11.5).color(t.faint));
                        side(ui, l.saved().get(h.old.clone()).unwrap_or(""), old_lines, '-', removed_bg, t.error_text);
                        side(ui, l.xml.get(h.new.clone()).unwrap_or(""), new_lines, '+', t.accent_soft, t.text);
                        ui.add_space(8.0);
                    }
                    if rv.hunks.len() > REVIEW_HUNKS {
                        ui.label(RichText::new(format!("\u{2026} and {} more changes", rv.hunks.len() - REVIEW_HUNKS)).color(t.muted));
                    }
                });
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if !rv.hunks.is_empty() && button(ui, &t, ic::SAVE, "Save", true).clicked() {
                    choice = Some(true);
                }
                if button(ui, &t, ic::X, if rv.hunks.is_empty() { "Close" } else { "Cancel" }, false).on_hover_text("Esc").clicked() {
                    choice = Some(false);
                }
            });
        });
        if modal.should_close() {
            choice = choice.or(Some(false));
        }
        match choice {
            Some(true) => {
                if self.save() {
                    self.review = None;
                }
            }
            Some(false) => self.review = None,
            None => {}
        }
    }

    fn pending_modal(&mut self, ctx: &egui::Context) {
        let Some(p) = self.pending.clone() else { return };
        let t = self.theme;
        let file = self.loaded.as_ref().and_then(|l| l.path.file_name()).map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let mut choice = None;
        egui::Modal::new(egui::Id::new("unsaved_changes")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(bold("Unsaved changes", 15.0).color(t.text));
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{file} has changes that are not saved.")).color(t.muted));
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if button(ui, &t, ic::SAVE, "Save", true).clicked() {
                    choice = Some("save");
                }
                if button(ui, &t, ic::TRASH, "Discard", false).clicked() {
                    choice = Some("discard");
                }
                if button(ui, &t, ic::X, "Cancel", false).clicked() {
                    choice = Some("cancel");
                }
            });
        });
        match choice {
            Some("save") if self.save() => {
                self.pending = None;
                self.run(ctx, p);
            }
            Some("discard") => {
                self.pending = None;
                self.discard();
                self.run(ctx, p);
            }
            Some("cancel") => self.pending = None,
            _ => {}
        }
    }

    fn header_window(&mut self, ctx: &egui::Context) {
        let t = self.theme;
        let decision = self.decision;
        let Some(form) = self.header_form.as_mut() else { return };
        let title = if matches!(form, HeaderForm::Input { .. }) { "Edit input" } else { "Edit output" };
        let mut open = true;
        let mut done = None;
        egui::Window::new(title).collapsible(false).resizable(false).open(&mut open).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            egui::Grid::new("header_form").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                let field = |ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str| {
                    ui.label(RichText::new(label).color(t.muted));
                    ui.add(egui::TextEdit::singleline(value).hint_text(hint).desired_width(280.0));
                    ui.end_row();
                };
                let type_row = |ui: &mut egui::Ui, value: &mut String| {
                    ui.label(RichText::new("Type").color(t.muted));
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(value).hint_text("none").desired_width(200.0));
                        egui::ComboBox::from_id_salt("header_type").selected_text("pick").width(70.0).show_ui(ui, |ui| {
                            for ty in TYPES {
                                ui.selectable_value(value, ty.to_string(), ty);
                            }
                        });
                    });
                    ui.end_row();
                };
                match form {
                    HeaderForm::Input { label, expression, type_ref, lang, allowed, .. } => {
                        field(ui, "Label", label, "shown as the column title");
                        field(ui, "Expression", expression, "variable name or expression");
                        type_row(ui, type_ref);
                        ui.label(RichText::new("Language").color(t.muted));
                        let shown = if lang.is_empty() { "default".to_string() } else { lang.clone() };
                        egui::ComboBox::from_id_salt("header_lang").selected_text(shown).show_ui(ui, |ui| {
                            for (label, v) in [("default", ""), ("FEEL", "feel"), ("JavaScript", "javascript"), ("JUEL", "juel")] {
                                ui.selectable_value(lang, v.to_string(), label);
                            }
                        });
                        ui.end_row();
                        field(ui, "Allowed values", allowed, "\"A\",\"B\"  (empty: anything)");
                    }
                    HeaderForm::Output { label, name, type_ref, allowed, .. } => {
                        field(ui, "Label", label, "shown as the column title");
                        field(ui, "Name", name, "result variable name");
                        type_row(ui, type_ref);
                        field(ui, "Allowed values", allowed, "\"A\",\"B\"  (empty: anything)");
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if button(ui, &t, ic::CHECK, "Apply", true).clicked() {
                    done = Some(true);
                }
                if button(ui, &t, ic::X, "Cancel", false).clicked() {
                    done = Some(false);
                }
            });
        });
        if !open {
            done = Some(false);
        }
        match done {
            Some(true) => {
                let edit = form.edit(decision);
                self.header_form = None;
                self.commit(edit);
            }
            Some(false) => self.header_form = None,
            None => {}
        }
    }

    fn apply(&mut self, ctx: &egui::Context, acts: Vec<Act>) {
        for a in acts {
            match a {
                Act::Select(r) => self.selected = if self.selected == Some(r) { None } else { Some(r) },
                Act::Jump(r) => {
                    self.selected = Some(r);
                    self.scroll_to = Some(r);
                    self.filter.clear();
                    self.view = View::Rules;
                }
                Act::View(v) => self.view = v,
                Act::Decision(d) => {
                    if let Some(ed) = self.editing.take() {
                        self.commit_editing(ed);
                    }
                    if self.decision != d {
                        self.tester.session.fields.clear();
                        self.selected = None;
                    }
                    self.decision = d;
                }
                Act::Open => self.guard(ctx, Pending::Open),
                Act::Reload => self.guard(ctx, Pending::Reload),
                Act::ToggleDark => {
                    self.theme = if self.theme.dark { Theme::light() } else { Theme::dark() };
                    apply_theme(ctx, &self.theme);
                }
                Act::SevFilter(s) => self.sev_filter = s,
                Act::ToggleLangs => self.show_langs = !self.show_langs,
                Act::Deselect => self.selected = None,
                Act::Edit(e) => self.commit(e),
                Act::BeginEdit(at, text) => {
                    if let Some(ed) = self.editing.take() {
                        self.commit_editing(ed);
                    }
                    self.editing = Some(Editing { at, original: text.clone(), text, focus: true });
                }
                Act::EndEdit { at, commit } => {
                    if self.editing.as_ref().is_some_and(|ed| ed.at == at) {
                        let ed = self.editing.take().expect("checked");
                        if commit {
                            self.commit_editing(ed);
                        }
                    }
                }
                Act::EditHeader(c) => self.header_form = self.loaded.as_ref().and_then(|l| l.defs.table(self.decision)).and_then(|t| HeaderForm::new(t, c)),
                Act::Save => {
                    self.save();
                }
                // While fixing, undo takes back the session's own fixes and nothing before them.
                Act::Undo if self.fixing.is_some() => self.fix_act(Act::FixUndo),
                Act::Redo if self.fixing.is_some() => {}
                Act::Undo => self.history(false),
                Act::Redo => self.history(true),
                Act::Review => self.open_review(),
                fix => self.fix_act(fix),
            }
        }
    }

    /// Recomputes which issues have a fix when the document or the skips changed since last time.
    fn refresh_fixable(&mut self) {
        let Some(l) = &self.loaded else { return };
        if self.fixable.as_ref().is_some_and(|f| f.revision == l.revision() && f.skipped == self.fix_skipped.len()) {
            return;
        }
        let keys = if l.editable { fix::queue(&l.defs, &l.issues, &HashSet::new(), None, None).into_iter().map(|c| c.key).collect() } else { HashSet::new() };
        self.fixable = Some(Fixable { revision: l.revision(), skipped: self.fix_skipped.len(), keys });
    }

    /// Fixable issues that are not skipped.
    fn fixable_count(&self) -> usize {
        self.fixable.as_ref().map_or(0, |f| f.keys.iter().filter(|k| !self.fix_skipped.contains(*k)).count())
    }

    /// Shows the issue on offer: its decision, and its first rule selected and scrolled to.
    fn follow_fix(&mut self) {
        let Some(c) = self.fixing.as_ref().and_then(|s| s.current()) else { return };
        if let Some(d) = c.issue.decision {
            self.decision = d;
        }
        let rule = c.issue.rules.iter().min().copied();
        self.selected = rule;
        self.scroll_to = rule;
    }

    fn fix_act(&mut self, a: Act) {
        if let Some(ed) = self.editing.take() {
            self.commit_editing(ed);
        }
        let Some(l) = self.loaded.as_mut() else { return };
        let skipped = &mut self.fix_skipped;
        let mut changed = false;
        let result = match (a, self.fixing.as_mut()) {
            (Act::FixStart(only), None) => {
                if !l.editable {
                    Err(READ_ONLY.to_string())
                } else {
                    self.header_form = None;
                    self.fixing = Some(FixSession::start(l, skipped, only));
                    self.filter.clear();
                    self.view = View::Rules;
                    Ok(())
                }
            }
            (Act::FixChoose(i), Some(s)) => {
                s.choose(i);
                Ok(())
            }
            (Act::FixMove { down }, Some(s)) => {
                s.move_choice(down);
                Ok(())
            }
            (Act::FixApply, Some(s)) => {
                changed = true;
                s.apply(l, skipped)
            }
            (Act::FixSkip, Some(s)) => {
                s.skip(l, skipped);
                Ok(())
            }
            (Act::FixStop, Some(s)) => {
                s.stop();
                Ok(())
            }
            (Act::FixUndo, Some(s)) => {
                changed = true;
                s.undo(l, skipped).map(|_| ())
            }
            (Act::FixUndoAll, Some(s)) => {
                changed = true;
                s.undo_all(l, skipped)
            }
            (Act::FixResetSkipped, Some(s)) => {
                skipped.clear();
                s.settle(l, skipped);
                Ok(())
            }
            (Act::FixResetSkipped, None) => {
                skipped.clear();
                Ok(())
            }
            (Act::FixClose, Some(_)) => {
                self.fixing = None;
                Ok(())
            }
            (Act::FixReview, Some(_)) => {
                self.fixing = None;
                self.open_review();
                Ok(())
            }
            _ => Ok(()),
        };
        if changed {
            self.tester.session.invalidate();
            self.clamp_selection();
        }
        match result {
            Ok(()) => {
                if self.error.as_deref().is_some_and(|e| e.starts_with("Fix not applied")) {
                    self.error = None;
                }
            }
            Err(e) => self.error = Some(format!("Fix not applied: {e}")),
        }
        self.follow_fix();
    }

    fn nav(&self, ui: &mut egui::Ui, suite_stats: Option<(usize, usize)>, acts: &mut Vec<Act>) {
        let t = &self.theme;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
            ui.painter().rect_filled(r, CornerRadius::same(6), t.accent);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, ic::GRID, FontId::proportional(14.0), t.on_accent);
            ui.label(bold("DMN Viewer", 14.5).color(t.text));
        });
        ui.add_space(14.0);
        if let Some(l) = &self.loaded {
            group_label(ui, t, "Decisions");
            for (i, d) in l.defs.decisions.iter().enumerate() {
                let errs = l.issues.iter().filter(|x| x.decision == Some(i) && x.severity == Severity::Error).count();
                let count = match &d.logic {
                    DecisionLogic::Table(tb) => tb.rules.len().to_string(),
                    DecisionLogic::Literal { .. } => "expr".into(),
                    DecisionLogic::None => "-".into(),
                };
                let active = self.decision == i;
                let badge = if errs > 0 {
                    (errs.to_string(), t.error, hex(0xffffff))
                } else if active {
                    (count, t.accent_soft, t.accent)
                } else {
                    (count, t.chip, t.muted)
                };
                let name = if d.name.is_empty() { &d.id } else { &d.name };
                if nav_item(ui, t, ic::DECISION, name, Some(badge), active).clicked() {
                    acts.push(Act::Decision(i));
                }
            }
            ui.add_space(12.0);
            group_label(ui, t, "View");
            let n = l.issues.iter().filter(|x| x.decision == Some(self.decision) || x.decision.is_none()).count();
            let errs = l.issues.iter().filter(|x| (x.decision == Some(self.decision) || x.decision.is_none()) && x.severity == Severity::Error).count();
            let issue_badge = if errs > 0 { (n.to_string(), t.error, hex(0xffffff)) } else { (n.to_string(), t.chip, t.muted) };
            let test_badge = suite_stats.map(|(p, total)| if p == total { (format!("{p}/{total}"), t.accent_soft, t.accent) } else { (format!("{p}/{total}"), t.error, hex(0xffffff)) });
            for (v, icon, label, badge) in
                [(View::Rules, ic::TABLE, "Rules", None), (View::Issues, ic::WARNING, "Issues", Some(issue_badge)), (View::Test, ic::FLASK, "Test", test_badge), (View::Xml, ic::CODE, "XML", None)]
            {
                if nav_item(ui, t, icon, label, badge, self.view == v).clicked() {
                    acts.push(Act::View(v));
                }
            }
            ui.add_space(12.0);
        }
        group_label(ui, t, "File");
        if nav_item(ui, t, ic::OPEN, "Open…", None, false).on_hover_text("Ctrl+O").clicked() {
            acts.push(Act::Open);
        }
        if self.loaded.is_some() && nav_item(ui, t, ic::RELOAD, "Reload", None, false).on_hover_text("F5").clicked() {
            acts.push(Act::Reload);
        }
        let (icon, label) = if t.dark { (ic::SUN, "Light mode") } else { (ic::MOON, "Dark mode") };
        if nav_item(ui, t, icon, label, None, false).clicked() {
            acts.push(Act::ToggleDark);
        }
    }

    fn header(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let t = self.theme;
        let Some(l) = &self.loaded else { return };
        let file = l.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        let dec = l.defs.decisions.get(self.decision);
        let dirty = l.dirty();
        let fixing = self.fixing.is_some();
        let fixable = self.fixable_count();
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new(format!("{file}{}  /", if dirty { " •" } else { "" })).color(t.faint)).on_hover_text(if dirty { "Unsaved changes (Ctrl+S to save)" } else { "Saved" });
            if let Some(d) = dec {
                ui.label(bold(if d.name.is_empty() { &d.id } else { &d.name }, 14.0).color(t.text));
                if let DecisionLogic::Table(tb) = &d.logic {
                    if l.editable && !fixing {
                        let current = tb.hit_policy.to_ascii_uppercase();
                        let mut hp = current.clone();
                        let mut agg = tb.aggregation.as_deref().map(str::to_ascii_uppercase);
                        let before = agg.clone();
                        egui::ComboBox::from_id_salt("hit_policy").selected_text(&hp).width(120.0).show_ui(ui, |ui| {
                            for p in HIT_POLICIES {
                                ui.selectable_value(&mut hp, p.to_string(), p);
                            }
                        });
                        if hp == "COLLECT" {
                            egui::ComboBox::from_id_salt("aggregation").selected_text(agg.clone().unwrap_or_else(|| "list".into())).width(70.0).show_ui(ui, |ui| {
                                ui.selectable_value(&mut agg, None, "list");
                                for a in ["SUM", "MIN", "MAX", "COUNT"] {
                                    ui.selectable_value(&mut agg, Some(a.to_string()), a);
                                }
                            });
                        }
                        if hp != current || agg != before {
                            acts.push(Act::Edit(Edit::HitPolicy { decision: self.decision, hit_policy: hp, aggregation: agg }));
                        }
                    } else {
                        pill(ui, &tb.hit_policy, t.chip, t.muted).on_hover_text("Hit policy");
                    }
                    pill(ui, format!("{} in · {} out", tb.inputs.len(), tb.outputs.len()), t.chip, t.muted);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(ui, &t, ic::PLAY, "Run tests", true).clicked() {
                    acts.push(Act::View(View::Test));
                }
                if fixing {
                    pill(ui, "Fixing: the table is read-only", t.warn.gamma_multiply(if t.dark { 0.25 } else { 0.15 }), t.warn_text);
                }
                if dirty && !fixing && button(ui, &t, ic::SAVE, "Save", false).on_hover_text("Ctrl+S").clicked() {
                    acts.push(Act::Save);
                }
                if dirty && !fixing && button(ui, &t, ic::SEARCH, "Review", false).on_hover_text("Review the unsaved changes (Ctrl+D)").clicked() {
                    acts.push(Act::Review);
                }
                if fixable > 0
                    && !fixing
                    && button(ui, &t, ic::CHECK, &format!("Fix issues ({fixable})"), false)
                        .on_hover_text("Go through the issues that have an automatic fix, one at a time. You confirm every change.")
                        .clicked()
                {
                    acts.push(Act::FixStart(None));
                }
                if self.view != View::Issues && button(ui, &t, ic::COPY, "Copy report", false).on_hover_text("Copy all issues as text").clicked() {
                    ui.ctx().copy_text(lint::report(&l.defs, &l.issues, &l.path.display().to_string()));
                }
                if self.view == View::Rules {
                    Frame::new()
                        .fill(if t.dark { t.surface } else { hex(0xf9fafb) })
                        .stroke(Stroke::new(1.0, t.line))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(10, 5))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if !self.filter.is_empty() && ui.add(egui::Button::new(RichText::new(ic::X).color(t.muted)).frame(false)).clicked() {
                                    self.filter.clear();
                                }
                                ui.add(egui::TextEdit::singleline(&mut self.filter).frame(Frame::NONE).hint_text("Filter rules").desired_width(170.0));
                                ui.label(RichText::new(ic::SEARCH).color(t.faint));
                            });
                        });
                }
            });
        });
    }

    fn rules_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let t = self.theme;
        let Some(l) = &self.loaded else { return };
        let Some(dec) = l.defs.decisions.get(self.decision) else { return };
        let tb = match &dec.logic {
            DecisionLogic::Table(tb) => tb,
            DecisionLogic::Literal { text, lang, variable } => {
                card(ui, &t, |ui| {
                    ui.label(bold("Literal expression", 14.0));
                    ui.label(RichText::new(format!("Language: {}   Variable: {}", lang_describe(lang), variable.as_deref().unwrap_or("-"))).color(t.muted));
                    ui.add(egui::TextEdit::multiline(&mut text.as_str()).code_editor().desired_width(f32::INFINITY));
                });
                return;
            }
            DecisionLogic::None => {
                ui.label(RichText::new("This decision has no logic.").color(t.muted));
                return;
            }
        };
        let nrules = tb.rules.len();
        let issues: Vec<&Issue> = l.issues.iter().filter(|i| i.decision == Some(self.decision)).collect();
        let fired: Vec<usize> = if self.tester.session.is_active() && self.tester.session.fields.len() == tb.inputs.len() {
            self.tester.session.outcome(&l.defs, self.decision).map(|o| o.fired.clone()).unwrap_or_default()
        } else {
            Vec::new()
        };
        // Cached with the suite's results; it only re-runs when the cases or the definitions change.
        let session = &mut self.tester.session;
        let coverage = (!session.suite().cases.is_empty() && suite::targets(&session.suite().decision, dec)).then(|| session.coverage(&l.defs, self.decision)).flatten();

        // During a fix session the fix panel takes the drawer's place.
        if self.fixing.is_none() {
            egui::Panel::right("drawer")
                .resizable(true)
                .default_size(330.0)
                .frame(Frame::new().fill(t.surface).inner_margin(Margin::same(0)))
                .show(ui, |ui| self.issue_list(ui, &issues, nrules, false, acts));
        }

        let rule_sev = |ri: usize| issues.iter().filter(|i| row_specific(i, nrules) && i.rules.contains(&ri)).map(|i| i.severity).min();
        let cell_sev = |ri: usize, c: Col| issues.iter().filter(|i| i.column == Some(c) && i.rules.contains(&ri) && row_specific(i, nrules)).map(|i| i.severity).min();
        let col_issue = |c: Col| issues.iter().filter(|i| i.column == Some(c) && i.rules.is_empty() && i.severity != Severity::Info).min_by_key(|i| i.severity).copied();

        let f = self.filter.to_lowercase();
        let visible: Vec<usize> = (0..nrules)
            .filter(|&ri| {
                let r = &tb.rules[ri];
                f.is_empty()
                    || (ri + 1).to_string() == f
                    || r.summary().to_lowercase().contains(&f)
                    || r.inputs.iter().map(|e| &e.text).chain(r.outputs.iter().map(|e| &e.text)).any(|text| text.to_lowercase().contains(&f))
            })
            .collect();
        let scroll_target = self.scroll_to.take().and_then(|r| visible.iter().position(|&x| x == r));
        let selected = self.selected;
        let show_langs = self.show_langs;
        let editable = l.editable && self.fixing.is_none();
        let decision = self.decision;

        if let Some(r) = selected.and_then(|ri| tb.rules.get(ri).map(|r| (ri, r))) {
            let (ri, rule) = r;
            egui::Panel::bottom("rule_detail").resizable(true).default_size(240.0).frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 20, right: 16, top: 10, bottom: 14 })).show(
                ui,
                |ui| {
                    card(ui, &t, |ui| {
                        ui.set_min_height(ui.available_height());
                        ui.horizontal(|ui| {
                            ui.label(bold(format!("Rule {}", ri + 1), 15.0).color(t.text));
                            ui.label(RichText::new(format!("{}  ·  XML line {}", rule.id, rule.line)).size(12.0).color(t.faint));
                            if fired.contains(&ri) {
                                pill(ui, "fired by tester", t.accent_soft, t.accent);
                            }
                            if let Some(cov) = &coverage {
                                coverage_badge(ui, &t, cov, ri);
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if small_button(ui, &t, ic::X, "").on_hover_text("Close (Esc)").clicked() {
                                    acts.push(Act::Deselect);
                                }
                            });
                        });
                        let summary = rule.summary();
                        if !summary.trim().is_empty() {
                            ui.label(RichText::new(summary.trim()).size(12.5).italics().color(t.muted));
                        }
                        ui.add_space(4.0);
                        let mut rows: Vec<(String, &str, String, &ExpressionLanguage, &str)> = Vec::new();
                        for (ci, inp) in tb.inputs.iter().enumerate() {
                            if let Some(e) = rule.inputs.get(ci).filter(|e| !e.is_any()) {
                                rows.push((inp.title().to_string(), "input", inp.type_ref.clone().unwrap_or_else(|| "-".into()), &e.lang, e.text.trim()));
                            }
                        }
                        for (oi, out) in tb.outputs.iter().enumerate() {
                            if let Some(e) = rule.outputs.get(oi).filter(|e| !e.is_any()) {
                                rows.push((out.title().to_string(), "output", out.type_ref.clone().unwrap_or_else(|| "-".into()), &e.lang, e.text.trim()));
                            }
                        }
                        let hidden = tb.inputs.len() + tb.outputs.len() - rows.len();
                        const W: [f32; 4] = [170.0, 64.0, 70.0, 150.0];
                        let fixed = |ui: &mut egui::Ui, w: f32, add: &mut dyn FnMut(&mut egui::Ui)| {
                            ui.allocate_ui_with_layout(vec2(w, 22.0), Layout::left_to_right(Align::Center), |ui| {
                                ui.set_width(w);
                                add(ui);
                            });
                        };
                        ui.horizontal(|ui| {
                            for (i, h) in ["Column", "Kind", "Type", "Language"].iter().enumerate() {
                                fixed(ui, W[i], &mut |ui| {
                                    ui.label(bold(h.to_uppercase(), 9.5).color(t.faint));
                                });
                            }
                            ui.label(bold("EXPRESSION", 9.5).color(t.faint));
                        });
                        egui::ScrollArea::vertical().id_salt("rule_detail_scroll").auto_shrink([false, false]).show(ui, |ui| {
                            for (name, kind, ty, lang, text) in &rows {
                                ui.horizontal_top(|ui| {
                                    fixed(ui, W[0], &mut |ui| {
                                        ui.add(egui::Label::new(bold(name.as_str(), 12.5).color(t.text)).wrap_mode(TextWrapMode::Truncate));
                                    });
                                    fixed(ui, W[1], &mut |ui| {
                                        ui.label(RichText::new(*kind).size(12.0).color(if *kind == "input" { t.in_label } else { t.out_label }));
                                    });
                                    fixed(ui, W[2], &mut |ui| {
                                        ui.label(RichText::new(ty.as_str()).size(12.0).color(t.muted));
                                    });
                                    fixed(ui, W[3], &mut |ui| {
                                        let short = lang.short_name();
                                        let (bg, fg) = if lang.is_explicit() { lang_colors(&t, &short) } else { (t.chip, t.faint) };
                                        pill(ui, if lang.is_explicit() { lang_long(&short).to_string() } else { format!("{} (default)", lang_long(&short)) }, bg, fg)
                                            .on_hover_text(lang_describe(lang));
                                    });
                                    let w = (ui.available_width() - 44.0).max(120.0);
                                    ui.allocate_ui_with_layout(vec2(w, 0.0), Layout::top_down(Align::Min), |ui| {
                                        ui.set_width(w);
                                        ui.add_space(3.0);
                                        ui.add(egui::Label::new(RichText::new(*text).font(FontId::monospace(12.0)).color(t.chip_text)).wrap());
                                    });
                                    if small_button(ui, &t, ic::COPY, "").on_hover_text("Copy expression").clicked() {
                                        ui.ctx().copy_text(text.to_string());
                                    }
                                });
                                ui.add_space(4.0);
                                let y = ui.cursor().top();
                                ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(1.0, t.line_soft));
                                ui.add_space(6.0);
                            }
                            if hidden > 0 {
                                ui.add_space(4.0);
                                ui.label(RichText::new(format!("{hidden} empty cells (match anything / no output) not listed")).size(11.5).color(t.faint));
                            }
                        });
                    });
                },
            );
        }

        let editing = &mut self.editing;
        egui::CentralPanel::default().frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 20, right: 0, top: 14, bottom: 0 })).show(ui, |ui| {
            ui.horizontal(|ui| {
                for (c, label) in [(t.warn, "warning"), (t.info, "info"), (t.accent, if fired.is_empty() { "fired by tester" } else { "fired by tester inputs" })] {
                    dot(ui, c, 8.0);
                    ui.label(RichText::new(label).size(12.0).color(t.muted));
                    ui.add_space(6.0);
                }
                if let Some(cov) = &coverage {
                    ui.add_space(6.0);
                    ui.allocate_ui_with_layout(vec2((ui.available_width() - 420.0).max(120.0), 20.0), Layout::left_to_right(Align::Center), |ui| coverage_line(ui, &t, cov));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(16.0);
                    let shown = if visible.len() == nrules { format!("{nrules} rules") } else { format!("{} of {nrules} rules", visible.len()) };
                    ui.label(RichText::new(shown).size(12.0).color(t.muted));
                    if editable
                        && small_button(ui, &t, ic::PLUS, "Rule").on_hover_text("Add an empty rule at the end. Right-click a rule for more; double-click a cell or header to edit it.").clicked()
                    {
                        acts.push(Act::Edit(Edit::InsertRule { decision, at: nrules, copy_of: None }));
                    }
                    ui.add_space(10.0);
                    let (bg, fg) = if show_langs { (t.text, t.surface) } else { (t.chip, t.muted) };
                    if pill(ui, if show_langs { "Languages: all" } else { "Languages: set only" }, bg, fg)
                        .interact(Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Show the expression language on every cell, including cells that use the default")
                        .clicked()
                    {
                        acts.push(Act::ToggleLangs);
                    }
                });
            });
            ui.add_space(6.0);
            Frame::new().fill(t.surface).stroke(Stroke::new(1.0, t.line)).corner_radius(CornerRadius { nw: 10, ne: 0, sw: 0, se: 0 }).show(ui, |ui| {
                egui::ScrollArea::horizontal().auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let mut builder = TableBuilder::new(ui)
                        .striped(false)
                        .resizable(true)
                        .sense(Sense::click())
                        .cell_layout(Layout::left_to_right(Align::Center))
                        .column(Column::exact(if coverage.is_some() { 76.0 } else { 52.0 }));
                    for _ in &tb.inputs {
                        builder = builder.column(Column::initial(140.0).at_least(60.0).clip(true));
                    }
                    for (oi, _) in tb.outputs.iter().enumerate() {
                        builder = builder.column(Column::initial(if oi == 0 { 300.0 } else { 140.0 }).at_least(60.0).clip(true));
                    }
                    builder = builder.column(Column::remainder().at_least(220.0).clip(true));
                    if let Some(row) = scroll_target {
                        builder = builder.scroll_to_row(row, Some(Align::Center));
                    }
                    let paint_cell = |ui: &mut egui::Ui, header: bool| {
                        let r = ui.max_rect();
                        ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(1.0, if header { t.line } else { t.line_soft }));
                        ui.add_space(12.0);
                    };
                    builder
                        .header(76.0, |mut h| {
                            h.col(|ui| {
                                paint_cell(ui, true);
                                ui.label(bold(&tb.hit_policy[..1.min(tb.hit_policy.len())], 13.0).color(t.muted)).on_hover_text(format!("Hit policy {}", tb.hit_policy));
                            });
                            for (ci, inp) in tb.inputs.iter().enumerate() {
                                h.col(|ui| {
                                    paint_cell(ui, true);
                                    let issue = col_issue(Col::Input(ci));
                                    let (tag, color) = match issue {
                                        Some(i) if i.code == "UNUSED_INPUT" => ("UNUSED", t.sev_text(i.severity)),
                                        Some(i) => ("INPUT", t.sev_text(i.severity)),
                                        None => ("INPUT", t.in_label),
                                    };
                                    let short = inp.lang.short_name();
                                    let resp = ui
                                        .vertical(|ui| {
                                            ui.add_space(8.0);
                                            ui.label(bold(tag, 9.5).color(color));
                                            ui.add(egui::Label::new(bold(inp.title(), 12.5).color(t.text)).wrap_mode(TextWrapMode::Truncate));
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing.x = 4.0;
                                                ui.label(RichText::new(inp.type_ref.as_deref().unwrap_or("-")).size(11.0).color(t.muted));
                                                ui.label(RichText::new("·").size(11.0).color(t.faint));
                                                if inp.lang.is_explicit() {
                                                    ui.label(bold(lang_long(&short), 11.0).color(lang_colors(&t, &short).1));
                                                } else {
                                                    ui.label(RichText::new(format!("{} (default)", lang_long(&short))).size(11.0).color(t.faint));
                                                }
                                            });
                                        })
                                        .response
                                        .interact(Sense::click())
                                        .on_hover_text(format!(
                                            "{}\nexpression: {}\ntype: {}\nexpression language: {}\nrule conditions: FEEL unary tests unless a cell says otherwise{}",
                                            inp.title(),
                                            inp.expression.trim(),
                                            inp.type_ref.as_deref().unwrap_or("-"),
                                            lang_describe(&inp.lang),
                                            inp.input_values.as_ref().map(|v| format!("\nallowed: {v}")).unwrap_or_default()
                                        ));
                                    if editable {
                                        if resp.double_clicked() {
                                            acts.push(Act::EditHeader(Col::Input(ci)));
                                        }
                                        resp.context_menu(|ui| column_menu(ui, decision, Col::Input(ci), tb.outputs.len(), acts));
                                    }
                                });
                            }
                            for (oi, out) in tb.outputs.iter().enumerate() {
                                h.col(|ui| {
                                    paint_cell(ui, true);
                                    let langs: std::collections::BTreeSet<String> = tb.rules.iter().filter_map(|r| r.outputs.get(oi)).filter(|e| !e.is_any()).map(|e| e.lang.short_name()).collect();
                                    let resp = ui
                                        .vertical(|ui| {
                                            ui.add_space(8.0);
                                            ui.label(bold("OUTPUT", 9.5).color(t.out_label));
                                            ui.add(egui::Label::new(bold(out.title(), 12.5).color(t.text)).wrap_mode(TextWrapMode::Truncate));
                                            ui.label(RichText::new(format!("{}  ·  {} name", out.type_ref.as_deref().unwrap_or("-"), out.name)).size(11.0).color(t.muted));
                                        })
                                        .response
                                        .interact(Sense::click())
                                        .on_hover_text(format!(
                                            "{}\nname: {}\ntype: {}\nlanguages used in this column: {}\n(the language is set per cell, not per column)",
                                            out.title(),
                                            out.name,
                                            out.type_ref.as_deref().unwrap_or("-"),
                                            if langs.is_empty() { "-".to_string() } else { langs.into_iter().map(|s| lang_long(&s).to_string()).collect::<Vec<_>>().join(", ") }
                                        ));
                                    if editable {
                                        if resp.double_clicked() {
                                            acts.push(Act::EditHeader(Col::Output(oi)));
                                        }
                                        resp.context_menu(|ui| column_menu(ui, decision, Col::Output(oi), tb.outputs.len(), acts));
                                    }
                                });
                            }
                            h.col(|ui| {
                                paint_cell(ui, true);
                                ui.vertical(|ui| {
                                    ui.add_space(9.0);
                                    ui.label(bold("NOTE", 9.5).color(t.faint));
                                    ui.label(bold("Annotation", 12.5).color(t.text));
                                });
                            });
                        })
                        .body(|body| {
                            body.rows(40.0, visible.len(), |mut row| {
                                let ri = visible[row.index()];
                                let r = &tb.rules[ri];
                                let is_sel = selected == Some(ri);
                                let is_fired = fired.contains(&ri);
                                let base = |ui: &mut egui::Ui| {
                                    let rect = ui.max_rect();
                                    if is_sel {
                                        ui.painter().rect_filled(rect, 0.0, t.sel_row);
                                    }
                                    paint_cell(ui, false);
                                };
                                row.col(|ui| {
                                    base(ui);
                                    let rs = rule_sev(ri);
                                    match (is_fired, rs) {
                                        (true, _) => dot(ui, t.accent, 7.0),
                                        (false, Some(s)) => dot(ui, t.sev(s), 7.0),
                                        _ => {
                                            ui.add_space(7.0);
                                        }
                                    }
                                    ui.add_space(6.0);
                                    let num = if is_fired { bold((ri + 1).to_string(), 12.5).color(t.accent) } else { RichText::new((ri + 1).to_string()).size(12.5).color(t.faint) };
                                    ui.label(num).on_hover_text(format!("{}  ·  XML line {}{}", r.id, r.line, if is_fired { "\nfired for the current test inputs" } else { "" }));
                                    if let Some(cov) = &coverage {
                                        ui.add_space(4.0);
                                        coverage_badge(ui, &t, cov, ri);
                                    }
                                });
                                if editable {
                                    row.response().context_menu(|ui| rule_menu(ui, decision, ri, nrules, acts));
                                }
                                let mut cell = |row: &mut egui_extras::TableRow, e: Option<(&str, &ExpressionLanguage)>, tests: Option<&UnaryTests>, c: Col, is_input: bool, ty: Option<&str>| {
                                    let raw = e.map_or("", |(text, _)| text);
                                    let e = e.filter(|(text, _)| !matches!(text.trim(), "" | "-"));
                                    let at = Cursor::Cell(ri, c);
                                    let (_, resp) = row.col(|ui| {
                                        base(ui);
                                        if let Some(ed) = editing.as_mut().filter(|ed| ed.at == at) {
                                            edit_box(ui, ed, acts);
                                            return;
                                        }
                                        if let Some(s) = cell_sev(ri, c) {
                                            let rr = ui.max_rect();
                                            ui.painter().hline(rr.x_range(), rr.bottom() - 1.0, Stroke::new(2.0, t.sev(s)));
                                        }
                                        draw_value(ui, &t, e.map(|(text, _)| text).unwrap_or(""), tests);
                                        if let Some((_, lang)) = e {
                                            if lang.is_explicit() || show_langs {
                                                let mask = if is_sel { t.sel_row } else { t.surface };
                                                paint_lang_tag(ui, &t, &lang.short_name(), lang.is_explicit(), mask);
                                            }
                                        }
                                    });
                                    if editable {
                                        if resp.double_clicked() {
                                            acts.push(Act::BeginEdit(at, raw.to_string()));
                                        }
                                        resp.context_menu(|ui| {
                                            if ui.button("Edit cell").clicked() {
                                                acts.push(Act::BeginEdit(at, raw.to_string()));
                                                ui.close();
                                            }
                                            ui.menu_button("Expression language", |ui| {
                                                for (label, lang) in CELL_LANGS {
                                                    if ui.button(label).clicked() {
                                                        acts.push(Act::Edit(Edit::CellLanguage { decision, rule: ri, col: c, lang: lang.map(str::to_string) }));
                                                        ui.close();
                                                    }
                                                }
                                            });
                                            ui.separator();
                                            rule_menu(ui, decision, ri, nrules, acts);
                                        });
                                    }
                                    if let Some((text, lang)) = e {
                                        let text: String = text.trim().lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
                                        resp.on_hover_text(format!(
                                            "{text}\n\nLanguage: {}\nColumn type: {}{}",
                                            lang_describe(lang),
                                            ty.unwrap_or("-"),
                                            if is_input { "\nKind: condition (unary test)" } else { "\nKind: output expression" }
                                        ));
                                    }
                                };
                                for ci in 0..tb.inputs.len() {
                                    let e = r.inputs.get(ci);
                                    cell(&mut row, e.map(|e| (e.text.as_str(), &e.lang)), e.map(|e| &e.tests), Col::Input(ci), true, tb.inputs[ci].type_ref.as_deref());
                                }
                                for oi in 0..tb.outputs.len() {
                                    cell(&mut row, r.outputs.get(oi).map(|e| (e.text.as_str(), &e.lang)), None, Col::Output(oi), false, tb.outputs[oi].type_ref.as_deref());
                                }
                                let note_at = Cursor::Note(ri);
                                let (_, resp) = row.col(|ui| {
                                    base(ui);
                                    if let Some(ed) = editing.as_mut().filter(|ed| ed.at == note_at) {
                                        edit_box(ui, ed, acts);
                                        return;
                                    }
                                    ui.add(egui::Label::new(RichText::new(one_line(&r.summary())).size(12.5).color(t.muted)).wrap_mode(TextWrapMode::Truncate));
                                });
                                if editable {
                                    if resp.double_clicked() {
                                        acts.push(Act::BeginEdit(note_at, tb.note(r).to_string()));
                                    }
                                    resp.context_menu(|ui| rule_menu(ui, decision, ri, nrules, acts));
                                }
                                if row.response().clicked() {
                                    acts.push(Act::Select(ri));
                                }
                            });
                        });
                });
            });
        });
    }

    fn issue_list(&self, ui: &mut egui::Ui, issues: &[&Issue], nrules: usize, wide: bool, acts: &mut Vec<Act>) {
        let t = &self.theme;
        let count = |s: Severity| issues.iter().filter(|i| i.severity == s).count();
        Frame::new().inner_margin(Margin { left: 16, right: 16, top: 16, bottom: 10 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(bold("Issues", 15.0).color(t.text));
                if wide {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(l) = &self.loaded {
                            if button(ui, t, ic::COPY, "Copy report", false).clicked() {
                                ui.ctx().copy_text(lint::report(&l.defs, &l.issues, &l.path.display().to_string()));
                            }
                        }
                    });
                }
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (sev, label) in [(None, "All"), (Some(Severity::Error), "Errors"), (Some(Severity::Warning), "Warnings"), (Some(Severity::Info), "Info")] {
                    let n = match sev {
                        None => issues.len(),
                        Some(s) => count(s),
                    };
                    let on = self.sev_filter == sev;
                    let (bg, fg) = if on { (t.text, t.surface) } else { (t.chip, t.muted) };
                    let resp = pill(ui, format!("{label} {n}"), bg, fg).interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    if resp.clicked() {
                        acts.push(Act::SevFilter(sev));
                    }
                }
            });
        });
        let pr = ui.max_rect();
        ui.painter().hline(pr.x_range(), ui.cursor().top(), Stroke::new(1.0, t.line_soft));
        let shown: Vec<&&Issue> = issues.iter().filter(|i| self.sev_filter.is_none_or(|s| i.severity == s)).collect();
        egui::ScrollArea::vertical().id_salt(if wide { "iss_wide" } else { "iss_side" }).auto_shrink([false, false]).show(ui, |ui| {
            Frame::new().inner_margin(Margin { left: 12, right: 12, top: 10, bottom: 14 }).show(ui, |ui| {
                if shown.is_empty() {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(ic::CHECK).color(t.accent).size(16.0));
                        ui.label(RichText::new("Nothing to show for this filter.").color(t.muted));
                    });
                }
                let mut last: Option<Severity> = None;
                for is in shown {
                    if wide && last != Some(is.severity) {
                        last = Some(is.severity);
                        ui.add_space(8.0);
                        let label = match is.severity {
                            Severity::Error => "Errors",
                            Severity::Warning => "Warnings",
                            Severity::Info => "Info",
                        };
                        ui.label(bold(format!("{label}  {}", count(is.severity)), 12.0).color(t.sev_text(is.severity)));
                        ui.add_space(2.0);
                    }
                    let resp = Frame::new()
                        .fill(t.surface)
                        .stroke(Stroke::new(1.0, t.line))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(12, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                dot(ui, t.sev(is.severity), 7.0);
                                ui.label(bold(is.title(), 12.5).color(t.text));
                                if wide {
                                    ui.label(RichText::new(is.code).font(FontId::monospace(11.0)).color(t.faint));
                                }
                                if !row_specific(is, nrules) && !is.rules.is_empty() {
                                    ui.label(RichText::new("table-wide").size(11.0).color(t.faint));
                                }
                                let key = self.loaded.as_ref().map(|l| fix::key(&l.defs, is)).filter(|k| self.fixable.as_ref().is_some_and(|f| f.keys.contains(k)));
                                if let Some(key) = key {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if self.fix_skipped.contains(&key) {
                                            pill(ui, "skipped", t.chip, t.faint).on_hover_text("Skipped in a fix session. Reload the file or use \"Show skipped again\" to be asked again.");
                                        } else if self.fixing.is_none() && small_button(ui, t, ic::CHECK, "Fix").on_hover_text("Review the automatic fix for this issue").clicked() {
                                            acts.push(Act::FixStart(Some(key)));
                                        }
                                    });
                                }
                            });
                            ui.add(egui::Label::new(RichText::new(&is.message).size(12.0).color(t.muted)).wrap());
                            if !is.rules.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                                    for part in lint::rules_label(&is.rules).split(", ") {
                                        let label = if part.contains('-') { format!("rules {part}") } else { format!("rule {part}") };
                                        if chip(ui, &label, t.chip, t.chip_text).interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                            if let Some(r) = first_rule(part) {
                                                acts.push(Act::Jump(r));
                                            }
                                        }
                                    }
                                });
                            }
                        })
                        .response;
                    let resp = resp.interact(Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_stroke(resp.rect, CornerRadius::same(8), Stroke::new(1.0, t.faint), StrokeKind::Inside);
                    }
                    if resp.clicked() {
                        if let Some(r) = is.rules.iter().min() {
                            acts.push(Act::Jump(*r));
                        }
                    }
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn issues_view(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let t = self.theme;
        let Some(l) = &self.loaded else { return };
        let issues: Vec<&Issue> = l.issues.iter().filter(|i| i.decision == Some(self.decision) || i.decision.is_none()).collect();
        let nrules = l.defs.table(self.decision).map(|t| t.rules.len()).unwrap_or(0);
        egui::CentralPanel::default().frame(Frame::new().fill(t.canvas).inner_margin(Margin::symmetric(20, 16))).show(ui, |ui| {
            let w = ui.available_width().min(920.0);
            ui.allocate_ui(vec2(w, ui.available_height()), |ui| {
                Frame::new().fill(t.surface).stroke(Stroke::new(1.0, t.line)).corner_radius(CornerRadius::same(10)).show(ui, |ui| {
                    ui.set_min_height(ui.available_height());
                    self.issue_list(ui, &issues, nrules, true, acts);
                });
            });
        });
    }

    fn xml_view(&self, ui: &mut egui::Ui) {
        let t = self.theme;
        let Some(l) = &self.loaded else { return };
        egui::CentralPanel::default().frame(Frame::new().fill(t.canvas).inner_margin(Margin::symmetric(20, 16))).show(ui, |ui| {
            Frame::new().fill(t.surface).stroke(Stroke::new(1.0, t.line)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(12)).show(ui, |ui| {
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut l.xml.as_str()).code_editor().frame(Frame::NONE).desired_width(f32::INFINITY));
                });
            });
        });
    }

    fn empty_view(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let t = self.theme;
        egui::CentralPanel::default().frame(Frame::new().fill(t.canvas)).show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.3);
                    ui.label(RichText::new(ic::TABLE).size(40.0).color(t.faint));
                    ui.add_space(8.0);
                    ui.label(bold("Open a DMN file", 18.0).color(t.text));
                    ui.label(RichText::new("Drop a .dmn file on this window, or press Ctrl+O.").color(t.muted));
                    ui.add_space(12.0);
                    if button(ui, &t, ic::OPEN, "Open file", true).clicked() {
                        acts.push(Act::Open);
                    }
                });
            });
        });
    }
}

/// One from → to line: where, the old text struck through, the new text.
fn change_row(ui: &mut egui::Ui, t: &Theme, c: &fix::Change) {
    ui.label(RichText::new(&c.place).size(11.5).color(t.faint));
    let removed_bg = if t.dark { hex(0x3a1f22) } else { hex(0xfdeeee) };
    let side = |ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32, strike: bool| {
        Frame::new().fill(bg).corner_radius(CornerRadius::same(5)).inner_margin(Margin::symmetric(6, 3)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let mut rt = RichText::new(if text.is_empty() { "(empty)" } else { text }).font(FontId::monospace(11.5)).color(fg);
            if strike {
                rt = rt.strikethrough();
            }
            ui.add(egui::Label::new(rt).wrap());
        });
    };
    side(ui, &c.from, removed_bg, t.error_text, true);
    side(ui, &c.to, t.accent_soft, t.text, false);
    ui.add_space(6.0);
}

fn note(ui: &mut egui::Ui, bg: Color32, fg: Color32, text: impl Into<String>) {
    Frame::new().fill(bg).corner_radius(CornerRadius::same(6)).inner_margin(Margin::symmetric(8, 6)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.add(egui::Label::new(RichText::new(text).size(12.0).color(fg)).wrap());
    });
}

/// The fix session's panel: the current step (options, from → to, test effect, XML lines) or the summary.
fn fix_panel(ui: &mut egui::Ui, t: &Theme, s: &mut FixSession, doc: &Document, suite: &suite::Suite, skipped: usize, acts: &mut Vec<Act>) {
    let warn_bg = t.warn.gamma_multiply(if t.dark { 0.25 } else { 0.15 });
    let error_bg = if t.dark { hex(0x3a1f22) } else { hex(0xfdeeee) };
    egui::ScrollArea::vertical().id_salt("fix_panel").auto_shrink([false, false]).show(ui, |ui| {
        Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let Some(c) = s.current().cloned() else {
                summary_panel(ui, t, s, doc, skipped, acts);
                return;
            };
            let (n, total) = s.progress();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Fix {n} of ~{total}")).size(12.0).color(t.muted));
                dot(ui, t.sev(c.issue.severity), 7.0);
                ui.label(RichText::new(c.issue.code).font(FontId::monospace(11.0)).color(t.faint));
            });
            ui.add_space(2.0);
            ui.label(bold(c.issue.title(), 15.0).color(t.text));
            ui.add(egui::Label::new(RichText::new(&c.issue.message).size(12.0).color(t.muted)).wrap());
            if !c.issue.rules.is_empty() {
                ui.label(RichText::new(format!("Rule {}", lint::rules_label(&c.issue.rules))).size(11.5).color(t.faint));
            }
            ui.add_space(8.0);
            for (i, f) in c.fixes.iter().enumerate() {
                let on = s.choice == Some(i);
                let (stroke, fill) = match (on, f.destructive) {
                    (true, true) => (t.error, error_bg),
                    (true, false) => (t.accent, t.accent_soft),
                    _ => (t.line, t.surface),
                };
                let resp = Frame::new()
                    .fill(fill)
                    .stroke(Stroke::new(1.0, stroke))
                    .corner_radius(CornerRadius::same(7))
                    .inner_margin(Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal_top(|ui| {
                            let _ = ui.radio(on, "");
                            ui.vertical(|ui| {
                                let label = if f.destructive { format!("{}  {}", ic::TRASH, f.label) } else { f.label.clone() };
                                ui.add(egui::Label::new(RichText::new(label).size(12.5).color(if f.destructive { t.error_text } else { t.text })).wrap());
                                if let Some(h) = &f.hint {
                                    ui.add(egui::Label::new(RichText::new(h).size(11.5).color(t.muted)).wrap());
                                }
                            });
                        });
                    })
                    .response
                    .interact(Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.clicked() {
                    acts.push(Act::FixChoose(i));
                }
                ui.add_space(4.0);
            }
            let destructive = s.choice.and_then(|i| c.fixes.get(i)).filter(|f| f.destructive).map(|f| f.label.clone());
            ui.add_space(4.0);
            match s.preview(doc, suite) {
                None => note(ui, error_bg, t.error_text, "This fix deletes something, so nothing is selected. Pick it yourself if you want it."),
                Some(p) => {
                    for ch in &p.changes {
                        change_row(ui, t, ch);
                    }
                    if let Some(e) = &p.error {
                        note(ui, error_bg, t.error_text, format!("This fix cannot be applied: {e}"));
                    }
                    if let Some(fx) = &p.tests {
                        let head = format!("Tests: {} → {} of {} passing.", fx.passed_before, fx.passed_after, fx.total);
                        if !fx.broken.is_empty() {
                            note(ui, warn_bg, t.warn_text, format!("{head} This would break: {}.", fx.broken.join(", ")));
                        } else if !fx.fixed.is_empty() {
                            note(ui, t.accent_soft, t.accent, format!("{head} Now passes: {}.", fx.fixed.join(", ")));
                        } else {
                            note(ui, t.chip, t.muted, format!("{head} No result changes."));
                        }
                        if !fx.changed.is_empty() {
                            note(ui, t.chip, t.muted, format!("Still failing, for a different reason: {}.", fx.changed.join(", ")));
                        }
                    }
                    if !p.lines.is_empty() {
                        ui.add_space(4.0);
                        egui::CollapsingHeader::new(RichText::new("XML lines that change").size(12.0).color(t.muted)).id_salt("fix_xml").show(ui, |ui| {
                            for (line, old, new) in &p.lines {
                                ui.label(RichText::new(format!("Line {line}")).size(11.0).color(t.faint));
                                for (text, mark, bg, fg) in [(old, '-', error_bg, t.error_text), (new, '+', t.accent_soft, t.text)] {
                                    if text.is_empty() {
                                        continue;
                                    }
                                    let shown: String = text.lines().map(|l| format!("{mark} {l}")).collect::<Vec<_>>().join("\n");
                                    Frame::new().fill(bg).corner_radius(CornerRadius::same(5)).inner_margin(Margin::symmetric(6, 3)).show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.add(egui::Label::new(RichText::new(shown).font(FontId::monospace(11.0)).color(fg)).wrap());
                                    });
                                }
                                ui.add_space(4.0);
                            }
                        });
                    }
                }
            }
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                let apply = match &destructive {
                    Some(label) => ui.add(
                        egui::Button::new(RichText::new(format!("{}  {label}", ic::TRASH)).color(hex(0xffffff)))
                            .fill(t.error)
                            .stroke(Stroke::new(1.0, t.error))
                            .corner_radius(CornerRadius::same(8))
                            .min_size(vec2(0.0, 32.0)),
                    ),
                    None => button(ui, t, ic::CHECK, "Apply", true),
                };
                if apply.on_hover_text("Enter").clicked() {
                    acts.push(Act::FixApply);
                }
                if button(ui, t, "", "Skip", false).on_hover_text("S: not now; it is not offered again until the file is reloaded").clicked() {
                    acts.push(Act::FixSkip);
                }
                if button(ui, t, "", "Stop", false).on_hover_text("Esc: keep the applied fixes and show the summary").clicked() {
                    acts.push(Act::FixStop);
                }
            });
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                let can = s.can_undo(doc);
                if ui.add_enabled(can, egui::Button::new(RichText::new("Undo last fix").size(12.0))).on_hover_text("Ctrl+Z").clicked() {
                    acts.push(Act::FixUndo);
                }
                if ui.add_enabled(can, egui::Button::new(RichText::new("Undo all fixes").size(12.0))).clicked() {
                    acts.push(Act::FixUndoAll);
                }
            });
        });
    });
}

fn summary_panel(ui: &mut egui::Ui, t: &Theme, s: &FixSession, doc: &Document, skipped: usize, acts: &mut Vec<Act>) {
    ui.label(bold("Fix summary", 15.0).color(t.text));
    ui.add_space(6.0);
    if s.applied.is_empty() {
        let text = if s.undone_all { "Every fix from this session was undone." } else { "No fixes applied." };
        ui.label(RichText::new(text).color(t.muted));
    } else {
        ui.label(RichText::new(format!("{} applied, not saved yet", s.applied.len())).size(12.5).color(t.text));
        ui.add_space(6.0);
        for a in &s.applied {
            ui.horizontal_wrapped(|ui| {
                ui.label(bold(a.title, 12.5).color(t.text));
                ui.label(RichText::new(&a.label).size(12.0).color(t.muted));
            });
            for ch in &a.changes {
                change_row(ui, t, ch);
            }
            ui.add_space(4.0);
        }
    }
    if !s.skipped_now.is_empty() {
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(format!("Skipped: {}", s.skipped_now.join(", "))).size(12.0).color(t.muted)).wrap());
    }
    if skipped > 0 && ui.link(RichText::new(format!("Show {skipped} skipped issue{} again", if skipped == 1 { "" } else { "s" })).size(12.0)).clicked() {
        acts.push(Act::FixResetSkipped);
    }
    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(s.can_undo(doc), egui::Button::new("Undo all fixes")).clicked() {
            acts.push(Act::FixUndoAll);
        }
        if doc.dirty() && button(ui, t, ic::SEARCH, "Review & save", true).clicked() {
            acts.push(Act::FixReview);
        }
        if button(ui, t, ic::X, "Close", false).on_hover_text("Esc. Each fix stays one Ctrl+Z step.").clicked() {
            acts.push(Act::FixClose);
        }
    });
}

fn test_view(ui: &mut egui::Ui, l: &Document, decision: usize, selected: Option<usize>, tester: &mut Tester, t: &Theme, acts: &mut Vec<Act>) {
    let Some(tb) = l.defs.table(decision) else {
        egui::CentralPanel::default().frame(Frame::new().fill(t.canvas).inner_margin(Margin::same(20))).show(ui, |ui| {
            ui.label(RichText::new("The tester works on decision tables. Pick one on the left.").color(t.muted));
        });
        return;
    };
    let dec_name = l.defs.decisions[decision].name.as_str();
    tester.session.fields.resize(tb.inputs.len(), String::new());

    egui::Panel::left("test_inputs").resizable(true).default_size(390.0).frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 20, right: 8, top: 16, bottom: 16 })).show(ui, |ui| {
        card(ui, t, |ui| {
            ui.set_min_height(ui.available_height());
            ui.horizontal(|ui| {
                ui.label(bold("Inputs", 15.0).color(t.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if small_button(ui, t, ic::ERASER, "Clear").clicked() {
                        tester.session.clear_inputs();
                    }
                    if ui.add_enabled_ui(selected.is_some(), |ui| small_button(ui, t, ic::IMPORT, "From rule")).inner.on_hover_text("Copy the selected rule's conditions into the inputs").clicked() {
                        if let Some(r) = selected.and_then(|r| tb.rules.get(r)) {
                            tester.session.fill_from_rule(r);
                        }
                    }
                });
            });
            ui.label(RichText::new("Empty means null. Type \"\" for an empty string.").size(11.5).color(t.faint));
            ui.add_space(6.0);
            egui::ScrollArea::vertical().id_salt("tin").auto_shrink([false, false]).show(ui, |ui| {
                for (i, inp) in tb.inputs.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(bold(inp.title(), 12.5).color(t.text));
                        ui.label(RichText::new(inp.type_ref.clone().unwrap_or_default()).size(11.0).color(t.faint));
                    });
                    let hint = inp.input_values.clone().unwrap_or_else(|| "null".into());
                    ui.add(egui::TextEdit::singleline(&mut tester.session.fields[i]).font(egui::TextStyle::Monospace).hint_text(hint).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 5)));
                    ui.add_space(4.0);
                }
            });
        });
    });

    let outcome = tester.session.outcome(&l.defs, decision).expect("decision is a table");
    let results = tester.session.results(&l.defs, decision).expect("decision is a table");
    let coverage = suite::targets(&tester.session.suite().decision, &l.defs.decisions[decision]).then(|| tester.session.coverage(&l.defs, decision)).flatten();

    egui::Panel::bottom("test_suite").resizable(true).default_size(300.0).frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 8, right: 20, top: 8, bottom: 16 })).show(ui, |ui| {
        card(ui, t, |ui| {
            ui.set_min_height(ui.available_height());
            let passed = results.iter().filter(|r| r.pass).count();
            ui.horizontal(|ui| {
                ui.label(bold("Test suite", 15.0).color(t.text));
                if !results.is_empty() {
                    if passed == results.len() {
                        pill(ui, format!("{passed}/{} passed", results.len()), t.accent_soft, t.accent);
                    } else {
                        pill(ui, format!("{} failing", results.len() - passed), t.error, hex(0xffffff));
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let default_name = suite::default_path(&l.path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    let mut save_to = None;
                    if small_button(ui, t, ic::SAVE, "Save as…").clicked() {
                        save_to = rfd::FileDialog::new().add_filter("Test suite", &["json"]).set_file_name(&default_name).save_file();
                    }
                    if small_button(ui, t, ic::SAVE, "Save").clicked() {
                        save_to = tester.session.suite_path().map(Path::to_path_buf).or_else(|| rfd::FileDialog::new().add_filter("Test suite", &["json"]).set_file_name(&default_name).save_file());
                    }
                    if small_button(ui, t, ic::OPEN, "Load…").clicked() {
                        if let Some(p) = rfd::FileDialog::new().add_filter("Test suite", &["json"]).pick_file() {
                            tester.status = match tester.session.load(&p) {
                                Ok(n) => format!("Loaded {n} cases"),
                                Err(e) => e,
                            };
                        }
                    }
                    if small_button(ui, t, ic::NEW, "New").clicked() {
                        tester.session.new_suite(dec_name);
                        tester.status = "New empty suite".into();
                    }
                    if let Some(p) = save_to {
                        tester.status = match tester.session.save(&p, dec_name) {
                            Ok(n) => format!("Saved {n} cases"),
                            Err(e) => e,
                        };
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut tester.case_name).hint_text("Name for the new case").desired_width(240.0).margin(Margin::symmetric(8, 4)));
                if small_button(ui, t, ic::PLUS, "Add current inputs").on_hover_text("Saves the inputs and the current result as the expected result").clicked() {
                    tester.session.add_case(&l.defs, decision, &tester.case_name);
                    tester.case_name.clear();
                }
                ui.label(RichText::new(&tester.status).size(11.5).color(t.faint));
            });
            if let Some(cov) = coverage.as_deref().filter(|_| !tester.session.suite().cases.is_empty()) {
                coverage_line(ui, t, cov);
            }
            ui.add_space(4.0);
            let mut load_case = None;
            let mut accept = None;
            let mut delete = None;
            egui::ScrollArea::vertical().id_salt("suite").auto_shrink([false, false]).show(ui, |ui| {
                if tester.session.suite().cases.is_empty() {
                    ui.label(RichText::new("No cases yet. Fill in the inputs, check the result, then add them as a case.").color(t.muted));
                }
                egui::Grid::new("suite_grid").num_columns(5).spacing([14.0, 6.0]).show(ui, |ui| {
                    for (i, (c, r)) in tester.session.suite().cases.iter().zip(results.iter()).enumerate() {
                        if r.pass {
                            pill(ui, format!("{} pass", ic::CHECK), t.accent_soft, t.accent);
                        } else {
                            pill(ui, format!("{} fail", ic::X), t.error, hex(0xffffff));
                        }
                        ui.label(RichText::new(&c.name).color(t.text));
                        let fired = if r.outcome.fired.is_empty() { "no rule".to_string() } else { format!("rule {}", lint::rules_label(&r.outcome.fired)) };
                        ui.label(RichText::new(fired).size(12.0).color(t.faint));
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            if small_button(ui, t, ic::IMPORT, "Load").on_hover_text("Copy this case's inputs into the tester").clicked() {
                                load_case = Some(i);
                            }
                            if !r.pass && small_button(ui, t, ic::CHECK, "Accept").on_hover_text("Use the actual result as the expected result").clicked() {
                                accept = Some(i);
                            }
                            if small_button(ui, t, ic::TRASH, "").on_hover_text("Delete case").clicked() {
                                delete = Some(i);
                            }
                        });
                        ui.label(RichText::new(r.problems.join("; ")).size(12.0).color(t.error_text));
                        ui.end_row();
                    }
                });
            });
            if let Some(i) = load_case {
                tester.session.fill_from_case(&l.defs, decision, i);
            }
            if let Some(i) = accept {
                tester.session.accept(&l.defs, decision, i);
            }
            if let Some(i) = delete {
                tester.session.delete(i);
            }
        });
    });

    egui::CentralPanel::default().frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 8, right: 20, top: 16, bottom: 8 })).show(ui, |ui| {
        card(ui, t, |ui| {
            ui.set_min_height(ui.available_height());
            egui::ScrollArea::vertical().id_salt("result").auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(bold("Result", 15.0).color(t.text));
                    pill(ui, &tb.hit_policy, t.chip, t.muted);
                    if outcome.error.is_none() {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for &r in &outcome.fired {
                            if chip(ui, &format!("rule {}", r + 1), t.accent_soft, t.accent).interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                acts.push(Act::Jump(r));
                            }
                        }
                    }
                });
                ui.add_space(6.0);
                if let Some(e) = &outcome.error {
                    Frame::new().fill(if t.dark { hex(0x3a1f22) } else { hex(0xfdeeee) }).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(bold("Evaluation fails in Camunda", 12.5).color(t.error_text));
                        ui.add(egui::Label::new(RichText::new(e).size(12.5).color(t.error_text)).wrap());
                    });
                } else if outcome.fired.is_empty() {
                    ui.label(RichText::new("No rule matched, so every output is null.").color(t.warn_text));
                }
                if outcome.error.is_none() {
                    let rows: Vec<Vec<Value>> = if outcome.rows.is_empty() { vec![vec![Value::Null; tb.outputs.len()]] } else { outcome.rows.clone() };
                    ui.add_space(4.0);
                    egui::Grid::new("out_grid").num_columns(3).spacing([18.0, 8.0]).show(ui, |ui| {
                        for (ri, row) in rows.iter().enumerate() {
                            for (oi, out) in tb.outputs.iter().enumerate() {
                                let v = row.get(oi).cloned().unwrap_or(Value::Null);
                                let name = if rows.len() > 1 { format!("{} #{}", out.title(), ri + 1) } else { out.title().to_string() };
                                ui.label(RichText::new(name).color(t.muted));
                                if v == Value::Null {
                                    ui.label(RichText::new("null").font(FontId::monospace(13.0)).color(t.faint));
                                } else {
                                    chip(ui, &v.display(), t.chip, t.text);
                                }
                                let mismatch = out.type_ref.as_deref().is_some_and(|x| x.eq_ignore_ascii_case("string")) && !matches!(v, Value::Str(_) | Value::Null);
                                ui.label(RichText::new(format!("{} / {}", v.type_name(), out.type_ref.as_deref().unwrap_or("-"))).size(11.5).color(if mismatch { t.error_text } else { t.faint }));
                                ui.end_row();
                            }
                        }
                        if let Some(a) = &outcome.aggregate {
                            ui.label(RichText::new(format!("{} ({})", tb.outputs.first().map(|o| o.title()).unwrap_or("result"), tb.aggregation.clone().unwrap_or_default())).color(t.muted));
                            chip(ui, &a.display(), t.chip, t.text);
                            ui.label(RichText::new(a.type_name()).size(11.5).color(t.faint));
                            ui.end_row();
                        }
                    });
                }
                ui.add_space(14.0);
                ui.label(bold(format!("Rule trace  ·  {} matched", outcome.matched.len()), 12.5).color(t.text));
                ui.add_space(4.0);
                egui::Grid::new("trace").num_columns(3).spacing([12.0, 5.0]).show(ui, |ui| {
                    for (ri, st) in outcome.status.iter().enumerate() {
                        if ui
                            .add(egui::Label::new(RichText::new(format!("rule {}", ri + 1)).size(12.5).color(t.accent)).sense(Sense::click()))
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            acts.push(Act::Jump(ri));
                        }
                        match st {
                            RuleStatus::Matched => {
                                let f = outcome.fired.contains(&ri);
                                ui.label(RichText::new(format!("{} {}", ic::CHECK, if f { "fired" } else { "matched" })).size(12.5).color(t.accent));
                                ui.label(RichText::new(if f { "" } else { "not used by the hit policy" }).size(12.0).color(t.faint));
                            }
                            RuleStatus::Failed(ci) => {
                                ui.label(RichText::new("no match").size(12.0).color(t.faint));
                                let entry = tb.rules[ri].inputs.get(*ci).map(|e| one_line(&e.text)).unwrap_or_default();
                                let val = outcome.inputs.get(*ci).map(|v| v.display()).unwrap_or_default();
                                ui.label(RichText::new(format!("{} = {val} does not satisfy {entry}", tb.inputs[*ci].title())).size(12.0).color(t.muted));
                            }
                            RuleStatus::Error(e) => {
                                ui.label(RichText::new("error").size(12.0).color(t.error_text));
                                ui.label(RichText::new(e).size(12.0).color(t.error_text));
                            }
                        }
                        ui.end_row();
                    }
                });
            });
        });
    });
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        let mut acts = Vec::new();
        let dirty = self.loaded.as_ref().is_some_and(|l| l.dirty());
        if ctx.input(|i| i.viewport().close_requested()) && dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.review = None;
            self.fixing = None;
            self.pending = Some(Pending::Close);
        }
        self.refresh_fixable();
        // Fix session keys, when no text box has focus: Enter apply, S skip, Esc stop, arrows choose.
        if self.fixing.is_some() && self.review.is_none() && self.pending.is_none() && ctx.memory(|m| m.focused().is_none()) {
            let step = self.fixing.as_ref().is_some_and(|s| s.current().is_some());
            let key = |k| ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, k));
            if step {
                if key(egui::Key::Enter) {
                    acts.push(Act::FixApply);
                } else if key(egui::Key::S) {
                    acts.push(Act::FixSkip);
                } else if key(egui::Key::Escape) {
                    acts.push(Act::FixStop);
                } else if key(egui::Key::ArrowDown) {
                    acts.push(Act::FixMove { down: true });
                } else if key(egui::Key::ArrowUp) {
                    acts.push(Act::FixMove { down: false });
                }
            } else if key(egui::Key::Escape) {
                acts.push(Act::FixClose);
            }
        }
        // While the review panel is up the document must not change, so other shortcuts wait;
        // a fix session changes it only through its own steps.
        let reviewing = self.review.is_some() || self.fixing.is_some();
        if let Some(p) = dropped.into_iter().next().filter(|_| !reviewing) {
            self.guard(&ctx, Pending::Load(p));
        }
        if !reviewing && self.loaded.is_some() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::D)) {
            acts.push(Act::Review);
        }
        if !reviewing && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            if let Some(ed) = &self.editing {
                acts.push(Act::EndEdit { at: ed.at, commit: true });
            }
            acts.push(Act::Save);
        }
        if self.review.is_none() && ctx.memory(|m| m.focused().is_none()) {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)) {
                acts.push(Act::Redo);
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
                acts.push(Act::Undo);
            }
        }
        if !reviewing && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            acts.push(Act::Open);
        }
        if !reviewing && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
            acts.push(Act::Reload);
        }
        if !reviewing && self.selected.is_some() && ctx.memory(|m| m.focused().is_none()) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            acts.push(Act::Deselect);
        }
        let t = self.theme;

        let suite_stats = self.loaded.as_ref().and_then(|l| {
            if self.tester.session.suite().cases.is_empty() {
                return None;
            }
            let results = self.tester.session.results(&l.defs, self.decision)?;
            Some((results.iter().filter(|r| r.pass).count(), results.len()))
        });

        egui::Panel::left("nav")
            .resizable(false)
            .default_size(224.0)
            .frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 12, right: 12, top: 16, bottom: 12 }))
            .show(ui, |ui| self.nav(ui, suite_stats, &mut acts));

        if let Some(e) = self.error.clone() {
            egui::Panel::top("err").frame(Frame::new().fill(if t.dark { hex(0x3a1f22) } else { hex(0xfdeeee) }).inner_margin(Margin::symmetric(20, 8))).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(e).color(t.error_text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new(ic::X).color(t.error_text)).frame(false)).clicked() {
                            self.error = None;
                        }
                    });
                });
            });
        }

        if self.loaded.is_none() {
            self.empty_view(ui, &mut acts);
            self.apply(&ctx, acts);
            return;
        }

        egui::Panel::top("header").exact_size(60.0).frame(Frame::new().fill(t.surface).inner_margin(Margin::symmetric(20, 0))).show(ui, |ui| self.header(ui, &mut acts));

        if let (Some(s), Some(l)) = (self.fixing.as_mut(), self.loaded.as_ref()) {
            let skipped = self.fix_skipped.len();
            let suite = self.tester.session.suite();
            egui::Panel::right("fix_panel")
                .resizable(true)
                .default_size(400.0)
                .frame(Frame::new().fill(t.surface).inner_margin(Margin::same(0)))
                .show(ui, |ui| fix_panel(ui, &t, s, l, suite, skipped, &mut acts));
        }

        match self.view {
            View::Rules => self.rules_view(ui, &mut acts),
            View::Issues => self.issues_view(ui, &mut acts),
            View::Xml => self.xml_view(ui),
            View::Test => {
                if let Some(l) = self.loaded.as_ref() {
                    test_view(ui, l, self.decision, self.selected, &mut self.tester, &t, &mut acts);
                }
            }
        }
        self.apply(&ctx, acts);
        self.header_window(&ctx);
        self.review_modal(&ctx);
        self.pending_modal(&ctx);
    }
}

#[derive(Default)]
struct Args {
    path: Option<PathBuf>,
    view: Option<View>,
    case: Option<usize>,
    rule: Option<usize>,
}

fn parse_args() -> Args {
    let mut a = Args::default();
    let mut it = std::env::args_os().skip(1);
    while let Some(x) = it.next() {
        match x.to_string_lossy().as_ref() {
            "--tab" | "--view" => {
                a.view = it.next().and_then(|v| match v.to_string_lossy().to_ascii_lowercase().as_str() {
                    "table" | "rules" => Some(View::Rules),
                    "issues" => Some(View::Issues),
                    "test" => Some(View::Test),
                    "xml" => Some(View::Xml),
                    _ => None,
                })
            }
            "--case" => a.case = it.next().and_then(|v| v.to_string_lossy().parse().ok()),
            "--rule" => a.rule = it.next().and_then(|v| v.to_string_lossy().parse().ok()),
            "--dark" => {}
            _ => a.path = Some(PathBuf::from(x)),
        }
    }
    a
}

fn main() -> eframe::Result {
    let args = parse_args();
    let dark = std::env::args().any(|a| a == "--dark");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DMN Viewer")
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).unwrap_or_default())
            .with_inner_size([1440.0, 880.0])
            .with_min_inner_size([900.0, 500.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "DMN Viewer",
        options,
        Box::new(move |cc| {
            let mut app = App::new(cc, args);
            if dark {
                app.theme = Theme::dark();
                apply_theme(&cc.egui_ctx, &app.theme);
            }
            Ok(Box::new(app))
        }),
    )
}
