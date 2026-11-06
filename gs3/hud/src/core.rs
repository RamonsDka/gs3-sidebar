use serde_json::Value;
use std::path::{Path, PathBuf};
use crate::ui::{self, Ui};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    pub input: u64, pub output: u64, pub read: u64, pub write: u64,
    pub used: u64, pub cost: f64, pub model: String, pub provider: String,
    pub thinking: String, pub limit: Option<u64>,
    /// Tool calls made by the assistant: total, `read`, `edit`/`write`, `mem_*`, and the other CUTE pill kinds.
    pub tools: u64, pub reads: u64, pub writes: u64, pub mem: u64,
    pub bash: u64, pub grep: u64, pub fetch: u64, pub search: u64, pub other: u64,
    /// Latest `todo` tool checklist (`details.gentleTodo.tasks` of its toolResult).
    pub todos: Vec<Todo>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TodoStatus { #[default] Pending, InProgress, Done }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Todo { pub title: String, pub status: TodoStatus, pub note: Option<String> }
/// Same normalization as the reference `normalizeTask`: non-empty title required, unknown status → pending.
fn todos(tasks: &[Value]) -> Vec<Todo> {
    tasks.iter().filter_map(|t| {
        let title = t["title"].as_str().filter(|s| !s.is_empty())?;
        let status = match text(t, "status") { "done" => TodoStatus::Done, "in_progress" => TodoStatus::InProgress, _ => TodoStatus::Pending };
        Some(Todo { title: title.into(), status, note: t["note"].as_str().map(Into::into) })
    }).collect()
}
#[derive(Default)]
pub struct Tail { pub session: Session, pending: Vec<u8>, pub offset: u64, pub limit_model: Option<String> }
impl Tail {
    pub fn ingest(&mut self, bytes: &[u8], reset: bool) {
        if reset { *self=Self::default(); }
        self.offset+=bytes.len() as u64;
        self.pending.extend_from_slice(bytes);
        let end=self.pending.iter().rposition(|b| *b==b'\n').map(|i|i+1).unwrap_or(0);
        let complete: Vec<u8>=self.pending.drain(..end).collect();
        for line in complete.split(|b|*b==b'\n') {
            let Ok(v)=serde_json::from_slice::<Value>(line) else {continue};
            let s=&mut self.session;
            match text(&v,"type") {
                "model_change" => { s.model=text(&v,"modelId").into(); s.provider=text(&v,"provider").into(); }
                "thinking_level_change" => s.thinking=text(&v,"thinkingLevel").into(),
                "message" if text(&v["message"],"role")=="toolResult" && text(&v["message"],"toolName")=="todo" => {
                    if let Some(t)=v["message"]["details"]["gentleTodo"]["tasks"].as_array() {s.todos=todos(t);}
                }
                "message" if text(&v["message"],"role")=="assistant" => {
                    let m=&v["message"]; let u=&m["usage"];
                    for (key,dst) in [("input",&mut s.input),("output",&mut s.output),("cacheRead",&mut s.read),("cacheWrite",&mut s.write)] { *dst+=u[key].as_u64().unwrap_or(0); }
                    if let Some(n)=u["totalTokens"].as_u64() {s.used=n;}
                    s.cost+=u["cost"]["total"].as_f64().unwrap_or(0.0);
                    if let Some(m)=m["model"].as_str() {s.model=m.into();}
                    if let Some(p)=m["provider"].as_str() {s.provider=p.into();}
                    for b in m["content"].as_array().into_iter().flatten().filter(|b|text(b,"type")=="toolCall") {
                        // Same buckets as the reference `recordToolCall`.
                        let name=text(b,"name").trim().to_lowercase(); if name.is_empty() {continue}
                        s.tools+=1;
                        match name.as_str() {"read"=>s.reads+=1,"edit"|"write"=>s.writes+=1,"bash"=>s.bash+=1,n if n.starts_with("mem_")=>s.mem+=1,
                            "grep"=>s.grep+=1,"fetch"|"fetch_content"=>s.fetch+=1,"search"|"web_search"=>s.search+=1,_=>s.other+=1}
                    }
                }
                _=>{}
            }
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Git {
    pub branch:String, pub dirty:Option<usize>, pub ahead_behind:Option<(u64,u64)>, pub commits:Vec<String>, pub error:Option<String>,
    /// `log --graph --oneline --decorate` lines (GIT tab graph view).
    pub graph:Vec<String>,
    /// Working tree (`status --porcelain=v1 -z`) with `diff HEAD --numstat` line counts and totals.
    pub files:Vec<FileChange>, pub counts:Counts, pub added:u64, pub deleted:u64,
    pub branches:Vec<Branch>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileKind { #[default] Modified, Added, Deleted, Untracked, Conflict }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileChange { pub path:String, pub kind:FileKind, pub added:u64, pub deleted:u64 }
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counts { pub staged:usize, pub modified:usize, pub untracked:usize, pub conflicts:usize }
impl Counts { pub fn clean(&self)->bool {*self==Counts::default()} }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Branch { pub name:String, pub current:bool, pub remote:bool, pub ahead:u64, pub behind:u64, pub gone:bool, pub date:String }
/// Branch or file the GIT tab is inspecting: the branch's changed files and/or one file's diff.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Detail { pub branch:Option<String>, pub file:Option<String>, pub files:Vec<FileChange>, pub added:u64, pub deleted:u64, pub diff:Vec<String>, pub more:usize, pub error:Option<String> }
/// Files whose content may hold credentials: their diff is never fetched nor shown.
pub fn sensitive(path:&str)->bool {
    let name=path.rsplit('/').next().unwrap_or(path).to_lowercase();
    name.starts_with(".env") || ["secret","credential","token","passw","auth",".pem",".key",".p12",".pfx","id_rsa","id_ed25519",".netrc",".npmrc"].iter().any(|k|name.contains(k))
}
/// `status --porcelain=v1 -z` → files and the reference's counters (`parseGitStatusPorcelain`).
pub fn parse_status(out:&str)->(Vec<FileChange>,Counts) {
    let mut files=Vec::new(); let mut c=Counts::default(); let mut it=out.split('\0');
    while let Some(e)=it.next() {
        let (Some(code),Some(path))=(e.get(..2),e.get(3..)) else {continue};
        let (x,y)=(code.as_bytes()[0],code.as_bytes()[1]);
        if matches!(x,b'R'|b'C') {it.next();}
        let kind=if code=="??" {c.untracked+=1; FileKind::Untracked}
            else if x==b'U'||y==b'U'||code=="AA"||code=="DD" {c.conflicts+=1; FileKind::Conflict}
            else {
                if x!=b' ' {c.staged+=1;} if y!=b' ' {c.modified+=1;}
                if x==b'A'||y==b'A' {FileKind::Added} else if x==b'D'||y==b'D' {FileKind::Deleted} else {FileKind::Modified}
            };
        files.push(FileChange{path:path.into(),kind,..FileChange::default()});
    }
    (files,c)
}
/// `diff --numstat -z` (`added\tdeleted\tpath`; binary files are `-`) → per-file counts and totals.
pub fn apply_numstat(files:&mut [FileChange],out:&str)->(u64,u64) {
    let (mut a,mut d)=(0,0);
    for r in out.split('\0') {
        let mut p=r.splitn(3,'\t'); let (Some(x),Some(y),Some(path))=(p.next(),p.next(),p.next()) else {continue};
        let (x,y)=(x.parse().unwrap_or(0),y.parse().unwrap_or(0)); a+=x; d+=y;
        if let Some(f)=files.iter_mut().find(|f|f.path==path) {f.added=x; f.deleted=y;}
    }
    (a,d)
}
/// `diff --name-status -z` → files of a branch diff.
pub fn parse_name_status(out:&str)->Vec<FileChange> {
    let mut files=Vec::new(); let mut it=out.split('\0').filter(|s|!s.is_empty());
    while let Some(code)=it.next() {
        if matches!(code.as_bytes()[0],b'R'|b'C') {it.next();}
        let Some(path)=it.next() else {break};
        let kind=match code.as_bytes()[0] {b'A'=>FileKind::Added,b'D'=>FileKind::Deleted,b'U'=>FileKind::Conflict,_=>FileKind::Modified};
        files.push(FileChange{path:path.into(),kind,..FileChange::default()});
    }
    files
}
/// `3 days ago` → `3d` (first unit only: `1 year, 2 months ago` → `1y`).
fn short_age(s:&str)->String {
    let mut w=s.split_whitespace(); let (Some(n),Some(u))=(w.next(),w.next()) else {return s.into()};
    let u=[("year","y"),("month","mo"),("week","w"),("day","d"),("hour","h"),("minute","m"),("second","s")].iter().find(|(k,_)|u.starts_with(k)).map(|(_,v)|*v);
    u.map_or_else(||s.into(),|u|format!("{n}{u}"))
}
/// `for-each-ref refs/heads refs/remotes` (refname, short, upstream, track, relative date; tab separated)
/// → current branch first, other locals, then remote-only branches (`fetchGitBranches`).
pub fn parse_branches(out:&str,current:&str)->Vec<Branch> {
    let rows:Vec<Vec<&str>>=out.lines().map(|l|l.split('\t').collect()).filter(|p:&Vec<&str>|p.len()>=2).collect();
    let local:Vec<&str>=rows.iter().filter(|p|p[0].starts_with("refs/heads/")).map(|p|p[1]).collect();
    let field=|p:&Vec<&str>,i:usize|p.get(i).copied().unwrap_or("").trim().to_string();
    let num=|t:&str,k:&str|t.split_once(k).and_then(|(_,r)|r.trim_start().split(|c:char|!c.is_ascii_digit()).next()?.parse().ok()).unwrap_or(0);
    let (mut locals,mut remotes)=(Vec::new(),Vec::new());
    for p in &rows {
        let (full,short)=(p[0].trim(),p[1].trim());
        if short.is_empty()||short=="origin"||full.ends_with("/HEAD") {continue}
        let date=short_age(&field(p,4));
        if full.starts_with("refs/heads/") {
            let t=field(p,3);
            locals.push(Branch{name:short.into(),current:short==current,remote:false,ahead:num(&t,"ahead"),behind:num(&t,"behind"),gone:t.contains("gone"),date});
        } else if !local.contains(&short.split_once('/').map_or(short,|(_,b)|b)) && !local.contains(&short) {
            remotes.push(Branch{name:short.into(),remote:true,date,..Branch::default()});
        }
    }
    locals.sort_by_key(|b|!b.current); locals.extend(remotes); locals
}
/// One `log --graph --oneline --decorate` commit line → (graph prefix, hash, refs, subject).
pub fn split_graph(line:&str)->Option<(&str,&str,Option<&str>,&str)> {
    let end=line.find(|c:char|!"*\\/|_ ".contains(c))?; let graph=&line[..end];
    if !graph.contains('*') {return None}
    let (hash,rest)=line[end..].split_once(' ').unwrap_or((&line[end..],""));
    if hash.len()<7 || !hash.chars().all(|c|c.is_ascii_hexdigit()) {return None}
    match rest.strip_prefix('(').and_then(|r|r.find(')').map(|i|(&r[..i],r[i+1..].trim_start()))) {
        Some((refs,subject))=>Some((graph,hash,Some(refs),subject)), None=>Some((graph,hash,None,rest))
    }
}
/// `hash subject` of every commit in the graph (what `head_moved` compares).
pub fn graph_commits(out:&str)->Vec<String> {
    out.lines().filter_map(split_graph).map(|(_,h,_,s)|if s.is_empty() {h.to_string()} else {format!("{h} {s}")}).collect()
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Engram {
    pub project:String, pub count:Option<i64>, pub error:Option<String>,
    /// Local daemon (`127.0.0.1:7437/health` says `status: ok`).
    pub online:bool,
    /// Project listed in `sync_enrolled_projects`; `sync` is the daemon's `/sync/status` (phase, last sync epoch).
    pub enrolled:bool, pub sync:Option<(String,Option<i64>)>,
    /// Latest `session_summary` observation of the project.
    pub handoff:Option<Handoff>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Handoff { pub id:i64, pub kind:String, pub at:Option<i64>, pub lines:Vec<String> }
/// Bounded handoff body: first lines only, each capped (the card shows even fewer).
pub fn handoff_lines(content:&str)->Vec<String> {
    content.lines().map(str::trim).filter(|l|!l.is_empty()).take(40).map(|l|l.chars().take(300).collect()).collect()
}
/// Reference `formatRelativeTime`.
pub fn ago(at:Option<i64>,now:i64)->String {
    let Some(at)=at else {return "nunca".into()}; let s=(now-at).max(0);
    if s<10 {"recién".into()} else if s<60 {format!("hace {s}s")} else if s<3600 {format!("hace {}m",s/60)} else if s<86_400 {format!("hace {}h",s/3600)} else {format!("hace {}d",s/86_400)}
}
pub fn now_secs()->i64 {std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d|d.as_secs() as i64).unwrap_or(0)}
/// One Stack · Ecosistema entry: installed version or None.
#[derive(Clone, Debug, PartialEq)]
pub struct Tool { pub name:&'static str, pub version:Option<String> }
/// Reference `cleanVersionString`: the first `x.y.z[-tag]`, else the last token without `v`.
pub fn clean_version(raw:&str)->Option<String> {
    let line=raw.trim().lines().next()?.trim(); if line.is_empty() {return None}
    for t in line.split_whitespace() {
        let t=t.trim_start_matches(['v','V']);
        let v:String=t.chars().take_while(|c|c.is_ascii_alphanumeric()||matches!(c,'.'|'-'|'_')).collect();
        let mut parts=v.splitn(3,'.');
        let num=|p:Option<&str>|p.is_some_and(|p|p.chars().next().is_some_and(|c|c.is_ascii_digit()));
        if num(parts.next()) && num(parts.next()) && num(parts.next()) {return Some(v.trim_end_matches(['.','-','_']).into())}
    }
    line.split_whitespace().last().map(|t|t.trim_start_matches(['v','V']).into())
}
/// TREE tab: one scan of the project root with only the `open` directories expanded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tree { pub project:String, pub open:Vec<String>, pub nodes:Vec<TreeNode>, pub error:Option<String> }
/// `rel` is relative to the git root (same as porcelain paths); `count` = visible entries of a directory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeNode { pub name:String, pub rel:String, pub dir:bool, pub count:usize, pub children:Vec<TreeNode> }
/// PROF roster: host defaults (`settings.json`), subagents (`agents/*.md`) and their task history.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Roster { pub model:String, pub thinking:String, pub agents:Vec<Agent>, pub total:usize, pub done:usize, pub active:usize, pub current:Option<String> }
/// One subagent: model/thinking from its frontmatter (`model`, `effort`/`thinking`), else its latest task, else the host.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Agent { pub name:String, pub model:String, pub thinking:String }
fn frontmatter<'a>(md:&'a str,key:&str)->Option<&'a str> {
    let body=md.strip_prefix("---")?; let end=body.find("\n---").unwrap_or(body.len());
    body[..end].lines().find_map(|l|l.strip_prefix(key)?.strip_prefix(':').map(|v|v.trim().trim_matches(['"','\''])).filter(|v|!v.is_empty()))
}
/// `agents` = (file stem, markdown); `tasks` = the `task` objects of `gentle-agents/tasks/*.json`. Names and models only.
pub fn roster(settings:&Value,agents:&[(String,String)],tasks:&[Value])->Roster {
    let model=match (text(settings,"defaultProvider"),text(settings,"defaultModel")) {(_,"")=>String::new(),("",m)=>m.into(),(p,m)=>format!("{p}/{m}")};
    let thinking=text(settings,"defaultThinkingLevel").to_string();
    let mut tasks:Vec<&Value>=tasks.iter().collect(); tasks.sort_by_key(|t|t["createdAt"].as_i64().unwrap_or(0));
    let mut list:Vec<Agent>=agents.iter().map(|(stem,md)|{
        let name=frontmatter(md,"name").unwrap_or(stem).to_string();
        let latest=tasks.iter().rev().find(|t|text(t,"agent")==name);
        let pick=|key:&str,task:&str,host:&str|frontmatter(md,key).map(str::to_owned).or_else(||latest.map(|t|text(t,task)).filter(|v|!v.is_empty()).map(str::to_owned)).unwrap_or_else(||host.into());
        Agent{model:pick("model","model",&model),thinking:frontmatter(md,"effort").map(str::to_owned).unwrap_or_else(||pick("thinking","thinking",&thinking)),name}
    }).collect();
    list.sort_by(|a,b|a.name.cmp(&b.name));
    let live=|t:&&&Value|matches!(text(t,"status"),"running"|"queued"|"pending"|"waiting"|"starting");
    let current=tasks.iter().rev().find(live).map(|t|format!("{} · {}",text(t,"agent"),text(t,"label")));
    Roster{model,thinking,agents:list,total:tasks.len(),done:tasks.iter().filter(|t|text(t,"status")=="completed").count(),active:tasks.iter().filter(live).count(),current}
}
/// One configured MCP server: only its name and transport, never its config values.
#[derive(Clone, Debug, PartialEq)]
pub struct Mcp { pub name:String, pub local:bool }
/// HEAD moved (branch switch or new/rewritten commits): worth a PR cycle. Dirty-only changes are not.
pub fn head_moved(old:&Git,new:&Git)->bool {old.branch!=new.branch || old.commits!=new.commits}
pub fn mcp_servers(v:&Value)->Vec<Mcp> {
    let mut out:Vec<Mcp>=v["mcpServers"].as_object().map(|o|o.iter().map(|(name,cfg)|Mcp{name:name.clone(),local:cfg.get("command").is_some()}).collect()).unwrap_or_default();
    out.sort_by(|a,b|a.name.cmp(&b.name)); out
}
/// Pi agent directory owning a session file (`<agent>/sessions/<cwd>/<id>.jsonl`).
pub fn agent_dir(session:&str)->Option<PathBuf> {session.split_once("/sessions/").map(|(a,_)|PathBuf::from(a))}
pub fn text<'a>(v:&'a Value,key:&str)->&'a str {v[key].as_str().unwrap_or("")}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Voice { pub online:bool, pub mute:bool, pub deaf:bool }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceCmd { Mute, Deaf, Open }
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub voice:Voice,
    pub pane:String, pub cwd:PathBuf, pub workspace:String, pub label:String, pub status:String,
    pub session_path:Option<String>, pub session:Session, pub connected:bool,
    pub git:Git, pub pr:Vec<String>, pub engram:Engram, pub mcp:Option<Vec<Mcp>>,
    /// PROF roster (None: no agent dir data) and Stack · Ecosistema versions (empty until probed).
    pub roster:Option<Roster>, pub stack:Vec<Tool>,
    /// TREE tab scan (None until the first answer).
    pub tree:Option<Tree>,
    /// GIT tab inspector (selected branch diff / file diff), fetched on demand.
    pub detail:Detail,
    pub quit:bool, pub refresh:bool, pub read_session:bool,
    /// Rail navigation and CONFIG (tabs, collapsed cards, scroll, name).
    pub ui:Ui,
}
impl State {
    pub fn new(pane:String,cwd:PathBuf)->Self {Self {voice:Voice::default(),pane,cwd,workspace:String::new(),label:String::new(),status:"unknown".into(),session_path:None,session:Session::default(),connected:false,git:Git::default(),pr:vec!["—".into()],engram:Engram::default(),mcp:None,roster:None,stack:Vec::new(),tree:None,detail:Detail::default(),quit:false,refresh:true,read_session:false,ui:Ui::default()}}
}
pub enum Event { Voice(Voice), Pane(Value), Status(String,String), Closed(String), Connection(bool), Session(Option<String>,Session), Git(Git), Pr(Vec<String>), Engram(Engram), Mcp(Option<Vec<Mcp>>), Roster(Option<Roster>), Stack(Vec<Tool>), Tree(Tree), Detail(Detail), Ui(ui::Action), Key(ui::Key) }
pub fn apply(s:&mut State,e:Event)->bool {
    let old=s.clone();
    match e {
        Event::Pane(v) if text(&v,"pane_id")==s.pane => {
            let ws=text(&v,"workspace_id"); if ws!=s.workspace {s.label.clear();} s.workspace=ws.into();
            // Only pane lookups carry the label; subscription events keep the last known one.
            if let Some(l)=v["workspace_label"].as_str().filter(|l|!l.is_empty()) {s.label=l.into();}
            let path=v["agent_session"]["value"].as_str().map(str::to_owned);
            if path!=s.session_path {s.session_path=path; s.session=Session::default(); s.read_session=true;}
            if let Some(status)=v["agent_status"].as_str() {apply(s,Event::Status(s.pane.clone(),status.into()));}
        }
        Event::Status(p,status) if p==s.pane => {if status!=s.status && matches!(status.as_str(),"idle"|"done") {s.refresh=true;} s.status=status;}
        Event::Closed(p) if p==s.pane => s.quit=true,
        Event::Voice(v)=>s.voice=v,
        Event::Connection(c)=>s.connected=c,
        Event::Session(path,v) if path==s.session_path=>s.session=v,
        Event::Git(v)=>{
            // A previewed working-tree file that is no longer changed drops its selection.
            if s.ui.git_branch.is_none() && s.ui.git_file.as_ref().is_some_and(|f|!v.files.iter().any(|x|&x.path==f)) {s.ui.git_file=None;}
            s.git=v;
        } Event::Pr(v)=>s.pr=v, Event::Engram(v)=>s.engram=v, Event::Mcp(v)=>s.mcp=v, Event::Roster(v)=>s.roster=v, Event::Stack(v)=>s.stack=v,
        // Only the answer for what is still selected is kept (clicks may outrun git).
        // Like Detail: only the scan of the current open set is kept.
        Event::Tree(t) if t.open==s.ui.tree_open => s.tree=Some(t),
        Event::Detail(d) if (&d.branch,&d.file)==(&s.ui.git_branch,&s.ui.git_file) => s.detail=d,
        Event::Ui(a)=>{
            let (branch,file)=(s.ui.git_branch.clone(),s.ui.git_file.clone()); ui::act(&mut s.ui,a);
            // A new selection never shows the previous answer.
            if s.ui.git_branch!=branch {s.detail=Detail::default();} else if s.ui.git_file!=file {s.detail.file=None; s.detail.diff.clear(); s.detail.more=0;}
        }
        Event::Key(k)=>ui::key(s,k),
        _=>{}
    }
    *s!=old
}
pub fn parse_wire(v:&Value)->(&str,Option<Event>) {
    if !v["error"].is_null() {return ("error",None)}
    match text(v,"event") {
        "pane_updated"=>("pane",Some(Event::Pane(v["data"]["pane"].clone()))),
        "pane_closed"=>("closed",Some(Event::Closed(text(&v["data"],"pane_id").into()))),
        // herdr emits this one with a dot (`pane.agent_status_changed`); accept both spellings.
        "pane_agent_status_changed"|"pane.agent_status_changed"=>("status",Some(Event::Status(text(&v["data"],"pane_id").into(),text(&v["data"],"agent_status").into()))),
        _ if text(&v["result"],"type")=="subscription_started" =>("started",None),
        _=>("other",None)
    }
}
pub fn context_window(v:&Value,id:&str)->Option<u64> {
    if v["id"].as_str()==Some(id) {if let Some(n)=v["contextWindow"].as_u64() {return Some(n)}}
    match v {Value::Object(o)=>o.values().find_map(|v|context_window(v,id)),Value::Array(a)=>a.iter().find_map(|v|context_window(v,id)),_=>None}
}
pub fn head_name(s:&str)->String {let s=s.trim(); if let Some(b)=s.strip_prefix("ref: refs/heads/") {b.into()} else {format!("detached {}",s.chars().take(8).collect::<String>())}}
pub fn gitdir_target(root:&Path,s:&str)->PathBuf {root.join(s.trim().strip_prefix("gitdir:").unwrap_or(s).trim())}
pub fn pr_delay(attempt:usize)->u64 {match attempt {0=>120,1=>300,_=>900}}
pub const PR_MIN_GAP:u64=30;
pub fn pr_due(last:Option<u64>,now:u64)->bool {last.is_none_or(|t|now.saturating_sub(t)>=PR_MIN_GAP)}
/// When a triggered PR cycle may run: now, unless the previous cycle was under PR_MIN_GAP ago.
pub fn pr_trigger_due(last:Option<u64>,now:u64)->u64 {last.map_or(now,|t|now.max(t+PR_MIN_GAP))}
pub fn truncate(s:&str,width:usize)->String {
    if s.width()<=width {return s.into()}
    // Grow by whole chars and re-measure the string: emoji sequences can be narrower than their chars.
    let mut out=String::new();
    for ch in s.chars() {out.push(ch); if out.width()>width {out.pop(); break}}
    out
}
/// Full path, with the home directory shortened to `~`.
pub fn display_path(p:&Path)->String {
    match dirs::home_dir().and_then(|h|p.strip_prefix(&h).ok().map(Path::to_path_buf)) {Some(r) if r.as_os_str().is_empty()=>"~".into(),Some(r)=>format!("~/{}",r.display()),None=>p.display().to_string()}
}
pub fn short_path(p:&Path,avail:usize)->String {
    let full=display_path(p); if full.width()<=avail {return full}
    format!("…/{}",p.file_name().unwrap_or_default().to_string_lossy())
}
pub fn fmt_tokens(n:u64)->String {
    let f=|v:f64,u:&str|if v>=100.0 || (u=="M" && v>=10.0) {format!("{}{u}",v.round())} else {format!("{v:.1}{u}")};
    if n>=1_000_000 {f(n as f64/1e6,"M")} else if n>=1000 {f(n as f64/1e3,"k")} else {n.to_string()}
}

/// CUTE palette (`themes/CinlodevCute.json` of pi-cinlodev-cute).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone { Border, BorderMuted, PinkBright, Pink, Gold, Yellow, Text, Muted, Dim, Mint, Orange, Coral, Cyan, Read, Salmon, Secondary, Blue, Green, Purple, VoiceRed, VoiceGreen }
impl Tone {
    pub fn rgb(self)->(u8,u8,u8) {
        match self {Tone::VoiceRed=>(0xf2,0x3f,0x43),Tone::VoiceGreen=>(0x23,0xa5,0x5a),Tone::Border=>(0x8e,0x44,0xad),Tone::BorderMuted=>(0x5c,0x2c,0x74),Tone::PinkBright=>(0xff,0xb1,0xdd),Tone::Pink=>(0xf0,0x95,0xc8),Tone::Gold=>(0xe0,0xc2,0x7a),
            Tone::Yellow=>(0xf2,0xb8,0x6d),Tone::Text=>(0xf6,0xef,0xf3),Tone::Muted=>(0xa7,0x8e,0x9b),Tone::Dim=>(0x76,0x61,0x6b),Tone::Mint=>(0xb4,0xe7,0xc7),Tone::Orange=>(0xe8,0x83,0x52),
            Tone::Coral=>(0xff,0x71,0x8f),Tone::Cyan=>(0xa5,0xcb,0xe8),Tone::Green=>(0x8e,0xc5,0xa6),Tone::Read=>(0xc5,0xa3,0xe0),Tone::Salmon=>(0xf0,0x97,0x8a),
            // secondary (remote refs) and syntaxFunction (graph edges, hunk headers).
            Tone::Secondary=>(0xd7,0xa0,0xb8),Tone::Blue=>(0xa9,0xc7,0xee),
            // Gear silhouette: a lighter shade of the frame purple (#8e44ad) so it reads on dark backgrounds.
            Tone::Purple=>(0xb5,0x7e,0xdc)}
    }
}
/// Context thresholds of the reference (`getContextThreshold`).
pub fn ctx_band(pct:f64)->(&'static str,Tone) {
    if pct>=80.0 {("● Crítico",Tone::Coral)} else if pct>=65.0 {("● Alerta",Tone::Orange)} else if pct>=40.0 {("● Medio",Tone::Gold)} else {("● Óptimo",Tone::Mint)}
}
pub fn fmt_cost(c:f64)->String {if c>=1.0 {format!("${c:.2}")} else {format!("${c:.3}")}}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn tail_aggregation_partial_and_truncation() {
        let line = b"{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"usage\":{\"input\":4,\"output\":8,\"cacheRead\":2,\"cacheWrite\":3,\"totalTokens\":17,\"cost\":{\"total\":0.5}}}}\n";
        let mut t = Tail::default();
        t.ingest(line, false); t.ingest(&line[..20], false);
        assert_eq!((t.session.input,t.session.output,t.session.read,t.session.write,t.session.used,t.session.cost),(4,8,2,3,17,0.5));
        t.ingest(&line[20..], false); assert_eq!(t.session.input,8);
        t.ingest(line,true); assert_eq!(t.session.input,4);
    }
    #[test]
    fn todo_tool_results_replace_the_task_list() {
        let todo=|tasks:&str,role:&str,tool:&str| format!("{{\"type\":\"message\",\"message\":{{\"role\":\"{role}\",\"toolName\":\"{tool}\",\"details\":{{\"gentleTodo\":{{\"tasks\":{tasks}}}}}}}}}\n");
        let mut t=Tail::default();
        t.ingest(todo(r#"[{"id":1,"title":"A","status":"done"},{"id":2,"title":"B","status":"in_progress","note":"n"},{"title":"C","status":"weird"},{"title":""},{"status":"done"},7]"#,"toolResult","todo").as_bytes(),false);
        assert_eq!(t.session.todos,vec![
            Todo{title:"A".into(),status:TodoStatus::Done,note:None},
            Todo{title:"B".into(),status:TodoStatus::InProgress,note:Some("n".into())},
            Todo{title:"C".into(),status:TodoStatus::Pending,note:None}]);
        // Other tools, other roles and a missing task array leave the list as it was; the latest list wins.
        for l in [todo(r#"[{"title":"X"}]"#,"toolResult","bash"),todo(r#"[{"title":"X"}]"#,"assistant","todo"),todo(r#"{}"#,"toolResult","todo")] {t.ingest(l.as_bytes(),false);}
        assert_eq!(t.session.todos.len(),3);
        t.ingest(todo("[]","toolResult","todo").as_bytes(),false); assert!(t.session.todos.is_empty());
    }
    #[test]
    fn git_porcelain_numstat_branches_and_graph() {
        // `status --porcelain=v1 -z --no-renames`: NUL-separated, never quoted.
        let (files,c)=parse_status("?? new.txt\0 M src/a.rs\0M  b.rs\0MM c.rs\0A  d.rs\0 D e.rs\0UU f.rs\0?? sp ace.txt\0");
        assert_eq!((c.staged,c.modified,c.untracked,c.conflicts),(3,3,2,1)); assert!(!c.clean()); assert!(Counts::default().clean());
        let kinds:Vec<(&str,FileKind)>=files.iter().map(|f|(f.path.as_str(),f.kind)).collect();
        assert_eq!(kinds,[("new.txt",FileKind::Untracked),("src/a.rs",FileKind::Modified),("b.rs",FileKind::Modified),("c.rs",FileKind::Modified),("d.rs",FileKind::Added),
            ("e.rs",FileKind::Deleted),("f.rs",FileKind::Conflict),("sp ace.txt",FileKind::Untracked)]);
        let mut files=files; let (a,d)=apply_numstat(&mut files,"3\t1\tsrc/a.rs\0-\t-\tb.rs\x0010\t0\td.rs\0");
        assert_eq!((files[1].added,files[1].deleted,files[2].added,files[4].added),(3,1,0,10)); assert_eq!((a,d),(13,1));
        // `diff --name-status -z` of a branch range.
        let b=parse_name_status("M\0src/a.rs\0A\0new.rs\0D\0old.rs\0T\0link\0");
        assert_eq!(b.iter().map(|f|f.kind).collect::<Vec<_>>(),[FileKind::Modified,FileKind::Added,FileKind::Deleted,FileKind::Modified]);
        let refs="refs/remotes/origin/main\torigin/main\t\t\t2 hours ago\nrefs/heads/main\tmain\torigin/main\t[ahead 2, behind 1]\t3 days ago\n\
refs/heads/feat/x\tfeat/x\t\t\t5 minutes ago\nrefs/remotes/origin/HEAD\torigin/HEAD\t\t\t1 week ago\nrefs/remotes/origin/old\torigin/old\t\t\t1 year, 2 months ago\n\
refs/heads/gone\tgone\torigin/gone\t[gone]\t2 weeks ago\n";
        let b=parse_branches(refs,"feat/x");
        let names:Vec<(&str,bool,bool)>=b.iter().map(|b|(b.name.as_str(),b.current,b.remote)).collect();
        assert_eq!(names,[("feat/x",true,false),("main",false,false),("gone",false,false),("origin/old",false,true)], "current first, remote-only after locals, no HEAD");
        assert_eq!((b[1].ahead,b[1].behind,b[1].date.as_str()),(2,1,"3d")); assert!(b[2].gone); assert_eq!((b[0].date.as_str(),b[3].date.as_str()),("5m","1y"));
        assert_eq!(graph_commits("* a1b2c3d (HEAD -> main, origin/main) top\n|\\  \n| * 9f8e7d6 side\n|/  \n* 1234567 base\n"),["a1b2c3d top","9f8e7d6 side","1234567 base"]);
    }
    #[test]
    fn sensitive_files_are_never_previewed() {
        for p in [".env",".env.local","config/secrets.yml","a/id_rsa","server.KEY","tls.pem","gh_token.txt","x/auth-store.json"] {assert!(sensitive(p),"{p}");}
        for p in ["src/main.rs","README.md","keyboard.rs","env.rs"] {assert!(!sensitive(p),"{p}");}
    }
    #[test]
    fn switch_and_events() {
        let mut s = State::new("p".into(),".".into());
        s.session.input=99;
        assert!(!apply(&mut s, Event::Pane(json!({"pane_id":"other"}))));
        apply(&mut s, Event::Pane(json!({"pane_id":"p","agent_session":{"value":"new"}})));
        assert_eq!(s.session.input,0); assert_eq!(s.session_path.as_deref(),Some("new")); assert!(s.read_session);
        let stale=Session {input:999,..Session::default()};
        assert!(!apply(&mut s,Event::Session(Some("old".into()),stale)));
        assert_eq!(s.session.input,0);
        s.refresh=false;
        apply(&mut s,Event::Status("other".into(),"done".into())); assert!(!s.refresh);
        apply(&mut s,Event::Status("p".into(),"idle".into())); assert!(s.refresh);
        s.refresh=false; apply(&mut s,Event::Status("p".into(),"done".into())); assert!(s.refresh);
        apply(&mut s,Event::Closed("other".into())); assert!(!s.quit);
        apply(&mut s,Event::Closed("p".into())); assert!(s.quit);
    }
    #[test]
    fn wire_messages() {
        for (v,kind) in [(json!({"event":"pane_updated","data":{"pane":{"pane_id":"p"}}}),"pane"),(json!({"event":"pane_closed","data":{"pane_id":"p"}}),"closed"),(json!({"event":"pane_agent_status_changed","data":{"pane_id":"p","agent_status":"idle"}}),"status"),(json!({"event":"pane.agent_status_changed","data":{"agent":"pi","agent_status":"working","pane_id":"p","workspace_id":"w"}}),"status"),(json!({"result":{"type":"subscription_started"}}),"started"),(json!({"error":{"message":"oops"}}),"error")] { assert_eq!(parse_wire(&v).0,kind); }
    }
    #[test]
    fn status_wire_payload_drives_only_bound_pane() {
        for spelling in ["pane_agent_status_changed","pane.agent_status_changed"] {
            let mut s=State::new("p".into(),".".into()); s.status="working".into(); s.refresh=false;
            let (_,event)=parse_wire(&json!({"event":spelling,"data":{"pane_id":"other","agent_status":"done"}}));
            assert!(!apply(&mut s,event.expect("status event"))); assert_eq!(s.status,"working"); assert!(!s.refresh);
            let (_,event)=parse_wire(&json!({"event":spelling,"data":{"pane_id":"p","agent_status":"done"}}));
            assert!(apply(&mut s,event.expect("status event"))); assert_eq!(s.status,"done"); assert!(s.refresh);
        }
    }
    #[test]
    fn token_format_and_context_band() {
        // Reference formatTokenCount: one decimal under 100k / 10M, rounded above.
        assert_eq!([0,950,33_677,179_200,999_999,1_000_000,12_400_000,370_000].map(fmt_tokens),["0","950","33.7k","179k","1000k","1.0M","12M","370k"]);
        assert_eq!(ctx_band(39.9),("● Óptimo",Tone::Mint)); assert_eq!(ctx_band(40.0),("● Medio",Tone::Gold));
        assert_eq!(ctx_band(65.0),("● Alerta",Tone::Orange)); assert_eq!(ctx_band(80.0),("● Crítico",Tone::Coral));
        assert_eq!((fmt_cost(0.25),fmt_cost(1.234)),("$0.250".to_string(),"$1.23".to_string()));
    }
    #[test]
    fn tool_calls_are_counted_by_kind() {
        let msg=json!({"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"x"},{"type":"toolCall","name":"read"},{"type":"toolCall","name":"edit"},{"type":"toolCall","name":"write"},{"type":"toolCall","name":"mem_save"},{"type":"toolCall","name":"bash"},{"type":"toolCall","name":"Bash"},{"type":"toolCall","name":"grep"},{"type":"toolCall","name":"fetch_content"},{"type":"toolCall","name":"web_search"},{"type":"toolCall","name":"todo"},{"type":"toolCall","name":""}]}});
        let user=json!({"type":"message","message":{"role":"user","content":[{"type":"toolCall","name":"read"}]}});
        let mut t=Tail::default(); t.ingest(format!("{msg}\n{user}\n").as_bytes(),false);
        let s=&t.session; assert_eq!((s.tools,s.reads,s.writes,s.mem),(10,1,2,1));
        assert_eq!((s.bash,s.grep,s.fetch,s.search,s.other),(2,1,1,1,1));
    }
    #[test]
    fn relative_time_versions_and_handoff_lines() {
        assert_eq!([None,Some(95),Some(70),Some(-200),Some(-7300),Some(-200_000)].map(|a|ago(a,100)),["nunca","recién","hace 30s","hace 5m","hace 2h","hace 2d"]);
        assert_eq!(clean_version("gentle-shell 4.5.1-main.14f44d11cacb").as_deref(),Some("4.5.1-main.14f44d11cacb"));
        assert_eq!(clean_version("engram 3.6.0\nmore").as_deref(),Some("3.6.0")); assert_eq!(clean_version("v1.2.3,").as_deref(),Some("1.2.3"));
        assert_eq!(clean_version("tool nightly").as_deref(),Some("nightly")); assert_eq!(clean_version("  "),None);
        let long="x".repeat(500); let body=format!("## Goal\n\n- a\n{long}\n")+&"l\n".repeat(60);
        let l=handoff_lines(&body); assert_eq!(l.len(),40); assert_eq!(&l[..2],["## Goal","- a"]); assert_eq!(l[2].chars().count(),300);
    }
    #[test]
    fn roster_merges_settings_agents_and_tasks() {
        let settings=json!({"defaultProvider":"cpamc","defaultModel":"gpt-6.1-sol","defaultThinkingLevel":"medium","apiKeyLike":"never-read"});
        let agents=vec![("jd-judge-a".to_string(),"---\nname: jd-judge-a\ndescription: d\n---\nbody model: nope".to_string()),
            ("x".to_string(),"---\nname: gentle-ai-explore\nmodel: other/fast-1\neffort: low\n---\n".to_string()),
            ("review-risk".to_string(),"no frontmatter".to_string())];
        let tasks=vec![json!({"agent":"jd-judge-a","status":"completed","label":"old","model":"cpamc/m-old","thinking":"high","createdAt":1}),
            json!({"agent":"jd-judge-a","status":"failed","label":"new","model":"cpamc/m-new","thinking":"xhigh","createdAt":5}),
            json!({"agent":"gentle-ai-explore","status":"running","label":"r","model":"cpamc/ignored","thinking":"high","createdAt":3}),
            json!({"agent":"gone","status":"completed","label":"g","createdAt":2})];
        let r=roster(&settings,&agents,&tasks);
        assert_eq!((r.model.as_str(),r.thinking.as_str(),r.total,r.done,r.active),("cpamc/gpt-6.1-sol","medium",4,2,1));
        assert_eq!(r.current.as_deref(),Some("gentle-ai-explore · r"));
        let a=|n:&str|r.agents.iter().find(|a|a.name==n).unwrap().clone();
        assert_eq!(r.agents.iter().map(|a|a.name.as_str()).collect::<Vec<_>>(),["gentle-ai-explore","jd-judge-a","review-risk"]);
        assert_eq!(a("gentle-ai-explore"),Agent{name:"gentle-ai-explore".into(),model:"other/fast-1".into(),thinking:"low".into()});
        assert_eq!(a("jd-judge-a"),Agent{name:"jd-judge-a".into(),model:"cpamc/m-new".into(),thinking:"xhigh".into()});
        assert_eq!(a("review-risk"),Agent{name:"review-risk".into(),model:"cpamc/gpt-6.1-sol".into(),thinking:"medium".into()});
        assert_eq!(roster(&json!({}),&[],&[]),Roster::default());
    }
    #[test]
    fn mcp_names_only_and_kind() {
        let v=json!({"mcpServers":{"remote-one":{"url":"https://x","headers":{"Authorization":"secret"}},"local-one":{"command":"node","args":["--token","secret"]}}});
        assert_eq!(mcp_servers(&v),vec![Mcp{name:"local-one".into(),local:true},Mcp{name:"remote-one".into(),local:false}]);
        assert_eq!(mcp_servers(&json!({})),vec![]);
        assert_eq!(agent_dir("/a/agent/sessions/--x--/s.jsonl"),Some(PathBuf::from("/a/agent")));
    }
    #[test]
    fn pr_cycle_follows_head_moves_only() {
        let g=Git{branch:"main".into(),dirty:Some(0),commits:vec!["a1 x".into()],..Git::default()};
        assert!(!head_moved(&g,&Git{dirty:Some(3),..g.clone()}));
        assert!(head_moved(&g,&Git{branch:"feat".into(),..g.clone()}));
        assert!(head_moved(&g,&Git{commits:vec!["b2 y".into(),"a1 x".into()],..g.clone()}));
    }
    #[test]
    fn model_shapes() {
        assert_eq!(context_window(&json!({"cpamc":{"models":[{"id":"m","contextWindow":200000}]}}),"m"),Some(200000));
        assert_eq!(context_window(&json!({"cache":[{"id":"m","contextWindow":123}]}),"m"),Some(123));
        assert_eq!(context_window(&json!({}),"m"),None);
    }
    #[test]
    fn heads_and_gitdir() {
        assert_eq!(head_name("ref: refs/heads/feature/test\n"),"feature/test");
        assert_eq!(head_name("123456789abcdef\n"),"detached 12345678");
        assert_eq!(gitdir_target(std::path::Path::new("/repo"),"gitdir: ../worktrees/x\n"),std::path::PathBuf::from("/repo/../worktrees/x"));
    }
    #[test]
    fn pr_schedule() {
        assert_eq!((pr_delay(0),pr_delay(1),pr_delay(2),pr_delay(3)),(120,300,900,900));
        assert!(!pr_due(Some(100),129)); assert!(pr_due(Some(100),130)); assert!(pr_due(None,0));
        assert_eq!(pr_trigger_due(None,5),5); assert_eq!(pr_trigger_due(Some(100),110),130); assert_eq!(pr_trigger_due(Some(100),200),200);
    }
}
