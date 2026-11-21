#![windows_subsystem = "windows"]

mod memo;
mod menu;
mod tabs;

use dmn_viewer::diff::{self, Hunk};
use dmn_viewer::edit::Edit;
use dmn_viewer::eval::{RuleStatus, Value};
use dmn_viewer::fix;
use dmn_viewer::lint::{self, Column as Col, Issue, Severity};
use dmn_viewer::model::*;
use dmn_viewer::suite::Coverage;
use dmn_viewer::unary::UnaryTests;
use dmn_viewer::workspace::{Cursor, Editing, FixView, View, Workspace};
use eframe::egui::{self, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Layout, Margin, RichText, Sense, Stroke, StrokeKind, TextWrapMode, vec2};
use egui_extras::{Column, TableBuilder};
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
    pub const SAVE_AS: &str = "\u{EAF4}";
    pub const EXPORT: &str = "\u{EAF0}";
    pub const CLOCK: &str = "\u{E1A0}";
    pub const UNDO: &str = "\u{E08A}";
    pub const REDO: &str = "\u{E08C}";
    pub const UNDO_ALL: &str = "\u{E038}";
    pub const REPLACE: &str = "\u{E83C}";
    pub const ZOOM_IN: &str = "\u{E310}";
    pub const ZOOM_OUT: &str = "\u{E30E}";
    pub const ROWS: &str = "\u{E5A2}";
    pub const SIDEBAR: &str = "\u{EC24}";
    pub const DIFF: &str = "\u{E27C}";
    pub const COMMAND: &str = "\u{E1C4}";
    pub const INFO: &str = "\u{E2CE}";
    pub const KEYBOARD: &str = "\u{E2D8}";
    pub const LIST: &str = "\u{E2F0}";
    pub const LAYOUT: &str = "\u{E434}";
    pub const CLIPBOARD: &str = "\u{E198}";
    pub const PASTE: &str = "\u{E198}";
    pub const UP: &str = "\u{E52E}";
    pub const DOWN: &str = "\u{E518}";
    pub const ROW_ABOVE: &str = "\u{E52E}";
    pub const NOTE: &str = "\u{E348}";
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

struct BadgeShape {
    font: FontId,
    pad: f32,
    height: f32,
    radius: u8,
    max_w: f32,
}

fn badge_shape(ui: &mut egui::Ui, text: String, shape: BadgeShape, bg: Color32, fg: Color32) -> egui::Response {
    let BadgeShape { font, pad, height, radius, max_w } = shape;
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
    badge_shape(ui, text.into(), BadgeShape { font: FontId::new(11.5, bold_family()), pad: 9.0, height: 22.0, radius: 11, max_w: 600.0 }, bg, fg)
}

fn chip(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32) -> egui::Response {
    let max_w = ui.available_width().max(40.0);
    badge_shape(ui, text.to_string(), BadgeShape { font: FontId::monospace(11.5), pad: 7.0, height: 22.0, radius: 5, max_w }, bg, fg)
}

fn coverage_badge(ui: &mut egui::Ui, t: &Theme, cov: &Coverage, ri: usize) {
    let (pass, fail) = cov.fired_by.get(ri).copied().unwrap_or_default();
    let (bg, fg, tip) = match (pass, fail) {
        (0, 0) => (t.chip, t.faint, "Not covered: no test case fires this rule".to_string()),
        (0, f) => (t.error, hex(0xffffff), format!("Covered only by {f} failing test case{}", if f == 1 { "" } else { "s" })),
        (p, f) => (t.accent_soft, t.accent, format!("Covered by {p} passing{} test case{}", if f > 0 { format!(" and {f} failing") } else { String::new() }, if p + f == 1 { "" } else { "s" })),
    };
    badge_shape(ui, (pass + fail).to_string(), BadgeShape { font: FontId::monospace(10.0), pad: 4.0, height: 16.0, radius: 8, max_w: 40.0 }, bg, fg).on_hover_text(tip);
}

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

fn glyph(ui: &egui::Ui, text: &str, font: FontId) -> Arc<egui::Galley> {
    ui.painter().layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER)
}

fn badge_glyph(ui: &egui::Ui, text: &str) -> Arc<egui::Galley> {
    glyph(ui, text, FontId::new(11.0, bold_family()))
}

struct Label {
    plain: Arc<egui::Galley>,
    bold: Arc<egui::Galley>,
}

impl Label {
    fn new(ui: &egui::Ui, text: &str) -> Label {
        Label { plain: glyph(ui, text, FontId::proportional(13.5)), bold: glyph(ui, text, FontId::new(13.5, bold_family())) }
    }

    fn pick(&self, active: bool) -> &Arc<egui::Galley> {
        if active { &self.bold } else { &self.plain }
    }
}

struct Glyphs {
    ppp: f32,
    brand: Arc<egui::Galley>,
    grid: Arc<egui::Galley>,
    decisions: Arc<egui::Galley>,
    views: Arc<egui::Galley>,
    decision: Arc<egui::Galley>,
    note: Arc<egui::Galley>,
    note_small: Arc<egui::Galley>,
    items: [(Arc<egui::Galley>, Label); 5],
    notes_badge: Option<(usize, Arc<egui::Galley>)>,
}

impl Glyphs {
    fn new(ui: &egui::Ui, ppp: f32) -> Glyphs {
        let icon = |s: &str| glyph(ui, s, FontId::proportional(16.0));
        let group = |s: &str| glyph(ui, s, FontId::new(10.5, bold_family()));
        Glyphs {
            ppp,
            brand: glyph(ui, "DMN Viewer", FontId::new(14.5, bold_family())),
            grid: glyph(ui, ic::GRID, FontId::proportional(14.0)),
            decisions: group("DECISIONS"),
            views: group("VIEW"),
            decision: icon(ic::DECISION),
            note: glyph(ui, ic::NOTE, FontId::proportional(14.0)),
            note_small: glyph(ui, ic::NOTE, FontId::proportional(13.0)),
            items: [
                (icon(ic::TABLE), Label::new(ui, "Rules")),
                (icon(ic::WARNING), Label::new(ui, "Issues")),
                (icon(ic::FLASK), Label::new(ui, "Test")),
                (icon(ic::CODE), Label::new(ui, "XML")),
                (icon(ic::NOTE), Label::new(ui, "Notes")),
            ],
            notes_badge: None,
        }
    }
}

struct NavRow {
    name: Label,
    badge: Arc<egui::Galley>,
    alert: bool,
}

#[derive(Default)]
struct NavCache {
    key: Option<(u64, f32)>,
    rows: Vec<NavRow>,
    tally: Vec<lint::Tally>,
    global: lint::Tally,
    issues: Option<(usize, Arc<egui::Galley>, bool)>,
    tests: Option<((usize, usize), Arc<egui::Galley>)>,
}

impl NavCache {
    fn sync(&mut self, ui: &egui::Ui, l: &Workspace, ppp: f32) {
        let key = (l.revision(), ppp);
        if self.key == Some(key) {
            return;
        }
        let defs = l.defs();
        let (tally, global) = lint::tally(l.issues(), defs.decisions.len());
        self.rows = defs
            .decisions
            .iter()
            .zip(&tally)
            .map(|(d, ty)| {
                let count = match (&d.logic, ty.errors) {
                    (_, e) if e > 0 => e.to_string(),
                    (DecisionLogic::Table(tb), _) => tb.rules.len().to_string(),
                    (DecisionLogic::Literal { .. }, _) => "expr".into(),
                    (DecisionLogic::None, _) => "-".into(),
                };
                NavRow { name: Label::new(ui, if d.name.is_empty() { &d.id } else { &d.name }), badge: badge_glyph(ui, &count), alert: ty.errors > 0 }
            })
            .collect();
        self.tally = tally;
        self.global = global;
        self.issues = None;
        if self.key.is_some_and(|(_, p)| p != ppp) {
            self.tests = None;
        }
        self.key = Some(key);
    }

    fn issue_badge(&mut self, ui: &egui::Ui, decision: usize) -> (Arc<egui::Galley>, bool) {
        if self.issues.as_ref().is_none_or(|(d, ..)| *d != decision) {
            let ty = self.tally.get(decision).copied().unwrap_or_default().with(self.global);
            self.issues = Some((decision, badge_glyph(ui, &ty.issues.to_string()), ty.errors > 0));
        }
        self.issues.as_ref().map_or_else(|| (badge_glyph(ui, "0"), false), |(_, g, e)| (g.clone(), *e))
    }

    fn test_badge(&mut self, ui: &egui::Ui, stats: Option<(usize, usize)>) -> Option<(Arc<egui::Galley>, bool)> {
        let Some(s) = stats else {
            self.tests = None;
            return None;
        };
        if self.tests.as_ref().is_none_or(|(k, _)| *k != s) {
            self.tests = Some((s, badge_glyph(ui, &format!("{}/{}", s.0, s.1))));
        }
        self.tests.as_ref().map(|(_, g)| (g.clone(), s.0 == s.1))
    }
}

