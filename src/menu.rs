use crate::{Act, App, Pending, View, bold, hex, ic, small_button};
use dmn_viewer::export::Format;
use dmn_viewer::lint;
use dmn_viewer::workspace::RuleOp;
use eframe::egui::{self, Align, Align2, CornerRadius, FontId, Frame, Key, KeyboardShortcut, Layout, Margin, Modifiers, RichText, Sense, Stroke, vec2};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

const RECENT_MAX: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Chrome {
    #[default]
    Classic,
    Palette,
}

impl Chrome {
    fn name(self) -> &'static str {
        match self {
            Chrome::Classic => "classic",
            Chrome::Palette => "palette",
        }
    }

    pub fn parse(s: &str) -> Option<Chrome> {
        match s.trim().to_ascii_lowercase().as_str() {
            "classic" | "menu" => Some(Chrome::Classic),
            "palette" | "toolbar" => Some(Chrome::Palette),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct Settings {
    pub chrome: Chrome,
    pub compact: bool,
    pub recent: Vec<PathBuf>,
}

fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("XDG_CONFIG_HOME")).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("dmn-viewer").join("settings.txt"))
}

impl Settings {
    pub fn load() -> Settings {
        let text = settings_path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        Settings::parse(&text)
    }

    fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            match line.split_once('=') {
                Some(("chrome", v)) => s.chrome = Chrome::parse(v).unwrap_or_default(),
                Some(("compact", v)) => s.compact = v.trim() == "1",
                Some(("recent", v)) if !v.is_empty() && s.recent.len() < RECENT_MAX => s.recent.push(PathBuf::from(v)),
                _ => {}
            }
        }
        s
    }

    fn render(&self) -> String {
        let mut out = format!("chrome={}\ncompact={}\n", self.chrome.name(), u8::from(self.compact));
        for p in &self.recent {
            out.push_str(&format!("recent={}\n", p.display()));
        }
        out
    }

    fn save(&self) {
        let Some(path) = settings_path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, self.render());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Open,
    OpenRecent(usize),
    ClearRecent,
    Reload,
    Revert,
    Save,
    SaveAs,
    Export(Format),
    Quit,
    Undo,
    Redo,
    Find,
    Replace,
    InsertAbove,
    InsertBelow,
    Duplicate,
    DeleteRule,
    MoveUp,
    MoveDown,
    CopyRule,
    PasteRule,
    Show(View),
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Density,
    Sidebar,
    Languages,
    ReviewChanges,
    Theme,
    SetChrome(Chrome),
    Go(usize),
    PrevIssue,
    NextIssue,
    FixIssues,
    ResetSkipped,
    RunTests,
    CopyReport,
    Palette,
    Shortcuts,
    About,
}

const STATIC: &[Cmd] = &[
    Cmd::Open,
    Cmd::Reload,
    Cmd::Revert,
    Cmd::Save,
    Cmd::SaveAs,
    Cmd::Export(Format::Csv),
    Cmd::Export(Format::Markdown),
    Cmd::Export(Format::Json),
    Cmd::ClearRecent,
    Cmd::Quit,
    Cmd::Undo,
    Cmd::Redo,
    Cmd::Find,
    Cmd::Replace,
    Cmd::InsertAbove,
    Cmd::InsertBelow,
    Cmd::Duplicate,
    Cmd::DeleteRule,
    Cmd::MoveUp,
    Cmd::MoveDown,
    Cmd::CopyRule,
    Cmd::PasteRule,
    Cmd::Show(View::Rules),
    Cmd::Show(View::Issues),
    Cmd::Show(View::Test),
    Cmd::Show(View::Xml),
    Cmd::ZoomIn,
    Cmd::ZoomOut,
    Cmd::ZoomReset,
    Cmd::Density,
    Cmd::Sidebar,
    Cmd::Languages,
    Cmd::ReviewChanges,
    Cmd::Theme,
    Cmd::SetChrome(Chrome::Classic),
    Cmd::SetChrome(Chrome::Palette),
    Cmd::PrevIssue,
    Cmd::NextIssue,
    Cmd::FixIssues,
    Cmd::ResetSkipped,
    Cmd::RunTests,
    Cmd::CopyReport,
    Cmd::Palette,
    Cmd::Shortcuts,
    Cmd::About,
];

