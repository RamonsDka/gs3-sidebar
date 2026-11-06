mod config;
mod core;
mod sources;
mod ui;
use anyhow::{anyhow, Result};
use core::*;
use crossterm::{event::{self, Event as Input, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind}, execute, style::Print, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};
use ratatui::{backend::CrosstermBackend, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::Paragraph, Terminal};
use std::{io, path::{Path, PathBuf}, sync::mpsc, thread};

const HELP:&str="gs3-hud --pane <HERDR_PANE_ID> [--socket <path>] [--cwd <dir>] [--config <file>] [--once]\n\nRead-only Pi telemetry rail (tabs MAIN GIT PROF MEM TREE and ⛭ = CONFIG).\nKeys: 1-6 switch tab (in that order), r refresh, q / Ctrl-C quit (in the CONFIG name editor: type, Backspace, Enter saves, Esc cancels).\nMouse: click a tab, a card's [-]/[+] to collapse or expand it, CONFIG rows to toggle tabs or edit the name;\nGIT: [ Grafo | Ramas ] switches view, a branch shows its diff against HEAD ([ ✕ ] closes), a file previews its diff inline (read-only git diff);\nMAIN: pixel icons mic / headphones toggle Vesktop mute/deafen (red active, green inactive, white offline; official Discord is not supported); the gear opens or focuses Vesktop at once; the small flame at the right end opens the Flameshot area selector (explicit click, ≥1 s apart); an explicit voice click may request one-time consent and save a private 0600 token in ~/.config/gs3/discord-token.json (never displayed). No voice sockets in --once.\nMAIN: click any model/effort row (or Status ▸) to focus the bound gs2 pane and send fixed F9 for its model picker; repeated clicks within 1 s are ignored, failures are silent;\nTREE: a folder opens/closes, [ ⊟ ] folds all, a changed file opens its diff in GIT, another file is selected (names only, symlinks never followed);\nwheel over the tab bar cycles tabs, elsewhere scrolls.\n--config is the HUD's own settings file {name, hidden, collapsed}: written atomically (in addition to the private Discord token); without it changes last until exit.\n--socket defaults to HERDR_SOCKET_PATH or ~/.config/herdr/herdr.sock\n--cwd defaults to the current directory\n--once prints a plain-text snapshot of every visible tab at width 42 (no terminal required)\nPR refresh cycles (view + open count) are spaced by at least 30 seconds.";
struct Args {pane:String,socket:PathBuf,cwd:PathBuf,config:Option<PathBuf>,once:bool}
fn args()->Result<Option<Args>> {
    let mut a=std::env::args().skip(1); let mut pane=None;
    let mut socket=std::env::var_os("HERDR_SOCKET_PATH").map(PathBuf::from).unwrap_or_else(||dirs::home_dir().unwrap_or_default().join(".config/herdr/herdr.sock"));
    let mut cwd=std::env::current_dir()?; let mut once=false; let mut config=None;
    while let Some(key)=a.next() {
        match key.as_str() {
            "--help"|"-h"=>{println!("{HELP}");return Ok(None)}
            "--once"=>once=true,
            "--pane"|"--socket"|"--cwd"|"--config"=>{let v=a.next().ok_or_else(||anyhow!("{key} requires a value"))?;match key.as_str(){"--pane"=>pane=Some(v),"--socket"=>socket=v.into(),"--config"=>config=Some(PathBuf::from(v)),_=>cwd=PathBuf::from(v).canonicalize()?}}
            _=>return Err(anyhow!("unknown option: {key}\n{HELP}"))
        }
    }
    Ok(Some(Args{pane:pane.filter(|s|!s.is_empty()).ok_or_else(||anyhow!("--pane is required\n{HELP}"))?,socket,cwd,config,once}))
}
// Button-only mouse reporting (1000) in SGR form (1006): clicks and wheel, never motion (1003).
const MOUSE_ON:&str="\x1b[?1000h\x1b[?1006h"; const MOUSE_OFF:&str="\x1b[?1006l\x1b[?1000l";
fn restore() {let _=disable_raw_mode(); let _=execute!(io::stdout(),Print(MOUSE_OFF),LeaveAlternateScreen);}
struct Guard;
impl Drop for Guard {fn drop(&mut self){restore();}}
enum Message {Data(Event),Input(Input)}
fn main()->Result<()> {
    let Some(a)=args()? else {return Ok(())};
    let mut state=State::new(a.pane.clone(),a.cwd.clone());
    state.ui=ui::Ui::new(a.config.as_deref().map(config::load).unwrap_or_default(),a.config.clone());
    if a.once {
        match sources::pane(&a.socket,&a.pane) {
            Ok(Some(p))=>{apply(&mut state,Event::Connection(true));apply(&mut state,Event::Pane(p));}
            Ok(None)=>return Ok(()),
            Err(e)=>eprintln!("herdr: {e}"),
        }
        if let Some(path)=&state.session_path {let mut tail=Tail::default(); if let Err(e)=sources::read_tail(&PathBuf::from(path),&mut tail) {eprintln!("session: {e}");} state.session=tail.session;}
        state.git=sources::git_snapshot(&a.cwd); state.engram=sources::engram(&a.cwd); state.pr=sources::pr_once(&a.cwd);
        state.mcp=state.session_path.as_deref().and_then(|p|sources::mcp(Path::new(p)));
        state.roster=state.session_path.as_deref().and_then(|p|sources::roster_of(Path::new(p))); state.stack=sources::stack(&a.cwd); state.tree=Some(sources::tree(&a.cwd,Vec::new()));
        for tab in state.ui.tabs() {state.ui.tab=tab; for line in ui::render_lines(&state,42) {println!("{line}");}} return Ok(())
    }
    let previous=std::panic::take_hook(); std::panic::set_hook(Box::new(move|info|{restore();previous(info);}));
    let _guard=Guard;
    enable_raw_mode()?; execute!(io::stdout(),EnterAlternateScreen,Print(MOUSE_ON))?;
    let mut terminal=Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let (tx,rx)=mpsc::channel(); let input_tx=tx.clone();
    thread::spawn(move||{while let Ok(e)=event::read() {if input_tx.send(Message::Input(e)).is_err(){break}}});
    let (data_tx,data_rx)=mpsc::channel(); let bridge=tx.clone();
    thread::spawn(move||{for e in data_rx {if bridge.send(Message::Data(e)).is_err(){break}}});
    let local=sources::local(a.cwd.clone(),a.socket.clone(),a.pane.clone(),data_tx.clone()); let details=sources::details(a.cwd.clone(),data_tx.clone()); let pr=sources::prs(a.cwd.clone(),data_tx.clone());
    // GIT inspector: ask for a branch/file diff when the selection changes, and again when the repo changes under it.
    let request=|s:&State|{let u=&s.ui; (u.git_branch.is_some()||u.git_file.is_some()).then(||sources::DetailRequest{branch:u.git_branch.clone(),file:u.git_file.clone(),
        untracked:u.git_branch.is_none() && s.git.files.iter().any(|f|Some(&f.path)==u.git_file.as_ref() && f.kind==FileKind::Untracked)})};
    let mut asked:Option<sources::DetailRequest>=None;
    // TREE: scan only while the tab is shown — again when folders open/close, on `r`, and when the repo changes.
    let trees=sources::trees(a.cwd.clone(),data_tx.clone()); let mut scanned:Option<Vec<String>>=None;
    // Stack · Ecosistema versions: probed once, off the UI thread.
    {let (cwd,tx)=(a.cwd.clone(),data_tx.clone()); thread::spawn(move||{let _=tx.send(Event::Stack(sources::stack(&cwd)));});}
    let picker=sources::model_picker(a.socket.clone(),a.pane.clone());
    let voice=sources::voice(data_tx.clone());
    let capture=sources::screenshot();
    sources::herdr(a.socket,a.pane,data_tx);
    let mut draw=true; let mut last:Option<ui::Frame>=None;
    loop {
        if let Some(cmd)=state.ui.voice_cmd.take() {let _=voice.send(cmd);}
        if state.ui.screenshot {state.ui.screenshot=false; let _=capture.send(());}
        if state.ui.open_picker {state.ui.open_picker=false; let _=picker.send(std::time::Instant::now());}
        if state.ui.save {state.ui.save=false; if let Some(p)=state.ui.config_path.clone() {let err=config::save(&p,&state.ui.config).err(); if err!=state.ui.save_error {state.ui.save_error=err; draw=true;}}}
        if state.read_session {state.read_session=false; let _=local.send(sources::Local::Session(state.session_path.clone()));}
        if state.refresh {state.refresh=false; scanned=None; let _=local.send(sources::Local::Refresh); let _=pr.send(());}
        // Leaving the tab forgets the scan, so coming back shows the tree as it is now.
        if state.ui.tab!=ui::Tab::Tree {scanned=None;}
        else if scanned.as_ref()!=Some(&state.ui.tree_open) {let _=trees.send(state.ui.tree_open.clone()); scanned=Some(state.ui.tree_open.clone());}
        if state.quit {break}
        if draw {terminal.draw(|f|{
            let area=f.area(); let frame=ui::frame(&state,area.width,area.height);
            let lines:Vec<Line>=frame.lines.iter().map(|l|Line::from(l.iter().map(|g|{let (r,gr,b)=g.tone.rgb(); let st=Style::default().fg(Color::Rgb(r,gr,b)); Span::styled(g.text.clone(),if g.bold {st.add_modifier(Modifier::BOLD)} else {st})}).collect::<Vec<_>>())).collect();
            f.render_widget(Paragraph::new(lines),area); last=Some(frame);
        })?;}
        draw=false;
        match rx.recv()? {
            Message::Input(Input::Resize(..))=>draw=true,
            Message::Input(Input::Key(k)) if k.kind!=KeyEventKind::Release => {
                let key=match k.code {
                    KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL)=>break,
                    KeyCode::Char(c) if !k.modifiers.intersects(KeyModifiers::CONTROL|KeyModifiers::ALT)=>Some(ui::Key::Char(c)),
                    KeyCode::Backspace=>Some(ui::Key::Back), KeyCode::Enter=>Some(ui::Key::Enter), KeyCode::Esc=>Some(ui::Key::Esc),
                    _=>None
                };
                if let Some(key)=key {draw=apply(&mut state,Event::Key(key));}
            }
            Message::Input(Input::Mouse(m))=>{
                let (col,row)=(usize::from(m.column),usize::from(m.row));
                let action=last.as_ref().and_then(|f|match m.kind {
                    MouseEventKind::Down(MouseButton::Left)=>ui::click(f,col,row),
                    MouseEventKind::ScrollDown=>ui::wheel(f,row,true), MouseEventKind::ScrollUp=>ui::wheel(f,row,false),
                    _=>None
                });
                if let Some(a)=action {draw=apply(&mut state,Event::Ui(a));}
            }
            Message::Data(e)=>{
                // A branch or commit change means HEAD moved; dirty-only updates don't fetch PRs.
                let git=matches!(e,Event::Git(_));
                if let Event::Git(g)=&e {if head_moved(&state.git,g) {let _=pr.send(());}}
                draw=apply(&mut state,e);
                if git && draw {asked=None; scanned=None;}
            }
            _=>{}
        }
        let want=request(&state); if want!=asked {if let Some(r)=&want {let _=details.send(r.clone());} asked=want;}
    }
    Ok(())
}