fn nav_item(
    ui: &mut egui::Ui,
    t: &Theme,
    icon: &Arc<egui::Galley>,
    label: &Arc<egui::Galley>,
    badge: Option<(&Arc<egui::Galley>, Color32, Color32)>,
    active: bool,
    mark: Option<&Arc<egui::Galley>>,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    let p = ui.painter();
    if active {
        p.rect(rect, CornerRadius::same(7), t.surface, Stroke::new(1.0, t.line), StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(7), if t.dark { t.hover } else { hex(0xeceef2) });
    }
    let y = rect.center().y;
    p.galley(egui::pos2(rect.left() + 10.0, y - icon.size().y / 2.0), icon.clone(), if active { t.accent } else { t.muted });
    p.galley(egui::pos2(rect.left() + 34.0, y - label.size().y / 2.0), label.clone(), if active { t.text } else { t.muted });
    let mut right = rect.right() - 8.0;
    if let Some((galley, bg, color)) = badge {
        let w = galley.size().x + 14.0;
        let r = egui::Rect::from_center_size(egui::pos2(right - w / 2.0, y), vec2(w, 18.0));
        p.rect_filled(r, CornerRadius::same(9), bg);
        p.galley(r.center() - galley.size() / 2.0, galley.clone(), color);
        right = r.left() - 4.0;
    }
    if let Some(g) = mark {
        p.galley(egui::pos2(right - g.size().x, y - g.size().y / 2.0), g.clone(), t.warn_text);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn group_glyph(ui: &mut egui::Ui, t: &Theme, g: &Arc<egui::Galley>) {
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(r.min, g.clone(), t.faint);
}

fn memo_preview(ui: &mut egui::Ui, t: &Theme, f: &memo::File, ws: Option<&Workspace>) {
    ui.separator();
    ui.set_max_width(320.0);
    for m in f.memos.iter().take(4) {
        let name = ws.and_then(|l| l.defs().decisions.iter().find(|d| d.id == *m.id)).map_or(&*m.id, |d| if d.name.is_empty() { d.id.as_str() } else { d.name.as_str() });
        ui.horizontal_top(|ui| {
            ui.label(RichText::new(ic::NOTE).color(t.warn_text));
            ui.label(RichText::new(format!("{name}: {}", m.text.lines().next().unwrap_or_default())).color(t.text));
        });
    }
    if f.memos.len() > 4 {
        ui.label(RichText::new(format!("and {} more", f.memos.len() - 4)).color(t.muted));
    }
}

fn card<R>(ui: &mut egui::Ui, t: &Theme, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    Frame::new().fill(t.surface).stroke(Stroke::new(1.0, t.line)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(14)).show(ui, add)
}

#[derive(Default)]
struct Tester {
    case_name: String,
    scroll_target: bool,
}

#[derive(Clone)]
enum Pending {
    Reload,
    CloseTab(usize),
}

struct Quit {
    rows: Vec<(u64, bool)>,
    picked: usize,
    label: std::borrow::Cow<'static, str>,
    error: Option<String>,
}

impl Quit {
    fn new(uids: Vec<u64>) -> Quit {
        let n = uids.len();
        Quit { rows: uids.into_iter().map(|u| (u, true)).collect(), picked: n, label: tabs::quit_label(n, n), error: None }
    }

    fn recount(&mut self) {
        self.picked = self.rows.iter().filter(|r| r.1).count();
        self.label = tabs::quit_label(self.picked, self.rows.len());
    }
}

struct MemoEdit {
    key: PathBuf,
    path: PathBuf,
    id: Arc<str>,
    title: String,
    text: String,
    focus: bool,
}

struct MemoBar {
    revision: u64,
    uid: u64,
    decision: usize,
    width: f32,
    galley: Arc<egui::Galley>,
}

enum Quitting {
    Save,
    Discard,
    Cancel,
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

const TYPES: [&str; 7] = ["string", "integer", "long", "double", "boolean", "date", "number"];

fn rule_menu(ui: &mut egui::Ui, decision: usize, ri: usize, nrules: usize, acts: &mut Vec<Act>) {
    if ui.button("Test this rule").on_hover_text("Fill the test inputs with values that make this rule fire, then open the Test tab").clicked() {
        acts.push(Act::TestRule(ri));
        ui.close();
    }
    ui.separator();
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

fn edit_box(ui: &mut egui::Ui, ed: &mut Editing, acts: &mut Vec<Act>) {
    let id = ui.id().with("inline_edit");
    if ui.memory(|m| m.has_focus(id)) {
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

enum Act {
    Select(usize),
    Jump(usize),
    TestRule(usize),
    View(View),
    Decision(usize),
    Open,
    Reload,
    Tab(usize),
    StepTab { forward: bool },
    CloseTab(usize),
    MoveTab { from: usize, to: usize },
    TabDropped,
    ToggleDark,
    ToggleLangs,
    Deselect,
    SevFilter(Option<Severity>),
    Edit(Edit),
    BeginEdit(Cursor, String),
    EndEdit { at: Cursor, commit: bool },
    EditHeader(Col),
    Save,
    Undo,
    Redo,
    Review,
    FixStart(Option<String>),
    FixChoose(usize),
    FixMove { down: bool },
    FixApply,
    FixSkip,
    FixStop,
    FixUndo,
    FixUndoAll,
    FixClose,
    FixReview,
    FixResetSkipped,
    Memo,
    MemoOpen,
    MemoJump(PathBuf, Arc<str>),
    Notes,
    Cmd(menu::Cmd),
}

struct Review {
    hunks: Vec<Hunk>,
    lines: Vec<(usize, usize)>,
}

const REVIEW_HUNKS: usize = 300;
const REVIEW_LINES: usize = 40;

const DETAIL_DELAY: f64 = 0.32;

struct Derived {
    rule: Vec<Option<Severity>>,
    cell: Vec<Option<Severity>>,
    notes: Vec<String>,
}

#[derive(Default)]
struct RuleCache {
    filter: String,
    visible: Option<(String, usize, u64, Arc<Vec<usize>>)>,
    derived: Option<(usize, u64, Arc<Derived>)>,
}

struct Tab {
    uid: u64,
    key: PathBuf,
    path: PathBuf,
    label: Arc<str>,
    title: Option<(f32, f32, Arc<egui::Galley>)>,
    ws: Option<Workspace>,
    tester: Tester,
    rules: RuleCache,
    sev_filter: Option<Severity>,
    header_form: Option<HeaderForm>,
    nav: NavCache,
    crumb: Option<(bool, f32, Arc<egui::Galley>)>,
}

impl Tab {
    fn new(ws: Workspace, key: PathBuf, uid: u64) -> Tab {
        let mut tab = Tab::lazy(ws.path().to_path_buf(), key, uid);
        tab.ws = Some(ws);
        tab
    }

    fn lazy(path: PathBuf, key: PathBuf, uid: u64) -> Tab {
        Tab {
            uid,
            key,
            label: tabs::label(&path),
            path,
            title: None,
            ws: None,
            tester: Tester::default(),
            rules: RuleCache::default(),
            sev_filter: None,
            header_form: None,
            nav: NavCache::default(),
            crumb: None,
        }
    }

    fn dirty(&self) -> bool {
        self.ws.as_ref().is_some_and(Workspace::dirty)
    }

    fn relabel(&mut self) {
        if let Some(ws) = &self.ws {
            self.path = ws.path().to_path_buf();
        }
        self.key = tabs::key(&self.path);
        self.label = tabs::label(&self.path);
        self.title = None;
        self.crumb = None;
    }

    fn title(&mut self, ui: &egui::Ui, max_width: f32) -> Arc<egui::Galley> {
        let ppp = ui.ctx().pixels_per_point();
        match &self.title {
            Some((w, p, g)) if *w == max_width && *p == ppp => g.clone(),
            _ => {
                let mut job = egui::text::LayoutJob::single_section(self.label.to_string(), egui::TextFormat::simple(FontId::proportional(12.5), Color32::PLACEHOLDER));
                job.wrap = egui::text::TextWrapping { max_width, max_rows: 1, break_anywhere: true, overflow_character: Some('\u{2026}') };
                let g = ui.painter().layout_job(job);
                self.title = Some((max_width, ppp, g.clone()));
                g
            }
        }
    }
}

struct App {
    tabs: Vec<Tab>,
    active: usize,
    next_uid: u64,
    reveal_tab: bool,
    error: Option<String>,
    detail_after: f64,
    detail_open: bool,
    show_langs: bool,
    theme: Theme,
    pending: Option<Pending>,
    quit: Option<Quit>,
    allow_close: bool,
    review: Option<Review>,
    memos: memo::Memos,
    memo: Option<MemoEdit>,
    memo_anchor: egui::Rect,
    memo_bar: Option<MemoBar>,
    memo_quiet: bool,
    memo_tick: bool,
    show_notes: bool,
    glyphs: Option<Glyphs>,
    settings: menu::Settings,
    shell: menu::Shell,
}

fn lang_colors(t: &Theme, l: Language) -> (Color32, Color32) {
    match l {
        Language::Js => (t.warn.gamma_multiply(if t.dark { 0.28 } else { 0.2 }), t.warn_text),
        Language::Juel => (t.info.gamma_multiply(if t.dark { 0.28 } else { 0.2 }), t.info_text),
        _ => (t.chip, t.muted),
    }
}

fn lang_describe(lang: &ExpressionLanguage) -> String {
    let source = match lang.source {
        LanguageSource::Cell => "set on cell",
        LanguageSource::FileDefault => "file default",
        LanguageSource::Default => "default, not set",
    };
    format!("{} ({source})", lang.long_name())
}

fn paint_lang_tag(ui: &mut egui::Ui, t: &Theme, lang: &ExpressionLanguage, mask: Color32) {
    let (bg, fg) = if lang.is_explicit() { lang_colors(t, lang.language) } else { (t.chip.gamma_multiply(0.7), t.faint) };
    let galley = ui.painter().layout_no_wrap(lang.short_name().into_owned(), FontId::new(9.5, bold_family()), fg);
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
            tabs: Vec::new(),
            active: 0,
            next_uid: 0,
            reveal_tab: false,
            error: None,
            detail_after: 0.0,
            detail_open: false,
            show_langs: false,
            theme,
            pending: None,
            quit: None,
            allow_close: false,
            review: None,
            memos: memo::Memos::default(),
            memo: None,
            memo_anchor: egui::Rect::NOTHING,
            memo_bar: None,
            memo_quiet: false,
            memo_tick: false,
            show_notes: false,
            glyphs: None,
            settings: menu::Settings::load(),
            shell: menu::Shell::default(),
        };
        if let Some(c) = args.chrome {
            app.settings.chrome = c;
        }
        app.restore_tabs();
        match args.path {
            Some(p) => app.load(p),
            None => app.load_active(),
        }
        if let Some(l) = app.ws_mut() {
            if let Some(v) = args.view {
                l.set_view(v);
            }
            if let Some(r) = args.rule.filter(|r| *r > 0) {
                l.select_and_reveal(r - 1);
            }
            if let Some(n) = args.case {
                l.tests().fill_from_case(n.saturating_sub(1));
            }
        }
        app
    }

    fn tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active)
    }

    fn tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active)
    }

    fn ws(&self) -> Option<&Workspace> {
        self.tab().and_then(|t| t.ws.as_ref())
    }

    fn ws_mut(&mut self) -> Option<&mut Workspace> {
        self.tab_mut().and_then(|t| t.ws.as_mut())
    }

    fn load(&mut self, path: PathBuf) {
        self.open_paths(vec![path]);
    }

    fn open_paths(&mut self, paths: Vec<PathBuf>) {
        let mut target = None;
        let mut known = Vec::new();
        let mut fresh: Vec<PathBuf> = Vec::with_capacity(paths.len());
        let mut keys: Vec<PathBuf> = Vec::with_capacity(paths.len());
        for path in paths {
            let key = tabs::key(&path);
            if let Some(i) = self.tabs.iter().position(|t| t.key == key) {
                target = Some(i);
                known.push(path);
            } else if !keys.contains(&key) {
                keys.push(key);
                fresh.push(path);
            }
        }
        let view = self.ws().map(Workspace::view);
        let mut failed = Vec::new();
        for ((path, key), opened) in fresh.iter().zip(keys).zip(Workspace::open_many(&fresh)) {
            match opened {
                Ok(mut ws) => {
                    if let Some(v) = view {
                        ws.set_view(v);
                    }
                    self.next_uid += 1;
                    self.tabs.push(Tab::new(ws, key, self.next_uid));
                    target = Some(self.tabs.len() - 1);
                    known.push(path.clone());
                }
                Err(e) => failed.push(format!("{}: {e}", path.display())),
            }
        }
        if let Some(i) = target {
            self.activate(i);
        }
        self.persist_tabs();
        self.remember_all(known);
        self.error = match failed.len() {
            0 => None,
            1 => Some(format!("Could not open {}", failed[0])),
            n => Some(format!("Could not open {n} files. {}", failed.join(" \u{b7} "))),
        };
    }

    fn reload(&mut self) {
        let Some(Tab { ws: Some(ws), header_form, .. }) = self.tabs.get_mut(self.active) else { return };
        let path = ws.path().to_path_buf();
        match ws.reload() {
            Ok(()) => {
                *header_form = None;
                self.error = None;
                self.remember(path);
            }
            Err(e) => self.error = Some(format!("Could not open {}: {e}", path.display())),
        }
    }

    fn restore_tabs(&mut self) {
        for path in std::mem::take(&mut self.settings.open) {
            let Ok(key) = std::fs::canonicalize(&path) else { continue };
            if self.tabs.iter().any(|t| t.key == key) {
                continue;
            }
            self.next_uid += 1;
            self.tabs.push(Tab::lazy(path, key, self.next_uid));
        }
        self.active = self.settings.active.min(self.tabs.len().saturating_sub(1));
        self.settings.open = self.tabs.iter().map(|t| t.path.clone()).collect();
    }

    fn persist_tabs(&mut self) {
        let same = self.settings.active == self.active && self.settings.open.len() == self.tabs.len() && self.settings.open.iter().zip(&self.tabs).all(|(p, t)| *p == t.path);
        if !same {
            self.settings.open = self.tabs.iter().map(|t| t.path.clone()).collect();
            self.settings.active = self.active;
            self.settings.save();
        }
    }

    fn load_tab(&mut self, i: usize) -> bool {
        let Some(tab) = self.tabs.get_mut(i) else { return false };
        if tab.ws.is_some() {
            return true;
        }
        match Workspace::open(tab.path.clone()) {
            Ok(ws) => {
                tab.ws = Some(ws);
                true
            }
            Err(e) => {
                self.error = Some(format!("Could not open {}: {e}", tab.path.display()));
                self.tabs.remove(i);
                self.active = tabs::after_close(self.active, i, self.tabs.len());
                self.persist_tabs();
                false
            }
        }
    }

    fn load_active(&mut self) {
        while self.tabs.get(self.active).is_some_and(|t| t.ws.is_none()) {
            self.load_tab(self.active);
        }
    }

    fn activate(&mut self, i: usize) {
        if !self.load_tab(i) {
            self.load_active();
            return;
        }
        if i != self.active {
            self.active = i;
            self.detail_open = false;
        }
        self.reveal_tab = true;
    }

    fn move_tab(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tabs.len() || to >= self.tabs.len() {
            return;
        }
        let uid = self.tabs[self.active].uid;
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        self.active = self.tabs.iter().position(|t| t.uid == uid).unwrap_or(to);
    }

    fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        self.tabs.remove(i);
        if i == self.active {
            self.detail_open = false;
        }
        self.active = tabs::after_close(self.active, i, self.tabs.len());
        self.reveal_tab = true;
        self.error = None;
        self.load_active();
        self.persist_tabs();
    }

    fn open_dialog(&mut self) {
        if let Some(ps) = rfd::FileDialog::new().set_title("Open DMN files").add_filter("DMN", &["dmn", "xml"]).add_filter("All files", &["*"]).pick_files() {
            self.open_paths(ps);
        }
    }

    fn unsaved(&mut self) -> Option<Quit> {
        let mut uids = Vec::new();
        for tab in &mut self.tabs {
            if let Some(ws) = tab.ws.as_mut() {
                ws.flush_editing();
                if ws.dirty() {
                    ws.fix_close();
                    uids.push(tab.uid);
                }
            }
        }
        (!uids.is_empty()).then(|| Quit::new(uids))
    }

    fn request_quit(&mut self, ctx: &egui::Context) {
        if self.quit.is_some() {
            return;
        }
        match self.unsaved() {
            Some(q) => {
                self.review = None;
                self.pending = None;
                self.quit = Some(q);
            }
            None => self.exit(ctx),
        }
    }

    fn exit(&mut self, ctx: &egui::Context) {
        self.quit = None;
        self.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn save_picked(&mut self, ctx: &egui::Context) {
        let Some(q) = self.quit.as_mut() else { return };
        let mut failed = None;
        q.rows.retain(|&(uid, on)| {
            if !on || failed.is_some() {
                return true;
            }
            let Some(ws) = self.tabs.iter_mut().find(|t| t.uid == uid).and_then(|t| t.ws.as_mut()) else { return false };
            if ws.save() {
                return false;
            }
            failed = Some(format!("Could not save {}. {}", tabs::label(ws.path()), ws.error().unwrap_or_default()));
            ws.clear_error();
            true
        });
        match failed {
            Some(e) => {
                q.error = Some(e);
                q.recount();
            }
            None => self.exit(ctx),
        }
    }

    fn guard(&mut self, p: Pending) {
        let dirty = match &p {
            Pending::Reload => self.ws().is_some_and(|l| l.dirty()).then_some(self.active),
            Pending::CloseTab(i) => self.tabs.get(*i).is_some_and(|t| t.dirty() || (!self.memo_quiet && self.memos.count(&t.key) > 0)).then_some(*i),
        };
        self.memo_tick = false;
        match dirty {
            Some(i) => {
                self.activate(i);
                self.pending = Some(match p {
                    Pending::CloseTab(_) => Pending::CloseTab(i),
                    p => p,
                });
            }
            None => self.run(p),
        }
    }

    fn run(&mut self, p: Pending) {
        match p {
            Pending::Reload => self.reload(),
            Pending::CloseTab(i) => self.close_tab(i),
        }
    }

    fn is_fixing(&self) -> bool {
        self.ws().is_some_and(|l| l.is_fixing())
    }

    fn decision(&self) -> usize {
        self.ws().map_or(0, |l| l.decision())
    }

    fn selected(&self) -> Option<usize> {
        self.ws().and_then(|l| l.selected())
    }

    fn view(&self) -> View {
        self.ws().map_or_else(View::default, |l| l.view())
    }

    fn save(&mut self) -> bool {
        self.ws_mut().is_some_and(|l| l.save())
    }

    fn open_review(&mut self) {
        if let Some(l) = self.ws_mut() {
            l.flush_editing();
        }
        let Some(l) = self.ws() else { return };
        let hunks = l.changes();
        let lines = hunks.iter().map(|h| (diff::line_count(&l.saved()[h.old.clone()]), diff::line_count(&l.xml()[h.new.clone()]))).collect();
        self.review = Some(Review { hunks, lines });
    }

    fn review_modal(&mut self, ctx: &egui::Context) {
        let (Some(rv), Some(l)) = (&self.review, self.ws()) else { return };
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
                        side(ui, l.xml().get(h.new.clone()).unwrap_or(""), new_lines, '+', t.accent_soft, t.text);
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
        let (file, dirty, notes) = self.tab().map_or_else(Default::default, |tab| (tab.label.clone(), tab.dirty(), self.memos.count(&tab.key)));
        let keep = (matches!(p, Pending::CloseTab(_)) && notes > 0)
            .then(|| format!("{notes} local note{} stay in memory and come back if you reopen this file before the app closes.", if notes == 1 { "" } else { "s" }));
        let tick = &mut self.memo_tick;
        let mut choice = None;
        egui::Modal::new(egui::Id::new("unsaved_changes")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(bold(if dirty { "Unsaved changes".to_string() } else { format!("Close {file}?") }, 15.0).color(t.text));
            ui.add_space(4.0);
            if dirty {
                ui.label(RichText::new(format!("{file} has changes that are not saved.")).color(t.muted));
            }
            if let Some(k) = &keep {
                ui.label(RichText::new(k.as_str()).color(t.muted));
            }
            if !dirty {
                ui.add_space(6.0);
                ui.checkbox(tick, "Don't ask again until the app closes");
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if dirty {
                    if button(ui, &t, ic::SAVE, "Save", true).clicked() {
                        choice = Some("save");
                    }
                    if button(ui, &t, ic::TRASH, "Discard", false).clicked() {
                        choice = Some("discard");
                    }
                } else if button(ui, &t, ic::X, "Close tab", true).clicked() {
                    choice = Some("close");
                }
                if button(ui, &t, ic::X, "Cancel", false).clicked() {
                    choice = Some("cancel");
                }
            });
        });
        match choice {
            Some("save") if self.save() => {
                self.pending = None;
                self.run(p);
            }
            Some("discard") => {
                self.pending = None;
                if let Some(l) = self.ws_mut() {
                    l.discard();
                }
                self.run(p);
            }
            Some("close") => {
                self.memo_quiet |= self.memo_tick;
                self.pending = None;
                self.run(p);
            }
            Some("cancel") => self.pending = None,
            _ => {}
        }
    }

    fn current_memo(&self) -> Option<&str> {
        let tab = self.tab()?;
        let l = tab.ws.as_ref()?;
        self.memos.get(&tab.key, &l.defs().decisions.get(l.decision())?.id)
    }

    fn memo_toggle(&mut self, keep_open: bool) {
        let Some(Tab { ws: Some(l), key, path, .. }) = self.tabs.get(self.active) else { return };
        let Some(d) = l.defs().decisions.get(l.decision()) else { return };
        if let Some(m) = &self.memo
            && m.key == *key
            && *m.id == *d.id
        {
            if !keep_open {
                self.memo = None;
            }
            return;
        }
        let text = self.memos.get(key, &d.id).unwrap_or_default().to_string();
        let title = if d.name.is_empty() { d.id.clone() } else { d.name.clone() };
        self.memo = Some(MemoEdit { key: key.clone(), path: path.clone(), id: d.id.as_str().into(), title, text, focus: true });
        self.show_notes = false;
    }

    fn memo_jump(&mut self, key: PathBuf, id: &str) {
        match self.tabs.iter().position(|t| t.key == key) {
            Some(i) => self.activate(i),
            None => {
                let path = self.memos.file(&key).map_or_else(|| key.clone(), |f| f.path.clone());
                self.load(path);
            }
        }
        let Some(Tab { ws: Some(l), key: k, .. }) = self.tabs.get_mut(self.active) else { return };
        if *k != key {
            return;
        }
        if let Some(d) = l.defs().decisions.iter().position(|d| d.id == id) {
            l.set_decision(d);
        }
        l.set_view(View::Rules);
        self.show_notes = false;
    }

    fn memo_popover(&mut self, ctx: &egui::Context) {
        let App { memo, memos, tabs, active, theme: t, memo_anchor, .. } = self;
        let Some(m) = memo.as_mut() else { return };
        let current = tabs.get(*active).and_then(|tab| Some((&tab.key, tab.ws.as_ref()?))).and_then(|(k, l)| Some((k, &l.defs().decisions.get(l.decision())?.id)));
        if !current.is_some_and(|(k, id)| *k == m.key && **id == *m.id) {
            *memo = None;
            return;
        }
        let width = 340.0;
        let screen = ctx.content_rect();
        let pos = egui::pos2((memo_anchor.right() - width).clamp(screen.left() + 8.0, (screen.right() - width - 8.0).max(screen.left() + 8.0)), memo_anchor.bottom() + 6.0);
        let mut close = false;
        let mut changed = false;
        let area = egui::Area::new(egui::Id::new("memo_popover")).order(egui::Order::Foreground).fixed_pos(pos).show(ctx, |ui| {
            Frame::popup(ui.style()).inner_margin(Margin::same(12)).show(ui, |ui| {
                ui.set_width(width - 24.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(ic::NOTE).color(t.warn_text));
                    ui.add(egui::Label::new(bold(m.title.as_str(), 13.5).color(t.text)).truncate());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new(ic::X).color(t.muted)).frame(false)).on_hover_text("Close (Esc)").clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(4.0);
                let edit = ui.add(
                    egui::TextEdit::multiline(&mut m.text)
                        .hint_text("Note for this decision. Only you see it, it isn't saved to the file, and it's gone when the app closes.")
                        .desired_rows(6)
                        .desired_width(f32::INFINITY),
                );
                if std::mem::take(&mut m.focus) {
                    edit.request_focus();
                }
                changed = edit.changed();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Kept until the app closes").size(11.5).color(t.faint));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !m.text.is_empty() && small_button(ui, t, ic::TRASH, "Delete").clicked() {
                            m.text.clear();
                            changed = true;
                            close = true;
                        }
                    });
                });
            });
        });
        if changed {
            memos.set(&m.key, &m.path, &m.id, &m.text);
        }
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !memo_anchor.contains(p)));
        if close || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            *memo = None;
        }
    }

    fn memo_strip(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let App { memos, tabs, active, theme: t, memo_bar, .. } = self;
        let Some(tab) = tabs.get(*active) else { return };
        let Some(l) = tab.ws.as_ref() else { return };
        let decision = l.decision();
        let Some(d) = l.defs().decisions.get(decision) else { return };
        let Some(text) = memos.get(&tab.key, &d.id) else { return };
        let width = (ui.available_width() - 26.0).max(40.0).floor();
        let fresh = memo_bar.as_ref().is_some_and(|b| b.revision == memos.revision() && b.uid == tab.uid && b.decision == decision && b.width == width);
        if !fresh {
            let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::TextFormat::simple(FontId::proportional(12.5), Color32::PLACEHOLDER));
            job.wrap = egui::text::TextWrapping { max_width: width, max_rows: 3, break_anywhere: false, overflow_character: Some('\u{2026}') };
            *memo_bar = Some(MemoBar { revision: memos.revision(), uid: tab.uid, decision, width, galley: ui.painter().layout_job(job) });
        }
        let Some(bar) = memo_bar.as_ref() else { return };
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), bar.galley.size().y.max(16.0)), Sense::click());
        let p = ui.painter();
        p.text(egui::pos2(rect.left(), rect.top() + 1.0), Align2::LEFT_TOP, ic::NOTE, FontId::proportional(14.0), t.warn_text);
        p.galley(egui::pos2(rect.left() + 22.0, rect.top()), bar.galley.clone(), t.warn_text);
        if resp.on_hover_text("Local note. Click to edit").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            acts.push(Act::MemoOpen);
        }
    }

    fn notes_view(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let t = &self.theme;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            Frame::new().inner_margin(Margin::symmetric(24, 18)).show(ui, |ui| {
                ui.label(bold("Notes", 16.0).color(t.text));
                ui.label(RichText::new("Only on this machine. Never written to a file, gone when the app closes.").color(t.muted));
                ui.add_space(12.0);
                if self.memos.total() == 0 {
                    ui.label(RichText::new("No notes yet. Use Add note in the header (Ctrl+Shift+N) to jot something about a decision.").color(t.faint));
                    return;
                }
                let open = self.tabs.iter().filter_map(|tab| self.memos.file(&tab.key).map(|f| (f, Some(tab))));
                let closed = self.memos.files().iter().filter(|f| !self.tabs.iter().any(|tab| tab.key == f.key)).map(|f| (f, None));
                for (f, tab) in open.chain(closed) {
                    ui.horizontal(|ui| {
                        let label = tab.map_or_else(|| tabs::label(&f.path), |tab| tab.label.clone());
                        ui.label(bold(label.as_ref(), 13.5).color(t.text)).on_hover_text(f.path.display().to_string());
                        if tab.is_none() {
                            pill(ui, "closed, click a note to reopen", t.chip, t.muted);
                        }
                    });
                    ui.add_space(4.0);
                    let defs = tab.and_then(|tab| tab.ws.as_ref()).map(Workspace::defs);
                    for memo in &f.memos {
                        let dec = defs.and_then(|defs| defs.decisions.iter().find(|d| d.id == *memo.id));
                        let name = dec.map_or(&*memo.id, |d| if d.name.is_empty() { d.id.as_str() } else { d.name.as_str() });
                        let card = Frame::new().fill(t.surface).stroke(Stroke::new(1.0, t.line)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(ic::NOTE).color(t.warn_text));
                                ui.label(bold(name, 13.0).color(t.text));
                                if defs.is_some() && dec.is_none() {
                                    ui.label(RichText::new("decision not in the file any more").size(11.5).color(t.faint));
                                }
                            });
                            ui.add(egui::Label::new(RichText::new(memo.text.as_str()).color(t.muted)).wrap_mode(TextWrapMode::Wrap));
                        });
                        let resp = card.response.interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                        if resp.hovered() {
                            ui.painter().rect_stroke(resp.rect, CornerRadius::same(8), Stroke::new(1.0, t.accent), StrokeKind::Inside);
                        }
                        if resp.clicked() {
                            acts.push(Act::MemoJump(f.key.clone(), memo.id.clone()));
                        }
                        ui.add_space(6.0);
                    }
                    ui.add_space(10.0);
                }
            });
        });
    }

    fn quit_modal(&mut self, ctx: &egui::Context) {
        let Some(q) = self.quit.as_mut() else { return };
        let t = self.theme;
        let tabs = &self.tabs;
        let mut choice = None;
        let mut toggled = false;
        let modal = egui::Modal::new(egui::Id::new("quit_unsaved")).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.label(bold("Unsaved changes", 15.0).color(t.text));
            ui.add_space(4.0);
            ui.label(
                RichText::new(if q.rows.len() == 1 {
                    "This file has changes that are not saved. Unticked files are discarded. Local notes are never saved."
                } else {
                    "These files have changes that are not saved. Unticked files are discarded. Local notes are never saved."
                })
                .color(t.muted),
            );
            ui.add_space(8.0);
            if q.rows.len() > 1 {
                let (mut all, some) = (q.picked == q.rows.len(), q.picked > 0 && q.picked < q.rows.len());
                if ui.add(egui::Checkbox::new(&mut all, RichText::new("All files").color(t.muted)).indeterminate(some)).changed() {
                    q.rows.iter_mut().for_each(|r| r.1 = all);
                    toggled = true;
                }
                ui.separator();
            }
            egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
                for (uid, on) in &mut q.rows {
                    let Some(tab) = tabs.iter().find(|t| t.uid == *uid) else { continue };
                    let r = ui.checkbox(on, tab.label.as_ref()).on_hover_ui(|ui| {
                        ui.label(tab.path.display().to_string());
                    });
                    toggled |= r.changed();
                }
            });
            if let Some(e) = &q.error {
                ui.add_space(6.0);
                ui.label(RichText::new(e.as_str()).color(t.error_text));
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if button(ui, &t, if q.picked == 0 { ic::TRASH } else { ic::SAVE }, &q.label, true).clicked() {
                    choice = Some(if q.picked == 0 { Quitting::Discard } else { Quitting::Save });
                }
                if q.picked > 0 && button(ui, &t, ic::TRASH, "Discard all", false).clicked() {
                    choice = Some(Quitting::Discard);
                }
                if button(ui, &t, ic::X, "Cancel", false).on_hover_text("Esc").clicked() {
                    choice = Some(Quitting::Cancel);
                }
            });
        });
        if toggled {
            q.recount();
        }
        if modal.should_close() {
            choice = choice.or(Some(Quitting::Cancel));
        }
        match choice {
            Some(Quitting::Save) => self.save_picked(ctx),
            Some(Quitting::Discard) => self.exit(ctx),
            Some(Quitting::Cancel) => self.quit = None,
            None => {}
        }
    }

    fn header_window(&mut self, ctx: &egui::Context) {
        let t = self.theme;
        let decision = self.decision();
        let Some(tab) = self.tabs.get_mut(self.active) else { return };
        let Some(form) = tab.header_form.as_mut() else { return };
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
                            for (label, v) in Language::CHOICES {
                                ui.selectable_value(lang, v.unwrap_or_default().to_string(), label);
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
                tab.header_form = None;
                if let Some(ws) = tab.ws.as_mut() {
                    ws.commit(edit);
                }
            }
            Some(false) => tab.header_form = None,
            None => {}
        }
    }

    fn apply(&mut self, ctx: &egui::Context, acts: Vec<Act>) {
        for a in acts {
            match a {
                Act::Select(r) => {
                    if let Some(l) = self.ws_mut() {
                        let opening = l.selected().is_none();
                        l.select(r);
                        if opening {
                            self.detail_after = ctx.input(|i| i.time) + DETAIL_DELAY;
                            ctx.request_repaint_after(std::time::Duration::from_secs_f64(DETAIL_DELAY));
                        }
                    }
                }
                Act::Jump(r) => {
                    if let Some(Tab { ws: Some(ws), rules, .. }) = self.tabs.get_mut(self.active) {
                        ws.jump(r);
                        rules.filter.clear();
                    }
                }
                Act::View(v) => {
                    self.show_notes = false;
                    if let Some(l) = self.ws_mut() {
                        l.set_view(v);
                    }
                }
                Act::TestRule(r) => {
                    if let Some(Tab { ws: Some(l), tester, .. }) = self.tabs.get_mut(self.active) {
                        if l.tests().fill_to_fire(r) {
                            l.select_and_reveal(r);
                            l.set_view(View::Test);
                            tester.scroll_target = true;
                        } else {
                            self.error = Some(format!("Could not find inputs that make rule {} fire.", r + 1));
                        }
                    }
                }
                Act::Decision(d) => {
                    self.show_notes = false;
                    if let Some(l) = self.ws_mut() {
                        l.set_decision(d);
                    }
                }
                Act::Open => self.open_dialog(),
                Act::Memo => self.memo_toggle(false),
                Act::MemoOpen => self.memo_toggle(true),
                Act::MemoJump(key, id) => self.memo_jump(key, &id),
                Act::Notes => {
                    self.show_notes = true;
                    self.memo = None;
                }
                Act::Tab(i) => self.activate(i),
                Act::StepTab { forward } => self.activate(tabs::step(self.active, self.tabs.len(), forward)),
                Act::CloseTab(i) => self.guard(Pending::CloseTab(i)),
                Act::MoveTab { from, to } => self.move_tab(from, to),
                Act::TabDropped => self.persist_tabs(),
                Act::Reload => self.guard(Pending::Reload),
                Act::ToggleDark => {
                    self.theme = if self.theme.dark { Theme::light() } else { Theme::dark() };
                    apply_theme(ctx, &self.theme);
                }
                Act::SevFilter(s) => {
                    if let Some(tab) = self.tab_mut() {
                        tab.sev_filter = s;
                    }
                }
                Act::ToggleLangs => self.show_langs = !self.show_langs,
                Act::Deselect => {
                    if let Some(l) = self.ws_mut() {
                        l.deselect();
                    }
                }
                Act::Edit(e) => {
                    if let Some(l) = self.ws_mut() {
                        l.commit(e);
                    }
                }
                Act::BeginEdit(at, text) => {
                    if let Some(l) = self.ws_mut() {
                        l.begin_edit(at, text);
                    }
                }
                Act::EndEdit { at, commit } => {
                    if let Some(l) = self.ws_mut() {
                        l.end_edit(at, commit);
                    }
                }
                Act::EditHeader(c) => {
                    if let Some(Tab { ws: Some(ws), header_form, .. }) = self.tabs.get_mut(self.active) {
                        *header_form = ws.defs().table(ws.decision()).and_then(|t| HeaderForm::new(t, c));
                    }
                }
                Act::Save => {
                    self.save();
                }
                Act::Undo => {
                    if let Some(l) = self.ws_mut() {
                        l.undo();
                    }
                }
                Act::Redo => {
                    if let Some(l) = self.ws_mut() {
                        l.redo();
                    }
                }
                Act::Review => self.open_review(),
                Act::Cmd(c) => self.run_cmd(ctx, c),
                fix => self.fix_act(fix),
            }
        }
    }

    fn fix_act(&mut self, a: Act) {
        let Some(Tab { ws: Some(l), header_form, rules, .. }) = self.tabs.get_mut(self.active) else { return };
        match a {
            Act::FixStart(only) => {
                if l.fix_start(only) {
                    *header_form = None;
                    rules.filter.clear();
                }
            }
            Act::FixChoose(i) => l.fix_choose(i),
            Act::FixMove { down } => l.fix_move(down),
            Act::FixApply => l.fix_apply(),
            Act::FixSkip => l.fix_skip(),
            Act::FixStop => l.fix_stop(),
            Act::FixUndo => l.fix_undo(),
            Act::FixUndoAll => l.fix_undo_all(),
            Act::FixResetSkipped => l.fix_reset_skipped(),
            Act::FixClose => l.fix_close(),
            Act::FixReview => {
                let open = l.is_fixing();
                l.fix_close();
                if open {
                    self.open_review();
                }
            }
            _ => {}
        }
    }

    fn tab_strip(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        self.glyphs(ui);
        let t = self.theme;
        let reveal = std::mem::take(&mut self.reveal_tab);
        let active = self.active;
        let gap = 2.0;
        let plus = 28.0;
        let count = self.tabs.len();
        let overflow = (ui.available_width() - plus - 6.0) / count as f32 - gap < tabs::MIN_WIDTH;
        let avail = (ui.available_width() - plus * (1.0 + f32::from(u8::from(overflow))) - 6.0).max(tabs::MIN_WIDTH);
        let w = tabs::width(avail, count, gap);
        let corner = CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            egui::ScrollArea::horizontal().id_salt("tab_strip").max_width(avail).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    let origin = ui.cursor().min.x;
                    for (i, tab) in self.tabs.iter_mut().enumerate() {
                        let on = i == active;
                        let (rect, _) = ui.allocate_exact_size(vec2(w, tabs::HEIGHT), Sense::hover());
                        let resp = ui.interact(rect, ui.id().with(("tab", tab.uid)), Sense::click_and_drag());
                        let close_rect = egui::Rect::from_center_size(egui::pos2(rect.right() - 14.0, rect.center().y), vec2(18.0, 18.0));
                        let close = ui.interact(close_rect, ui.id().with(("tab_close", tab.uid)), Sense::click());
                        let hovered = resp.hovered() || close.hovered();
                        let mut x = rect.left() + 11.0;
                        let dirty = tab.dirty();
                        if dirty {
                            x += 13.0;
                        }
                        let notes = self.memos.count(&tab.key);
                        let note_glyph = self.glyphs.as_ref().map(|g| g.note_small.clone());
                        let end = close_rect.left() - 4.0 - if notes > 0 { 15.0 } else { 0.0 };
                        let title = tab.title(ui, (end - x).max(0.0).floor());
                        let p = ui.painter();
                        if on {
                            p.rect(rect, corner, t.surface, Stroke::new(1.0, t.line), StrokeKind::Inside);
                            p.line_segment([rect.left_bottom() + vec2(1.0, -0.5), rect.right_bottom() + vec2(-1.0, -0.5)], Stroke::new(2.0, t.surface));
                        } else if hovered {
                            p.rect_filled(rect, corner, t.hover);
                        }
                        if dirty {
                            p.circle_filled(egui::pos2(rect.left() + 14.5, rect.center().y), 3.5, t.accent);
                        }
                        p.galley(egui::pos2(x, rect.center().y - title.size().y / 2.0), title, if on { t.text } else { t.muted });
                        if notes > 0
                            && let Some(g) = note_glyph
                        {
                            p.galley(egui::pos2(close_rect.left() - 3.0 - g.size().x, rect.center().y - g.size().y / 2.0), g, t.warn_text);
                        }
                        if on || hovered {
                            if close.hovered() {
                                p.rect_filled(close_rect, CornerRadius::same(4), t.chip);
                            }
                            let (c, d) = (close_rect.center(), 3.5);
                            let s = Stroke::new(1.4, if close.hovered() { t.text } else { t.faint });
                            p.line_segment([c - vec2(d, d), c + vec2(d, d)], s);
                            p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], s);
                        }
                        if close.clicked() || resp.middle_clicked() {
                            acts.push(Act::CloseTab(i));
                        } else if resp.clicked() && !on {
                            acts.push(Act::Tab(i));
                        }
                        if on && reveal {
                            ui.scroll_to_rect(rect, None);
                        }
                        if resp.drag_started() && !on {
                            acts.push(Act::Tab(i));
                        }
                        if resp.dragged() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                            if let Some(pos) = resp.interact_pointer_pos() {
                                let to = tabs::slot_at(pos.x, origin, w, gap, count);
                                if to != i {
                                    acts.push(Act::MoveTab { from: i, to });
                                }
                            }
                        }
                        if resp.drag_stopped() {
                            acts.push(Act::TabDropped);
                        }
                        close.on_hover_text("Close (Ctrl+W, or middle-click the tab)");
                        let memos = &self.memos;
                        resp.on_hover_ui(|ui| {
                            ui.label(tab.path.display().to_string());
                            if let Some(f) = memos.file(&tab.key) {
                                memo_preview(ui, &t, f, tab.ws.as_ref());
                            }
                        });
                    }
                });
            });
            if overflow {
                let (rect, resp) = ui.allocate_exact_size(vec2(plus, tabs::HEIGHT), Sense::click());
                let p = ui.painter();
                if resp.hovered() {
                    p.rect_filled(rect.shrink(1.0), CornerRadius::same(6), t.chip);
                }
                let (c, d) = (rect.center(), 4.0);
                let s = Stroke::new(1.5, if resp.hovered() { t.text } else { t.muted });
                p.line_segment([c + vec2(-d, -d / 2.0), c + vec2(0.0, d / 2.0)], s);
                p.line_segment([c + vec2(0.0, d / 2.0), c + vec2(d, -d / 2.0)], s);
                let resp = resp.on_hover_text("All open tabs");
                egui::Popup::menu(&resp).show(|ui| {
                    ui.set_min_width(220.0);
                    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                        for (i, tab) in self.tabs.iter().enumerate() {
                            let text = if tab.dirty() { format!("\u{25cf} {}", tab.label) } else { tab.label.to_string() };
                            if ui.selectable_label(i == active, text).on_hover_text(tab.path.display().to_string()).clicked() {
                                acts.push(Act::Tab(i));
                            }
                        }
                    });
                });
            }
            let (rect, resp) = ui.allocate_exact_size(vec2(plus, tabs::HEIGHT), Sense::click());
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(rect.shrink(1.0), CornerRadius::same(6), t.chip);
            }
            let (c, d) = (rect.center(), 5.0);
            let s = Stroke::new(1.5, if resp.hovered() { t.text } else { t.muted });
            p.line_segment([c - vec2(d, 0.0), c + vec2(d, 0.0)], s);
            p.line_segment([c - vec2(0.0, d), c + vec2(0.0, d)], s);
            if resp.on_hover_text("Open a file in a new tab (Ctrl+O)").clicked() {
                acts.push(Act::Open);
            }
        });
    }

    fn glyphs(&mut self, ui: &egui::Ui) {
        let ppp = ui.ctx().pixels_per_point();
        if self.glyphs.as_ref().is_none_or(|g| g.ppp != ppp) {
            self.glyphs = Some(Glyphs::new(ui, ppp));
        }
    }

    fn nav(&mut self, ui: &mut egui::Ui, suite_stats: Option<(usize, usize)>, acts: &mut Vec<Act>) {
        self.glyphs(ui);
        let App { glyphs: Some(g), tabs, active, theme: t, memos, show_notes, .. } = self else { return };
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
            ui.painter().rect_filled(r, CornerRadius::same(6), t.accent);
            ui.painter().galley(r.center() - g.grid.size() / 2.0, g.grid.clone(), t.on_accent);
            let (r, _) = ui.allocate_exact_size(g.brand.size(), Sense::hover());
            ui.painter().galley(r.min, g.brand.clone(), t.text);
        });
        ui.add_space(14.0);
        let Some(Tab { ws: Some(l), nav, key, .. }) = tabs.get_mut(*active) else { return };
        let l = &*l;
        nav.sync(ui, l, g.ppp);
        let decision = l.decision();
        let view = l.view();
        group_glyph(ui, t, &g.decisions);
        let file = memos.file(key);
        for (i, (d, row)) in l.defs().decisions.iter().zip(&nav.rows).enumerate() {
            let active = decision == i;
            let (bg, fg) = if row.alert {
                (t.error, hex(0xffffff))
            } else if active {
                (t.accent_soft, t.accent)
            } else {
                (t.chip, t.muted)
            };
            let note = file.and_then(|f| f.get(&d.id));
            let mut resp = nav_item(ui, t, &g.decision, row.name.pick(active), Some((&row.badge, bg, fg)), active, note.map(|_| &g.note));
            if let Some(text) = note {
                resp = resp.on_hover_ui(|ui| {
                    ui.set_max_width(300.0);
                    ui.label(RichText::new(text).color(t.text));
                });
            }
            if resp.clicked() {
                acts.push(Act::Decision(i));
            }
        }
        ui.add_space(12.0);
        group_glyph(ui, t, &g.views);
        let (issues, alert) = nav.issue_badge(ui, decision);
        let issue_colors = if alert { (t.error, hex(0xffffff)) } else { (t.chip, t.muted) };
        let tests = nav.test_badge(ui, suite_stats);
        for (v, (icon, label)) in [View::Rules, View::Issues, View::Test, View::Xml].into_iter().zip(&g.items) {
            let badge = match v {
                View::Issues => Some((&issues, issue_colors.0, issue_colors.1)),
                View::Test => tests.as_ref().map(|(b, pass)| if *pass { (b, t.accent_soft, t.accent) } else { (b, t.error, hex(0xffffff)) }),
                _ => None,
            };
            let on = !*show_notes && view == v;
            if nav_item(ui, t, icon, label.pick(on), badge, on, None).clicked() {
                acts.push(Act::View(v));
            }
        }
        let total = memos.total();
        if total == 0 {
            g.notes_badge = None;
        } else if g.notes_badge.as_ref().is_none_or(|(n, _)| *n != total) {
            g.notes_badge = Some((total, badge_glyph(ui, &total.to_string())));
        }
        let (icon, label) = &g.items[4];
        let badge = g.notes_badge.as_ref().map(|(_, b)| (b, t.warn.gamma_multiply(if t.dark { 0.28 } else { 0.2 }), t.warn_text));
        if nav_item(ui, t, icon, label.pick(*show_notes), badge, *show_notes, None).on_hover_text("Local notes from every tab (Ctrl+5)").clicked() {
            acts.push(Act::Notes);
        }
        ui.add_space(12.0);
    }

    fn header(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let t = self.theme;
        let Some(Tab { ws: Some(l), rules, label: file, key, crumb, .. }) = self.tabs.get_mut(self.active) else { return };
        let l = &*l;
        let decision = l.decision();
        let view = l.view();
        let dec = l.defs().decisions.get(decision);
        let dirty = l.dirty();
        let fixing = l.is_fixing();
        let fixable = l.fixable_count();
        let dmn_version = l.defs().dmn_version;
        ui.horizontal_centered(|ui| {
            let ppp = ui.ctx().pixels_per_point();
            if crumb.as_ref().is_none_or(|(d, p, _)| *d != dirty || *p != ppp) {
                *crumb = Some((dirty, ppp, glyph(ui, &format!("{file}{}  /", if dirty { " \u{2022}" } else { "" }), egui::TextStyle::Body.resolve(ui.style()))));
            }
            if let Some((_, _, g)) = crumb.as_ref() {
                let (r, resp) = ui.allocate_exact_size(g.size(), Sense::hover());
                ui.painter().galley(r.min, g.clone(), t.faint);
                resp.on_hover_text(if dirty { "Unsaved changes (Ctrl+S to save)" } else { "Saved" });
            }
            if let Some(d) = dec {
                ui.label(bold(if d.name.is_empty() { &d.id } else { &d.name }, 14.0).color(t.text));
                if let DecisionLogic::Table(tb) = &d.logic {
                    if l.editable() && !fixing {
                        let current = &tb.hit_policy;
                        let mut picked_hp = None;
                        egui::ComboBox::from_id_salt("hit_policy").selected_text(current.name()).width(120.0).show_ui(ui, |ui| {
                            for p in HitPolicy::ALL {
                                if ui.selectable_label(current.same_kind(&p), p.name()).clicked() && !current.same_kind(&p) {
                                    picked_hp = Some(p);
                                }
                            }
                        });
                        if let HitPolicy::Collect(agg) = current {
                            egui::ComboBox::from_id_salt("aggregation").selected_text(agg.as_ref().map_or("list", Aggregation::name)).width(70.0).show_ui(ui, |ui| {
                                if ui.selectable_label(agg.is_none(), "list").clicked() && agg.is_some() {
                                    picked_hp = Some(HitPolicy::Collect(None));
                                }
                                for a in Aggregation::ALL {
                                    let on = agg.as_ref() == Some(&a);
                                    if ui.selectable_label(on, a.name()).clicked() && !on {
                                        picked_hp = Some(HitPolicy::Collect(Some(a)));
                                    }
                                }
                            });
                        }
                        if let Some(hit_policy) = picked_hp {
                            acts.push(Act::Edit(Edit::HitPolicy { decision, hit_policy }));
                        }
                        let file_lang = l.defs().expression_language.clone();
                        let shown = Language::CHOICES
                            .iter()
                            .find(|(_, v)| v.map(str::to_string) == file_lang.as_ref().map(|s| s.to_ascii_lowercase()))
                            .map_or_else(|| file_lang.clone().unwrap_or_default(), |(n, _)| n.to_string());
                        let shown = if file_lang.is_none() { "FEEL (default)".to_string() } else { shown };
                        let mut picked = file_lang.clone();
                        egui::ComboBox::from_id_salt("file_language")
                            .selected_text(shown)
                            .width(130.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut picked, None, "FEEL (default)");
                                for (name, v) in Language::CHOICES.iter().skip(1) {
                                    ui.selectable_value(&mut picked, v.map(str::to_string), *name);
                                }
                            })
                            .response
                            .on_hover_text("Expression language for the whole file. Cells and inputs can override it.");
                        if picked != file_lang {
                            acts.push(Act::Edit(Edit::FileLanguage { lang: picked }));
                        }
                    } else {
                        pill(ui, tb.hit_policy.to_string(), t.chip, t.muted).on_hover_text("Hit policy");
                    }
                    pill(ui, format!("{} in · {} out", tb.inputs.len(), tb.outputs.len()), t.chip, t.muted);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(v) = dmn_version {
                    pill(ui, format!("DMN {v}"), t.chip, t.muted).on_hover_text("DMN version, from the file's namespace");
                }
                if button(ui, &t, ic::PLAY, "Run tests", true).clicked() {
                    acts.push(Act::View(View::Test));
                }
                if let Some(d) = dec {
                    let has = self.memos.get(key, &d.id).is_some();
                    let r = button(ui, &t, ic::NOTE, if has { "Note" } else { "Add note" }, false)
                        .on_hover_text("Local note for this decision. Only on this machine, gone when the app closes (Ctrl+Shift+N)");
                    self.memo_anchor = r.rect;
                    if r.clicked() {
                        acts.push(Act::Memo);
                    }
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
                if view != View::Issues && button(ui, &t, ic::COPY, "Copy report", false).on_hover_text("Copy all issues as text").clicked() {
                    ui.ctx().copy_text(lint::report(l.defs(), l.issues(), &l.path().display().to_string()));
                }
                if view == View::Rules {
                    Frame::new()
                        .fill(if t.dark { t.surface } else { hex(0xf9fafb) })
                        .stroke(Stroke::new(1.0, t.line))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::symmetric(10, 5))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if !rules.filter.is_empty() && ui.add(egui::Button::new(RichText::new(ic::X).color(t.muted)).frame(false)).clicked() {
                                    rules.filter.clear();
                                }
                                let filter = ui.add(egui::TextEdit::singleline(&mut rules.filter).frame(Frame::NONE).hint_text("Filter rules").desired_width(170.0));
                                if std::mem::take(&mut self.shell.focus_filter) {
                                    filter.request_focus();
                                }
                                ui.label(RichText::new(ic::SEARCH).color(t.faint));
                            });
                        });
                }
            });
        });
    }

    fn rules_view(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let Some(Tab { ws: Some(ws), rules, .. }) = self.tabs.get_mut(self.active) else { return };
        let mut editing = ws.take_editing();
        let mut scroll = ws.take_scroll();
        let mut cache = std::mem::take(rules);
        self.rules_body(ui, acts, &mut editing, &mut scroll, &mut cache);
        if let Some(Tab { ws: Some(ws), rules, .. }) = self.tabs.get_mut(self.active) {
            ws.restore_editing(editing);
            ws.restore_scroll(scroll);
            *rules = cache;
        }
    }

    fn rules_body(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>, editing: &mut Option<Editing>, scroll: &mut Option<usize>, cache: &mut RuleCache) {
        let t = self.theme;
        let (outcome, coverage) = {
            let Some(l) = self.ws_mut() else { return };
            let mut tests = l.tests();
            match tests.table() {
                Some(_) => {
                    let outcome = if tests.is_active() { tests.outcome() } else { None };
                    let coverage = tests.coverage();
                    (outcome, coverage)
                }
                None => (None, None),
            }
        };
        let fired: &[usize] = outcome.as_ref().map_or(&[], |o| &o.fired);
        let Some(l) = self.tabs.get(self.active).and_then(|t| t.ws.as_ref()) else { return };
        let decision = l.decision();
        let Some(dec) = l.defs().decisions.get(decision) else { return };
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
        let row_height = self.row_height();
        let issues: Vec<&Issue> = l.issues().iter().filter(|i| i.decision == Some(decision)).collect();
        if !self.is_fixing() {
            egui::Panel::right("drawer")
                .resizable(true)
                .default_size(330.0)
                .frame(Frame::new().fill(t.surface).inner_margin(Margin::same(0)))
                .show(ui, |ui| self.issue_list(ui, &issues, nrules, false, acts));
        }

        let (n_in, n_out) = (tb.inputs.len(), tb.outputs.len());
        let revision = l.revision();
        let derived = match &cache.derived {
            Some((d, rev, s)) if *d == decision && *rev == revision => s.clone(),
            _ => {
                let mut rule = vec![None; nrules];
                let mut cell = vec![None; nrules * (n_in + n_out)];
                for i in issues.iter().filter(|i| row_specific(i, nrules)) {
                    let slot = i.column.and_then(|c| match c {
                        Col::Input(k) if k < n_in => Some(k),
                        Col::Output(k) if k < n_out => Some(n_in + k),
                        _ => None,
                    });
                    for &ri in i.rules.iter().filter(|&&ri| ri < nrules) {
                        rule[ri] = Some(rule[ri].map_or(i.severity, |old: Severity| old.min(i.severity)));
                        if let Some(s) = slot {
                            let c = &mut cell[ri * (n_in + n_out) + s];
                            *c = Some(c.map_or(i.severity, |old: Severity| old.min(i.severity)));
                        }
                    }
                }
                let notes = tb.rules.iter().map(|r| one_line(&r.summary())).collect();
                let s = Arc::new(Derived { rule, cell, notes });
                cache.derived = Some((decision, revision, s.clone()));
                s
            }
        };
        let rule_sev = |ri: usize| derived.rule[ri];
        let cell_sev = |ri: usize, c: Col| match c {
            Col::Input(k) if k < n_in => derived.cell[ri * (n_in + n_out) + k],
            Col::Output(k) if k < n_out => derived.cell[ri * (n_in + n_out) + n_in + k],
            _ => None,
        };
        let col_issue = |c: Col| issues.iter().filter(|i| i.column == Some(c) && i.rules.is_empty() && i.severity != Severity::Info).min_by_key(|i| i.severity).copied();

        let visible = match &cache.visible {
            Some((f, d, rev, v)) if *d == decision && *rev == revision && *f == cache.filter => v.clone(),
            _ => {
                let f = cache.filter.to_lowercase();
                let v: Arc<Vec<usize>> = Arc::new(
                    (0..nrules)
                        .filter(|&ri| {
                            let r = &tb.rules[ri];
                            f.is_empty()
                                || (ri + 1).to_string() == f
                                || r.summary().to_lowercase().contains(&f)
                                || r.inputs.iter().map(|e| &e.text).chain(r.outputs.iter().map(|e| &e.text)).any(|text| text.to_lowercase().contains(&f))
                        })
                        .collect(),
                );
                cache.visible = Some((cache.filter.clone(), decision, revision, v.clone()));
                v
            }
        };
        let scroll_target = scroll.take().and_then(|r| visible.iter().position(|&x| x == r));
        let selected = l.selected();
        let show_langs = self.show_langs;
        let editable = l.editable() && !self.is_fixing();
        let show_detail = selected.is_some() && (editing.is_some() || ui.input(|i| i.time) >= self.detail_after);
        let reveal_selected = show_detail && !self.detail_open;
        self.detail_open = show_detail;
        let keep_in_view = reveal_selected.then(|| selected.and_then(|ri| visible.iter().position(|&x| x == ri))).flatten();

        if let Some(r) = selected.filter(|_| show_detail).and_then(|ri| tb.rules.get(ri).map(|r| (ri, r))) {
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
                        let mut rows: Vec<(&str, &str, &str, &ExpressionLanguage, &str)> = Vec::with_capacity(n_in + n_out);
                        for (ci, inp) in tb.inputs.iter().enumerate() {
                            if let Some(e) = rule.inputs.get(ci).filter(|e| !e.is_any()) {
                                rows.push((inp.title(), "input", inp.type_ref.as_deref().unwrap_or("-"), &e.lang, e.text.trim()));
                            }
                        }
                        for (oi, out) in tb.outputs.iter().enumerate() {
                            if let Some(e) = rule.outputs.get(oi).filter(|e| !e.is_any()) {
                                rows.push((out.title(), "output", out.type_ref.as_deref().unwrap_or("-"), &e.lang, e.text.trim()));
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
                                        ui.add(egui::Label::new(bold(*name, 12.5).color(t.text)).wrap_mode(TextWrapMode::Truncate));
                                    });
                                    fixed(ui, W[1], &mut |ui| {
                                        ui.label(RichText::new(*kind).size(12.0).color(if *kind == "input" { t.in_label } else { t.out_label }));
                                    });
                                    fixed(ui, W[2], &mut |ui| {
                                        ui.label(RichText::new(*ty).size(12.0).color(t.muted));
                                    });
                                    fixed(ui, W[3], &mut |ui| {
                                        let (bg, fg) = if lang.is_explicit() { lang_colors(&t, lang.language) } else { (t.chip, t.faint) };
                                        pill(ui, lang.describe(), bg, fg).on_hover_ui(|ui| {
                                            ui.label(lang_describe(lang));
                                        });
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
                    let drag_id = egui::Id::new("rule_drag");
                    let slot_id = egui::Id::new("rule_drop_slot");
                    let dragging: Option<usize> = ui.data(|d| d.get_temp(drag_id));
                    let drop_slot: Option<usize> = dragging.and_then(|_| ui.data(|d| d.get_temp(slot_id)));
                    let can_drag = editable && editing.is_none() && visible.len() == nrules;
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
                    } else if let Some(row) = keep_in_view {
                        builder = builder.scroll_to_row(row, None);
                    }
                    let paint_cell = |ui: &mut egui::Ui, header: bool| {
                        ui.style_mut().interaction.selectable_labels = false;
                        let r = ui.max_rect();
                        ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(1.0, if header { t.line } else { t.line_soft }));
                        ui.add_space(12.0);
                    };
                    builder
                        .header(76.0, |mut h| {
                            h.col(|ui| {
                                paint_cell(ui, true);
                                ui.label(bold(&tb.hit_policy.name()[..1.min(tb.hit_policy.name().len())], 13.0).color(t.muted)).on_hover_text(format!("Hit policy {}", tb.hit_policy));
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
                                                    ui.label(bold(inp.lang.long_name(), 11.0).color(lang_colors(&t, inp.lang.language).1));
                                                } else {
                                                    ui.label(RichText::new(inp.lang.describe()).size(11.0).color(t.faint));
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
                                    let resp = ui
                                        .vertical(|ui| {
                                            ui.add_space(8.0);
                                            ui.label(bold("OUTPUT", 9.5).color(t.out_label));
                                            ui.add(egui::Label::new(bold(out.title(), 12.5).color(t.text)).wrap_mode(TextWrapMode::Truncate));
                                            ui.label(RichText::new(format!("{}  ·  {} name", out.type_ref.as_deref().unwrap_or("-"), out.name)).size(11.0).color(t.muted));
                                        })
                                        .response
                                        .interact(Sense::click())
                                        .on_hover_ui(|ui| {
                                            let langs: std::collections::BTreeSet<_> = tb.rules.iter().filter_map(|r| r.outputs.get(oi)).filter(|e| !e.is_any()).map(|e| e.lang.long_name()).collect();
                                            ui.label(format!(
                                                "{}\nname: {}\ntype: {}\nlanguages used in this column: {}\n(the language is set per cell, not per column)",
                                                out.title(),
                                                out.name,
                                                out.type_ref.as_deref().unwrap_or("-"),
                                                if langs.is_empty() { "-".to_string() } else { langs.into_iter().collect::<Vec<_>>().join(", ") }
                                            ));
                                        });
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
                            body.rows(row_height, visible.len(), |mut row| {
                                let ri = visible[row.index()];
                                let r = &tb.rules[ri];
                                let is_sel = selected == Some(ri);
                                let is_fired = fired.contains(&ri);
                                let is_drag_src = dragging == Some(ri);
                                let base = |ui: &mut egui::Ui| {
                                    let rect = ui.max_rect();
                                    if is_sel {
                                        ui.painter().rect_filled(rect, 0.0, t.sel_row);
                                    }
                                    if is_drag_src {
                                        ui.painter().rect_filled(rect, 0.0, t.accent_soft);
                                    }
                                    paint_cell(ui, false);
                                    if let Some(slot) = drop_slot {
                                        let y = if slot == ri {
                                            Some(rect.top())
                                        } else if slot == ri + 1 && ri + 1 == nrules {
                                            Some(rect.bottom())
                                        } else {
                                            None
                                        };
                                        if let Some(y) = y {
                                            ui.painter().hline(rect.x_range(), y, Stroke::new(2.0, t.accent));
                                        }
                                    }
                                };
                                row.col(|ui| {
                                    base(ui);
                                    if can_drag {
                                        let rect = ui.max_rect();
                                        let grip = egui::Rect::from_min_size(rect.min, vec2(20.0, rect.height()));
                                        let handle = ui.interact(grip, egui::Id::new(("rule_grip", ri)), Sense::drag()).on_hover_cursor(egui::CursorIcon::Grab);
                                        if handle.drag_started() {
                                            ui.data_mut(|d| d.insert_temp(drag_id, ri));
                                        }
                                    }
                                    if let (Some(_), Some(p)) = (dragging, ui.input(|i| i.pointer.interact_pos())) {
                                        let rect = ui.max_rect();
                                        if p.y >= rect.top() && p.y < rect.bottom() {
                                            let slot = ri + usize::from(p.y > rect.center().y);
                                            ui.data_mut(|d| d.insert_temp(slot_id, slot));
                                        }
                                    }
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
                                    ui.label(num).on_hover_ui(|ui| {
                                        ui.label(format!("{}  ·  XML line {}{}", r.id, r.line, if is_fired { "\nfired for the current test inputs" } else { "" }));
                                    });
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
                                        if let Some((_, lang)) = e
                                            && (lang.is_explicit() || show_langs)
                                        {
                                            let mask = if is_sel { t.sel_row } else { t.surface };
                                            paint_lang_tag(ui, &t, lang, mask);
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
                                                for (label, lang) in Language::CHOICES {
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
                                        resp.on_hover_ui(|ui| {
                                            let text: String = text.trim().lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
                                            ui.label(format!(
                                                "{text}\n\nLanguage: {}\nColumn type: {}{}",
                                                lang_describe(lang),
                                                ty.unwrap_or("-"),
                                                if is_input { "\nKind: condition (unary test)" } else { "\nKind: output expression" }
                                            ));
                                        });
                                    }
                                };
                                for (ci, inp) in tb.inputs.iter().enumerate() {
                                    let e = r.inputs.get(ci);
                                    cell(&mut row, e.map(|e| (e.text.as_str(), &e.lang)), e.map(|e| &e.tests), Col::Input(ci), true, inp.type_ref.as_deref());
                                }
                                for (oi, out) in tb.outputs.iter().enumerate() {
                                    cell(&mut row, r.outputs.get(oi).map(|e| (e.text.as_str(), &e.lang)), None, Col::Output(oi), false, out.type_ref.as_deref());
                                }
                                let note_at = Cursor::Note(ri);
                                let (_, resp) = row.col(|ui| {
                                    base(ui);
                                    if let Some(ed) = editing.as_mut().filter(|ed| ed.at == note_at) {
                                        edit_box(ui, ed, acts);
                                        return;
                                    }
                                    ui.add(egui::Label::new(RichText::new(derived.notes[ri].as_str()).size(12.5).color(t.muted)).wrap_mode(TextWrapMode::Truncate));
                                });
                                if editable {
                                    if resp.double_clicked() {
                                        acts.push(Act::BeginEdit(note_at, tb.note(r).to_string()));
                                    }
                                    resp.context_menu(|ui| rule_menu(ui, decision, ri, nrules, acts));
                                }
                                let rr = row.response();
                                if rr.clicked() && !rr.double_clicked() && dragging.is_none() {
                                    acts.push(Act::Select(ri));
                                }
                            });
                        });
                    if let Some(from) = dragging {
                        let ctx = ui.ctx().clone();
                        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
                        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                            ctx.data_mut(|d| {
                                d.remove_temp::<usize>(drag_id);
                                d.remove_temp::<usize>(slot_id);
                            });
                        } else if ctx.input(|i| i.pointer.any_released()) {
                            let slot: Option<usize> = ctx.data(|d| d.get_temp(slot_id));
                            ctx.data_mut(|d| {
                                d.remove_temp::<usize>(drag_id);
                                d.remove_temp::<usize>(slot_id);
                            });
                            if let Some(slot) = slot {
                                let to = if slot > from { slot - 1 } else { slot };
                                if to != from && to < nrules {
                                    acts.push(Act::Edit(Edit::MoveRule { decision, from, to }));
                                }
                            }
                        } else {
                            ctx.request_repaint();
                        }
                    }
                });
            });
        });
    }

    fn issue_list(&self, ui: &mut egui::Ui, issues: &[&Issue], nrules: usize, wide: bool, acts: &mut Vec<Act>) {
        let t = &self.theme;
        let sev_filter = self.tab().and_then(|t| t.sev_filter);
        let count = |s: Severity| issues.iter().filter(|i| i.severity == s).count();
        Frame::new().inner_margin(Margin { left: 16, right: 16, top: 16, bottom: 10 }).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(bold("Issues", 15.0).color(t.text));
                if wide {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(l) = self.ws()
                            && button(ui, t, ic::COPY, "Copy report", false).clicked()
                        {
                            ui.ctx().copy_text(lint::report(l.defs(), l.issues(), &l.path().display().to_string()));
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
                    let on = sev_filter == sev;
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
        let shown: Vec<&&Issue> = issues.iter().filter(|i| sev_filter.is_none_or(|s| i.severity == s)).collect();
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
                                let key = self.ws().and_then(|l| l.fixable_key(is));
                                if let Some(key) = key {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if self.ws().is_some_and(|l| l.is_skipped(&key)) {
                                            pill(ui, "skipped", t.chip, t.faint).on_hover_text("Skipped in a fix session. Reload the file or use \"Show skipped again\" to be asked again.");
                                        } else if !self.is_fixing() && small_button(ui, t, ic::CHECK, "Fix").on_hover_text("Review the automatic fix for this issue").clicked() {
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
                                        if chip(ui, &label, t.chip, t.chip_text).interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
                                            && let Some(r) = first_rule(part)
                                        {
                                            acts.push(Act::Jump(r));
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
                    if resp.clicked()
                        && let Some(r) = is.rules.iter().min()
                    {
                        acts.push(Act::Jump(*r));
                    }
                    ui.add_space(8.0);
                }
            });
        });
    }

    fn issues_view(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let t = self.theme;
        let Some(l) = self.ws() else { return };
        let issues: Vec<&Issue> = l.issues().iter().filter(|i| i.decision == Some(self.decision()) || i.decision.is_none()).collect();
        let nrules = l.defs().table(self.decision()).map(|t| t.rules.len()).unwrap_or(0);
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
        let Some(l) = self.ws() else { return };
        egui::CentralPanel::default().frame(Frame::new().fill(t.canvas).inner_margin(Margin::symmetric(20, 16))).show(ui, |ui| {
            Frame::new().fill(t.surface).stroke(Stroke::new(1.0, t.line)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::same(12)).show(ui, |ui| {
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    let mut text = l.xml();
                    ui.add(egui::TextEdit::multiline(&mut text).code_editor().frame(Frame::NONE).desired_width(f32::INFINITY));
                });
            });
        });
    }

    fn empty_view(&self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let t = self.theme;
        egui::CentralPanel::default().frame(Frame::new().fill(t.canvas)).show(ui, |ui| {
            let r = ui.max_rect();
            paint_grid(ui.painter(), r, t.text.gamma_multiply(if t.dark { 0.05 } else { 0.06 }));
            ui.vertical_centered(|ui| {
                ui.add_space((r.height() * 0.5 - 104.0).max(16.0));
                paint_mark(ui, &t, 56.0);
                ui.add_space(14.0);
                ui.label(bold("DMN Viewer", 28.0).color(t.text));
                ui.label(RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).size(12.0).color(t.faint));
                ui.add_space(26.0);
                let hints: [(&[&str], &str, Act); 3] =
                    [(&["Ctrl", "O"], "Open file", Act::Open), (&["Ctrl", "K"], "Command palette", Act::Cmd(menu::Cmd::Palette)), (&["F1"], "Shortcuts", Act::Cmd(menu::Cmd::Shortcuts))];
                if let Some(a) = hint_row(ui, &t, hints) {
                    acts.push(a);
                }
            });
        });
    }
}