enum Bind {
    Own(Modifiers, Key),
    Elsewhere(Modifiers, Key),
    Text(&'static str),
}

fn keys(c: Cmd) -> Option<Bind> {
    let ctrl = Modifiers::COMMAND;
    let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
    Some(match c {
        Cmd::Open => Bind::Elsewhere(ctrl, Key::O),
        Cmd::Reload => Bind::Elsewhere(Modifiers::NONE, Key::F5),
        Cmd::Save => Bind::Elsewhere(ctrl, Key::S),
        Cmd::SaveAs => Bind::Own(ctrl_shift, Key::S),
        Cmd::Quit => Bind::Own(ctrl, Key::Q),
        Cmd::Undo => Bind::Elsewhere(ctrl, Key::Z),
        Cmd::Redo => Bind::Elsewhere(ctrl, Key::Y),
        Cmd::Find => Bind::Own(ctrl, Key::F),
        Cmd::Replace => Bind::Own(ctrl, Key::H),
        Cmd::InsertAbove => Bind::Own(ctrl_shift, Key::Enter),
        Cmd::InsertBelow => Bind::Own(ctrl, Key::Enter),
        Cmd::Duplicate => Bind::Own(ctrl, Key::J),
        Cmd::DeleteRule => Bind::Own(ctrl, Key::Delete),
        Cmd::MoveUp => Bind::Own(Modifiers::ALT, Key::ArrowUp),
        Cmd::MoveDown => Bind::Own(Modifiers::ALT, Key::ArrowDown),
        Cmd::CopyRule => Bind::Own(ctrl_shift, Key::C),
        Cmd::PasteRule => Bind::Own(ctrl_shift, Key::V),
        Cmd::Show(View::Rules) => Bind::Own(ctrl, Key::Num1),
        Cmd::Show(View::Issues) => Bind::Own(ctrl, Key::Num2),
        Cmd::Show(View::Test) => Bind::Own(ctrl, Key::Num3),
        Cmd::Show(View::Xml) => Bind::Own(ctrl, Key::Num4),
        Cmd::ZoomIn => Bind::Text("Ctrl++"),
        Cmd::ZoomOut => Bind::Text("Ctrl+-"),
        Cmd::ZoomReset => Bind::Text("Ctrl+0"),
        Cmd::Sidebar => Bind::Own(ctrl, Key::B),
        Cmd::ReviewChanges => Bind::Elsewhere(ctrl, Key::D),
        Cmd::PrevIssue => Bind::Own(Modifiers::SHIFT, Key::F8),
        Cmd::NextIssue => Bind::Own(Modifiers::NONE, Key::F8),
        Cmd::FixIssues => Bind::Own(ctrl_shift, Key::X),
        Cmd::RunTests => Bind::Own(ctrl, Key::R),
        Cmd::Palette => Bind::Own(ctrl, Key::K),
        Cmd::Shortcuts => Bind::Own(Modifiers::NONE, Key::F1),
        _ => return None,
    })
}

fn needs_idle(c: Cmd) -> bool {
    matches!(c, Cmd::InsertAbove | Cmd::InsertBelow | Cmd::Duplicate | Cmd::DeleteRule | Cmd::MoveUp | Cmd::MoveDown | Cmd::CopyRule | Cmd::PasteRule)
}

pub struct State {
    doc: bool,
    editable: bool,
    dirty: bool,
    locked: bool,
    table: bool,
    nrules: usize,
    selected: Option<usize>,
    can_undo: bool,
    can_redo: bool,
    fixable: usize,
    skipped: usize,
    has_clip: bool,
    view: View,
    dark: bool,
    compact: bool,
    sidebar: bool,
    langs: bool,
    chrome: Chrome,
}

pub struct Info {
    label: Cow<'static, str>,
    icon: &'static str,
    group: &'static str,
    enabled: bool,
    on: Option<bool>,
    tip: Option<String>,
}

#[derive(Default)]
pub struct Palette {
    query: String,
    sel: usize,
    scroll: bool,
}

pub struct Replace {
    find: String,
    with: String,
    case: bool,
    focus: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Help {
    Shortcuts,
    About,
}

#[derive(Default)]
pub struct Shell {
    palette: Option<Palette>,
    replace: Option<Replace>,
    help: Option<Help>,
    notice: Option<String>,
    pub focus_filter: bool,
    hide_nav: bool,
}

impl Shell {
    pub fn show_nav(&self) -> bool {
        !self.hide_nav
    }
}

fn fuzzy(query: &str, text: &str) -> Option<i32> {
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut at, mut last, mut score) = (0, None, 0);
    for q in query.to_lowercase().chars().filter(|c| !c.is_whitespace()) {
        let found = at + t[at..].iter().position(|&c| c == q)?;
        score -= (found - at) as i32;
        if found == 0 || !t[found - 1].is_alphanumeric() {
            score += 5;
        }
        if last.is_some_and(|l| l + 1 == found) {
            score += 3;
        }
        last = Some(found);
        at = found + 1;
    }
    Some(score)
}

fn info_of(label: Cow<'static, str>, icon: &'static str, group: &'static str, enabled: bool) -> Info {
    Info { label, icon, group, enabled, on: None, tip: None }
}

fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(|| p.display().to_string(), |f| f.to_string_lossy().into_owned())
}

impl App {
    pub fn row_height(&self) -> f32 {
        if self.settings.compact { 30.0 } else { 40.0 }
    }

    pub fn remember(&mut self, path: PathBuf) {
        self.settings.recent.retain(|p| *p != path);
        self.settings.recent.insert(0, path);
        self.settings.recent.truncate(RECENT_MAX);
        self.settings.save();
    }

    fn state(&self) -> State {
        let l = self.loaded.as_ref();
        State {
            doc: l.is_some(),
            editable: l.is_some_and(|l| l.editable()),
            dirty: l.is_some_and(|l| l.dirty()),
            locked: self.review.is_some() || self.is_fixing(),
            table: l.and_then(|l| l.defs().table(self.decision())).is_some(),
            nrules: l.and_then(|l| l.defs().table(self.decision())).map_or(0, |t| t.rules.len()),
            selected: self.selected(),
            can_undo: l.is_some_and(|l| l.undo_depth() > 0),
            can_redo: l.is_some_and(|l| l.can_redo()),
            fixable: l.map_or(0, |l| l.fixable_count()),
            skipped: l.map_or(0, |l| l.skipped_count()),
            has_clip: l.is_some_and(|l| l.has_clip()),
            view: self.view(),
            dark: self.theme.dark,
            compact: self.settings.compact,
            sidebar: self.shell.show_nav(),
            langs: self.show_langs,
            chrome: self.settings.chrome,
        }
    }

