//! CUTE rail (pi-cinlodev-cute sidebar): banner, tab bar, double-framed collapsible cards,
//! scroll and mouse hit-test. Pure: `frame()` draws, `act()`/`key()` change only `Ui`.
use crate::config::{clean_name, Config, NAME_MAX};
use crate::core::*;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab { #[default] Main, Git, Prof, Mem, Tree, Config }
impl Tab {
    pub const ALL: [Tab; 6] = [Tab::Main, Tab::Git, Tab::Prof, Tab::Mem, Tab::Tree, Tab::Config];
    pub fn index(self) -> usize { self as usize }
    pub fn key(self) -> char { (b'1' + self as u8) as char }
    pub fn label(self) -> &'static str { ["MAIN", "GIT", "PROF", "MEM", "TREE", "CONFIG"][self.index()] }
    /// Tab-bar name (no numbers; CONFIG is its icon) and the two-letter fallback for narrow panes.
    pub fn name(self, short: bool) -> &'static str {
        if short { ["MA", "GI", "PR", "ME", "TR", CONFIG_ICON][self.index()] } else { ["MAIN", "GIT", "PROF", "MEM", "TREE", CONFIG_ICON][self.index()] }
    }
    pub fn id(self) -> &'static str { ["main", "git", "prof", "mem", "tree", "config"][self.index()] }
    pub fn from_id(id: &str) -> Option<Tab> { Self::ALL.into_iter().find(|t| t.id() == id) }
    pub fn from_key(c: char) -> Option<Tab> { Self::ALL.into_iter().find(|t| t.key() == c) }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ui {
    pub tab: Tab, pub config: Config, pub scroll: [usize; 6],
    /// Name being typed in CONFIG (`None` when not editing).
    pub editing: Option<String>,
    /// Where CONFIG is persisted (`--config`); `None` keeps changes in memory only.
    pub config_path: Option<PathBuf>, pub save: bool, pub save_error: Option<String>,
    /// GIT tab: graph or branches view, branch being diffed against HEAD, file whose diff is previewed.
    pub git_view: GitView, pub git_branch: Option<String>, pub git_file: Option<String>,
    /// TREE tab: expanded folders (sorted, root-relative) and the selected file.
    pub tree_open: Vec<String>, pub tree_pick: Option<String>,
    /// Explicit model-section click; main consumes this intent off the render path.
    pub open_picker: bool,
    pub voice_cmd: Option<VoiceCmd>,
    /// Explicit click on the Flameshot icon; main consumes this intent off the render path.
    pub screenshot: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GitView { #[default] Graph, Branches }
impl Ui {
    pub fn new(config: Config, config_path: Option<PathBuf>) -> Self {
        let tab = Tab::ALL.into_iter().find(|t| config.visible(*t)).unwrap_or(Tab::Config);
        Ui { tab, config, config_path, ..Ui::default() }
    }
    pub fn tabs(&self) -> Vec<Tab> { Tab::ALL.into_iter().filter(|t| self.config.visible(*t)).collect() }
    pub fn collapsed(&self, id: &str) -> bool { self.config.collapsed.iter().any(|c| c == id) }
}

/// What a click, wheel step or key does. Pure intents: nothing here performs I/O.
#[derive(Clone, Debug, PartialEq)]
pub enum Action { Tab(Tab), Cycle(i32), Collapse(String), Hide(Tab), Edit, ScrollTo(usize), GitView(GitView), Branch(Option<String>), File(String),
    /// TREE: toggle a folder, fold every folder, open a changed file's diff in GIT, select an unchanged file.
    Dir(String), Fold, Inspect(String), Pick(String), ModelPicker, VoiceMute, VoiceDeaf, VoiceOpen, Screenshot }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Key { Char(char), Back, Enter, Esc }

pub fn act(ui: &mut Ui, a: Action) {
    match a {
        Action::Tab(t) if ui.config.visible(t) => { if ui.tab != t { ui.editing = None; } ui.tab = t; }
        Action::Cycle(d) => {
            let tabs = ui.tabs(); let i = tabs.iter().position(|t| *t == ui.tab).unwrap_or(0) as i32;
            act(ui, Action::Tab(tabs[(i + d).rem_euclid(tabs.len() as i32) as usize]));
        }
        Action::Collapse(id) => {
            let c = &mut ui.config.collapsed;
            match c.iter().position(|x| *x == id) { Some(i) => { c.remove(i); } None => { c.push(id); c.sort(); } }
            ui.save = true;
        }
        Action::Hide(t) if t != Tab::Config => {
            let h = &mut ui.config.hidden;
            match h.iter().position(|x| *x == t) { Some(i) => { h.remove(i); } None => { h.push(t); h.sort_by_key(|t| t.index()); } }
            ui.save = true;
        }
        Action::ModelPicker => ui.open_picker = true,
        Action::VoiceMute => ui.voice_cmd = Some(VoiceCmd::Mute),
        Action::VoiceDeaf => ui.voice_cmd = Some(VoiceCmd::Deaf),
        Action::VoiceOpen => ui.voice_cmd = Some(VoiceCmd::Open),
        Action::Screenshot => ui.screenshot = true,
        Action::Edit => ui.editing = Some(ui.config.name.clone()),
        Action::ScrollTo(n) => ui.scroll[ui.tab.index()] = n,
        Action::GitView(v) => ui.git_view = v,
        // Clicking the selected branch again (or ✕) closes the inspector and its file.
        Action::Branch(b) => { ui.git_file = None; ui.git_branch = if b.is_some() && b == ui.git_branch { None } else { b }; }
        Action::File(f) => ui.git_file = if ui.git_file.as_ref() == Some(&f) { None } else { Some(f) },
        Action::Dir(d) => { let o = &mut ui.tree_open; match o.iter().position(|x| *x == d) { Some(i) => { o.remove(i); } None => { o.push(d); o.sort(); } } }
        Action::Fold => ui.tree_open.clear(),
        // Navigation only: the diff opens inside the HUD, in the GIT tab.
        Action::Inspect(p) => { ui.tree_pick = Some(p.clone()); ui.git_branch = None; ui.git_file = Some(p); act(ui, Action::Tab(Tab::Git)); }
        Action::Pick(p) => ui.tree_pick = if ui.tree_pick.as_ref() == Some(&p) { None } else { Some(p) },
        _ => {}
    }
}
pub fn key(s: &mut State, k: Key) {
    if let Some(buf) = &mut s.ui.editing {
        match k {
            Key::Char(c) if !c.is_control() && buf.chars().count() < NAME_MAX => buf.push(c),
            Key::Back => { buf.pop(); }
            Key::Enter => { s.ui.config.name = clean_name(buf); s.ui.editing = None; s.ui.save = true; }
            Key::Esc => s.ui.editing = None,
            _ => {}
        }
        return;
    }
    match k {
        Key::Char('q') => s.quit = true,
        Key::Char('r') => s.refresh = true,
        Key::Char(c) => if let Some(t) = Tab::from_key(c) { act(&mut s.ui, Action::Tab(t)) },
        _ => {}
    }
}

// ---- width-safe segments --------------------------------------------------------------
#[derive(Clone, Debug, PartialEq)]
pub struct Seg { pub text: String, pub tone: Tone, pub bold: bool }
pub fn seg(text: impl Into<String>, tone: Tone) -> Seg { Seg { text: text.into(), tone, bold: false } }
fn bold(text: impl Into<String>, tone: Tone) -> Seg { Seg { text: text.into(), tone, bold: true } }
/// CONFIG tab icon: the gearbox `⛭` (U+26ED, 1 column, monochrome), always purple.
pub const CONFIG_ICON: &str = "⛭";
/// Theme mark of the banner and card titles: the power symbol `⌁` (U+2301, 1 column, monochrome), always purple.
pub const MARK: &str = "⌁";
/// Card title `⌁ name`: purple mark, name in the card's tone.
fn mark(name: &str, tone: Tone) -> Vec<Seg> { vec![seg(MARK, Tone::Purple), seg(format!(" {name}"), tone)] }
/// A tab name; CONFIG is its purple icon.
fn tab_name(t: Tab, short: bool, tone: Tone, strong: bool) -> Seg { if t == Tab::Config { seg(CONFIG_ICON, Tone::Purple) } else { Seg { text: t.name(short).into(), tone, bold: strong } } }
pub fn plain(line: &[Seg]) -> String { line.iter().map(|g| g.text.as_str()).collect() }
fn width_of(line: &[Seg]) -> usize { line.iter().map(|g| g.text.width()).sum() }
fn fit(line: Vec<Seg>, width: usize) -> Vec<Seg> {
    let mut out = Vec::new(); let mut room = width;
    for g in line {
        if room == 0 { break }
        let t = truncate(&g.text, room); let whole = t.len() == g.text.len(); room -= t.width();
        out.push(Seg { text: t, ..g }); if !whole { break }
    }
    out
}
fn centered(line: Vec<Seg>, w: usize) -> Vec<Seg> {
    let line = fit(line, w); let pad = w.saturating_sub(width_of(&line)) / 2;
    let mut l = vec![seg(" ".repeat(pad), Tone::Text)]; l.extend(line); l
}

// ---- CUTE double frame ----------------------------------------------------------------
/// Title-bar controls drawn before `[-]` (e.g. `[ Grafo | Ramas ]`); each segment may be clickable.
type Extra = Vec<(Seg, Option<Action>)>;
/// `╔═ title ═…═ [extra] [-] ═╗`: the toggle is drawn when the card is wide enough for it,
/// the extra controls only while expanded and when the title keeps at least 4 columns.
fn top(title: Vec<Seg>, extra: &Extra, w: usize, collapsed: bool) -> (Vec<Seg>, Option<(usize, usize)>, Vec<(usize, usize, Action)>) {
    let toggle = w >= 16;
    let ew: usize = extra.iter().map(|(g, _)| g.text.width()).sum();
    let show = toggle && !collapsed && ew > 0 && w >= 16 + ew;
    let cost = if show { ew + 1 } else { 0 };
    let room = w.saturating_sub(if toggle { 11 + cost + usize::from(!show) } else { 6 });
    let title = fit(title, room); let tw = width_of(&title);
    let mut l = vec![seg("╔═ ", Tone::Border)]; l.extend(title);
    if toggle {
        let fill = w.saturating_sub(11 + cost + tw);
        l.push(seg(format!(" {} ", "═".repeat(fill)), Tone::Border));
        let mut hits = Vec::new();
        if show {
            let mut x = 3 + tw + 2 + fill;
            for (g, a) in extra { let n = g.text.width(); if let Some(a) = a { hits.push((x, x + n, a.clone())); } x += n; l.push(g.clone()); }
            l.push(seg(" ", Tone::Border));
        }
        l.push(seg("[", Tone::Dim)); l.push(bold(if collapsed { "+" } else { "-" }, Tone::PinkBright)); l.push(seg("]", Tone::Dim));
        l.push(seg(" ═╗", Tone::Border));
        (l, Some((w - 6, w - 3)), hits)
    } else {
        l.push(seg(format!(" {}╗", "═".repeat(w.saturating_sub(5 + tw))), Tone::Border)); (l, None, Vec::new())
    }
}
fn bottom(w: usize) -> Vec<Seg> { vec![seg(format!("╚{}╝", "═".repeat(w.saturating_sub(2))), Tone::Border)] }
fn divider(w: usize) -> Vec<Seg> { vec![seg(format!("╠{}╣", "═".repeat(w.saturating_sub(2))), Tone::Border)] }
fn pad(left: Vec<Seg>, right: Vec<Seg>, w: usize) -> Vec<Seg> {
    let inner = w.saturating_sub(4); let (lw, rw) = (width_of(&left), width_of(&right));
    let mut l = vec![seg("║ ", Tone::Border)]; l.extend(left); l.push(seg(" ".repeat(inner.saturating_sub(lw + rw)), Tone::Text)); l.extend(right); l.push(seg(" ║", Tone::Border)); l
}
/// `║ left ··· right ║`; the right side wins when space runs out.
fn row(left: Vec<Seg>, right: Vec<Seg>, w: usize) -> Vec<Seg> {
    let inner = w.saturating_sub(4);
    let right = fit(right, inner); let rw = width_of(&right);
    let left = fit(left, inner.saturating_sub(if rw > 0 { rw + 1 } else { 0 }));
    pad(left, right, w)
}
/// `║ Label ··· value ║`; the label stays whole and the value is cut.
fn keyed(label: &str, value: Vec<Seg>, w: usize) -> Vec<Seg> {
    let inner = w.saturating_sub(4);
    let left = fit(vec![seg(label, Tone::Gold)], inner); let lw = width_of(&left);
    let right = fit(value, inner.saturating_sub(lw + 1));
    pad(left, right, w)
}

/// One framed card: `id` keys its collapsed state, rows may carry click actions.
struct Card { id: &'static str, title: Vec<Seg>, extra: Extra, w: usize, rows: Vec<Vec<Seg>>, hits: Vec<(usize, usize, usize, Action)> }
impl Card {
    fn new(id: &'static str, title: Vec<Seg>, w: usize) -> Self { Card { id, title, extra: Vec::new(), w, rows: Vec::new(), hits: Vec::new() } }
    fn row(&mut self, l: Vec<Seg>, r: Vec<Seg>) { self.rows.push(row(l, r, self.w)); }
    fn keyed(&mut self, label: &str, v: Vec<Seg>) { self.rows.push(keyed(label, v, self.w)); }
    fn divider(&mut self) { self.rows.push(divider(self.w)); }
    /// The whole inner width of the last row triggers `a`.
    fn on_click(&mut self, a: Action) { let i = self.rows.len() - 1; self.hits.push((i, 2, self.w.saturating_sub(2), a)); }
}

pub struct Hit { pub row: usize, pub x0: usize, pub x1: usize, pub action: Action }
pub struct Frame { pub lines: Vec<Vec<Seg>>, pub hits: Vec<Hit>, pub tab_row: usize, pub scroll: usize, pub max_scroll: usize }
/// The narrowest region under the cursor wins (a title toggle beats its whole row).
pub fn click(f: &Frame, col: usize, row: usize) -> Option<Action> {
    f.hits.iter().filter(|h| h.row == row && (h.x0..h.x1).contains(&col)).min_by_key(|h| h.x1 - h.x0).map(|h| h.action.clone())
}
/// Wheel over the tab bar cycles tabs; anywhere else scrolls the body (clamped, 3 lines a step).
pub fn wheel(f: &Frame, row: usize, down: bool) -> Option<Action> {
    if row <= f.tab_row + 1 && row + 1 >= f.tab_row { return Some(Action::Cycle(if down { 1 } else { -1 })) }
    let n = if down { (f.scroll + 3).min(f.max_scroll) } else { f.scroll.saturating_sub(3) };
    (n != f.scroll).then_some(Action::ScrollTo(n))
}

const PAD: usize = 1;
struct Body { lines: Vec<Vec<Seg>>, hits: Vec<Hit>, glued: bool }
impl Body {
    fn push(&mut self, ui: &Ui, c: Card) {
        if !self.lines.is_empty() && !std::mem::take(&mut self.glued) { self.lines.push(Vec::new()); }
        let collapsed = ui.collapsed(c.id); let start = self.lines.len();
        let (line, toggle, extra) = top(c.title, &c.extra, c.w, collapsed); self.lines.push(line);
        for (x0, x1, a) in extra { self.hits.push(Hit { row: start, x0: PAD + x0, x1: PAD + x1, action: a }); }
        let toggle_at = |(a, b): (usize, usize)| Hit { row: start, x0: PAD + a, x1: PAD + b, action: Action::Collapse(c.id.into()) };
        self.hits.push(Hit { row: start, x0: PAD, x1: PAD + c.w, action: Action::Collapse(c.id.into()) });
        if let Some(t) = toggle { self.hits.push(toggle_at(t)); }
        if !collapsed {
            for (i, x0, x1, a) in c.hits { self.hits.push(Hit { row: start + 1 + i, x0: PAD + x0, x1: PAD + x1, action: a }); }
            self.lines.extend(c.rows);
        }
        self.lines.push(bottom(c.w));
    }
}

fn tab_bar(ui: &Ui, s: &State, w: usize) -> (Vec<Seg>, Vec<(usize, usize, Tab)>) {
    let sync = match s.git.ahead_behind { Some((a, b)) if a + b > 0 => format!(" {}{}", if a > 0 { format!("▲{a}") } else { String::new() }, if b > 0 { format!("▼{b}") } else { String::new() }), _ => String::new() };
    let tabs = ui.tabs(); let compact = w < 44;
    // Names only (S8): `[ MAIN ]` active (`[MAIN]` under 44 columns), the others muted; CONFIG is the gear.
    let visual = |short: bool| -> Vec<(Tab, Vec<Seg>)> {
        tabs.iter().map(|&t| {
            let extra = if t == Tab::Git { sync.as_str() } else { "" };
            if t == ui.tab {
                let (open, close) = if compact { ("[", "]") } else { ("[ ", " ]") };
                (t, vec![bold(open, Tone::PinkBright), tab_name(t, short, Tone::PinkBright, true), bold(format!("{extra}{close}"), Tone::PinkBright)])
            } else { (t, vec![tab_name(t, short, Tone::Muted, false), seg(extra, Tone::Yellow)]) }
        }).collect()
    };
    // Too narrow for the names: two-letter names, so every tab stays visible and clickable.
    let mut items = visual(false);
    let needed = |items: &[(Tab, Vec<Seg>)]| items.iter().map(|(_, v)| width_of(v)).sum::<usize>() + items.len().saturating_sub(1);
    if needed(&items) > w { items = visual(true); }
    let total: usize = items.iter().map(|(_, v)| width_of(v)).sum();
    let gaps = items.len().saturating_sub(1); let rest = w.saturating_sub(total);
    let (base, mut extra) = if gaps > 0 { (rest / gaps, rest % gaps) } else { (0, 0) };
    let (mut line, mut boxes, mut x) = (Vec::new(), Vec::new(), 0);
    for (i, (t, v)) in items.into_iter().enumerate() {
        let len = width_of(&v); boxes.push((PAD + x, PAD + (x + len).min(w), t)); x += len; line.extend(v);
        if i < gaps { let g = base + usize::from(extra > 0); extra = extra.saturating_sub(1); line.push(seg(" ".repeat(g), Tone::Text)); x += g; }
    }
    (fit(line, w), boxes.into_iter().filter(|b| b.0 < PAD + w).collect())
}

/// Draw the rail for a `width`×`height` pane; `height` 0 draws everything (no scrolling).
pub fn frame(s: &State, width: u16, height: u16) -> Frame {
    use Tone::*;
    let w = width as usize; let net = w.saturating_sub(2 * PAD); let ui = &s.ui;
    let name = if ui.config.name.is_empty() { " - - ".to_string() } else { format!(" {} ", ui.config.name) };
    let mut head: Vec<Vec<Seg>> = vec![Vec::new(),
        centered(vec![seg(MARK, Purple), bold(name, PinkBright), seg(MARK, Purple)], net),
        centered(vec![seg("GENTLE-SHELL", Text)], net), Vec::new()];
    let (tabs, boxes) = tab_bar(ui, s, net); let tab_row = head.len(); head.push(tabs);
    let mut hits: Vec<Hit> = boxes.into_iter().map(|(x0, x1, t)| Hit { row: tab_row, x0, x1, action: Action::Tab(t) }).collect();
    head.push(vec![seg("─".repeat(net), BorderMuted)]);
    // MAIN: the voice icons sit right on the divider; other tabs keep one blank line.
    if ui.tab != Tab::Main { head.push(Vec::new()); }

    let mut body = Body { lines: Vec::new(), hits: Vec::new(), glued: false };
    let cw = net;
    match ui.tab {
        Tab::Main => { voice_row(&mut body, s.voice, cw); body.push(ui, status_card(s, cw)); body.push(ui, context_card(s, cw)); body.push(ui, todos_card(s, cw)); }
        Tab::Git => { body.push(ui, git_card(s, cw)); body.push(ui, worktree_card(s, cw)); body.push(ui, pr_card(s, cw)); }
        Tab::Prof => body.push(ui, profiles_card(s, cw)),
        Tab::Mem => { for c in [engram_card(s, cw), handoff_card(s, cw), stack_card(s, cw), tools_card(s, cw), mcp_card(s, cw)] { body.push(ui, c); } }
        Tab::Tree => body.push(ui, tree_card(s, cw)),
        Tab::Config => { for c in config_cards(s, cw) { body.push(ui, c); } }
    }
    if body.lines.is_empty() { body.lines.push(centered(vec![seg("Sin información disponible en esta pestaña.", Dim)], net)); }

    let view = if height == 0 { body.lines.len() } else { (height as usize).saturating_sub(head.len()) };
    let max_scroll = body.lines.len().saturating_sub(view);
    let scroll = ui.scroll[ui.tab.index()].min(max_scroll);
    if max_scroll > 0 {
        // More above/below: arrows at the right end of the divider.
        let mark = match (scroll > 0, scroll < max_scroll) { (true, true) => " ↑↓", (true, false) => " ↑", _ => " ↓" };
        head[tab_row + 1] = vec![seg("─".repeat(net.saturating_sub(mark.width())), BorderMuted), seg(mark, Muted)];
    }
    let top_rows = head.len();
    hits.extend(body.hits.into_iter().filter(|h| (scroll..scroll + view).contains(&h.row)).map(|h| Hit { row: h.row - scroll + top_rows, ..h }));
    let lines = head.into_iter().chain(body.lines.into_iter().skip(scroll).take(view))
        .map(|l| { if l.is_empty() { return l } let mut p = vec![seg(" ".repeat(PAD), Text)]; p.extend(l); fit(p, w) }).collect();
    Frame { lines, hits, tab_row, scroll, max_scroll }
}
pub fn render_lines(s: &State, width: u16) -> Vec<String> { frame(s, width, 0).lines.iter().map(|l| plain(l)).collect() }

/// Voice icons drawn 5 columns × 2 rows with sextant "pixels" (no frame), 3 columns apart,
/// right under the divider and glued to Status. Each icon plus one column on either side is clickable.
const VOICE_ICONS: [[&str; 2]; 3] = [
    ["\u{1fb1e}\u{2590}\u{2588}\u{258c}\u{1fb0f}", " \u{1fb30}\u{2588}\u{1fb30} "],
    ["\u{1fb16}\u{1fb02}\u{1fb02}\u{1fb02}\u{1fb22}", "\u{2588}\u{258c} \u{2590}\u{2588}"],
    ["\u{1fb1e}\u{1fb3b}\u{1fb0b}\u{1fb3a}\u{1fb0f}", "\u{1fb01}\u{1fb2c}\u{1fb0b}\u{1fb1d}\u{1fb00}"],
];
/// Flameshot flame, 3 columns × 2 rows of sextants, flush right on the voice rows (smaller than the 5-column voice icons).
const FLAME_ICON: [&str; 2] = ["\u{1fb1e}\u{1fb3a}\u{1fb1e}", "\u{1fb2b}\u{2588}\u{1fb1d}"];
fn voice_row(body: &mut Body, voice: Voice, w: usize) {
    const ICON: usize = 5; const GAP: usize = 3;
    let tone = |active| if !voice.online { Tone::Text } else if active { Tone::VoiceRed } else { Tone::VoiceGreen };
    let tones = [tone(voice.mute || voice.deaf), tone(voice.deaf), tone(false)];
    let start = body.lines.len();
    // The voice icons stay centered; the flame goes flush right when at least 2 columns remain between it and the gear.
    let (flame, row_w) = (FLAME_ICON[0].width(), 3 * ICON + 2 * GAP);
    let lead = w.saturating_sub(row_w) / 2;
    let show_flame = lead + row_w + 2 + flame <= w;
    for r in 0..2 {
        let mut l = vec![seg(" ".repeat(lead), Tone::Text)];
        for (i, icon) in VOICE_ICONS.iter().enumerate() { if i > 0 { l.push(seg(" ".repeat(GAP), Tone::Text)); } l.push(bold(icon[r], tones[i])); }
        if show_flame { l.push(seg(" ".repeat(w - lead - row_w - flame), Tone::Text)); l.push(bold(FLAME_ICON[r], Tone::Yellow)); }
        body.lines.push(l);
    }
    if show_flame { for r in 0..2 { body.hits.push(Hit { row: start + r, x0: PAD + w - flame - 1, x1: PAD + w + 1, action: Action::Screenshot }); } }
    let x = PAD + lead;
    for (i, action) in [Action::VoiceMute, Action::VoiceDeaf, Action::VoiceOpen].into_iter().enumerate() {
        let x0 = x + i * (ICON + GAP);
        for r in 0..2 { body.hits.push(Hit { row: start + r, x0: x0.saturating_sub(1), x1: x0 + ICON + 1, action: action.clone() }); }
    }
    body.glued = true;
}

fn dash(v: &str) -> String { if v.is_empty() { "—".into() } else { v.into() } }

// ---- 1:MAIN ---------------------------------------------------------------------------
fn status_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let mut c = Card::new("status", mark("Status", PinkBright), w);
    c.extra = vec![(seg("▸", Dim), Some(Action::ModelPicker))];
    c.keyed("Project", vec![seg(short_path(&s.cwd, w.saturating_sub(4 + 8)), Text)]);
    let mut branch = vec![seg(format!("⎇ {}", if s.git.branch.is_empty() { "no-git" } else { &s.git.branch }), Text)];
    if let Some(n) = s.git.dirty.filter(|n| *n > 0) { branch.push(seg(format!(" ±{n}"), Yellow)); }
    c.keyed("Branch", branch);
    // Model (S10): its own section, spaced capitals like the reference image, `◆ THINKING ◆` below.
    c.divider(); model_section(&mut c, s); c.divider();
    let (icon, tone) = match s.status.as_str() { "working" => ("●", Cyan), "idle" => ("○", Muted), "blocked" => ("!", Coral), "done" => ("✓", Mint), _ => ("?", Dim) };
    let place = if s.label.is_empty() { dash(&s.workspace) } else { s.label.clone() };
    c.row(vec![seg(format!("{icon} {}", s.status), tone)], vec![seg(place, Muted)]);
    let (dot, word, tone) = if s.connected { ("●", "conectado", Mint) } else { ("○", "desconectado", Coral) };
    c.row(vec![seg("herdr ", Muted), seg(dot, tone), seg(format!(" {}", s.pane), Muted)], vec![seg(word, tone)]);
    c
}
fn model_section(c: &mut Card, s: &State) {
    use Tone::*;
    let inner = c.w.saturating_sub(4);
    let center = |c: &mut Card, g: Seg| { let g = fit(vec![g], inner); let pad = inner.saturating_sub(width_of(&g)) / 2; let mut l = vec![seg(" ".repeat(pad), Text)]; l.extend(g); c.row(l, vec![]); c.on_click(Action::ModelPicker); };
    // Before the first assistant turn the session has no model yet: use the host defaults (settings.json), like gs2.
    let host = s.roster.as_ref().filter(|_| s.session.model.is_empty());
    let (model, thinking) = match host { Some(r) => (r.model.as_str(), r.thinking.as_str()), None => (s.session.model.as_str(), s.session.thinking.as_str()) };
    let model = model.rsplit('/').next().unwrap_or_default().to_uppercase();
    if model.is_empty() { center(c, seg("sin modelo", Dim)); return }
    let spaced = model.chars().map(String::from).collect::<Vec<_>>().join(" ");
    if spaced.width() <= inner { center(c, bold(spaced, PinkBright)); } else { for part in wrap(&model, inner) { center(c, bold(part, PinkBright)); } }
    if !thinking.is_empty() && thinking != "off" { center(c, seg(format!("◆ {} ◆", thinking.to_uppercase()), Gold)); }
}
fn context_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let t = &s.session; let mut c = Card::new("context", mark("Context", PinkBright), w);
    let pct = t.limit.map(|n| (100.0 * t.used as f64 / n.max(1) as f64).clamp(0.0, 100.0));
    let (label, tone) = ctx_band(pct.unwrap_or(0.0));
    let limit = t.limit.map_or("—".into(), fmt_tokens);
    let right = match pct { Some(p) => bold(format!("{label} {}%", if p > 0.0 && p < 10.0 { format!("{p:.1}") } else { format!("{}", p.round()) }), tone), None => seg("sin límite", Dim) };
    c.row(vec![bold(fmt_tokens(t.used), tone), seg(" / ", Muted), seg(limit, Muted), seg(" tokens", Muted)], vec![right]);
    let cells = w.saturating_sub(4); let filled = ((pct.unwrap_or(0.0) / 100.0) * cells as f64).round() as usize;
    c.row(vec![seg("▰".repeat(filled.min(cells)), tone), seg("▱".repeat(cells - filled.min(cells)), Dim)], vec![]);
    let cost = vec![seg("Cost ", Gold), seg(fmt_cost(t.cost), Text)];
    let io = |sep: &str| vec![seg("▲ ", Muted), seg(fmt_tokens(t.input + t.read), Text), seg(format!(" in{sep}·{sep}"), Muted), seg("▼ ", Muted), seg(fmt_tokens(t.output), Text), seg(" out", Muted)];
    // Double-spaced like the reference when it fits next to the cost, single-spaced otherwise.
    let wide = io("  "); let left = if width_of(&wide) + width_of(&cost) < w.saturating_sub(4) { wide } else { io(" ") };
    c.row(left, cost);
    c
}

/// Word wrap like the reference `wrapWords`; a word wider than the row is cut char by char.
fn wrap(text: &str, w: usize) -> Vec<String> {
    let w = w.max(1); let mut out: Vec<String> = Vec::new(); let mut cur = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        if !cur.is_empty() && cur.width() + 1 + word.width() <= w { cur.push(' '); cur.push_str(&word); continue }
        if !cur.is_empty() { out.push(std::mem::take(&mut cur)); }
        while word.width() > w { let head = truncate(&word, w); let head = if head.is_empty() { word.chars().next().unwrap().to_string() } else { head }; word = word[head.len()..].to_string(); out.push(head); }
        cur = word;
    }
    if !cur.is_empty() || out.is_empty() { out.push(cur); }
    out
}
fn todos_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let todos = &s.session.todos; let done = todos.iter().filter(|t| t.status == TodoStatus::Done).count();
    let mut c = Card::new("todos", [mark("Todos", PinkBright), vec![seg(format!(" · {done} of {}", todos.len()), Text)]].concat(), w);
    if todos.is_empty() { c.row(vec![seg("sin tareas en esta sesión", Dim)], vec![]); }
    let inner = w.saturating_sub(4);
    for t in todos {
        let (glyph, gt, tt) = match t.status { TodoStatus::Done => ("✓", Mint, Dim), TodoStatus::InProgress => ("◉", Gold, PinkBright), TodoStatus::Pending => ("○", Dim, Text) };
        for (i, l) in wrap(&t.title, inner.saturating_sub(2)).into_iter().enumerate() {
            c.row(vec![if i == 0 { seg(format!("{glyph} "), gt) } else { seg("  ", Text) }, seg(l, tt)], vec![]);
        }
        if let (Some(n), TodoStatus::InProgress) = (&t.note, t.status) {
            for (i, l) in wrap(n, inner.saturating_sub(4)).into_iter().enumerate() { c.row(vec![seg(if i == 0 { "  ↳ " } else { "    " }, Dim), seg(l, Dim)], vec![]); }
        }
    }
    c
}

// ---- 2:GIT ----------------------------------------------------------------------------
/// `formatGitStatusBadges`: `✔ clean`, or conflicts · mod · staged · untracked.
fn status_badge(g: &Git) -> Vec<Seg> {
    use Tone::*;
    if g.dirty.is_none() { return vec![seg("—", Dim)] }
    let c = &g.counts; if c.clean() { return vec![seg("✔ clean", Mint)] }
    let mut out = Vec::new();
    for (n, text, tone) in [(c.conflicts, format!("✖ {} conflict", c.conflicts), Coral), (c.modified, format!("● {} mod", c.modified), Yellow),
        (c.staged, format!("+{} staged", c.staged), Mint), (c.untracked, format!("?{} untracked", c.untracked), Secondary)] {
        if n == 0 { continue }
        if !out.is_empty() { out.push(seg(" · ", Muted)); }
        out.push(seg(text, tone));
    }
    out
}
/// Graph glyphs: nodes pink, edges blue (`colorizeGitGraphLine`).
fn edges(g: &str) -> Vec<Seg> {
    let mut v: Vec<Seg> = Vec::new();
    for ch in g.chars() {
        let t = match ch { '*' => Tone::Pink, ' ' => Tone::Text, _ => Tone::Blue };
        match v.last_mut() { Some(last) if last.tone == t => last.text.push(ch), _ => v.push(seg(ch.to_string(), t)) }
    }
    v
}
fn graph_line(line: &str) -> Vec<Seg> {
    use Tone::*;
    let Some((g, hash, refs, subject)) = split_graph(line) else { return edges(line) };
    let mut v = edges(g); v.push(seg(hash, Yellow));
    if let Some(r) = refs {
        v.push(seg(" ", Text)); v.push(seg("(", Muted));
        for (i, item) in r.split(',').map(str::trim).enumerate() {
            if i > 0 { v.push(seg(", ", Muted)); }
            let t = if item.contains("HEAD") { Mint } else if item.contains("origin") || item.contains("upstream") { Secondary } else { Gold };
            v.push(seg(item, t));
        }
        v.push(seg(")", Muted));
    }
    if !subject.is_empty() { v.push(seg(format!(" {subject}"), Text)); }
    v
}
fn git_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let (g, ui) = (&s.git, &s.ui);
    let mut c = Card::new("git", vec![seg("⎇ git graph", Gold)], w);
    let compact = w < 38;
    let label = |v: GitView, long: &'static str| -> (Seg, Option<Action>) {
        let text = if compact { &long[..1] } else { long };
        (if ui.git_view == v { bold(text, Pink) } else { seg(text, Dim) }, Some(Action::GitView(v)))
    };
    c.extra = vec![(seg("[ ", Dim), None), label(GitView::Graph, "Grafo"), (seg(" | ", Dim), None), label(GitView::Branches, "Ramas"), (seg(" ]", Dim), None)];
    if let Some(e) = &g.error { c.row(vec![seg(e.clone(), Coral)], vec![]); }
    match ui.git_view {
        GitView::Graph => {
            c.row(vec![], vec![]); c.row(vec![seg(dash(&g.branch), Text)], status_badge(g)); c.row(vec![], vec![]);
            for l in &g.graph { c.row(graph_line(l), vec![]); }
            if g.added + g.deleted > 0 {
                let n = g.counts.modified + g.counts.staged;
                c.row(vec![], vec![]);
                c.row(vec![seg("✎ ", Pink), seg(format!("{n} {} · ", if n == 1 { "file" } else { "files" }), Text), seg(format!("+{}", g.added), Mint), seg(" ", Text), seg(format!("−{}", g.deleted), Coral)], vec![]);
            }
        }
        GitView::Branches => {
            let head = vec![seg(format!("● {}", g.branches.len()), Yellow), seg(" ramas en repo", Dim)];
            let badge = status_badge(g); let fits = width_of(&head) + 1 + width_of(&badge) <= w.saturating_sub(4);
            c.row(head, if fits { badge } else { vec![] });
            for b in &g.branches {
                let sel = ui.git_branch.as_deref() == Some(b.name.as_str());
                let name = match b.name.strip_prefix("origin/") { Some(n) if b.remote => format!("rem/{n}"), _ => b.name.clone() };
                let (icon, it, nt) = if b.current { ("⎇ ", Mint, Mint) } else if sel { ("▶ ", Pink, Pink) } else { ("● ", Dim, if b.remote { Secondary } else { Cyan }) };
                let mut right: Vec<Seg> = Vec::new();
                let add = |r: &mut Vec<Seg>, g: Seg| { if !r.is_empty() { r.push(seg(" ", Text)); } r.push(g); };
                if b.current { add(&mut right, seg("actual", Mint)); } else if sel { add(&mut right, seg("diff", Pink)); } else {
                    if b.ahead > 0 { add(&mut right, seg(format!("↑{}", b.ahead), Yellow)); }
                    if b.behind > 0 { add(&mut right, seg(format!("↓{}", b.behind), Coral)); }
                    if b.gone { add(&mut right, seg("gone", Coral)); }
                    if right.is_empty() && !b.date.is_empty() { add(&mut right, seg(b.date.clone(), Dim)); }
                }
                c.row(vec![seg(icon, it), seg(name, nt)], right);
                if !b.current { c.on_click(Action::Branch(Some(b.name.clone()))); }
            }
        }
    }
    c
}
/// Diff preview rows under the selected file: hunks only (headers dropped), capped at DIFF_ROWS.
const DIFF_ROWS: usize = 80;
fn diff_rows(c: &mut Card, s: &State, path: &str) {
    use Tone::*;
    let (d, ui) = (&s.detail, &s.ui);
    let note = |c: &mut Card, t: &str| c.row(vec![seg(format!("  {t}"), Dim)], vec![]);
    if sensitive(path) { return note(c, "contenido oculto · posible secreto") }
    if d.file != ui.git_file || d.branch != ui.git_branch { return note(c, "cargando diff…") }
    let lines = &d.diff[d.diff.iter().position(|l| l.starts_with("@@")).unwrap_or(0)..];
    if lines.is_empty() { return note(c, "sin cambios de texto") }
    for l in lines.iter().take(DIFF_ROWS) {
        let t = if l.starts_with("@@") { Blue } else if l.starts_with('+') { Mint } else if l.starts_with('-') { Coral } else { Muted };
        c.row(vec![seg("  ", Text), seg(l.replace('\t', "    ").replace('\r', ""), t)], vec![]);
    }
    let more = lines.len().saturating_sub(DIFF_ROWS) + d.more;
    if more > 0 { note(c, &format!("… {more} líneas más")); }
}
fn file_rows(c: &mut Card, s: &State, files: &[FileChange]) {
    use Tone::*;
    for f in files {
        let (b, t) = match f.kind { FileKind::Modified => ("M", Yellow), FileKind::Added => ("A", Mint), FileKind::Deleted => ("D", Coral), FileKind::Untracked => ("?", Pink), FileKind::Conflict => ("U", Coral) };
        let (dir, name) = f.path.rfind('/').map_or(("", f.path.as_str()), |i| (&f.path[..=i], &f.path[i + 1..]));
        let mut right = vec![seg("[", Dim), seg("diff", Pink), seg("]", Dim)];
        if f.added > 0 { right.extend([seg(" ", Text), seg(format!("+{}", f.added), Mint)]); }
        if f.deleted > 0 { right.extend([seg(" ", Text), seg(format!("−{}", f.deleted), Coral)]); }
        // Too long: the directory shrinks to `…/` so the file name stays readable.
        let room = c.w.saturating_sub(4 + width_of(&right) + 1 + 3);
        let dir = if !dir.is_empty() && dir.width() + name.width() > room { "…/" } else { dir };
        let mut left = vec![seg(b, t), seg("  ", Text)]; if !dir.is_empty() { left.push(seg(dir, Dim)); } left.push(seg(name, Cyan));
        c.row(left, right); c.on_click(Action::File(f.path.clone()));
        if s.ui.git_file.as_deref() == Some(f.path.as_str()) { diff_rows(c, s, &f.path); }
    }
}
/// Working tree changes, or (a branch selected in Ramas) that branch's diff against HEAD.
fn worktree_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let mut c = Card::new("worktree", vec![seg("📁 Working Tree", Gold)], w);
    let (files, added, deleted, branch) = match &s.ui.git_branch {
        Some(b) => {
            c.title = vec![seg("📁", Pink), seg(" Diff", Gold), seg(" · ", Dim), seg(b.clone(), Cyan)];
            let close = Some(Action::Branch(None)); c.extra = vec![(seg("[ ", Dim), close.clone()), (seg("✕", Coral), close.clone()), (seg(" ]", Dim), close)];
            if s.detail.branch.as_ref() != Some(b) { c.row(vec![seg("cargando…", Dim)], vec![]); return c }
            if let Some(e) = &s.detail.error { c.row(vec![seg(e.clone(), Coral)], vec![]); return c }
            (&s.detail.files, s.detail.added, s.detail.deleted, true)
        }
        None => (&s.git.files, s.git.added, s.git.deleted, false),
    };
    if files.is_empty() {
        let l = if branch { vec![seg("✔ ", Text), seg("Sin diferencias con HEAD", Mint)] } else { vec![seg("✔ ", Text), seg("Clean", Mint), seg(" · ", Dim), seg("Sin cambios pendientes", Dim)] };
        c.row(l, vec![]); return c
    }
    let n = files.len();
    let noun = match (branch, n == 1) { (true, true) => "archivo", (true, false) => "archivos", (false, true) => "archivo modificado", (false, false) => "archivos modificados" };
    let deltas = if added + deleted > 0 { vec![seg(format!("+{added}"), Mint), seg("  ", Text), seg(format!("−{deleted}"), Coral)] } else { vec![] };
    c.row(vec![seg(format!("● {n}"), Yellow), seg(format!(" {noun}"), Dim)], deltas);
    c.row(vec![seg("click: ver diff", Dim)], vec![]);
    c.divider();
    file_rows(&mut c, s, files);
    c
}
fn pr_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let mut c = Card::new("pr", mark("Pull requests", PinkBright), w);
    for l in &s.pr { let tone = if l.contains("error") { Coral } else if l.starts_with('#') { Cyan } else { Text }; c.row(vec![seg(l.clone(), tone)], vec![]); }
    c
}