fn paint_grid(p: &egui::Painter, r: egui::Rect, color: Color32) {
    const CW: f32 = 140.0;
    const CH: f32 = 36.0;
    let c = r.center();
    let (hw, hh) = (r.width() * 0.5, r.height() * 0.5);
    let alpha = |x: f32, y: f32| {
        let d = (((x - c.x) / hw).powi(2) + ((y - c.y) / hh).powi(2)).sqrt() * std::f32::consts::FRAC_1_SQRT_2;
        1.0 - ((d - 0.2) / 0.5).clamp(0.0, 1.0)
    };
    let seg = |a: egui::Pos2, b: egui::Pos2| {
        let k = alpha((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
        if k > 0.02 {
            p.line_segment([a, b], Stroke::new(1.0, color.gamma_multiply(k)));
        }
    };
    let mut x = r.left().round() + 0.5;
    while x < r.right() {
        let mut y = r.top();
        while y < r.bottom() {
            let y2 = (y + CH).min(r.bottom());
            seg(egui::pos2(x, y), egui::pos2(x, y2));
            y = y2;
        }
        x += CW;
    }
    let mut y = r.top().round() + 0.5;
    while y < r.bottom() {
        let mut x = r.left();
        while x < r.right() {
            let x2 = (x + CW).min(r.right());
            seg(egui::pos2(x, y), egui::pos2(x2, y));
            x = x2;
        }
        y += CH;
    }
}

fn paint_mark(ui: &mut egui::Ui, t: &Theme, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let p = ui.painter();
    let k = size / 56.0;
    let at = |x: f32, y: f32, w: f32, h: f32| egui::Rect::from_min_size(rect.min + vec2(x * k, y * k), vec2(w * k, h * k));
    let round = |r: f32| CornerRadius::same((r * k).round() as u8);
    p.rect(rect.shrink(0.5), round(12.0), t.surface, Stroke::new(1.0, t.line), StrokeKind::Inside);
    p.rect_filled(at(9.0, 11.0, 9.0, 34.0), round(3.0), t.accent);
    p.rect_filled(at(22.0, 11.0, 25.0, 8.0), round(3.0), t.accent_soft);
    for (y, w) in [(24.0, 25.0), (32.0, 18.0), (40.0, 22.0)] {
        p.rect_filled(at(22.0, y, w, 3.0), round(1.5), t.line);
    }
}

fn hint_row<const N: usize>(ui: &mut egui::Ui, t: &Theme, hints: [(&[&str], &str, Act); N]) -> Option<Act> {
    const GAP: f32 = 22.0;
    const CAP_H: f32 = 20.0;
    let mono = FontId::monospace(11.5);
    let body = FontId::proportional(12.5);
    let p = ui.painter().clone();
    let width = |keys: &[&str], label: &str| {
        let caps: f32 = keys.iter().map(|k| p.layout_no_wrap((*k).to_string(), mono.clone(), t.muted).size().x + 12.0 + 4.0).sum();
        caps + 4.0 + p.layout_no_wrap(label.to_string(), body.clone(), t.muted).size().x
    };
    let total: f32 = hints.iter().map(|(k, l, _)| width(k, l)).sum::<f32>() + GAP * (N.saturating_sub(1)) as f32;
    let (row, _) = ui.allocate_exact_size(vec2(total, CAP_H + 2.0), Sense::hover());
    let mut x = row.left();
    let mut picked = None;
    for (i, (keys, label, act)) in hints.into_iter().enumerate() {
        let w = width(keys, label);
        let area = egui::Rect::from_min_size(egui::pos2(x, row.top()), vec2(w, row.height()));
        let resp = ui.interact(area, ui.id().with(("hint", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let fg = if resp.hovered() { t.text } else { t.muted };
        let mut cx = x;
        for k in keys {
            let g = p.layout_no_wrap((*k).to_string(), mono.clone(), fg);
            let cap = egui::Rect::from_min_size(egui::pos2(cx, row.top()), vec2(g.size().x + 12.0, CAP_H));
            p.rect(cap, CornerRadius::same(5), t.surface, Stroke::new(1.0, t.line), StrokeKind::Inside);
            p.hline(cap.x_range().shrink(2.0), cap.bottom() - 0.5, Stroke::new(1.0, t.line));
            p.galley(cap.center() - g.size() * 0.5, g, fg);
            cx = cap.right() + 4.0;
        }
        p.text(egui::pos2(cx + 4.0, row.top() + CAP_H * 0.5), Align2::LEFT_CENTER, label, body.clone(), fg);
        if resp.clicked() {
            picked = Some(act);
        }
        x += w + GAP;
    }
    picked
}

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

fn fix_panel(ui: &mut egui::Ui, t: &Theme, v: &FixView, acts: &mut Vec<Act>) {
    let warn_bg = t.warn.gamma_multiply(if t.dark { 0.25 } else { 0.15 });
    let error_bg = if t.dark { hex(0x3a1f22) } else { hex(0xfdeeee) };
    egui::ScrollArea::vertical().id_salt("fix_panel").auto_shrink([false, false]).show(ui, |ui| {
        Frame::new().inner_margin(Margin::same(16)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let Some(c) = v.current else {
                summary_panel(ui, t, v, acts);
                return;
            };
            let (n, total) = v.progress;
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
                let on = v.choice == Some(i);
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
            let destructive = v.choice.and_then(|i| c.fixes.get(i)).filter(|f| f.destructive).map(|f| f.label.clone());
            ui.add_space(4.0);
            match v.preview {
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
                let can = v.can_undo;
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

fn summary_panel(ui: &mut egui::Ui, t: &Theme, v: &FixView, acts: &mut Vec<Act>) {
    ui.label(bold("Fix summary", 15.0).color(t.text));
    ui.add_space(6.0);
    if v.applied.is_empty() {
        let text = if v.undone_all { "Every fix from this session was undone." } else { "No fixes applied." };
        ui.label(RichText::new(text).color(t.muted));
    } else {
        ui.label(RichText::new(format!("{} applied, not saved yet", v.applied.len())).size(12.5).color(t.text));
        ui.add_space(6.0);
        for a in v.applied {
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
    if !v.skipped_now.is_empty() {
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(format!("Skipped: {}", v.skipped_now.join(", "))).size(12.0).color(t.muted)).wrap());
    }
    if v.skipped > 0 && ui.link(RichText::new(format!("Show {} skipped issue{} again", v.skipped, if v.skipped == 1 { "" } else { "s" })).size(12.0)).clicked() {
        acts.push(Act::FixResetSkipped);
    }
    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(v.can_undo, egui::Button::new("Undo all fixes")).clicked() {
            acts.push(Act::FixUndoAll);
        }
        if v.dirty && button(ui, t, ic::SEARCH, "Review & save", true).clicked() {
            acts.push(Act::FixReview);
        }
        if button(ui, t, ic::X, "Close", false).on_hover_text("Esc. Each fix stays one Ctrl+Z step.").clicked() {
            acts.push(Act::FixClose);
        }
    });
}

fn test_view(ui: &mut egui::Ui, ws: &mut Workspace, selected: Option<usize>, tester: &mut Tester, t: &Theme, acts: &mut Vec<Act>) {
    let mut tests = ws.tests();
    let Some(tb) = tests.table() else {
        egui::CentralPanel::default().frame(Frame::new().fill(t.canvas).inner_margin(Margin::same(20))).show(ui, |ui| {
            ui.label(RichText::new("The tester works on decision tables. Pick one on the left.").color(t.muted));
        });
        return;
    };

    egui::Panel::left("test_inputs").resizable(true).default_size(390.0).frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 20, right: 8, top: 16, bottom: 16 })).show(ui, |ui| {
        card(ui, t, |ui| {
            ui.set_min_height(ui.available_height());
            ui.horizontal(|ui| {
                ui.label(bold("Inputs", 15.0).color(t.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if small_button(ui, t, ic::ERASER, "Clear").clicked() {
                        tests.clear_inputs();
                    }
                    if ui.add_enabled_ui(selected.is_some(), |ui| small_button(ui, t, ic::IMPORT, "From rule")).inner.on_hover_text("Copy the selected rule's conditions into the inputs").clicked()
                        && let Some(r) = selected.filter(|&r| r < tb.rules.len())
                    {
                        tests.fill_from_rule(r);
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
                    if let Some(text) = tests.input_mut(i) {
                        ui.add(egui::TextEdit::singleline(text).font(egui::TextStyle::Monospace).hint_text(hint).desired_width(f32::INFINITY).margin(Margin::symmetric(8, 5)));
                    }
                    ui.add_space(4.0);
                }
            });
        });
    });

    let outcome = tests.outcome().expect("decision is a table");
    let results = tests.results().expect("decision is a table");
    let coverage = tests.coverage();

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
                    let default_name = tests.default_suite_path().file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    let mut save_to = None;
                    if small_button(ui, t, ic::SAVE, "Save as…").clicked() {
                        save_to = rfd::FileDialog::new().add_filter("Test suite", &["json"]).set_file_name(&default_name).save_file();
                    }
                    if small_button(ui, t, ic::SAVE, "Save").clicked() {
                        save_to = tests.suite_path().map(Path::to_path_buf).or_else(|| rfd::FileDialog::new().add_filter("Test suite", &["json"]).set_file_name(&default_name).save_file());
                    }
                    if small_button(ui, t, ic::OPEN, "Load…").clicked()
                        && let Some(p) = rfd::FileDialog::new().add_filter("Test suite", &["json"]).pick_file()
                    {
                        let _ = tests.load(&p);
                    }
                    if small_button(ui, t, ic::NEW, "New").clicked() {
                        tests.new_suite();
                    }
                    if let Some(p) = save_to {
                        let _ = tests.save(&p);
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut tester.case_name).hint_text("Name for the new case").desired_width(240.0).margin(Margin::symmetric(8, 4)));
                if small_button(ui, t, ic::PLUS, "Add current inputs").on_hover_text("Saves the inputs and the current result as the expected result").clicked() {
                    tests.add_case(&tester.case_name);
                    tester.case_name.clear();
                }
                ui.label(RichText::new(tests.status()).size(11.5).color(t.faint));
            });
            if let Some(cov) = coverage.as_deref().filter(|_| !tests.suite().cases.is_empty()) {
                coverage_line(ui, t, cov);
            }
            ui.add_space(4.0);
            let mut load_case = None;
            let mut accept = None;
            let mut delete = None;
            egui::ScrollArea::vertical().id_salt("suite").auto_shrink([false, false]).show(ui, |ui| {
                if tests.suite().cases.is_empty() {
                    ui.label(RichText::new("No cases yet. Fill in the inputs, check the result, then add them as a case.").color(t.muted));
                }
                egui::Grid::new("suite_grid").num_columns(5).spacing([14.0, 6.0]).show(ui, |ui| {
                    for (i, (c, r)) in tests.suite().cases.iter().zip(results.iter()).enumerate() {
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
                tests.fill_from_case(i);
            }
            if let Some(i) = accept {
                tests.accept(i);
            }
            if let Some(i) = delete {
                tests.delete(i);
            }
        });
    });

    let verdict = tests.verdict().expect("decision is a table");
    let target = tests.target();
    let against = verdict.against.as_deref();
    let mut add_case = false;
    let mut snapshot = false;
    let mut clear_expected = false;
    let mut clear_target = false;
    let step = ui.input_mut(|i| {
        if i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowDown) {
            1
        } else if i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowUp) {
            -1
        } else {
            0
        }
    });
    if step != 0 && !tb.rules.is_empty() {
        let next = target.map_or(0, |r| r.saturating_add_signed(step).min(tb.rules.len() - 1));
        acts.push(Act::TestRule(next));
    }
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)) {
        add_case = true;
    }
    let bad_bg = if t.dark { hex(0x3a1f22) } else { hex(0xfdeeee) };
    let target_bg = if t.dark { hex(0x1d2a4a) } else { hex(0xe8f0ff) };

    egui::CentralPanel::default().frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 8, right: 20, top: 16, bottom: 8 })).show(ui, |ui| {
        card(ui, t, |ui| {
            ui.set_min_height(ui.available_height());
            ui.horizontal(|ui| {
                ui.label(bold("Result", 15.0).color(t.text));
                pill(ui, tb.hit_policy.to_string(), t.chip, t.muted);
                if outcome.error.is_none() {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for &r in &outcome.fired {
                        if chip(ui, &format!("rule {}", r + 1), t.accent_soft, t.accent).interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            acts.push(Act::Jump(r));
                        }
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if small_button(ui, t, ic::PLUS, "Add as case").on_hover_text("Ctrl+Enter. Saves the inputs and the current result as the expected result").clicked() {
                        add_case = true;
                    }
                });
            });
            ui.add_space(6.0);
            if let Some(r) = target {
                Frame::new().fill(target_bg).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(9)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(bold(format!("Testing rule {}", r + 1), 12.5).color(t.text));
                        if outcome.error.is_some() {
                            pill(ui, "evaluation error", t.error, hex(0xffffff));
                        } else if outcome.fired.contains(&r) {
                            pill(ui, format!("{} fired", ic::CHECK), t.accent_soft, t.accent);
                        } else if let Some(&by) = outcome.fired.first() {
                            pill(ui, format!("shadowed by rule {}", by + 1), t.chip, t.warn_text);
                        } else {
                            pill(ui, "did not match", t.error, hex(0xffffff));
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if small_button(ui, t, ic::X, "Clear").clicked() {
                                clear_target = true;
                            }
                        });
                    });
                    ui.label(RichText::new("Inputs come from the rule, expected outputs from its cells.").size(11.5).color(t.muted));
                });
                ui.add_space(6.0);
            }
            let (bg, fg) = match against {
                None => (t.chip, t.muted),
                Some(_) if verdict.pass() => (t.accent_soft, t.accent),
                Some(_) => (bad_bg, t.error_text),
            };
            if let Some(e) = &outcome.error {
                Frame::new().fill(bad_bg).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(bold("Evaluation fails in Camunda", 12.5).color(t.error_text));
                    ui.add(egui::Label::new(RichText::new(e).size(12.5).color(t.error_text)).wrap());
                });
                if let Some(a) = against {
                    ui.add_space(6.0);
                    Frame::new().fill(bg).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(9)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let text = if verdict.pass() { format!("{} Pass: {a} expects this error", ic::CHECK) } else { format!("{} Fail: {a} expects a result", ic::X) };
                        ui.label(bold(text, 12.5).color(fg));
                    });
                }
            } else {
                Frame::new().fill(bg).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(9)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        match against {
                            None => ui.label(bold("No expected result", 12.5).color(fg)),
                            Some(a) if verdict.pass() => ui.label(bold(format!("{} Pass: all {} checks match {a}", ic::CHECK, verdict.checks), 12.5).color(fg)),
                            Some(a) => ui.label(bold(format!("{} Fail: {} of {} checks differ from {a}", ic::X, verdict.failed.max(1), verdict.checks.max(1)), 12.5).color(fg)),
                        };
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if against.is_some() && small_button(ui, t, ic::X, "").on_hover_text("Forget the expected result").clicked() {
                                clear_expected = true;
                            }
                            if small_button(ui, t, ic::CHECK, if against.is_none() { "Use actual as expected" } else { "Accept actual" }).clicked() {
                                snapshot = true;
                            }
                        });
                    });
                    if against.is_some() && !verdict.pass() {
                        for p in &verdict.problems {
                            ui.label(RichText::new(p).size(11.5).color(t.error_text));
                        }
                    }
                });
                if outcome.fired.is_empty() {
                    ui.label(RichText::new("No rule matched, so every output is null.").color(t.warn_text));
                }
                ui.add_space(6.0);
                let multi = verdict.rows > 1;
                egui::Grid::new("out_grid").num_columns(5).spacing([18.0, 7.0]).show(ui, |ui| {
                    for h in ["OUTPUT", "EXPECTED", "ACTUAL", "TYPE", ""] {
                        ui.label(RichText::new(h).size(10.5).color(t.faint));
                    }
                    ui.end_row();
                    for c in &verdict.cells {
                        let out = &tb.outputs[c.output];
                        let title = out.title();
                        match (&outcome.aggregate, multi) {
                            (Some(_), _) if c.output == 0 => ui.label(RichText::new(format!("{title} ({})", tb.hit_policy.aggregation().map_or("", Aggregation::name))).color(t.muted)),
                            (None, true) => ui.label(RichText::new(format!("{title} #{}", c.row + 1)).color(t.muted)),
                            _ => ui.label(RichText::new(title).color(t.muted)),
                        };
                        match (&c.expected, c.checked) {
                            (Some(w), true) => value_cell(ui, t, w, t.chip, t.text),
                            (None, true) => {
                                ui.label(RichText::new("no row").color(t.faint));
                            }
                            _ => {
                                ui.label(RichText::new("-").color(t.faint));
                            }
                        }
                        let (abg, afg) = match (c.checked, c.matches()) {
                            (true, true) => (t.accent_soft, t.accent),
                            (true, false) => (bad_bg, t.error_text),
                            _ => (t.chip, t.text),
                        };
                        match &c.actual {
                            Some(a) => {
                                value_cell(ui, t, a, abg, afg);
                                ui.label(RichText::new(format!("{} / {}", a.type_name(), out.type_ref.as_deref().unwrap_or("-"))).size(11.5).color(if c.type_mismatch {
                                    t.error_text
                                } else {
                                    t.faint
                                }));
                            }
                            None => {
                                ui.label(RichText::new("no row").color(if c.checked { t.error_text } else { t.faint }));
                                ui.label("");
                            }
                        }
                        match (c.checked, c.matches()) {
                            (true, true) => ui.label(bold(ic::CHECK, 13.0).color(t.accent)),
                            (true, false) => ui.label(bold(ic::X, 13.0).color(t.error)),
                            _ => ui.label(RichText::new("·").color(t.faint)),
                        };
                        ui.end_row();
                    }
                    if let Some((want, got)) = &verdict.rules {
                        let ok = want == got;
                        ui.label(RichText::new("Fired rules").color(t.muted));
                        ui.label(RichText::new(rule_list(want)).font(FontId::monospace(12.0)).color(t.text));
                        ui.label(RichText::new(rule_list(got)).font(FontId::monospace(12.0)).color(if ok { t.accent } else { t.error_text }));
                        ui.label("");
                        ui.label(bold(if ok { ic::CHECK } else { ic::X }, 13.0).color(if ok { t.accent } else { t.error }));
                        ui.end_row();
                    }
                });
            }
            ui.add_space(12.0);
            ui.label(bold(format!("Rule trace  ·  {} matched", outcome.matched.len()), 12.5).color(t.text));
            ui.add_space(4.0);
            let row_h = 22.0;
            let pitch = row_h + ui.spacing().item_spacing.y;
            let mut area = egui::ScrollArea::vertical().id_salt("trace").auto_shrink([false, false]);
            if let Some(r) = target.filter(|_| tester.scroll_target) {
                area = area.vertical_scroll_offset((r as f32 * pitch - ui.available_height() / 2.0).max(0.0));
                tester.scroll_target = false;
            }
            area.show_rows(ui, row_h, outcome.status.len(), |ui, range| {
                for ri in range {
                    let st = &outcome.status[ri];
                    let row = egui::Rect::from_min_size(ui.available_rect_before_wrap().min, vec2(ui.available_width(), row_h));
                    let is_target = target == Some(ri);
                    if is_target {
                        ui.painter().rect_filled(row, CornerRadius::same(5), target_bg);
                    }
                    ui.allocate_ui_with_layout(vec2(ui.available_width(), row_h), Layout::left_to_right(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        let label = RichText::new(format!("rule {}", ri + 1)).size(12.5).color(t.accent);
                        let label = if is_target { label.strong() } else { label };
                        let jump = ui.add_sized([52.0, row_h], egui::Label::new(label).sense(Sense::click())).on_hover_cursor(egui::CursorIcon::PointingHand);
                        if jump.clicked() {
                            acts.push(Act::Jump(ri));
                        }
                        let (state, detail, color) = match st {
                            RuleStatus::Matched if outcome.fired.contains(&ri) => (format!("{} fired", ic::CHECK), String::new(), t.accent),
                            RuleStatus::Matched => (format!("{} matched", ic::CHECK), "not used by the hit policy".to_string(), t.accent),
                            RuleStatus::Failed(ci) => {
                                let entry = tb.rules[ri].inputs.get(*ci).map(|e| one_line(&e.text)).unwrap_or_default();
                                let val = outcome.inputs.get(*ci).map(|v| v.display()).unwrap_or_default();
                                ("no match".to_string(), format!("{} = {val} does not satisfy {entry}", tb.inputs[*ci].title()), t.faint)
                            }
                            RuleStatus::Error(e) => ("error".to_string(), e.clone(), t.error_text),
                        };
                        ui.add_sized([76.0, row_h], egui::Label::new(RichText::new(state).size(12.5).color(color)));
                        let detail_w = (ui.available_width() - 118.0 - ui.spacing().item_spacing.x).max(40.0);
                        ui.allocate_ui_with_layout(vec2(detail_w, row_h), Layout::left_to_right(Align::Center), |ui| {
                            ui.add(egui::Label::new(RichText::new(detail).size(12.0).color(t.muted)).truncate());
                        });
                        if small_button(ui, t, ic::FLASK, "Test this rule").clicked() {
                            acts.push(Act::TestRule(ri));
                        }
                    });
                }
            });
        });
    });
    if clear_target {
        tests.clear_target();
    }
    if clear_expected {
        tests.clear_expected();
    }
    if snapshot {
        tests.snapshot();
    }
    if add_case {
        tests.add_case(&tester.case_name);
        tester.case_name.clear();
    }
}