    fn info(&self, c: Cmd, s: &State) -> Info {
        let edit = s.doc && s.editable && !s.locked;
        let rule = edit && s.table && s.selected.is_some_and(|r| r < s.nrules);
        let i = |label: &'static str, icon, group, enabled| info_of(Cow::Borrowed(label), icon, group, enabled);
        let toggle = |mut inf: Info, on| {
            inf.on = Some(on);
            inf
        };
        match c {
            Cmd::Open => i("Open…", ic::OPEN, "File", !s.locked),
            Cmd::OpenRecent(n) => {
                let p = self.settings.recent.get(n);
                Info { tip: p.map(|p| p.display().to_string()), ..info_of(Cow::Owned(p.map_or_else(String::new, |p| file_name(p))), ic::NEW, "Recent", p.is_some() && !s.locked) }
            }
            Cmd::ClearRecent => i("Clear recent files", ic::ERASER, "File", !self.settings.recent.is_empty()),
            Cmd::Reload => i("Reload", ic::RELOAD, "File", s.doc && !s.locked),
            Cmd::Revert => i("Revert to saved", ic::UNDO_ALL, "File", s.dirty && !s.locked),
            Cmd::Save => i("Save", ic::SAVE, "File", s.dirty && !s.locked),
            Cmd::SaveAs => i("Save as…", ic::SAVE_AS, "File", s.doc && s.editable && !s.locked),
            Cmd::Export(f) => info_of(Cow::Owned(format!("Export as {}…", f.label())), ic::EXPORT, "File", s.table),
            Cmd::Quit => i("Exit", ic::X, "File", true),
            Cmd::Undo => i("Undo", ic::UNDO, "Edit", s.doc && s.can_undo),
            Cmd::Redo => i("Redo", ic::REDO, "Edit", s.doc && s.can_redo && !s.locked),
            Cmd::Find => i("Find rules", ic::SEARCH, "Edit", s.doc && !s.locked),
            Cmd::Replace => i("Find and replace…", ic::REPLACE, "Edit", edit && s.table),
            Cmd::InsertAbove => i("Insert rule above", ic::ROW_ABOVE, "Edit", rule),
            Cmd::InsertBelow => i("Insert rule below", ic::PLUS, "Edit", edit && s.table),
            Cmd::Duplicate => i("Duplicate rule", ic::COPY, "Edit", rule),
            Cmd::DeleteRule => i("Delete rule", ic::TRASH, "Edit", rule),
            Cmd::MoveUp => i("Move rule up", ic::UP, "Edit", rule && s.selected.is_some_and(|r| r > 0)),
            Cmd::MoveDown => i("Move rule down", ic::DOWN, "Edit", rule && s.selected.is_some_and(|r| r + 1 < s.nrules)),
            Cmd::CopyRule => i("Copy rule", ic::CLIPBOARD, "Edit", s.doc && s.table && s.selected.is_some_and(|r| r < s.nrules) && !s.locked),
            Cmd::PasteRule => i("Paste rule", ic::PASTE, "Edit", edit && s.table && s.has_clip),
            Cmd::Show(v) => {
                let (label, icon) = match v {
                    View::Rules => ("Rules", ic::TABLE),
                    View::Issues => ("Issues", ic::WARNING),
                    View::Test => ("Test", ic::FLASK),
                    View::Xml => ("XML", ic::CODE),
                };
                toggle(i(label, icon, "View", s.doc), s.view == v)
            }
            Cmd::ZoomIn => i("Zoom in", ic::ZOOM_IN, "View", true),
            Cmd::ZoomOut => i("Zoom out", ic::ZOOM_OUT, "View", true),
            Cmd::ZoomReset => i("Reset zoom", ic::SEARCH, "View", true),
            Cmd::Density => toggle(i("Compact rows", ic::ROWS, "View", true), s.compact),
            Cmd::Sidebar => toggle(i("Show sidebar", ic::SIDEBAR, "View", true), s.sidebar),
            Cmd::Languages => toggle(i("Show expression languages", ic::CODE, "View", s.doc), s.langs),
            Cmd::ReviewChanges => i("Compare with saved file", ic::DIFF, "View", s.dirty && !s.locked),
            Cmd::Theme => i(if s.dark { "Light mode" } else { "Dark mode" }, if s.dark { ic::SUN } else { ic::MOON }, "View", true),
            Cmd::SetChrome(Chrome::Classic) => toggle(i("Layout: menu bar", ic::LIST, "View", true), s.chrome == Chrome::Classic),
            Cmd::SetChrome(Chrome::Palette) => toggle(i("Layout: toolbar and palette", ic::COMMAND, "View", true), s.chrome == Chrome::Palette),
            Cmd::Go(n) => {
                let name = self.loaded.as_ref().and_then(|l| l.defs().decisions.get(n)).map(|d| if d.name.is_empty() { d.id.clone() } else { d.name.clone() }).unwrap_or_default();
                toggle(info_of(Cow::Owned(name), ic::DECISION, "Go to", s.doc), s.doc && self.decision() == n)
            }
            Cmd::PrevIssue => i("Previous issue", ic::UP, "Go", s.doc && s.table),
            Cmd::NextIssue => i("Next issue", ic::DOWN, "Go", s.doc && s.table),
            Cmd::FixIssues => info_of(if s.fixable > 0 { Cow::Owned(format!("Fix issues ({})…", s.fixable)) } else { Cow::Borrowed("Fix issues…") }, ic::CHECK, "Tools", s.fixable > 0 && !s.locked),
            Cmd::ResetSkipped => i("Reset skipped fixes", ic::RELOAD, "Tools", s.skipped > 0),
            Cmd::RunTests => i("Run tests", ic::PLAY, "Tools", s.doc),
            Cmd::CopyReport => i("Copy issues report", ic::COPY, "Tools", s.doc),
            Cmd::Palette => i("Command palette", ic::COMMAND, "Help", true),
            Cmd::Shortcuts => i("Keyboard shortcuts", ic::KEYBOARD, "Help", true),
            Cmd::About => i("About DMN Viewer", ic::INFO, "Help", true),
        }
    }