// ---- 5:TREE ---------------------------------------------------------------------------
/// Reference `getFileIcon` (no Nerd Font; ⚙️ is replaced by the single-width-safe 🔧).
fn file_icon(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "json" | "yaml" | "yml" | "toml" => "🔧", "md" | "txt" | "markdown" => "📝", "css" | "scss" | "sass" | "less" => "🎨",
        "html" | "htm" => "🌐", "png" | "jpg" | "jpeg" | "gif" | "svg" | "ico" | "webp" => "🖼", "sh" | "bash" | "zsh" => "💻", "lock" => "🔒", _ => "📄",
    }
}
const TREE_ROWS: usize = 300;
fn tree_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let mut c = Card::new("tree", mark("PROYECTO", Gold), w);
    let Some(t) = &s.tree else { c.row(vec![seg("Escaneando…", Dim)], vec![]); return c };
    if !t.project.is_empty() { c.title.extend([seg(" · ", Dim), seg(t.project.clone(), Cyan)]); }
    if !s.ui.tree_open.is_empty() { c.extra = vec![(seg("[ ", Dim), Some(Action::Fold)), (seg("⊟", Pink), Some(Action::Fold)), (seg(" ]", Dim), Some(Action::Fold))]; }
    if let Some(e) = &t.error { c.row(vec![seg(e.clone(), Coral)], vec![]); return c }
    if t.nodes.is_empty() { c.row(vec![seg("Sin archivos en el proyecto", Dim), seg(" · ", Dim), seg(MARK, Purple)], vec![]); return c }
    // Git badges: porcelain paths are root-relative, like `TreeNode.rel`.
    let kind = |rel: &str| s.git.files.iter().find(|f| f.path == rel).map(|f| f.kind);
    let dirty = |rel: &str| s.git.files.iter().any(|f| f.path.len() > rel.len() && f.path.starts_with(rel) && f.path.as_bytes()[rel.len()] == b'/');
    let mut rows: Vec<(String, &TreeNode)> = Vec::new();
    fn walk<'a>(nodes: &'a [TreeNode], prefix: &str, open: &[String], out: &mut Vec<(String, &'a TreeNode)>) {
        for (i, n) in nodes.iter().enumerate() {
            let last = i + 1 == nodes.len();
            out.push((format!("{prefix}{}", if last { "└── " } else { "├── " }), n));
            if n.dir && open.contains(&n.rel) { walk(&n.children, &format!("{prefix}{}", if last { "    " } else { "│   " }), open, out); }
        }
    }
    walk(&t.nodes, "", &s.ui.tree_open, &mut rows);
    let more = rows.len().saturating_sub(TREE_ROWS);
    for (prefix, n) in rows.into_iter().take(TREE_ROWS) {
        let mut left = vec![seg(prefix, Dim)];
        if n.dir {
            let open = s.ui.tree_open.contains(&n.rel); let d = dirty(&n.rel);
            left.extend([seg(if open { "📂 " } else { "📁 " }, Text), seg(n.name.clone(), if d { Yellow } else { Cyan }), seg(format!(" ({} items)", n.count), Dim)]);
            c.row(left, vec![seg(if d { "●" } else { " " }, Yellow)]); c.on_click(Action::Dir(n.rel.clone()));
        } else {
            let base = match n.name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).as_deref() {
                Some("rs" | "ts" | "tsx" | "js" | "jsx" | "mjs") => Mint, Some("json" | "yaml" | "yml" | "toml") => Gold, _ => Text };
            let (badge, tone) = match kind(&n.rel) {
                Some(FileKind::Modified) => ("M", Yellow), Some(FileKind::Added) => ("A", Mint), Some(FileKind::Untracked) => ("?", Pink),
                Some(FileKind::Conflict) => ("U", Coral), Some(FileKind::Deleted) => ("D", Coral), None => (" ", base) };
            let name = if s.ui.tree_pick.as_deref() == Some(n.rel.as_str()) { bold(n.name.clone(), Pink) } else { seg(n.name.clone(), tone) };
            left.extend([seg(format!("{} ", file_icon(&n.name)), Text), name]);
            c.row(left, vec![seg(badge, tone)]);
            c.on_click(if kind(&n.rel).is_some() { Action::Inspect(n.rel.clone()) } else { Action::Pick(n.rel.clone()) });
        }
    }
    if more > 0 { c.row(vec![seg(format!("… {more} más"), Dim)], vec![]); }
    c
}