fn rule_list(rules: &[usize]) -> String {
    if rules.is_empty() { "none".into() } else { rules.iter().map(usize::to_string).collect::<Vec<_>>().join(", ") }
}

fn value_cell(ui: &mut egui::Ui, t: &Theme, v: &Value, bg: Color32, fg: Color32) {
    if *v == Value::Null {
        ui.label(RichText::new("null").font(FontId::monospace(13.0)).color(t.faint));
    } else {
        badge_shape(ui, v.display(), BadgeShape { font: FontId::monospace(11.5), pad: 7.0, height: 22.0, radius: 5, max_w: 420.0 }, bg, fg);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dropped: Vec<PathBuf> = ctx.input(|i| if i.raw.dropped_files.is_empty() { Vec::new() } else { i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect() });
        let mut acts = Vec::new();
        self.shortcuts(&ctx, &mut acts);
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.allow_close {
                self.persist_tabs();
            } else if self.quit.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            } else if let Some(q) = self.unsaved() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.review = None;
                self.pending = None;
                self.quit = Some(q);
            } else {
                self.persist_tabs();
            }
        }
        if self.is_fixing() && self.review.is_none() && self.pending.is_none() && self.quit.is_none() && ctx.memory(|m| m.focused().is_none()) {
            let step = self.ws().is_some_and(|l| l.fix_offering());
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
        let reviewing = self.review.is_some() || self.quit.is_some() || self.is_fixing();
        if !dropped.is_empty() && !reviewing {
            self.open_paths(dropped);
        }
        if !reviewing && !self.tabs.is_empty() && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::D)) {
            acts.push(Act::Review);
        }
        if !reviewing && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            if let Some(at) = self.ws().and_then(|l| l.editing_at()) {
                acts.push(Act::EndEdit { at, commit: true });
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
        if !reviewing && self.memo.is_none() && self.selected().is_some() && ctx.memory(|m| m.focused().is_none()) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            acts.push(Act::Deselect);
        }
        let t = self.theme;

        let suite_stats = self.ws_mut().and_then(|l| {
            let mut tests = l.tests();
            if tests.suite().cases.is_empty() {
                return None;
            }
            let results = tests.results()?;
            Some((results.iter().filter(|r| r.pass).count(), results.len()))
        });

        self.chrome_bar(ui, &mut acts);
        self.notice_bar(ui);

        if !self.tabs.is_empty() {
            egui::Panel::top("tabs")
                .exact_size(tabs::HEIGHT + 6.0)
                .frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 8, right: 6, top: 6, bottom: 0 }))
                .show(ui, |ui| self.tab_strip(ui, &mut acts));
        }

        if self.shell.show_nav() {
            egui::Panel::left("nav")
                .resizable(false)
                .default_size(224.0)
                .frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 12, right: 12, top: 16, bottom: 12 }))
                .show(ui, |ui| self.nav(ui, suite_stats, &mut acts));
        }

        if let Some(e) = self.error.as_deref().or_else(|| self.ws().and_then(|l| l.error())).map(RichText::new) {
            egui::Panel::top("err").frame(Frame::new().fill(if t.dark { hex(0x3a1f22) } else { hex(0xfdeeee) }).inner_margin(Margin::symmetric(20, 8))).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(e.color(t.error_text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new(ic::X).color(t.error_text)).frame(false)).clicked() {
                            self.error = None;
                            if let Some(l) = self.ws_mut() {
                                l.clear_error();
                            }
                        }
                    });
                });
            });
        }

        if self.tabs.is_empty() {
            self.empty_view(ui, &mut acts);
            self.palette_ui(&ctx, &mut acts);
            self.help_window(&ctx);
            self.apply(&ctx, acts);
            return;
        }

        egui::Panel::top("header").exact_size(60.0).frame(Frame::new().fill(t.surface).inner_margin(Margin::symmetric(20, 0))).show(ui, |ui| self.header(ui, &mut acts));

        if !self.show_notes && self.memos.total() > 0 && self.current_memo().is_some() {
            let fill = t.warn.gamma_multiply(if t.dark { 0.18 } else { 0.12 });
            egui::Panel::top("memo_bar").frame(Frame::new().fill(fill).inner_margin(Margin::symmetric(20, 8))).show(ui, |ui| self.memo_strip(ui, &mut acts));
        }

        if let Some(v) = self.ws_mut().and_then(|l| l.fix_view()) {
            egui::Panel::right("fix_panel").resizable(true).default_size(400.0).frame(Frame::new().fill(t.surface).inner_margin(Margin::same(0))).show(ui, |ui| fix_panel(ui, &t, &v, &mut acts));
        }

        let uid = self.tab().map_or(0, |tab| tab.uid);
        ui.push_id(uid, |ui| match self.view() {
            _ if self.show_notes => self.notes_view(ui, &mut acts),
            View::Rules => self.rules_view(ui, &mut acts),
            View::Issues => self.issues_view(ui, &mut acts),
            View::Xml => self.xml_view(ui),
            View::Test => {
                if let Some(Tab { ws: Some(ws), tester, .. }) = self.tabs.get_mut(self.active) {
                    let selected = ws.selected();
                    test_view(ui, ws, selected, tester, &t, &mut acts);
                }
            }
        });
        self.palette_ui(&ctx, &mut acts);
        self.apply(&ctx, acts);
        self.header_window(&ctx);
        self.replace_window(&ctx);
        self.help_window(&ctx);
        self.review_modal(&ctx);
        self.pending_modal(&ctx);
        self.quit_modal(&ctx);
        self.memo_popover(&ctx);
    }
}

#[derive(Default)]
struct Args {
    path: Option<PathBuf>,
    view: Option<View>,
    case: Option<usize>,
    rule: Option<usize>,
    chrome: Option<menu::Chrome>,
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
            "--chrome" | "--layout" => a.chrome = it.next().and_then(|v| menu::Chrome::parse(&v.to_string_lossy())),
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