    fn key_text(&self, ctx: &egui::Context, c: Cmd) -> Option<String> {
        match keys(c)? {
            Bind::Own(m, k) | Bind::Elsewhere(m, k) => Some(ctx.format_shortcut(&KeyboardShortcut::new(m, k))),
            Bind::Text(t) => Some(t.to_string()),
        }
    }

    pub fn shortcuts(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        if self.pending.is_some() || !ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))) {
            return;
        }
        let s = self.state();
        let idle = ctx.memory(|m| m.focused().is_none());
        for shifted in [true, false] {
            for &c in STATIC {
                let Some(Bind::Own(m, k)) = keys(c) else { continue };
                if m.shift != shifted || (needs_idle(c) && !idle) || !ctx.input(|i| i.key_pressed(k) && i.modifiers.matches_logically(m)) || !self.info(c, &s).enabled {
                    continue;
                }
                if ctx.input_mut(|i| i.consume_key(m, k)) {
                    acts.push(Act::Cmd(c));
                }
            }
        }
    }

    pub fn run_cmd(&mut self, ctx: &egui::Context, c: Cmd) {
        self.shell.notice = None;
        if let Some(l) = self.loaded.as_mut() {
            l.clear_notice();
            l.flush_editing();
        }
        let act = |app: &mut App, a: Act| app.apply(ctx, vec![a]);
        match c {
            Cmd::Open => act(self, Act::Open),
            Cmd::OpenRecent(n) => {
                if let Some(p) = self.settings.recent.get(n).cloned() {
                    self.guard(ctx, Pending::Load(p));
                }
            }
            Cmd::ClearRecent => {
                self.settings.recent.clear();
                self.settings.save();
            }
            Cmd::Reload => act(self, Act::Reload),
            Cmd::Revert => {
                if let Some(l) = self.loaded.as_mut() {
                    l.discard();
                }
            }
            Cmd::Save => act(self, Act::Save),
            Cmd::SaveAs => self.save_as(),
            Cmd::Export(f) => self.export(f),
            Cmd::Quit => self.guard(ctx, Pending::Close),
            Cmd::Undo => act(self, Act::Undo),
            Cmd::Redo => act(self, Act::Redo),
            Cmd::Find => {
                if let Some(l) = self.loaded.as_mut() {
                    l.set_view(View::Rules);
                }
                self.shell.focus_filter = true;
            }
            Cmd::Replace => self.shell.replace = Some(Replace { find: String::new(), with: String::new(), case: false, focus: true }),
            Cmd::InsertAbove | Cmd::InsertBelow | Cmd::Duplicate | Cmd::DeleteRule | Cmd::MoveUp | Cmd::MoveDown => {
                let op = match c {
                    Cmd::InsertAbove => RuleOp::InsertAbove,
                    Cmd::InsertBelow => RuleOp::InsertBelow,
                    Cmd::Duplicate => RuleOp::Duplicate,
                    Cmd::DeleteRule => RuleOp::Delete,
                    Cmd::MoveUp => RuleOp::MoveUp,
                    _ => RuleOp::MoveDown,
                };
                if let Some(l) = self.loaded.as_mut() {
                    l.rule_op(op);
                }
            }
            Cmd::CopyRule => {
                if let Some(l) = self.loaded.as_mut() {
                    l.copy_rule();
                }
            }
            Cmd::PasteRule => {
                if let Some(l) = self.loaded.as_mut() {
                    l.paste_rule();
                }
            }
            Cmd::Show(v) => act(self, Act::View(v)),
            Cmd::ZoomIn => ctx.set_zoom_factor((ctx.zoom_factor() * 1.1).min(3.0)),
            Cmd::ZoomOut => ctx.set_zoom_factor((ctx.zoom_factor() / 1.1).max(0.5)),
            Cmd::ZoomReset => ctx.set_zoom_factor(1.0),
            Cmd::Density => {
                self.settings.compact = !self.settings.compact;
                self.settings.save();
            }
            Cmd::Sidebar => self.shell.hide_nav = !self.shell.hide_nav,
            Cmd::Languages => act(self, Act::ToggleLangs),
            Cmd::ReviewChanges => act(self, Act::Review),
            Cmd::Theme => act(self, Act::ToggleDark),
            Cmd::SetChrome(ch) => {
                self.settings.chrome = ch;
                self.settings.save();
            }
            Cmd::Go(n) => act(self, Act::Decision(n)),
            Cmd::PrevIssue => {
                if let Some(l) = self.loaded.as_mut() {
                    l.step_issue(false);
                }
            }
            Cmd::NextIssue => {
                if let Some(l) = self.loaded.as_mut() {
                    l.step_issue(true);
                }
            }
            Cmd::FixIssues => act(self, Act::FixStart(None)),
            Cmd::ResetSkipped => act(self, Act::FixResetSkipped),
            Cmd::RunTests => act(self, Act::View(View::Test)),
            Cmd::CopyReport => {
                if let Some(l) = &self.loaded {
                    ctx.copy_text(lint::report(l.defs(), l.issues(), &l.path().display().to_string()));
                    self.shell.notice = Some("Issues report copied".into());
                }
            }
            Cmd::Palette => self.shell.palette = if self.shell.palette.is_some() { None } else { Some(Palette::default()) },
            Cmd::Shortcuts => self.shell.help = Some(Help::Shortcuts),
            Cmd::About => self.shell.help = Some(Help::About),
        }
    }

    fn save_as(&mut self) {
        let Some(l) = self.loaded.as_ref().filter(|l| l.editable()) else { return };
        let mut dialog = rfd::FileDialog::new().add_filter("DMN", &["dmn", "xml"]).set_file_name(file_name(l.path()));
        if let Some(dir) = l.path().parent() {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.save_file() else { return };
        let Some(l) = self.loaded.as_mut() else { return };
        if l.save_as(path.clone()) {
            self.shell.notice = Some(format!("Saved as {}", file_name(&path)));
            self.remember(path);
        }
    }

    fn export(&mut self, f: Format) {
        let Some((name, text)) = self.loaded.as_ref().and_then(|l| l.export(f)) else { return };
        let safe: String = name.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' }).collect();
        let Some(path) = rfd::FileDialog::new().add_filter(f.label(), &[f.extension()]).set_file_name(format!("{safe}.{}", f.extension())).save_file() else { return };
        match std::fs::write(&path, text) {
            Ok(()) => self.shell.notice = Some(format!("Exported {}", file_name(&path))),
            Err(e) => self.error = Some(format!("Could not write {}: {e}", path.display())),
        }
    }

    pub fn chrome_bar(&mut self, ui: &mut egui::Ui, acts: &mut Vec<Act>) {
        let t = self.theme;
        let s = self.state();
        match self.settings.chrome {
            Chrome::Classic => {
                egui::Panel::top("menubar").exact_size(34.0).frame(Frame::new().fill(t.surface).inner_margin(Margin::symmetric(12, 0)).stroke(Stroke::new(1.0, t.line))).show(ui, |ui| {
                    egui::MenuBar::new().ui(ui, |ui| {
                        ui.spacing_mut().button_padding = vec2(10.0, 4.0);
                        let tops = [
                            ui.menu_button("File", |ui| self.menu_file(ui, &s, acts)).response,
                            ui.menu_button("Edit", |ui| self.menu_edit(ui, &s, acts)).response,
                            ui.menu_button("View", |ui| self.menu_view(ui, &s, acts)).response,
                            ui.menu_button("Go", |ui| self.menu_go(ui, &s, acts)).response,
                            ui.menu_button("Tools", |ui| self.menu_tools(ui, &s, acts)).response,
                            ui.menu_button("Help", |ui| self.menu_help(ui, &s, acts)).response,
                        ];
                        let ctx = ui.ctx();
                        if egui::Popup::is_any_open(ctx)
                            && let Some(over) = ctx.pointer_hover_pos().and_then(|pos| tops.iter().find(|r| r.rect.contains(pos)))
                        {
                            let id = egui::Popup::default_response_id(over);
                            if !egui::Popup::is_id_open(ctx, id) {
                                egui::Popup::open_id(ctx, id);
                                ctx.request_repaint();
                            }
                        }
                    });
                });
            }
            Chrome::Palette => {
                egui::Panel::top("toolbar")
                    .exact_size(46.0)
                    .frame(Frame::new().fill(t.surface).inner_margin(Margin::symmetric(14, 0)).stroke(Stroke::new(1.0, t.line)))
                    .show(ui, |ui| self.toolbar(ui, &s, acts));
            }
        }
    }

    fn item(&self, ui: &mut egui::Ui, s: &State, c: Cmd, acts: &mut Vec<Act>) {
        let inf = self.info(c, s);
        let mut b = egui::Button::new(format!("{}  {}", inf.icon, inf.label));
        if let Some(k) = self.key_text(ui.ctx(), c) {
            b = b.shortcut_text(k);
        }
        if let Some(on) = inf.on {
            b = b.selected(on);
        }
        let mut r = ui.add_enabled(inf.enabled, b);
        if let Some(tip) = &inf.tip {
            r = r.on_hover_text(tip);
        }
        if r.clicked() {
            acts.push(Act::Cmd(c));
            ui.close();
        }
    }

    fn menu_file(&self, ui: &mut egui::Ui, s: &State, acts: &mut Vec<Act>) {
        ui.set_min_width(250.0);
        self.item(ui, s, Cmd::Open, acts);
        ui.menu_button(format!("{}  Open recent", ic::CLOCK), |ui| {
            if self.settings.recent.is_empty() {
                ui.add_enabled(false, egui::Label::new("No recent files"));
            }
            for n in 0..self.settings.recent.len() {
                self.item(ui, s, Cmd::OpenRecent(n), acts);
            }
            if !self.settings.recent.is_empty() {
                ui.separator();
                self.item(ui, s, Cmd::ClearRecent, acts);
            }
        });
        self.item(ui, s, Cmd::Reload, acts);
        self.item(ui, s, Cmd::Revert, acts);
        ui.separator();
        self.item(ui, s, Cmd::Save, acts);
        self.item(ui, s, Cmd::SaveAs, acts);
        ui.menu_button(format!("{}  Export table", ic::EXPORT), |ui| {
            for f in Format::ALL {
                self.item(ui, s, Cmd::Export(f), acts);
            }
        });
        ui.separator();
        self.item(ui, s, Cmd::Quit, acts);
    }

    fn menu_edit(&self, ui: &mut egui::Ui, s: &State, acts: &mut Vec<Act>) {
        ui.set_min_width(250.0);
        for c in [Cmd::Undo, Cmd::Redo] {
            self.item(ui, s, c, acts);
        }
        ui.separator();
        for c in [Cmd::Find, Cmd::Replace] {
            self.item(ui, s, c, acts);
        }
        ui.separator();
        for c in [Cmd::InsertAbove, Cmd::InsertBelow, Cmd::Duplicate, Cmd::DeleteRule, Cmd::MoveUp, Cmd::MoveDown] {
            self.item(ui, s, c, acts);
        }
        ui.separator();
        for c in [Cmd::CopyRule, Cmd::PasteRule] {
            self.item(ui, s, c, acts);
        }
    }

    fn menu_view(&self, ui: &mut egui::Ui, s: &State, acts: &mut Vec<Act>) {
        ui.set_min_width(270.0);
        for v in [View::Rules, View::Issues, View::Test, View::Xml] {
            self.item(ui, s, Cmd::Show(v), acts);
        }
        ui.separator();
        for c in [Cmd::ZoomIn, Cmd::ZoomOut, Cmd::ZoomReset] {
            self.item(ui, s, c, acts);
        }
        ui.separator();
        for c in [Cmd::Density, Cmd::Sidebar, Cmd::Languages, Cmd::ReviewChanges, Cmd::Theme] {
            self.item(ui, s, c, acts);
        }
        ui.separator();
        ui.menu_button(format!("{}  Layout", ic::LAYOUT), |ui| {
            for ch in [Chrome::Classic, Chrome::Palette] {
                self.item(ui, s, Cmd::SetChrome(ch), acts);
            }
        });
    }

    fn menu_go(&self, ui: &mut egui::Ui, s: &State, acts: &mut Vec<Act>) {
        ui.set_min_width(250.0);
        let n = self.loaded.as_ref().map_or(0, |l| l.defs().decisions.len());
        if n == 0 {
            ui.add_enabled(false, egui::Label::new("No file open"));
        }
        for i in 0..n {
            self.item(ui, s, Cmd::Go(i), acts);
        }
        ui.separator();
        for c in [Cmd::PrevIssue, Cmd::NextIssue] {
            self.item(ui, s, c, acts);
        }
    }

    fn menu_tools(&self, ui: &mut egui::Ui, s: &State, acts: &mut Vec<Act>) {
        ui.set_min_width(250.0);
        for c in [Cmd::RunTests, Cmd::FixIssues, Cmd::ResetSkipped, Cmd::CopyReport] {
            self.item(ui, s, c, acts);
        }
    }

    fn menu_help(&self, ui: &mut egui::Ui, s: &State, acts: &mut Vec<Act>) {
        ui.set_min_width(230.0);
        for c in [Cmd::Palette, Cmd::Shortcuts, Cmd::About] {
            self.item(ui, s, c, acts);
        }
    }

    fn tool(&self, ui: &mut egui::Ui, s: &State, c: Cmd, acts: &mut Vec<Act>) {
        let t = self.theme;
        let inf = self.info(c, s);
        let ctx = ui.ctx().clone();
        let tip = |ui: &mut egui::Ui| {
            ui.label(match self.key_text(&ctx, c) {
                Some(k) => format!("{}  ({k})", inf.label),
                None => inf.label.to_string(),
            });
        };
        let r = ui.add_enabled_ui(inf.enabled, |ui| small_button(ui, &t, inf.icon, "")).inner;
        if r.on_hover_ui(tip).on_disabled_hover_ui(tip).clicked() {
            acts.push(Act::Cmd(c));
        }
    }

    fn toolbar(&self, ui: &mut egui::Ui, s: &State, acts: &mut Vec<Act>) {
        let t = self.theme;
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let groups: [&[Cmd]; 5] = [
                &[Cmd::Open, Cmd::Save, Cmd::SaveAs],
                &[Cmd::Undo, Cmd::Redo],
                &[Cmd::InsertBelow, Cmd::Duplicate, Cmd::DeleteRule, Cmd::MoveUp, Cmd::MoveDown],
                &[Cmd::Find, Cmd::Replace, Cmd::PrevIssue, Cmd::NextIssue],
                &[Cmd::RunTests, Cmd::FixIssues],
            ];
            for (n, g) in groups.iter().enumerate() {
                if n > 0 {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                }
                for &c in *g {
                    self.tool(ui, s, c, acts);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.menu_button(RichText::new(ic::LIST).size(15.0), |ui| {
                    ui.set_min_width(180.0);
                    ui.menu_button("File", |ui| self.menu_file(ui, s, acts));
                    ui.menu_button("Edit", |ui| self.menu_edit(ui, s, acts));
                    ui.menu_button("View", |ui| self.menu_view(ui, s, acts));
                    ui.menu_button("Go", |ui| self.menu_go(ui, s, acts));
                    ui.menu_button("Tools", |ui| self.menu_tools(ui, s, acts));
                    ui.menu_button("Help", |ui| self.menu_help(ui, s, acts));
                });
                self.tool(ui, s, Cmd::Theme, acts);
                let hint = RichText::new(format!("{}  Search commands", ic::SEARCH)).color(t.muted);
                let mut pill =
                    egui::Button::new(hint).fill(if t.dark { t.canvas } else { hex(0xf9fafb) }).stroke(Stroke::new(1.0, t.line)).corner_radius(CornerRadius::same(8)).min_size(vec2(260.0, 28.0));
                if let Some(k) = self.key_text(ui.ctx(), Cmd::Palette) {
                    pill = pill.shortcut_text(k);
                }
                if ui.add(pill).clicked() {
                    acts.push(Act::Cmd(Cmd::Palette));
                }
            });
        });
    }

    pub fn notice_bar(&mut self, ui: &mut egui::Ui) {
        let Some(text) = self.loaded.as_ref().and_then(|l| l.notice()).map(str::to_string).or_else(|| self.shell.notice.clone()) else { return };
        let t = self.theme;
        let mut dismissed = false;
        egui::Panel::top("notice").frame(Frame::new().fill(t.accent_soft).inner_margin(Margin::symmetric(20, 6))).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(text).color(t.accent));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    dismissed = ui.add(egui::Button::new(RichText::new(ic::X).color(t.accent)).frame(false)).clicked();
                });
            });
        });
        if dismissed {
            self.shell.notice = None;
            if let Some(l) = self.loaded.as_mut() {
                l.clear_notice();
            }
        }
    }

    fn palette_items(&self, s: &State, query: &str) -> Vec<(Cmd, Info, i32)> {
        let n_decisions = self.loaded.as_ref().map_or(0, |l| l.defs().decisions.len());
        let dynamic = (0..n_decisions).map(Cmd::Go).chain((0..self.settings.recent.len()).map(Cmd::OpenRecent));
        let mut items: Vec<(Cmd, Info, i32)> = STATIC
            .iter()
            .copied()
            .chain(dynamic)
            .filter_map(|c| {
                let inf = self.info(c, s);
                let score = fuzzy(query, &format!("{} {}", inf.group, inf.label))?;
                Some((c, inf, score))
            })
            .collect();
        items.sort_by_key(|(_, inf, score)| (!inf.enabled, -score));
        items
    }

    pub fn palette_ui(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        let Some(mut p) = self.shell.palette.take() else { return };
        let t = self.theme;
        let s = self.state();
        let items = self.palette_items(&s, &p.query);
        let n = items.len();
        let (down, up, enter, esc) = ctx.input_mut(|i| {
            (i.consume_key(Modifiers::NONE, Key::ArrowDown), i.consume_key(Modifiers::NONE, Key::ArrowUp), i.consume_key(Modifiers::NONE, Key::Enter), i.consume_key(Modifiers::NONE, Key::Escape))
        });
        if down {
            p.sel = (p.sel + 1).min(n.saturating_sub(1));
            p.scroll = true;
        }
        if up {
            p.sel = p.sel.saturating_sub(1);
            p.scroll = true;
        }
        p.sel = p.sel.min(n.saturating_sub(1));
        let mut run = enter.then(|| items.get(p.sel).filter(|it| it.1.enabled).map(|it| it.0)).flatten();
        let mut close = esc;
        let width = 540.0;
        let area = egui::Area::new(egui::Id::new("palette")).order(egui::Order::Foreground).anchor(Align2::CENTER_TOP, vec2(0.0, 70.0)).show(ctx, |ui| {
            Frame::popup(ui.style()).fill(t.surface).stroke(Stroke::new(1.0, t.line)).corner_radius(CornerRadius::same(12)).inner_margin(Margin::same(10)).show(ui, |ui| {
                ui.set_width(width);
                let edit = ui.add(egui::TextEdit::singleline(&mut p.query).hint_text("Type a command…").desired_width(f32::INFINITY).margin(Margin::symmetric(8, 6)).font(FontId::proportional(15.0)));
                edit.request_focus();
                if edit.changed() {
                    p.sel = 0;
                    p.scroll = true;
                }
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(360.0).auto_shrink([false, true]).show(ui, |ui| {
                    if items.is_empty() {
                        ui.label(RichText::new("No matching command").color(t.muted));
                    }
                    for (idx, (c, inf, _)) in items.iter().enumerate() {
                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
                        let active = idx == p.sel;
                        if active {
                            ui.painter().rect_filled(rect, CornerRadius::same(6), t.accent_soft);
                            if p.scroll {
                                resp.scroll_to_me(None);
                            }
                        } else if resp.hovered() && inf.enabled {
                            ui.painter().rect_filled(rect, CornerRadius::same(6), t.chip);
                        }
                        let fg = if !inf.enabled {
                            t.faint
                        } else if active {
                            t.accent
                        } else {
                            t.text
                        };
                        ui.painter().text(rect.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, inf.icon, FontId::proportional(15.0), if inf.enabled { t.muted } else { t.faint });
                        ui.painter().text(rect.left_center() + vec2(36.0, 0.0), Align2::LEFT_CENTER, &inf.label, FontId::proportional(13.5), fg);
                        let right = match self.key_text(ctx, *c) {
                            Some(k) => format!("{}   {k}", inf.group),
                            None => inf.group.to_string(),
                        };
                        ui.painter().text(rect.right_center() - vec2(10.0, 0.0), Align2::RIGHT_CENTER, right, FontId::proportional(11.5), t.faint);
                        if inf.enabled && resp.clicked() {
                            run = Some(*c);
                        }
                    }
                });
            });
        });
        p.scroll = false;
        if ctx.input(|i| i.pointer.any_pressed()) && ctx.pointer_interact_pos().is_some_and(|pos| !area.response.rect.contains(pos)) {
            close = true;
        }
        if let Some(c) = run {
            acts.push(Act::Cmd(c));
            close = true;
        }
        if !close {
            self.shell.palette = Some(p);
        }
    }

    pub fn replace_window(&mut self, ctx: &egui::Context) {
        let Some(mut f) = self.shell.replace.take() else { return };
        let t = self.theme;
        let matches = self.loaded.as_ref().map_or(0, |l| l.replace_count(&f.find, &f.with, f.case));
        let mut open = true;
        let mut apply = false;
        egui::Window::new("Find and replace").collapsible(false).resizable(false).open(&mut open).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            egui::Grid::new("replace_form").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(RichText::new("Find").color(t.muted));
                let r = ui.add(egui::TextEdit::singleline(&mut f.find).desired_width(260.0));
                if std::mem::take(&mut f.focus) {
                    r.request_focus();
                }
                ui.end_row();
                ui.label(RichText::new("Replace with").color(t.muted));
                ui.add(egui::TextEdit::singleline(&mut f.with).desired_width(260.0));
                ui.end_row();
            });
            ui.checkbox(&mut f.case, "Match case");
            ui.add_space(4.0);
            let summary = if f.find.is_empty() { "Searches every input and output cell of this decision.".to_string() } else { format!("{matches} cell{} match", if matches == 1 { "" } else { "s" }) };
            ui.label(RichText::new(summary).color(t.muted));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(matches > 0, |ui| {
                    if crate::button(ui, &t, ic::REPLACE, "Replace all", true).clicked() {
                        apply = true;
                    }
                });
            });
        });
        if apply && self.loaded.as_mut().is_some_and(|l| l.replace_all(&f.find, &f.with, f.case)) {
            open = false;
        }
        if open {
            self.shell.replace = Some(f);
        }
    }

    pub fn help_window(&mut self, ctx: &egui::Context) {
        let Some(which) = self.shell.help else { return };
        let t = self.theme;
        let mut open = true;
        match which {
            Help::About => {
                egui::Window::new("About DMN Viewer").collapsible(false).resizable(false).open(&mut open).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
                    ui.label(bold("DMN Viewer", 16.0).color(t.text));
                    ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).color(t.muted));
                    ui.add_space(6.0);
                    ui.label("View, edit, check, test and fix Camunda DMN decision tables.");
                    ui.label(RichText::new("MIT licence · by th3pajay").color(t.faint));
                });
            }
            Help::Shortcuts => {
                let s = self.state();
                egui::Window::new("Keyboard shortcuts").collapsible(false).resizable(false).open(&mut open).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                        egui::Grid::new("shortcut_list").num_columns(3).spacing([18.0, 6.0]).show(ui, |ui| {
                            for &c in STATIC {
                                let Some(k) = self.key_text(ctx, c) else { continue };
                                let inf = self.info(c, &s);
                                ui.label(RichText::new(inf.group).color(t.faint));
                                ui.label(inf.label.as_ref());
                                ui.label(RichText::new(k).monospace().color(t.muted));
                                ui.end_row();
                            }
                        });
                    });
                });
            }
        }
        if !open || ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.shell.help = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_needs_the_letters_in_order() {
        assert!(fuzzy("sva", "File Save as…").is_some());
        assert!(fuzzy("vsa", "File Save as…").is_none());
        assert_eq!(fuzzy("", "anything"), Some(0));
    }

    #[test]
    fn fuzzy_prefers_word_starts_and_runs() {
        let at_start = fuzzy("save", "File Save").unwrap();
        let scattered = fuzzy("save", "Show all views entirely").unwrap();
        assert!(at_start > scattered);
    }

    #[test]
    fn settings_round_trip() {
        let s = Settings { chrome: Chrome::Palette, compact: true, recent: vec![PathBuf::from("a.dmn"), PathBuf::from("b dir/c.dmn")] };
        let back = Settings::parse(&s.render());
        assert_eq!(back.chrome, Chrome::Palette);
        assert!(back.compact);
        assert_eq!(back.recent, s.recent);
    }

    #[test]
    fn no_two_commands_share_a_key() {
        let mut seen: Vec<(Modifiers, Key)> = Vec::new();
        for &c in STATIC {
            if let Some(Bind::Own(m, k) | Bind::Elsewhere(m, k)) = keys(c) {
                assert!(!seen.contains(&(m, k)), "{c:?} reuses a key");
                seen.push((m, k));
            }
        }
    }
}