// ---- 3:PROF ---------------------------------------------------------------------------
/// `• name ··· model (effort)` on one row when it fits, else the model on its own indented row.
fn model_rows(c: &mut Card, left: Vec<Seg>, model: &str, effort: &str) {
    use Tone::*;
    let short = model.rsplit('/').next().unwrap_or_default(); let short = short.strip_suffix("-high").unwrap_or(short);
    let mut right = vec![seg(if short.is_empty() { "—" } else { short }, Gold)];
    if !effort.is_empty() { right.push(seg(format!(" ({effort})"), Dim)); }
    if width_of(&left) + width_of(&right) + 2 <= c.w.saturating_sub(4) { c.row(left, right); }
    else { c.row(left, vec![]); let mut sub = vec![seg("  ", Text)]; sub.extend(right); c.row(sub, vec![]); }
}
/// Subagent families of the reference (`getAgentStyle`): bullet and name tones.
fn agent_tones(name: &str) -> (Tone, Tone) {
    use Tone::*;
    if name.starts_with("gentle-ai-") { (Cyan, Cyan) } else if name.starts_with("jd-") { (Gold, Gold) }
    else if name.starts_with("review-") { (Mint, Mint) } else if name.starts_with("research-") { (Salmon, Salmon) } else { (Pink, Text) }
}
fn profiles_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let mut c = Card::new("profiles", mark("Profiles & Clusters", PinkBright), w);
    let r = s.roster.clone().unwrap_or_default();
    model_rows(&mut c, vec![seg("🎯 ", Border), bold("host / orquestador", Border)], &r.model, &r.thinking);
    if r.agents.is_empty() { c.row(vec![], vec![]); c.row(vec![seg("sin subagentes configurados", Dim)], vec![]); }
    for a in &r.agents {
        c.row(vec![], vec![]);
        let (bt, nt) = agent_tones(&a.name);
        model_rows(&mut c, vec![seg("• ", bt), seg(a.name.clone(), nt)], &a.model, &a.thinking);
    }
    if r.total > 0 {
        c.divider();
        let mut right = vec![seg(format!("{}/{}", r.done, r.total), Mint)];
        if r.active > 0 { right.push(seg(" · ", Text)); right.push(seg(format!("{} en curso", r.active), Gold)); }
        c.row(vec![seg("📋 Task Manager", Pink)], right);
        if let Some(t) = &r.current { c.row(vec![seg("   ▶ ", Text), seg(t.clone(), Text)], vec![]); }
    }
    c
}

// ---- 4:MEM ----------------------------------------------------------------------------
fn engram_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let e = &s.engram;
    let mut title = vec![seg("🧠 Engram", Pink)]; if !e.project.is_empty() { title.push(seg(": ", Pink)); title.push(seg(e.project.clone(), Cyan)); }
    let mut c = Card::new("engram", title, w);
    let (dot, tone) = if e.online { ("● Online", Mint) } else { ("○ Caído", Coral) };
    let obs = e.count.map_or("— obs".to_string(), |n| format!("{n} obs"));
    c.row(vec![seg("💻 ", Text), seg("Local (7437)", Text)], vec![seg(dot, tone), seg(" · ", Text), seg(obs, Cyan)]);
    if let Some(err) = &e.error { c.row(vec![seg(err.clone(), Coral)], vec![]); }
    c.divider();
    c.row(vec![seg("☁ ", Text), seg("Cloud", Text)], vec![]);
    match (&e.sync, e.enrolled) {
        (Some((phase, _)), true) if phase == "syncing" => c.row(vec![seg("   Sync:", Dim)], vec![seg("⟳ Sincronizando…", Yellow)]),
        (sync, true) => c.row(vec![seg("   Sync:", Dim)], vec![seg(format!("● Enrolado ({})", ago(sync.as_ref().and_then(|s| s.1), now_secs())), Mint)]),
        _ => c.row(vec![seg("   Estado:", Dim)], vec![seg("○ No sincronizado", Dim)]),
    }
    c
}
/// Handoff prose highlights of the reference: base branches mint, `#N` gold, arrows as a pink `➔`.
fn highlight(text: &str, base: Tone) -> Vec<Seg> {
    let mut out = Vec::new();
    for (i, word) in text.split(' ').enumerate() {
        if i > 0 { out.push(seg(" ", base)); }
        let bare = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '#');
        let tone = match bare { "main" | "develop" | "master" => Tone::Mint, _ if bare.len() > 1 && bare.starts_with('#') && bare[1..].chars().all(|c| c.is_ascii_digit()) => Tone::Gold, _ => base };
        match word { "->" | "➔" => out.push(seg("➔", Tone::Pink)), _ => out.push(seg(word, tone)) }
    }
    out
}
const HANDOFF_ROWS: usize = 24;
fn handoff_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let mut c = Card::new("handoff", vec![seg("🧠 Active Handoff", Gold)], w);
    let Some(h) = &s.engram.handoff else {
        c.row(vec![seg("Sin resumen de sesión previo", Dim)], vec![]); c.row(vec![seg("(se registra con mem_session_summary)", Dim)], vec![]); return c;
    };
    c.row(vec![seg(format!("#{}", h.id), Cyan), seg(" ", Text), seg(h.kind.clone(), Gold)], vec![seg(ago(h.at, now_secs()), Muted)]);
    c.divider(); c.row(vec![], vec![]);
    let inner = w.saturating_sub(4); let mut body: Vec<Vec<Seg>> = Vec::new();
    for l in &h.lines {
        if let Some(item) = ["- ", "* ", "• "].iter().find_map(|p| l.strip_prefix(p)) {
            let mut r = vec![seg("  • ", Pink)]; r.extend(fit(highlight(item.trim(), Cyan), inner.saturating_sub(5))); body.push(r);
        } else if l.ends_with(':') || l.starts_with('#') {
            body.push(vec![seg("✦ ", Pink), seg(l.trim_start_matches('#').trim(), Gold)]);
        } else { for p in wrap(l, inner.saturating_sub(2)) { body.push(highlight(&p, Cyan)); } }
    }
    let more = body.len().saturating_sub(HANDOFF_ROWS);
    for r in body.into_iter().take(HANDOFF_ROWS) { c.row(r, vec![]); }
    if more > 0 { c.row(vec![seg(format!("… {more} líneas más"), Dim)], vec![]); }
    c.row(vec![], vec![]);
    c
}
fn stack_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let mut c = Card::new("stack", [mark("Stack", Gold), vec![seg(" · ", Muted), seg("Ecosistema", Pink)]].concat(), w);
    if s.stack.is_empty() { c.row(vec![seg("Detectando herramientas…", Dim)], vec![]); }
    for t in &s.stack {
        let accent = match t.name { "pi" | "herdr" => Cyan, "gs3-hud" => Pink, "gentle-shell" => Gold, "gentle-ai" => Mint, "engram" => Secondary, _ => Text };
        match &t.version {
            Some(v) => c.row(vec![seg("● ", Mint), seg(t.name, accent), seg(format!(" v{v}"), Text)], vec![]),
            None => c.row(vec![seg("○ ", Dim), seg(t.name, Dim), seg(" -", Dim)], vec![]),
        }
    }
    c
}
/// Reference `formatToolPill` order and tones; zero counts are skipped.
fn tool_pills(t: &Session) -> Vec<Vec<Seg>> {
    use Tone::*;
    [(t.reads, "✎", "read", Read), (t.writes, "✎", "write", Cyan), (t.bash, ">_", "bash", Green), (t.mem, "🧠", "engram", Salmon),
     (t.grep, "🔍", "grep", Yellow), (t.fetch, "↓", "fetch", Pink), (t.search, "◆", "search", Secondary), (t.other, "⚙", "other", Dim)]
        .into_iter().filter(|p| p.0 > 0).map(|(n, g, l, tone)| vec![seg(format!("{g} {n} {l}"), tone)]).collect()
}
fn tools_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let t = &s.session; let mut c = Card::new("tools", vec![seg("🔧 tools", Gold)], w);
    c.extra = vec![(seg(format!("{} calls", t.tools), Dim), None)];
    let pills = tool_pills(t);
    if pills.is_empty() { c.row(vec![seg("sin llamadas todavía", Dim)], vec![]); }
    // `wrapToolPills`: 3 spaces between pills, a new row when the next one does not fit.
    let inner = w.saturating_sub(4); let mut line: Vec<Seg> = Vec::new();
    for p in pills {
        if !line.is_empty() && width_of(&line) + 3 + width_of(&p) > inner { c.row(std::mem::take(&mut line), vec![]); }
        if !line.is_empty() { line.push(seg("   ", Text)); }
        line.extend(p);
    }
    if !line.is_empty() { c.row(line, vec![]); }
    c
}
fn mcp_card(s: &State, w: usize) -> Card {
    use Tone::*;
    let mut title = mark("MCP", PinkBright);
    if let Some(l) = &s.mcp { title.push(seg(format!(" · {}", l.len()), Muted)); }
    let mut c = Card::new("mcp", title, w);
    match &s.mcp {
        Some(list) => for m in list { c.row(vec![seg("● ", Mint), seg(m.name.clone(), Text)], vec![seg(if m.local { "[local]" } else { "[remote]" }, Dim)]); },
        None => c.row(vec![seg("sin mcp.json", Dim)], vec![]),
    }
    c
}

// ---- 6:CONFIG -------------------------------------------------------------------------
fn config_cards(s: &State, w: usize) -> Vec<Card> {
    use Tone::*;
    let ui = &s.ui;
    let mut name = Card::new("config-name", mark("Nombre", PinkBright), w);
    match &ui.editing {
        Some(buf) => {
            name.row(vec![seg("> ", Pink), seg(buf.clone(), Text), bold("▏", PinkBright)], vec![seg(format!("{}/{NAME_MAX}", buf.chars().count()), Dim)]);
            name.row(vec![seg("Enter guarda · Esc cancela", Dim)], vec![]);
        }
        None => {
            name.keyed("Nombre", vec![if ui.config.name.is_empty() { seg(" - - ", Dim) } else { seg(ui.config.name.clone(), PinkBright) }]);
            name.row(vec![seg("✎ editar", Pink)], vec![seg("vacío = \" - - \"", Dim)]); name.on_click(Action::Edit);
        }
    }
    let mut tabs = Card::new("config-tabs", mark("Pestañas", PinkBright), w);
    for t in Tab::ALL {
        let on = ui.config.visible(t);
        let right = if t == Tab::Config { seg("fija", Dim) } else if on { seg("visible", Mint) } else { seg("oculta", Dim) };
        let mut l = vec![seg(if on { "[✓] " } else { "[ ] " }, if on { Mint } else { Dim })];
        if t == Tab::Config { l.extend([seg(CONFIG_ICON, Purple), seg(" CONFIG", Text)]); } else { l.push(seg(t.label(), if on { Text } else { Muted })); }
        tabs.row(l, vec![right]);
        if t != Tab::Config { tabs.on_click(Action::Hide(t)); }
    }
    let mut mon = Card::new("config-monitor", mark("Monitor", PinkBright), w);
    mon.keyed("Panel", vec![seg(s.pane.clone(), Text)]);
    let file = match &ui.config_path { Some(p) => seg(display_path(p), Text), None => seg("sin --config (no se guarda)", Dim) };
    mon.keyed("Archivo", vec![file]);
    if let Some(e) = &ui.save_error { mon.row(vec![seg(format!("error al guardar: {e}"), Coral)], vec![]); }
    mon.divider();
    mon.row(vec![seg("1–6 ", Gold), seg("pestañas · ", Muted), seg("r ", Gold), seg("refrescar · ", Muted), seg("q ", Gold), seg("salir", Muted)], vec![]);
    mon.row(vec![seg("[-] ", Gold), seg("minimiza/maximiza · rueda: scroll", Muted)], vec![]);
    vec![name, tabs, mon]
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr as W;
    fn find<'a>(lines: &'a [String], needle: &str) -> &'a str { lines.iter().find(|l| l.contains(needle)).unwrap_or_else(|| panic!("no line with {needle:?} in {lines:#?}")) }
    fn on(s: &State, t: Tab, w: u16) -> Vec<String> { let mut s = s.clone(); s.ui.tab = t; render_lines(&s, w) }
    fn at(f: &Frame, needle: &str) -> (usize, usize) {
        let lines: Vec<String> = f.lines.iter().map(|l| plain(l)).collect();
        let r = lines.iter().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("no {needle:?} in {lines:#?}"));
        (lines[r][..lines[r].find(needle).unwrap()].width(), r)
    }
    fn sample() -> State {
        let mut s = State::new("w1:p1".into(), "/srv/repo".into()); s.status = "working".into(); s.connected = true;
        s.session = Session { input: 136, output: 14_400, read: 33_536, write: 0, used: 33_677, cost: 0.25, model: "cpamc/gpt-6.1-sol".into(), provider: "cpamc".into(), thinking: "medium".into(), limit: Some(370_000), tools: 5, reads: 1, writes: 2, mem: 1, bash: 1, ..Session::default() };
        s.git = Git { branch: "feat/x".into(), dirty: Some(2), ahead_behind: Some((1, 0)), commits: vec!["abc1234 first commit".into()], ..Git::default() };
        s.engram = Engram { project: "repo".into(), count: Some(7), online: true, ..Engram::default() };
        s.mcp = Some(vec![Mcp { name: "engram".into(), local: true }]);
        s
    }
    #[test]
    fn voice_row_colors_clicks_and_apply() {
        let mut s = sample();
        assert!(!apply(&mut s, Event::Voice(Voice::default())));
        for online in [false, true] { for mute in [false, true] { for deaf in [false, true] {
            let voice = Voice { online, mute, deaf };
            let changed = s.voice != voice;
            assert_eq!(apply(&mut s, Event::Voice(voice)), changed);
            assert!(!apply(&mut s, Event::Voice(voice)));
            let f = frame(&s, 42, 0);
            let icons = VOICE_ICONS.map(|i| i.map(String::from));
            let (bx, y) = at(&f, &icons[0][0]);
            assert_eq!(y, f.tab_row + 2, "glued to the divider, no blank line above");
            assert!(plain(&f.lines[y]).starts_with(&format!("{}{}   {}   {}", " ".repeat(bx), icons[0][0], icons[1][0], icons[2][0])));
            assert!(plain(&f.lines[y + 1]).starts_with(&format!("{}{}   {}   {}", " ".repeat(bx), icons[0][1], icons[1][1], icons[2][1])));
            assert_eq!(at(&f, "Status").1, y + 2, "Status starts right below the icons");
            assert!(!plain(&f.lines[y]).contains('╭') && !plain(&f.lines[y]).contains('│'), "no frame");
            for row in VOICE_ICONS.iter().flatten() { assert_eq!(row.width(), 5); }
            for (i, (action, cmd, active)) in [(Action::VoiceMute, VoiceCmd::Mute, mute || deaf), (Action::VoiceDeaf, VoiceCmd::Deaf, deaf), (Action::VoiceOpen, VoiceCmd::Open, false)].into_iter().enumerate() {
                let x0 = bx + i * 8;
                for row in [y, y + 1] {
                    let g = f.lines[row].iter().find(|g| g.text == VOICE_ICONS[i][row - y]).unwrap();
                    assert_eq!((g.tone, g.bold), (if !online { Tone::Text } else if active { Tone::VoiceRed } else { Tone::VoiceGreen }, true));
                    for col in x0 - 1..x0 + 6 { assert_eq!(click(&f, col, row), Some(action.clone()), "icon {i} col {col} row {row}"); }
                }
                if i < 2 { assert_eq!(click(&f, x0 + 6, y), None, "the gap middle is neutral"); }
                s.ui.voice_cmd = None;
                assert!(apply(&mut s, Event::Ui(action)));
                assert_eq!(s.ui.voice_cmd, Some(cmd)); assert!(!s.ui.save);
            }
            assert_eq!(click(&f, bx - 2, y), None); assert_eq!(click(&f, bx, y - 1), None);
        }}}
        for t in [Tab::Git, Tab::Prof, Tab::Mem, Tab::Tree, Tab::Config] { assert!(!on(&s, t, 42).iter().any(|l| l.contains(VOICE_ICONS[0][0]))); }
        assert_eq!(Tone::VoiceRed.rgb(), (242,63,67)); assert_eq!(Tone::VoiceGreen.rgb(), (35,165,90));
    }
    #[test]
    fn flameshot_icon_sits_at_the_right_end_and_opens_the_capture_tool() {
        let mut s = sample();
        for voice in [Voice::default(), Voice { online: true, mute: true, deaf: true }] {
            apply(&mut s, Event::Voice(voice));
            let f = frame(&s, 42, 0);
            let (bx, y) = at(&f, VOICE_ICONS[0][0]);
            let (fx, fy) = at(&f, FLAME_ICON[0]);
            assert_eq!(fy, y, "same rows as the voice icons");
            assert_eq!(fx, 42 - 1 - FLAME_ICON[0].width(), "flush with the right edge");
            assert_eq!(at(&f, FLAME_ICON[1]), (fx, y + 1));
            assert_eq!(bx, 10, "the Discord icons stay centered where they were");
            assert!(FLAME_ICON.iter().all(|r| r.width() == 3), "smaller than the 5-column voice icons");
            for g in f.lines[y].iter().filter(|g| g.text == FLAME_ICON[0]) { assert_eq!(g.tone, Tone::Yellow); }
            for col in fx - 1..fx + 4 { for row in [y, y + 1] { assert_eq!(click(&f, col, row), Some(Action::Screenshot), "col {col} row {row}"); } }
            assert_eq!(click(&f, fx - 2, y), None); assert_eq!(click(&f, fx, y - 1), None);
            assert_ne!(click(&f, bx + 16, y), Some(Action::Screenshot), "the gear keeps its own click area");
            s.ui.screenshot = false;
            assert!(apply(&mut s, Event::Ui(Action::Screenshot)));
            assert!(s.ui.screenshot && !s.ui.save && s.ui.voice_cmd.is_none());
        }
        for t in [Tab::Git, Tab::Prof, Tab::Mem, Tab::Tree, Tab::Config] { assert!(!on(&s, t, 42).iter().any(|l| l.contains(FLAME_ICON[0]))); }
        // Too narrow to sit beside the gear without touching it: the flame is dropped, the voice icons stay.
        let narrow = render_lines(&s, 28);
        assert!(!narrow.iter().any(|l| l.contains(FLAME_ICON[0])) && narrow.iter().any(|l| l.contains(VOICE_ICONS[0][0])));
    }
    #[test]
    fn model_picker_clicks_every_model_row() {
        let mut s = sample();
        for needle in ["G P T - 6 . 1 - S O L", "◆ MEDIUM ◆"] {
            s.ui.open_picker = false;
            let f = frame(&s, 42, 0); let (x, y) = at(&f, needle);
            assert_eq!(click(&f, x, y), Some(Action::ModelPicker));
            assert!(apply(&mut s, Event::Ui(click(&f, x, y).unwrap())));
            assert!(s.ui.open_picker); assert!(!s.ui.save);
        }
        s.session.model.clear();
        let f = frame(&s, 42, 0); let (x, y) = at(&f, "sin modelo");
        assert_eq!(click(&f, x, y), Some(Action::ModelPicker));
        s.session.model = "provider/long-model-name-that-wraps-across-rows".into();
        let f = frame(&s, 24, 0);
        let first = at(&f, "LONG-MODEL").1;
        for y in first..at(&f, "MEDIUM").1 + 1 {
            assert_eq!(click(&f, 3, y), Some(Action::ModelPicker));
        }
        // Existing title collapse is still separate, and collapsed rows have no picker hits.
        apply(&mut s, Event::Ui(Action::Collapse("status".into())));
        let f = frame(&s, 42, 0);
        assert!(!f.hits.iter().any(|h| h.action == Action::ModelPicker));
        let (x, y) = at(&f, "Status");
        assert_eq!(click(&f, x, y), Some(Action::Collapse("status".into())));
    }
    #[test]
    fn banner_placeholder_name_and_tab_bar() {
        let mut s = sample(); let l = render_lines(&s, 60);
        assert_eq!(l[0], ""); assert!(l[1].contains("⌁ - - ⌁"), "{l:#?}"); assert!(l[2].trim() == "GENTLE-SHELL");
        assert_eq!(l[2].find('G').unwrap(), 1 + (58 - 12) / 2, "centered in the padded rail");
        // Wide: `[ MAIN ]` active, other tabs by name (no numbers), CONFIG is the gear, git sync badge, gaps spread evenly.
        let tabs = &l[4]; assert!(tabs.starts_with(" [ MAIN ]") && tabs.contains("GIT ▲1") && tabs.trim_end().ends_with('⛭'), "{tabs:?}");
        assert_eq!(tabs.split_whitespace().collect::<Vec<_>>(), ["[", "MAIN", "]", "GIT", "▲1", "PROF", "MEM", "TREE", "⛭"]);
        assert_eq!(tabs.width(), 59); assert!(l[5].trim().chars().all(|c| c == '─') && l[5].width() == 59); assert!(l[6].contains(VOICE_ICONS[0][0]), "MAIN: icons right under the divider");
        for t in [Tab::Git, Tab::Prof, Tab::Mem, Tab::Tree, Tab::Config] { let mut o = s.clone(); o.ui.tab = t; let f = frame(&o, 60, 0); assert!(f.lines[f.tab_row + 2].is_empty() || plain(&f.lines[f.tab_row + 2]).trim().is_empty(), "{t:?} keeps its blank line"); }
        // Under 44 columns: compact `[MAIN]`; hidden tabs leave no gap.
        let bar = |s: &State, w: u16| render_lines(s, w)[4].split_whitespace().map(String::from).collect::<Vec<_>>();
        assert_eq!(bar(&s, 42), ["[MAIN]", "GIT", "▲1", "PROF", "MEM", "TREE", "⛭"]); assert_eq!(bar(&s, 37), bar(&s, 42), "the real 37-col pane keeps full names");
        s.git.ahead_behind = None; s.ui.config.hidden = vec![Tab::Tree]; assert_eq!(bar(&s, 42), ["[MAIN]", "GIT", "PROF", "MEM", "⛭"]);
        assert!(!render_lines(&s, 42)[4].chars().any(|c| c.is_ascii_digit()), "no tab numbers");
        s.git.ahead_behind = Some((1, 0)); s.ui.config.hidden.clear();
        // Too narrow for the names: two-letter names, still one per tab and clickable.
        assert_eq!(bar(&s, 24), ["[MA]", "GI", "▲1", "PR", "ME", "TR", "⛭"]);
        s.ui.tab = Tab::Config; assert_eq!(bar(&s, 42).last().unwrap(), "[⛭]"); s.ui.tab = Tab::Main;
        // S11: CONFIG tab is the gearbox `⛭`, banner and card titles the power mark `⌁`; both purple, 1 column, no `⚙` left.
        let f = frame(&s, 42, 0); let icon = |c: char| f.lines.iter().flatten().filter(|g| g.text.contains(c)).cloned().collect::<Vec<_>>();
        let (tab, mark) = (icon('⛭'), icon('⌁')); assert!(tab.len() == 1 && mark.len() >= 4, "{tab:?} {mark:?}");
        assert!(tab.iter().chain(&mark).all(|g| (g.text == "⛭" || g.text == "⌁") && g.tone == Tone::Purple && g.text.width() == 1));
        assert_eq!(icon('⚙').len(), 0, "the voice gear is pixel art now"); assert!(f.lines[f.tab_row].iter().any(|g| g.text == "⛭"));
        assert!(f.lines.iter().flatten().any(|g| g.text == " Status" && g.tone == Tone::PinkBright));
        s.ui.config.name = "RamonsDev".into(); let l = render_lines(&s, 42);
        assert!(l[1].contains("⌁ RamonsDev ⌁") && !l[1].contains("- -"));
        for w in [0u16, 1, 5, 20, 42, 80] { for t in Tab::ALL { for l in on(&s, t, w) { assert!(l.width() <= w as usize, "{w} {l:?}"); } } }
    }
    #[test]
    fn cards_use_the_double_frame_full_width() {
        let s = sample();
        for t in Tab::ALL { for l in on(&s, t, 42).iter().skip(7).filter(|l| !l.trim().is_empty() && !l.contains("Sin información") && !l.contains(VOICE_ICONS[0][0]) && !l.contains(VOICE_ICONS[0][1])) {
            assert_eq!(l.width(), 41, "{l:?}"); let c = l.trim_start().chars().next().unwrap(); let z = l.chars().last().unwrap();
            assert!(matches!((c, z), ('╔', '╗') | ('║', '║') | ('╠', '╣') | ('╚', '╝')), "{l:?}");
        } }
        let l = on(&s, Tab::Main, 42);
        assert!(find(&l, "⌁ Status").starts_with(" ╔═ ⌁ Status ═") && find(&l, "⌁ Status").ends_with(" [-] ═╗"));
        assert!(find(&l, "Project").contains("/srv/repo")); assert!(find(&l, "Branch").contains("⎇ feat/x ±2"));
        // Model: its own section between dividers, spaced capitals (no provider) and `◆ MEDIUM ◆`, both centered.
        assert!(!l.iter().any(|x| x.contains("Model") || x.contains("CPAMC")));
        let i = l.iter().position(|x| x.contains("G P T - 6 . 1 - S O L")).unwrap();
        assert!(l[i - 1].contains("╠═") && l[i + 1].contains("◆ MEDIUM ◆") && l[i + 2].contains("╠═"), "{:#?}", &l[i - 1..i + 3]);
        for r in [&l[i], &l[i + 1]] { let inner = r.trim_start().trim_start_matches('║').trim_end().trim_end_matches('║'); let (a, b) = (inner.len() - inner.trim_start().len(), inner.len() - inner.trim_end().len()); assert!(a.abs_diff(b) <= 1, "centered {r:?}"); }
        let f = frame(&s, 42, 0); let tone = |t: &str| f.lines.iter().flatten().find(|g| g.text.contains(t)).map(|g| (g.tone, g.bold));
        assert_eq!([tone("G P T"), tone("MEDIUM")], [Some((Tone::PinkBright, true)), Some((Tone::Gold, false))]);
        // Too long to space out: plain capitals, wrapped inside the card; no thinking (or off): no ◆ row; no model: dim placeholder.
        let mut m = s.clone(); m.session.model = "cpamc/claude-opus-4-1-20250805-extended".into(); m.session.thinking = "off".into();
        let l = render_lines(&m, 30); let rows: Vec<String> = l.iter().filter(|x| x.contains("CLAUDE") || x.contains("EXTENDED")).map(|x| x.trim().trim_matches('║').trim().to_string()).collect();
        assert_eq!(rows.concat(), "CLAUDE-OPUS-4-1-20250805-EXTENDED"); assert!(rows.len() == 2 && !l.iter().any(|x| x.contains('◆')), "{l:#?}");
        m.session.model.clear(); find(&render_lines(&m, 42), "sin modelo");
        // A fresh session (no assistant turn yet) shows the host defaults from settings.json, like the gs2 footer.
        m.session.thinking.clear(); m.roster = roster_sample().roster; let l = render_lines(&m, 42); find(&l, "G P T - 6 . 1 - S O L"); find(&l, "◆ MEDIUM ◆");
        for w in [20u16, 30, 36, 42, 80] { for x in render_lines(&s, w) { assert!(x.width() <= w as usize, "{w} {x:?}"); } }
        let l = on(&s, Tab::Main, 42);
        assert!(find(&l, "● working").contains("—")); assert!(find(&l, "herdr ● w1:p1").contains("conectado"));
        assert!(find(&l, "33.7k / 370k tokens").contains("● Óptimo 9.1%"));
        let gauge = find(&l, "▰"); assert_eq!(gauge.matches('▰').count() + gauge.matches('▱').count(), 36);
        assert!(find(&l, "▲ 33.7k in").contains("▼ 14.4k out") && find(&l, "▲ 33.7k in").contains("Cost $0.250"));
        find(&on(&s, Tab::Tree, 42), "Escaneando…");
    }
    fn tree_sample() -> State {
        let mut s = git_sample(); s.ui.tab = Tab::Tree;
        let n = |rel: &str, dir: bool, count: usize, children: Vec<TreeNode>| TreeNode { name: rel.rsplit('/').next().unwrap().into(), rel: rel.into(), dir, count, children };
        s.tree = Some(Tree { project: "repo".into(), open: vec!["src".into()], error: None, nodes: vec![
            n("docs", true, 2, vec![]), n("src", true, 2, vec![n("src/a.rs", false, 0, vec![]), n("src/lib.rs", false, 0, vec![])]),
            n("Cargo.toml", false, 0, vec![]), n("new.txt", false, 0, vec![]), n("README.md", false, 0, vec![])] });
        s.ui.tree_open = vec!["src".into()]; s
    }
    #[test]
    fn tree_tab_shows_project_tree_with_git_badges() {
        let mut s = tree_sample(); let l = render_lines(&s, 42);
        assert!(find(&l, "⌁ PROYECTO · repo").contains("[ ⊟ ] [-] ═╗"));
        assert!(find(&l, "├── 📁 docs (2 items)").ends_with("  ║")); assert!(find(&l, "├── 📂 src (2 items)").ends_with("● ║"));
        assert!(find(&l, "│   ├── 📄 a.rs").ends_with("M ║")); assert!(find(&l, "│   └── 📄 lib.rs").ends_with("  ║"));
        find(&l, "├── 🔧 Cargo.toml"); assert!(find(&l, "├── 📝 new.txt").ends_with("? ║")); find(&l, "└── 📝 README.md");
        let f = frame(&s, 42, 0); let tone = |needle: &str| f.lines.iter().flatten().find(|g| g.text == needle).map(|g| (g.tone, g.bold));
        assert_eq!([tone("src"), tone("docs"), tone("a.rs"), tone("lib.rs"), tone("Cargo.toml"), tone("new.txt"), tone("├── ")],
            [Some((Tone::Yellow, false)), Some((Tone::Cyan, false)), Some((Tone::Yellow, false)), Some((Tone::Mint, false)), Some((Tone::Gold, false)), Some((Tone::Pink, false)), Some((Tone::Dim, false))]);
        // Folder click toggles it; a stale scan (old open set) is dropped, the matching one is kept.
        let (x, y) = at(&f, "docs"); let a = click(&f, x, y).unwrap(); assert_eq!(a, Action::Dir("docs".into()));
        apply(&mut s, Event::Ui(a)); assert_eq!(s.ui.tree_open, ["docs", "src"]);
        let old = s.tree.clone().unwrap(); let mut t = old.clone(); t.nodes.clear();
        assert!(!apply(&mut s, Event::Tree(t.clone()))); assert_eq!(s.tree.as_ref(), Some(&old));
        t.open = s.ui.tree_open.clone(); assert!(apply(&mut s, Event::Tree(t))); find(&render_lines(&s, 42), "Sin archivos en el proyecto · ⌁");
        // A changed file opens its diff in GIT; an unchanged one is only selected (bold pink).
        let mut s = tree_sample(); let f = frame(&s, 42, 0);
        let (x, y) = at(&f, "a.rs"); let a = click(&f, x, y).unwrap(); assert_eq!(a, Action::Inspect("src/a.rs".into()));
        apply(&mut s, Event::Ui(a)); assert_eq!((s.ui.tab, s.ui.git_file.as_deref(), s.ui.git_branch.as_deref()), (Tab::Git, Some("src/a.rs"), None));
        let mut s = tree_sample(); let f = frame(&s, 42, 0);
        let (x, y) = at(&f, "lib.rs"); let a = click(&f, x, y).unwrap(); assert_eq!(a, Action::Pick("src/lib.rs".into()));
        apply(&mut s, Event::Ui(a)); assert_eq!(s.ui.tab, Tab::Tree);
        let f = frame(&s, 42, 0); assert_eq!(f.lines.iter().flatten().find(|g| g.text == "lib.rs").map(|g| (g.tone, g.bold)), Some((Tone::Pink, true)));
        // [ ⊟ ] folds everything.
        let (x, y) = at(&f, "⊟"); let a = click(&f, x, y).unwrap(); assert_eq!(a, Action::Fold);
        apply(&mut s, Event::Ui(a)); let l = render_lines(&s, 42);
        find(&l, "├── 📁 src (2 items)"); assert!(!l.iter().any(|x| x.contains("a.rs") || x.contains("⊟")));
        // Loading, error, very long trees, narrow widths.
        s.tree = None; find(&render_lines(&s, 42), "Escaneando…");
        s.tree = Some(Tree { error: Some("permission denied".into()), ..Tree::default() }); find(&render_lines(&s, 42), "permission denied");
        let many = (0..400).map(|i| TreeNode { name: format!("f{i:03}.txt"), rel: format!("f{i:03}.txt"), ..TreeNode::default() }).collect();
        s.tree = Some(Tree { project: "repo".into(), nodes: many, ..Tree::default() }); s.ui.tree_open.clear();
        let l = render_lines(&s, 42); find(&l, "f299.txt"); assert!(!l.iter().any(|x| x.contains("f300.txt"))); find(&l, "… 100 más");
        for w in [20u16, 30, 36, 42, 80] { for x in render_lines(&tree_sample(), w) { assert!(x.width() <= w as usize, "{w} {x:?}"); } }
    }
    fn roster_sample() -> State {
        let mut s = sample();
        let a = |n: &str, m: &str, t: &str| Agent { name: n.into(), model: m.into(), thinking: t.into() };
        s.roster = Some(Roster { model: "cpamc/gpt-6.1-sol".into(), thinking: "medium".into(), total: 6, done: 5, active: 1, current: Some("gentle-ai-worker · build".into()),
            agents: vec![a("gentle-ai-explore", "cpamc/gpt-6.1-sol", "medium"), a("jd-judge-a", "x/claude-opus-high", "high"), a("my-helper", "y/long-model-name-v2", "xhigh"), a("review-risk", "", "")] });
        s
    }
    #[test]
    fn prof_tab_lists_host_and_subagents() {
        let s = roster_sample(); let l = on(&s, Tab::Prof, 60);
        let host = find(&l, "🎯 host / orquestador"); assert!(host.contains("gpt-6.1-sol (medium)") && !host.contains("cpamc/"), "{host}");
        assert!(find(&l, "• gentle-ai-explore").ends_with("gpt-6.1-sol (medium) ║"));
        assert!(find(&l, "• jd-judge-a").contains("claude-opus (high)"), "-high suffix dropped like shortModelName");
        assert!(find(&l, "• review-risk").contains("— "), "missing model shows a dash");
        assert!(find(&l, "📋 Task Manager").contains("5/6 · 1 en curso")); find(&l, "▶ gentle-ai-worker · build");
        // Family tones of the reference: ODD core cyan, Judgment Day gold, review mint, custom pink bullet + text.
        let mut p = s.clone(); p.ui.tab = Tab::Prof; let f = frame(&p, 60, 0); let tone = |needle: &str| f.lines.iter().flatten().find(|g| g.text == needle).map(|g| g.tone);
        assert_eq!((tone("gentle-ai-explore"), tone("jd-judge-a"), tone("review-risk"), tone("my-helper")), (Some(Tone::Cyan), Some(Tone::Gold), Some(Tone::Mint), Some(Tone::Text)));
        // Narrow: name and model do not fit together → the model moves to its own row, nothing is cut.
        let l = on(&s, Tab::Prof, 36); let i = l.iter().position(|x| x.contains("• my-helper")).unwrap();
        assert!(!l[i].contains("long-model") && l[i + 1].contains("long-model-name-v2 (xhigh)"), "{:#?}", &l[i..i + 2]);
        for w in [20u16, 36, 38, 42, 80] { for x in on(&s, Tab::Prof, w) { assert!(x.width() <= w as usize, "{w} {x:?}"); } }
        let mut e = sample(); e.roster = None; let l = on(&e, Tab::Prof, 42);
        find(&l, "⌁ Profiles & Clusters"); find(&l, "sin subagentes configurados"); assert!(!l.iter().any(|x| x.contains("Task Manager")));
    }
    #[test]
    fn mem_tab_engram_handoff_stack_tools_and_mcp() {
        let mut s = sample(); s.ui.tab = Tab::Mem;
        s.session.grep = 2; s.session.fetch = 1; s.session.search = 3; s.session.other = 4; s.session.tools = 14;
        s.engram.handoff = Some(Handoff { id: 214, kind: "session_summary".into(), at: Some(now_secs() - 7200),
            lines: vec!["## Goal".into(), "Ship PR #12 to main -> done".into(), "- merged #12 into develop".into()] });
        s.stack = vec![Tool { name: "pi", version: Some("1.5.0".into()) }, Tool { name: "gentle-ai", version: None }];
        let l = render_lines(&s, 42);
        assert!(find(&l, "🧠 Engram: repo").contains("[-]"));
        assert!(find(&l, "💻 Local (7437)").ends_with("● Online · 7 obs ║")); find(&l, "☁ Cloud");
        assert!(find(&l, "Estado:").contains("○ No sincronizado"));
        assert!(find(&l, "#214 session_summary").contains("hace 2h")); find(&l, "✦ Goal"); find(&l, "• merged #12 into develop");
        let f = frame(&s, 42, 0); let tone = |needle: &str| f.lines.iter().flatten().find(|g| g.text == needle).map(|g| g.tone);
        assert_eq!((tone("main"), tone("#12"), tone("➔"), tone("done")), (Some(Tone::Mint), Some(Tone::Gold), Some(Tone::Pink), Some(Tone::Cyan)));
        find(&l, "⌁ Stack · Ecosistema"); find(&l, "● pi v1.5.0"); find(&l, "○ gentle-ai -");
        // Tool pills wrap (3-space gap) instead of being cut; the call count sits in the title bar.
        let w38 = render_lines(&s, 38); assert!(find(&w38, "🔧 tools").contains("14 calls [-]"), "{w38:#?}");
        let pills: String = w38.iter().skip_while(|x| !x.contains("🔧 tools")).skip(1).take_while(|x| !x.contains('╚')).cloned().collect();
        for p in ["✎ 1 read", "✎ 2 write", ">_ 1 bash", "🧠 1 engram", "🔍 2 grep", "↓ 1 fetch", "◆ 3 search", "⚙ 4 other"] { assert!(pills.contains(p), "{p} in {pills}"); }
        assert!(find(&l, "⌁ MCP · 1").contains("[-]"));
        // Enrolled → Sync row with the last sync; syncing phase; daemon down and DB error.
        s.engram.enrolled = true; s.engram.sync = Some(("idle".into(), Some(now_secs() - 120)));
        assert!(find(&render_lines(&s, 42), "Sync:").contains("● Enrolado (hace 2m)"));
        s.engram.sync = Some(("syncing".into(), None)); assert!(find(&render_lines(&s, 42), "Sync:").contains("⟳ Sincronizando…"));
        s.engram = Engram { project: "repo".into(), error: Some("unable to open".into()), ..Engram::default() }; s.stack.clear(); s.session = Session::default();
        let l = render_lines(&s, 42);
        assert!(find(&l, "Local (7437)").contains("○ Caído · — obs")); find(&l, "unable to open");
        find(&l, "Sin resumen de sesión previo"); find(&l, "Detectando herramientas…"); find(&l, "sin llamadas todavía");
        for w in [20u16, 36, 38, 42, 80] { for x in render_lines(&s, w) { assert!(x.width() <= w as usize, "{w} {x:?}"); } }
    }
    #[test]
    fn long_handoff_is_bounded() {
        let mut s = sample(); s.ui.tab = Tab::Mem;
        s.engram.handoff = Some(Handoff { id: 1, kind: "session_summary".into(), at: None, lines: (0..40).map(|i| format!("line {i}")).collect() });
        let l = render_lines(&s, 42); find(&l, "#1 session_summary").contains("nunca"); find(&l, "line 23"); assert!(!l.iter().any(|x| x.contains("line 24")));
        find(&l, "… 16 líneas más");
    }
    #[test]
    fn todos_card_mirrors_the_session_checklist() {
        let mut s = sample(); let l = render_lines(&s, 42);
        assert!(find(&l, "⌁ Todos · 0 of 0").starts_with(" ╔═ ")); find(&l, "sin tareas en esta sesión");
        let t = |title: &str, status, note: Option<&str>| Todo { title: title.into(), status, note: note.map(Into::into) };
        s.session.todos = vec![t("Escribir el parser", TodoStatus::Done, Some("hidden when done")),
            t("Conectar el mouse y las teclas del panel lateral de monitoreo", TodoStatus::InProgress, Some("probando clicks reales en herdr con SGR")),
            t("Docs", TodoStatus::Pending, None)];
        let l = render_lines(&s, 42); let i = l.iter().position(|l| l.contains("⌁ Todos · 1 of 3")).unwrap();
        let body: Vec<&str> = l[i + 1..].iter().take_while(|l| !l.contains('╚')).map(|l| l[" ║ ".len()..l.len() - " ║".len()].trim_end()).collect();
        assert_eq!(body, ["✓ Escribir el parser", "◉ Conectar el mouse y las teclas del", "  panel lateral de monitoreo",
            "  ↳ probando clicks reales en herdr", "    con SGR", "○ Docs"]);
        assert!(!l.iter().any(|l| l.contains("hidden when done")));
        // Tones: done dim, in progress pinkBright with a dim note, glyphs mint / gold / dim.
        let f = frame(&s, 42, 0); let rows: Vec<&Vec<Seg>> = f.lines.iter().filter(|l| plain(l).contains("Escribir") || plain(l).contains("Conectar") || plain(l).contains("↳") || plain(l).contains("○ Docs")).collect();
        let tone = |r: &Vec<Seg>, txt: &str| r.iter().find(|g| g.text.contains(txt)).map(|g| g.tone);
        assert_eq!((tone(rows[0], "✓"), tone(rows[0], "Escribir")), (Some(Tone::Mint), Some(Tone::Dim)));
        assert_eq!((tone(rows[1], "◉"), tone(rows[1], "Conectar")), (Some(Tone::Gold), Some(Tone::PinkBright)));
        assert_eq!(tone(rows[2], "↳"), Some(Tone::Dim)); assert_eq!((tone(rows[3], "○"), tone(rows[3], "Docs")), (Some(Tone::Dim), Some(Tone::Text)));
        // A word longer than the row is cut, never overflowing.
        s.session.todos = vec![t(&"x".repeat(80), TodoStatus::Pending, None)];
        for w in [20u16, 42, 80] { for l in render_lines(&s, w) { assert!(l.width() <= w as usize); } }
    }
    fn git_sample() -> State {
        let mut s = sample(); s.ui.tab = Tab::Git;
        s.git.graph = vec!["* abc1234 (HEAD -> feat/x, origin/feat/x) first commit".into(), "|\\".into(), "| * 9f8e7d6 side".into(), "|/".into(), "* 1234567 base".into()];
        let f = |path: &str, kind, added, deleted| FileChange { path: path.into(), kind, added, deleted };
        s.git.files = vec![f("src/a.rs", FileKind::Modified, 3, 1), f("new.txt", FileKind::Untracked, 0, 0), f("gs3/hud/src/a_very_long_file_name_here.rs", FileKind::Added, 10, 0)];
        s.git.counts = Counts { staged: 0, modified: 1, untracked: 1, conflicts: 0 }; s.git.added = 13; s.git.deleted = 1; s.git.dirty = Some(3);
        let b = |name: &str, current, remote, ahead, behind, gone, date: &str| Branch { name: name.into(), current, remote, ahead, behind, gone, date: date.into() };
        s.git.branches = vec![b("feat/x", true, false, 1, 0, false, "5m"), b("main", false, false, 2, 1, false, "3d"), b("gone", false, false, 0, 0, true, "2w"), b("origin/old", false, true, 0, 0, false, "1y")];
        s
    }
    #[test]
    fn git_graph_card_with_view_toggle() {
        let mut s = git_sample(); let l = render_lines(&s, 42);
        let t = find(&l, "⎇ git graph"); assert!(t.contains("[ Grafo | Ramas ] [-] ═╗") && t.width() == 41, "{t:?}");
        assert!(find(&l, "feat/x ").contains("● 1 mod · ?1 untracked"), "branch row with status badges");
        let f = frame(&s, 80, 0); let row = f.lines.iter().find(|l| plain(l).contains("abc1234")).unwrap();
        let tone = |txt: &str| row.iter().find(|g| g.text.contains(txt)).map(|g| g.tone);
        assert_eq!([tone("*"), tone("abc1234"), tone("HEAD -> feat/x"), tone("origin/feat/x"), tone("first commit"), tone("(")],
            [Some(Tone::Pink), Some(Tone::Yellow), Some(Tone::Mint), Some(Tone::Secondary), Some(Tone::Text), Some(Tone::Muted)]);
        assert_eq!(f.lines.iter().find(|l| plain(l).contains("|/")).unwrap().iter().find(|g| g.text.contains('|')).unwrap().tone, Tone::Blue);
        find(&l, "✎ 1 file · +13 −1"); let f = frame(&s, 42, 0);
        // Click "Ramas": the branches view.
        let (x, y) = at(&f, "Ramas"); let a = click(&f, x + 1, y).unwrap(); assert_eq!(a, Action::GitView(GitView::Branches));
        apply(&mut s, Event::Ui(a)); let l = render_lines(&s, 42);
        find(&l, "● 4 ramas en repo"); assert!(find(&on(&s, Tab::Git, 60), "● 4 ramas en repo").ends_with("● 1 mod · ?1 untracked ║"), "badge when it fits");
        assert!(find(&l, "⎇ feat/x").ends_with("actual ║")); assert!(find(&l, "● main").ends_with("↑2 ↓1 ║"));
        assert!(find(&l, "● gone").ends_with("gone ║")); assert!(find(&l, "● rem/old").ends_with("1y ║"));
        assert!(!l.iter().any(|l| l.contains("abc1234")));
        let f = frame(&s, 42, 0); let (x, y) = at(&f, "Grafo"); apply(&mut s, Event::Ui(click(&f, x + 1, y).unwrap())); assert_eq!(s.ui.git_view, GitView::Graph);
        // Narrow card: compact `[ G | R ]`; collapsed: no toggle.
        assert!(find(&render_lines(&s, 38), "⎇ git graph").contains("[ G | R ] [-] ═╗"));
        s.ui.config.collapsed = vec!["git".into()]; assert!(find(&render_lines(&s, 42), "⎇ git graph").ends_with("═ [+] ═╗"));
        s.git.files.clear(); s.git.counts = Counts::default(); s.git.dirty = Some(0); s.ui.config.collapsed.clear();
        assert!(find(&render_lines(&s, 42), "feat/x ").contains("✔ clean"));
    }
    #[test]
    fn working_tree_card_lists_changes() {
        let mut s = git_sample(); let l = render_lines(&s, 42);
        assert!(find(&l, "📁 Working Tree").starts_with(" ╔═ "));
        assert!(find(&l, "● 3 archivos modificados").contains("+13  −1"));
        assert!(find(&l, "M  src/a.rs").ends_with("[diff] +3 −1 ║")); assert!(find(&l, "?  new.txt").ends_with("[diff] ║"));
        assert!(find(&l, "A  …/a_very_long_file_").ends_with("[diff] +10 ║"), "the right side wins; the directory shrinks first");
        let f = frame(&s, 42, 0); let row = f.lines.iter().find(|l| plain(l).contains("src/a.rs")).unwrap();
        let tone = |txt: &str| row.iter().find(|g| g.text == txt).map(|g| g.tone);
        assert_eq!([tone("M"), tone("src/"), tone("a.rs"), tone("diff"), tone("+3"), tone("−1")], [Some(Tone::Yellow), Some(Tone::Dim), Some(Tone::Cyan), Some(Tone::Pink), Some(Tone::Mint), Some(Tone::Coral)]);
        assert!(find(&l, "click: ver diff").starts_with(" ║ "));
        s.git.files.clear(); find(&render_lines(&s, 42), "✔ Clean · Sin cambios pendientes");
        for w in [20u16, 30, 38, 42, 80] { for l in render_lines(&git_sample(), w) { assert!(l.width() <= w as usize, "{w} {l:?}"); } }
    }
    #[test]
    fn clicking_a_file_previews_its_diff_inline() {
        let mut s = git_sample(); let f = frame(&s, 42, 0);
        let (x, y) = at(&f, "src/a.rs"); let a = click(&f, x, y).unwrap(); assert_eq!(a, Action::File("src/a.rs".into()));
        apply(&mut s, Event::Ui(a)); assert_eq!(s.ui.git_file.as_deref(), Some("src/a.rs"));
        find(&render_lines(&s, 42), "cargando diff…");
        let diff = ["diff --git a/src/a.rs b/src/a.rs", "index 1..2 100644", "--- a/src/a.rs", "+++ b/src/a.rs", "@@ -1,2 +1,2 @@", " keep\tit", "-old", "+new"];
        apply(&mut s, Event::Detail(Detail { file: Some("src/a.rs".into()), diff: diff.iter().map(|l| l.to_string()).collect(), ..Detail::default() }));
        let l = render_lines(&s, 42); let i = l.iter().position(|l| l.contains("M  src/a.rs")).unwrap();
        let rows: Vec<&str> = l[i + 1..i + 5].iter().map(|l| l[" ║ ".len()..l.len() - " ║".len()].trim_end()).collect();
        assert_eq!(rows, ["  @@ -1,2 +1,2 @@", "   keep    it", "  -old", "  +new"]); assert!(l[i + 5].contains("?  new.txt"));
        let f = frame(&s, 42, 0); let tone = |txt: &str| f.lines.iter().flatten().find(|g| g.text.contains(txt)).map(|g| g.tone);
        assert_eq!([tone("@@ -1"), tone("-old"), tone("+new")], [Some(Tone::Blue), Some(Tone::Coral), Some(Tone::Mint)]);
        // Long diffs are capped; a second click closes the preview.
        s.detail.diff = (0..200).map(|i| format!("+line {i}")).collect(); let l = render_lines(&s, 42);
        find(&l, "… 120 líneas más"); assert!(!l.iter().any(|l| l.contains("+line 80")));
        let f = frame(&s, 42, 0); let (x, y) = at(&f, "src/a.rs"); apply(&mut s, Event::Ui(click(&f, x, y).unwrap()));
        assert_eq!(s.ui.git_file, None); assert!(!render_lines(&s, 42).iter().any(|l| l.contains("+line")));
        // Possible secrets are never previewed; a file that left the working tree drops its selection.
        s.ui.git_file = Some(".env.local".into()); s.git.files.push(FileChange { path: ".env.local".into(), kind: FileKind::Untracked, ..FileChange::default() });
        find(&render_lines(&s, 42), "contenido oculto · posible secreto");
        let mut g = s.git.clone(); g.files.retain(|f| f.path != ".env.local"); apply(&mut s, Event::Git(g)); assert_eq!(s.ui.git_file, None);
    }
    #[test]
    fn clicking_a_branch_inspects_its_diff() {
        let mut s = git_sample(); s.ui.git_view = GitView::Branches;
        let f = frame(&s, 42, 0); let (x, y) = at(&f, "⎇ feat/x"); assert_eq!(click(&f, x + 3, y), None, "the current branch is not a diff target");
        let (x, y) = at(&f, "● main"); let a = click(&f, x + 3, y).unwrap(); assert_eq!(a, Action::Branch(Some("main".into())));
        apply(&mut s, Event::Ui(a)); let l = render_lines(&s, 42);
        assert!(find(&l, "▶ main").ends_with("diff ║")); assert!(find(&l, "📁 Diff · main").contains("[ ✕ ] [-] ═╗")); find(&l, "cargando…");
        let fc = FileChange { path: "src/b.rs".into(), kind: FileKind::Added, added: 4, deleted: 0 };
        apply(&mut s, Event::Detail(Detail { branch: Some("main".into()), files: vec![fc], added: 4, ..Detail::default() }));
        let l = render_lines(&s, 42); assert!(find(&l, "● 1 archivo").contains("+4  −0")); assert!(find(&l, "A  src/b.rs").ends_with("[diff] +4 ║"));
        assert!(!l.iter().any(|l| l.contains("src/a.rs")), "the branch diff replaces the working tree");
        // Clicking a file of the branch diff selects it; ✕ closes the inspector and its file.
        let f = frame(&s, 42, 0); let (x, y) = at(&f, "src/b.rs"); apply(&mut s, Event::Ui(click(&f, x, y).unwrap())); assert_eq!(s.ui.git_file.as_deref(), Some("src/b.rs"));
        let f = frame(&s, 42, 0); let (x, y) = at(&f, "✕"); assert_eq!(click(&f, x, y), Some(Action::Branch(None)));
        apply(&mut s, Event::Ui(Action::Branch(None))); assert_eq!((s.ui.git_branch.clone(), s.ui.git_file.clone()), (None, None));
        find(&render_lines(&s, 42), "📁 Working Tree");
        apply(&mut s, Event::Ui(Action::Branch(Some("main".into())))); apply(&mut s, Event::Ui(Action::Branch(Some("main".into())))); assert_eq!(s.ui.git_branch, None, "same branch toggles");
        s.detail.files.clear(); s.ui.git_branch = Some("main".into()); s.detail.branch = Some("main".into()); find(&render_lines(&s, 42), "✔ Sin diferencias con HEAD");
    }
    #[test]
    fn clicking_tabs_and_wheel_on_the_bar() {
        let mut s = sample(); let f = frame(&s, 60, 40);
        let (x, y) = at(&f, "MEM"); assert_eq!(y, f.tab_row);
        let a = click(&f, x + 1, y).expect("tab hit"); assert_eq!(a, Action::Tab(Tab::Mem));
        assert!(apply(&mut s, Event::Ui(a))); assert_eq!(s.ui.tab, Tab::Mem);
        let l = render_lines(&s, 60); assert!(l[4].contains("[ MEM ]")); find(&l, "🧠 Engram: repo");
        // The gear tab opens CONFIG, also at the 37-col pane width.
        let f = frame(&s, 37, 40); let bar = plain(&f.lines[f.tab_row]); let x = bar[..bar.find('⛭').unwrap()].width();
        assert_eq!(click(&f, x, f.tab_row), Some(Action::Tab(Tab::Config)));
        assert_eq!(click(&f, 0, 0), None);
        let f = frame(&s, 60, 40);
        apply(&mut s, Event::Ui(wheel(&f, f.tab_row, true).unwrap())); assert_eq!(s.ui.tab, Tab::Tree);
        apply(&mut s, Event::Ui(Action::Cycle(-1))); apply(&mut s, Event::Ui(Action::Cycle(-1))); assert_eq!(s.ui.tab, Tab::Prof);
        apply(&mut s, Event::Ui(Action::Tab(Tab::Main))); apply(&mut s, Event::Ui(Action::Cycle(-1))); assert_eq!(s.ui.tab, Tab::Config, "wraps");
        for (c, t) in [('2', Tab::Git), ('5', Tab::Tree), ('6', Tab::Config), ('1', Tab::Main)] { apply(&mut s, Event::Key(Key::Char(c))); assert_eq!(s.ui.tab, t); }
        apply(&mut s, Event::Key(Key::Char('r'))); assert!(s.refresh); apply(&mut s, Event::Key(Key::Char('q'))); assert!(s.quit);
    }
    #[test]
    fn dash_collapses_and_restores_a_card() {
        let mut s = sample(); let f = frame(&s, 42, 0);
        let (x, y) = at(&f, "[-]"); let a = click(&f, x + 1, y).unwrap(); assert_eq!(a, Action::Collapse("status".into()));
        apply(&mut s, Event::Ui(a)); assert!(s.ui.save);
        let l = render_lines(&s, 42); assert!(find(&l, "⌁ Status").ends_with("[+] ═╗")); assert!(!l.iter().any(|l| l.contains("Project")));
        let i = l.iter().position(|l| l.contains("⌁ Status")).unwrap(); assert!(l[i + 1].trim_start().starts_with('╚'));
        find(&l, "⌁ Context");
        let f = frame(&s, 42, 0); let (x, y) = at(&f, "[+]"); apply(&mut s, Event::Ui(click(&f, x + 1, y).unwrap()));
        find(&render_lines(&s, 42), "Project");
        assert_eq!(s.ui.config.collapsed, Vec::<String>::new());
    }
    #[test]
    fn body_scrolls_with_the_wheel_and_clamps() {
        let mut s = sample(); s.ui.tab = Tab::Config;
        let full = frame(&s, 42, 0).lines.len(); let f = frame(&s, 42, 20);
        assert_eq!(f.lines.len(), 20); assert_eq!(f.max_scroll, full - 20); assert!(plain(&f.lines[5]).ends_with(" ↓"));
        let mut n = 0; while let Some(a) = wheel(&frame(&s, 42, 20), 15, true) { apply(&mut s, Event::Ui(a)); n += 1; assert!(n < 50); }
        let f = frame(&s, 42, 20); assert_eq!(f.scroll, f.max_scroll); assert!(plain(&f.lines[5]).ends_with(" ↑"));
        assert!(plain(f.lines.last().unwrap()).trim_start().starts_with('╚'));
        // Hits follow the scrolled rows.
        let (x, y) = at(&f, "salir"); assert!(y < 20); let _ = x;
        apply(&mut s, Event::Ui(wheel(&f, 15, false).unwrap())); assert_eq!(s.ui.scroll[5], f.max_scroll - 3);
        // Other tabs keep their own offset.
        apply(&mut s, Event::Key(Key::Char('1'))); assert_eq!(frame(&s, 42, 20).scroll, 0);
    }
    #[test]
    fn config_hides_tabs_and_sets_the_name() {
        let mut s = sample(); apply(&mut s, Event::Key(Key::Char('6')));
        let f = frame(&s, 42, 0); let (x, y) = at(&f, "[✓] TREE"); let a = click(&f, x, y).unwrap();
        assert_eq!(a, Action::Hide(Tab::Tree)); apply(&mut s, Event::Ui(a)); assert!(s.ui.save);
        let l = render_lines(&s, 42); assert!(!l[4].contains("TREE")); assert!(find(&l, "[ ] TREE").contains("oculta"));
        apply(&mut s, Event::Key(Key::Char('5'))); assert_eq!(s.ui.tab, Tab::Config, "hidden tab keys are ignored");
        assert!(find(&l, "[✓] ⛭ CONFIG").contains("fija"));
        let f = frame(&s, 42, 0); let (x, y) = at(&f, "⛭ CONFIG"); assert_eq!(click(&f, x, y + 0).filter(|a| matches!(a, Action::Hide(_))), None);
        // Name: click "editar", type, backspace, Enter; `q` is text while editing.
        s.ui.save = false; let f = frame(&s, 42, 0); let (x, y) = at(&f, "✎ editar"); apply(&mut s, Event::Ui(click(&f, x, y).unwrap()));
        for k in "Ramonsq".chars().map(Key::Char).chain([Key::Back, Key::Char('D'), Key::Char('e'), Key::Char('v')]) { apply(&mut s, Event::Key(k)); }
        assert!(!s.quit); find(&render_lines(&s, 42), "> RamonsDev▏");
        apply(&mut s, Event::Key(Key::Enter)); assert_eq!(s.ui.config.name, "RamonsDev"); assert!(s.ui.save);
        assert!(render_lines(&s, 42)[1].contains("⌁ RamonsDev ⌁"));
        // Esc cancels; an empty name restores the placeholder.
        apply(&mut s, Event::Ui(Action::Edit)); apply(&mut s, Event::Key(Key::Char('x'))); apply(&mut s, Event::Key(Key::Esc)); assert_eq!(s.ui.config.name, "RamonsDev");
        apply(&mut s, Event::Ui(Action::Edit)); for _ in 0..30 { apply(&mut s, Event::Key(Key::Back)); } apply(&mut s, Event::Key(Key::Enter));
        assert!(render_lines(&s, 42)[1].contains("⌁ - - ⌁"));
        // Max length.
        apply(&mut s, Event::Ui(Action::Edit)); for _ in 0..40 { apply(&mut s, Event::Key(Key::Char('a'))); } apply(&mut s, Event::Key(Key::Enter));
        assert_eq!(s.ui.config.name.chars().count(), NAME_MAX);
        // Hiding every tab still leaves CONFIG; a config that hides MAIN starts elsewhere.
        let ui = Ui::new(Config { hidden: vec![Tab::Main, Tab::Git], ..Config::default() }, None); assert_eq!(ui.tab, Tab::Prof);
    }
}
