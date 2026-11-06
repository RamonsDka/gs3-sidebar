mod screenshot;
mod voice;
pub use screenshot::screenshot;
pub use voice::voice;
use crate::core::*;
use anyhow::{anyhow, Context, Result};
use notify::{RecursiveMode, Watcher};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::{fs, io::{BufRead, BufReader, Read, Seek, SeekFrom, Write}, os::unix::net::UnixStream, path::{Path, PathBuf}, process::{Command, Stdio}, sync::mpsc::{self, Sender}, thread, time::{Duration, Instant}};

// Drain both pipes concurrently: even a noisy subprocess cannot deadlock the timeout.
pub fn run(cwd:&Path,program:&str,args:&[&str],seconds:u64)->Result<String> {
    let mut child=Command::new(program).args(args).current_dir(cwd).env("GIT_OPTIONAL_LOCKS","0").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    fn drain<R:Read+Send+'static>(mut pipe:R)->thread::JoinHandle<String> {thread::spawn(move||{let mut s=String::new(); let _=pipe.read_to_string(&mut s); s})}
    let out=drain(child.stdout.take().unwrap()); let err=drain(child.stderr.take().unwrap());
    let deadline=Instant::now()+Duration::from_secs(seconds);
    let status=loop {if let Some(s)=child.try_wait()? {break s} if Instant::now()>=deadline {let _=child.kill(); let _=child.wait(); return Err(anyhow!("{program}: timeout ({seconds}s)"))} thread::sleep(Duration::from_millis(20));};
    let stdout=out.join().unwrap_or_default(); let stderr=err.join().unwrap_or_default();
    if status.success() {Ok(stdout.trim_end().into())} else {Err(anyhow!("{}: {}",program,stderr.trim()))}
}
pub fn git(cwd:&Path,args:&[&str])->Result<String> {run(cwd,"git",args,3)}
pub fn git_root(cwd:&Path)->Option<PathBuf> {git(cwd,&["rev-parse","--show-toplevel"]).ok().map(PathBuf::from)}
pub fn git_dir(root:&Path)->PathBuf {
    let p=root.join(".git");
    if p.is_file() {gitdir_target(root,&fs::read_to_string(&p).unwrap_or_default())} else {p}
}
pub fn git_snapshot(cwd:&Path)->Git {
    let Some(root)=git_root(cwd) else {return Git{error:Some("not a git repo".into()),..Git::default()}};
    let head=fs::read_to_string(git_dir(&root).join("HEAD")).unwrap_or_default();
    let mut g=Git{branch:head_name(&head),..Git::default()}; let mut errors=Vec::new();
    // Porcelain paths are relative to the repository root: every command runs there.
    let cwd=root.as_path();
    match git(cwd,&["status","--porcelain=v1","-z","--no-renames","--untracked-files=all"]) {Ok(s)=>{let (files,counts)=parse_status(&s); g.dirty=Some(files.len()); g.files=files; g.counts=counts;},Err(e)=>errors.push(e.to_string())}
    // No commits yet: there is no HEAD to diff against.
    if let Ok(s)=git(cwd,&["diff","HEAD","--numstat","-z","--no-renames"]) {(g.added,g.deleted)=apply_numstat(&mut g.files,&s);}
    if let Ok(s)=git(cwd,&["rev-list","--left-right","--count","@{u}...HEAD"]) {let n:Vec<u64>=s.split_whitespace().filter_map(|x|x.parse().ok()).collect(); if n.len()==2 {g.ahead_behind=Some((n[1],n[0]));}}
    match git(cwd,&["log","--graph","--oneline","--decorate=short","--color=never","-n","30"]) {
        Ok(s)=>{g.commits=graph_commits(&s); g.graph=s.lines().map(str::to_owned).collect();}
        Err(e) if e.to_string().contains("does not have any commits")=>{},
        Err(e)=>errors.push(e.to_string())
    }
    if let Ok(s)=git(cwd,&["for-each-ref","--format=%(refname)%09%(refname:short)%09%(upstream:short)%09%(upstream:track)%09%(committerdate:relative)","refs/heads","refs/remotes"]) {g.branches=parse_branches(&s,&g.branch);}
    if !errors.is_empty() {g.error=Some(errors.join("; "));}
    g
}
/// Names the TREE tab never lists (reference `DEFAULT_IGNORED_NAMES` + Rust `target`).
const TREE_SKIP:[&str;9]=[".git","node_modules","dist",".cache",".next","coverage",".turbo","build","target"];
const TREE_MAX:usize=2000;
/// Scan `cwd`'s git root (or `cwd`): dirs first, then files, both sorted case-insensitively. Only directories in
/// `open` (whose parents are open too) are read further; the others only count their entries. Symlinks are listed
/// as files and never followed. Names only: no file is read.
pub fn tree(cwd:&Path,open:Vec<String>)->Tree {
    let root=git_root(cwd).unwrap_or_else(||cwd.to_owned());
    let project=root.file_name().unwrap_or_default().to_string_lossy().into_owned();
    fn entries(dir:&Path)->std::io::Result<Vec<(String,bool)>> {
        let mut v:Vec<(String,bool)>=fs::read_dir(dir)?.flatten().filter_map(|e|{let name=e.file_name().to_string_lossy().into_owned();
            (!TREE_SKIP.contains(&name.as_str())).then(||(name,e.file_type().is_ok_and(|t|t.is_dir())))}).collect();
        v.sort_by(|a,b|b.1.cmp(&a.1).then_with(||a.0.to_lowercase().cmp(&b.0.to_lowercase()))); Ok(v)
    }
    fn scan(dir:&Path,rel:&str,open:&[String],budget:&mut usize)->std::io::Result<Vec<TreeNode>> {
        let mut out=Vec::new();
        for (name,is_dir) in entries(dir)? {
            if *budget==0 {break} *budget-=1;
            let r=if rel.is_empty() {name.clone()} else {format!("{rel}/{name}")};
            let (count,children)=if !is_dir {(0,Vec::new())} else if open.contains(&r) {
                let c=scan(&dir.join(&name),&r,open,budget).unwrap_or_default(); (c.len(),c)
            } else {(entries(&dir.join(&name)).map(|e|e.len()).unwrap_or(0),Vec::new())};
            out.push(TreeNode{name,rel:r,dir:is_dir,count,children});
        }
        Ok(out)
    }
    let mut budget=TREE_MAX;
    match scan(&root,"",&open,&mut budget) {Ok(nodes)=>Tree{project,open,nodes,error:None},Err(e)=>Tree{project,open,nodes:Vec::new(),error:Some(e.to_string())}}
}
/// TREE scan worker: answers only the newest open set.
pub fn trees(cwd:PathBuf,tx:Sender<Event>)->Sender<Vec<String>> {
    let (send,rx)=mpsc::channel::<Vec<String>>();
    thread::spawn(move||{while let Ok(mut open)=rx.recv() {
        while let Ok(n)=rx.try_recv() {open=n;}
        if tx.send(Event::Tree(tree(&cwd,open))).is_err() {break}
    }}); send
}
/// What the GIT tab inspects: a branch diffed against HEAD and/or one file (untracked files are read directly).
#[derive(Clone, Debug, PartialEq)]
pub struct DetailRequest { pub branch:Option<String>, pub file:Option<String>, pub untracked:bool }
const DIFF_KEEP:usize=2000; const UNTRACKED_MAX:u64=64*1024;
fn diff_lines(d:&mut Detail,text:&str) {let all:Vec<&str>=text.lines().collect(); d.more=all.len().saturating_sub(DIFF_KEEP); d.diff=all.into_iter().take(DIFF_KEEP).map(str::to_owned).collect();}
pub fn detail(cwd:&Path,r:&DetailRequest)->Detail {
    let mut d=Detail{branch:r.branch.clone(),file:r.file.clone(),..Detail::default()};
    let Some(root)=git_root(cwd) else {d.error=Some("not a git repo".into()); return d};
    if let Some(b)=&r.branch {
        let range=format!("HEAD...{b}");
        match git(&root,&["diff","--name-status","-z","--no-renames","--end-of-options",&range]) {
            Ok(s)=>{d.files=parse_name_status(&s); if let Ok(n)=git(&root,&["diff","--numstat","-z","--no-renames","--end-of-options",&range]) {(d.added,d.deleted)=apply_numstat(&mut d.files,&n);}}
            Err(e)=>d.error=Some(e.to_string()),
        }
    }
    let Some(f)=&r.file else {return d};
    // Never read nor diff what may hold credentials (the UI says so instead).
    if sensitive(f) {return d}
    let out=match &r.branch {
        Some(b)=>git(&root,&["diff","--no-renames","--end-of-options",&format!("HEAD...{b}"),"--",f]),
        None if r.untracked=>{
            let p=root.join(f);
            // Symlinks could point outside the repository: not followed.
            match fs::symlink_metadata(&p) {
                Ok(m) if m.file_type().is_symlink()=>Ok("enlace simbólico (no se sigue)".into()),
                Ok(m) if m.is_dir()=>Ok("directorio sin seguimiento".into()),
                Ok(m) if m.len()>UNTRACKED_MAX=>Ok(format!("archivo nuevo de {} KiB (no se muestra)",m.len()/1024)),
                Ok(_)=>fs::read(&p).map_err(Into::into).map(|b|if b.contains(&0) {"archivo binario".into()} else {
                    let t=String::from_utf8_lossy(&b); format!("@@ archivo nuevo @@\n{}",t.lines().map(|l|format!("+{l}")).collect::<Vec<_>>().join("\n"))}),
                Err(e)=>Err(e.into()),
            }
        }
        None=>git(&root,&["diff","HEAD","--no-renames","--",f]),
    };
    match out {Ok(s)=>diff_lines(&mut d,&s),Err(e)=>d.diff=vec![format!("error: {e}")]}
    d
}
/// Inspector worker: only the latest pending request is answered (clicks may outrun git).
pub fn details(cwd:PathBuf,tx:Sender<Event>)->Sender<DetailRequest> {
    let (send,rx)=mpsc::channel::<DetailRequest>();
    thread::spawn(move||{while let Ok(mut r)=rx.recv() {
        while let Ok(n)=rx.try_recv() {r=n;}
        if tx.send(Event::Detail(detail(&cwd,&r))).is_err() {break}
    }}); send
}
pub fn open_engram(path:&Path)->rusqlite::Result<Connection> {Connection::open_with_flags(path,OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX)}
/// `GET http://127.0.0.1:<port><path>` (HTTP/1.0, bounded by short timeouts) → JSON body of a 200 answer.
pub fn http_json(port:u16,path:&str)->Option<Value> {
    let mut s=std::net::TcpStream::connect_timeout(&([127,0,0,1],port).into(),Duration::from_millis(400)).ok()?;
    s.set_read_timeout(Some(Duration::from_millis(1200))).ok()?; s.set_write_timeout(Some(Duration::from_millis(400))).ok()?;
    s.write_all(format!("GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\nAccept: application/json\r\n\r\n").as_bytes()).ok()?;
    let mut buf=Vec::new(); s.take(256*1024).read_to_end(&mut buf).ok()?;
    let text=String::from_utf8_lossy(&buf); let (head,body)=text.split_once("\r\n\r\n")?;
    head.lines().next().is_some_and(|l|l.split_whitespace().nth(1)==Some("200")).then(||serde_json::from_str(body).ok()).flatten()
}
fn query_escape(s:&str)->String {s.bytes().map(|b|if b.is_ascii_alphanumeric()||b"-_.~".contains(&b) {(b as char).to_string()} else {format!("%{b:02X}")}).collect()}
pub fn engram(cwd:&Path)->Engram {
    let root=git_root(cwd).unwrap_or_else(||cwd.to_owned());
    let project=root.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let path=std::env::var_os("ENGRAM_DB_PATH").map(PathBuf::from).unwrap_or_else(||dirs::home_dir().unwrap_or_default().join(".engram/engram.db"));
    let port=std::env::var("ENGRAM_PORT").ok().and_then(|p|p.parse().ok()).unwrap_or(7437);
    let mut e=Engram{online:http_json(port,"/health").is_some_and(|v|text(&v,"status")=="ok"),..Engram::default()};
    let db=open_engram(&path);
    let result=db.as_ref().map_err(|e|anyhow!("{e}")).and_then(|db|->Result<()>{
        e.count=Some(db.query_row("SELECT count(*) FROM observations WHERE project=?1 AND deleted_at IS NULL",[project.as_str()],|r|r.get(0))?);
        e.enrolled=db.query_row("SELECT count(*) FROM sync_enrolled_projects WHERE project=?1",[project.as_str()],|r|r.get::<_,i64>(0)).unwrap_or(0)>0;
        e.handoff=db.query_row("SELECT id,type,content,CAST(strftime('%s',created_at) AS INTEGER) FROM observations WHERE project=?1 AND type='session_summary' AND deleted_at IS NULL ORDER BY id DESC LIMIT 1",
            [project.as_str()],|r|Ok(Handoff{id:r.get(0)?,kind:r.get(1)?,lines:handoff_lines(&r.get::<_,String>(2)?),at:r.get(3)?})).ok();
        Ok(())
    });
    if let Err(err)=result {e.error=Some(err.to_string());}
    if e.online && e.enrolled {
        e.sync=http_json(port,&format!("/sync/status?project={}",query_escape(&project))).map(|v|{
            let last=v["last_sync_at"].as_str().and_then(|t|db.as_ref().ok()?.query_row("SELECT CAST(strftime('%s',?1) AS INTEGER)",[t],|r|r.get(0)).ok());
            (text(&v,"phase").to_string(),last)
        });
    }
    e.project=project; e
}
/// PROF roster from the session's agent dir: `settings.json` (default model keys only), `agents/*.md`, `gentle-agents/tasks/*.json`.
pub fn roster_of(session:&Path)->Option<Roster> {
    let dir=agent_dir(&session.to_string_lossy())?;
    let settings:Value=fs::read(dir.join("settings.json")).ok().and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
    let files=|sub:&str,ext:&str|->Vec<PathBuf> {let mut v:Vec<PathBuf>=fs::read_dir(dir.join(sub)).into_iter().flatten().flatten().map(|e|e.path()).filter(|p|p.extension().is_some_and(|x|x==ext)).collect(); v.sort(); v};
    let agents:Vec<(String,String)>=files("agents","md").into_iter().take(64).filter_map(|p|Some((p.file_stem()?.to_string_lossy().into_owned(),fs::read_to_string(&p).ok()?))).collect();
    let tasks:Vec<Value>=files("gentle-agents/tasks","json").into_iter().rev().take(200).filter_map(|p|serde_json::from_slice::<Value>(&fs::read(p).ok()?).ok()).map(|v|v["task"].clone()).filter(|t|t.is_object()).collect();
    (settings.is_object()||!agents.is_empty()).then(||roster(&settings,&agents,&tasks))
}
/// Stack · Ecosistema: `<tool> --version` once at start (each bounded by run()'s timeout) plus the HUD's own version.
pub fn stack(cwd:&Path)->Vec<Tool> {
    let probe=|p:&str,a:&str|run(cwd,p,&[a],3).ok().and_then(|o|clean_version(&o));
    vec![Tool{name:"pi",version:probe("pi","--version")},Tool{name:"gs3-hud",version:Some(env!("CARGO_PKG_VERSION").into())},
        Tool{name:"gentle-shell",version:probe("gentle-shell","--version")},Tool{name:"gentle-ai",version:probe("gentle-ai","--version")},
        Tool{name:"engram",version:probe("engram","version").or_else(||probe("engram","--version"))},Tool{name:"herdr",version:probe("herdr","--version")}]
}
/// MCP servers configured for the session's Pi agent dir (`mcp.json`); names only.
pub fn mcp(session:&Path)->Option<Vec<Mcp>> {
    let dir=agent_dir(&session.to_string_lossy())?;
    let v:Value=serde_json::from_slice(&fs::read(dir.join("mcp.json")).ok()?).ok()?;
    Some(mcp_servers(&v))
}
pub fn read_tail(path:&Path,tail:&mut Tail)->Result<()> {
    let mut f=fs::File::open(path)?; let reset=f.metadata()?.len()<tail.offset;
    if reset {*tail=Tail::default();}
    f.seek(SeekFrom::Start(tail.offset))?; let mut bytes=Vec::new(); f.read_to_end(&mut bytes)?; tail.ingest(&bytes,false);
    // Context window only depends on the model: re-read the model stores only when it changes.
    if tail.limit_model.as_deref()==Some(tail.session.model.as_str()) {return Ok(())}
    tail.limit_model=Some(tail.session.model.clone());
    if let Some((agent,_))=path.to_string_lossy().split_once("/sessions/") {
        // The cpamc cache is what Pi registers at runtime (its footer ctx % uses it); the store is a fallback.
        tail.session.limit=["cpamc-models-cache.json","models-store.json"].iter().find_map(|name|{
            let v:Value=serde_json::from_slice(&fs::read(Path::new(agent).join(name)).ok()?).ok()?;
            context_window(&v,&tail.session.model)
        });
    }
    Ok(())
}
pub fn request(socket:&Path,method:&str,params:Value)->Result<Value> {
    let mut stream=UnixStream::connect(socket)?; stream.set_read_timeout(Some(Duration::from_secs(3)))?; stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    writeln!(stream,"{}",json!({"id":"hud","method":method,"params":params}))?;
    let mut line=String::new(); BufReader::new(stream).read_line(&mut line)?;
    let v:Value=serde_json::from_str(&line)?;
    if !v["error"].is_null() {return Err(anyhow!("{}",v["error"]))} Ok(v["result"].clone())
}
/// The only pane-control exception: fixed F9 to the pane bound at startup, after focus succeeds.
fn model_picker_requests(id:&str)->Vec<(&'static str,Value)> {
    vec![
        ("pane.focus",json!({"pane_id":id})),
        ("pane.send_keys",json!({"pane_id":id,"keys":["f9"]})),
    ]
}
/// Explicit clicks only; coalesce pending requests and discard clicks within one second of the last attempt.
pub fn model_picker(socket:PathBuf,id:String)->Sender<Instant> {
    let (send,rx)=mpsc::channel::<Instant>();
    thread::spawn(move||{
        let mut last:Option<Instant>=None;
        while let Ok(mut at)=rx.recv() {
            while let Ok(next)=rx.try_recv() {at=next;}
            if last.is_some_and(|prev|at.saturating_duration_since(prev)<Duration::from_secs(1)) {continue}
            last=Some(at);
            for (method,params) in model_picker_requests(&id) {
                if request(&socket,method,params).is_err() {break}
            }
        }
    }); send
}
pub fn pane(socket:&Path,id:&str)->Result<Option<Value>> {
    let found=match request(socket,"pane.get",json!({"pane_id":id})) {
        Ok(v) if v["pane"].is_object()=>Some(v["pane"].clone()),
        _=>{let v=request(socket,"pane.list",json!({}))?;
            let list=v["panes"].as_array().or_else(||v.as_array()).context("invalid pane.list")?;
            list.iter().find(|v|text(v,"pane_id")==id).cloned()}
    };
    Ok(found.map(|mut p|{
        // Panes only carry workspace_id; the human label lives on the workspace.
        if let Ok(w)=request(socket,"workspace.get",json!({"workspace_id":text(&p,"workspace_id")})) {p["workspace_label"]=w["workspace"]["label"].clone();}
        p
    }))
}
pub fn herdr(socket:PathBuf,id:String,tx:Sender<Event>) {
    thread::spawn(move||{
        let mut backoff=1;
        loop {
            let result=(||->Result<()>{
                let mut stream=UnixStream::connect(&socket)?;
                stream.set_write_timeout(Some(Duration::from_secs(3)))?;
                writeln!(stream,"{}",json!({"id":"events","method":"events.subscribe","params":{"subscriptions":[{"type":"pane.updated"},{"type":"pane.closed"},{"type":"pane.agent_status_changed","pane_id":id},{"type":"workspace.focused"}]}}))?;
                for line in BufReader::new(stream).lines() {
                    let v:Value=serde_json::from_str(&line?)?; let (kind,event)=parse_wire(&v);
                    if kind=="error" {return Err(anyhow!("subscription rejected"))}
                    if kind=="started" {
                        tx.send(Event::Connection(true))?; backoff=1;
                        match pane(&socket,&id)? {Some(p)=>tx.send(Event::Pane(p))?,None=>{tx.send(Event::Closed(id.clone()))?;return Ok(())}}
                    }
                    let relevant=match &event {Some(Event::Pane(p))=>text(p,"pane_id")==id,Some(Event::Status(p,_))|Some(Event::Closed(p))=>p==&id,_=>false};
                    let closed=matches!(&event,Some(Event::Closed(p)) if p==&id);
                    if let Some(e)=event {tx.send(e)?;}
                    if closed {return Ok(())}
                    if relevant {if let Ok(p)=pane(&socket,&id) {match p {Some(p)=>tx.send(Event::Pane(p))?,None=>{tx.send(Event::Closed(id.clone()))?;return Ok(())}}}}
                }
                Err(anyhow!("socket closed"))
            })();
            if result.is_ok() {break}
            if tx.send(Event::Connection(false)).is_err() {break}
            thread::sleep(Duration::from_secs(backoff)); backoff=(backoff*2).min(30);
        }
    });
}

pub enum Local { Refresh, Session(Option<String>), Changed(notify::Event) }
/// Pi creates a new session file on `/new` / `/resume`, but herdr does not always emit
/// `pane_updated` for the change: a new `.jsonl` next to the bound session re-queries the pane.
pub fn is_new_session_file(e:&notify::Event,session:&Path)->bool {
    matches!(e.kind,notify::EventKind::Create(_)) && e.paths.iter().any(|p|p!=session && p.parent()==session.parent() && p.extension().is_some_and(|x|x=="jsonl"))
}
pub fn local(cwd:PathBuf,socket:PathBuf,pane_id:String,tx:Sender<Event>)->Sender<Local> {
    let (send,rx)=mpsc::channel(); let notify_tx=send.clone();
    thread::spawn(move||{
        let mut watcher=notify::recommended_watcher(move|r:notify::Result<notify::Event>|{if let Ok(e)=r {let _=notify_tx.send(Local::Changed(e));}}).ok();
        if let (Some(w),Some(root))=(&mut watcher,git_root(&cwd)) {
            let dir=git_dir(&root);
            for p in [dir.join("HEAD"),dir.join("index")] {let _=w.watch(&p,RecursiveMode::NonRecursive);}
            let _=w.watch(&dir.join("refs"),RecursiveMode::Recursive);
            // Watch only direct gitdir entries too: HEAD/index are commonly atomically replaced.
            let _=w.watch(&dir,RecursiveMode::NonRecursive);
            if let Ok(common)=fs::read_to_string(dir.join("commondir")) {let _=w.watch(&dir.join(common.trim()).join("refs"),RecursiveMode::Recursive);}
        }
        let mut session:Option<PathBuf>=None; let mut tail=Tail::default(); let mut git_deadline:Option<Instant>=None;
        // Last MCP list sent; matches State::new so unchanged lists are never re-sent.
        let mut mcp_sent:Option<Vec<Mcp>>=None;
        let send_mcp=|session:&Option<PathBuf>,sent:&mut Option<Vec<Mcp>>|{let now=session.as_deref().and_then(mcp); if now!=*sent {*sent=now.clone(); let _=tx.send(Event::Mcp(now));}};
        // Same rule for the PROF roster (agent dir files): only a changed roster becomes an event.
        let mut roster_sent:Option<Roster>=None;
        let send_roster=|session:&Option<PathBuf>,sent:&mut Option<Roster>|{let now=session.as_deref().and_then(roster_of); if now!=*sent {*sent=now.clone(); let _=tx.send(Event::Roster(now));}};
        loop {
            let msg=match git_deadline {Some(t)=>rx.recv_timeout(t.saturating_duration_since(Instant::now())),None=>rx.recv().map_err(|_|mpsc::RecvTimeoutError::Disconnected)};
            match msg {
                Ok(Local::Refresh)=>{let _=tx.send(Event::Git(git_snapshot(&cwd))); let _=tx.send(Event::Engram(engram(&cwd))); send_mcp(&session,&mut mcp_sent); send_roster(&session,&mut roster_sent);}
                Ok(Local::Session(path))=>{
                    if let (Some(w),Some(p))=(&mut watcher,&session) {let _=w.unwatch(p.parent().unwrap_or(p));}
                    session=path.map(PathBuf::from); tail=Tail::default();
                    if let Some(p)=&session {
                        if let Some(w)=&mut watcher {let _=w.watch(p.parent().unwrap_or(p),RecursiveMode::NonRecursive);}
                        let _=read_tail(p,&mut tail);
                    }
                    let _=tx.send(Event::Session(session.as_ref().map(|p|p.to_string_lossy().into_owned()),tail.session.clone()));
                    send_mcp(&session,&mut mcp_sent); send_roster(&session,&mut roster_sent);
                }
                Ok(Local::Changed(e))=>{
                    if !matches!(e.kind,notify::EventKind::Create(_)|notify::EventKind::Modify(_)|notify::EventKind::Remove(_)) {continue}
                    if session.as_ref().is_some_and(|p|is_new_session_file(&e,p)) {if let Ok(Some(p))=pane(&socket,&pane_id) {let _=tx.send(Event::Pane(p));}}
                    else if session.as_ref().is_some_and(|p|e.paths.contains(p)) {if let Some(p)=&session {let _=read_tail(p,&mut tail); let _=tx.send(Event::Session(Some(p.to_string_lossy().into_owned()),tail.session.clone()));}}
                    else if e.paths.iter().any(|p|{let name=p.file_name().unwrap_or_default(); name=="HEAD"||name=="index"||p.components().any(|c|c.as_os_str()=="refs")}) {git_deadline=Some(Instant::now()+Duration::from_millis(300));}
                }
                Err(mpsc::RecvTimeoutError::Timeout)=>{git_deadline=None; let _=tx.send(Event::Git(git_snapshot(&cwd)));}
                Err(_)=>break
            }
        }
    }); send
}

fn github(cwd:&Path)->bool {git(cwd,&["remote","get-url","origin"]).is_ok_and(|s|s.contains("github.com"))}
fn pr_current(cwd:&Path)->Result<Vec<String>> {
    let output=run(cwd,"gh",&["pr","view","--json","number,title,state,reviewDecision,statusCheckRollup"],8);
    match output {
        Err(e) if e.to_string().contains("no pull requests found")=>Ok(vec![format!("no PR for {}",git(cwd,&["branch","--show-current"]).unwrap_or_default())]),
        Err(e)=>Err(e),
        Ok(s)=>{let v:Value=serde_json::from_str(&s)?; let mut counts=[0;3];
            if let Some(checks)=v["statusCheckRollup"].as_array() {for c in checks {let state=c["conclusion"].as_str().filter(|s|!s.is_empty()).unwrap_or(text(c,"state")); let i=match state {"SUCCESS"|"NEUTRAL"|"SKIPPED"=>0,"FAILURE"|"ERROR"|"CANCELLED"|"TIMED_OUT"|"ACTION_REQUIRED"=>1,_=>2}; counts[i]+=1;}}
            Ok(vec![format!("#{} {}",v["number"],text(&v,"title")),format!("{} · {}",text(&v,"state"),text(&v,"reviewDecision")),format!("checks ✓{} ✗{} …{}",counts[0],counts[1],counts[2])])}
    }
}
// One PR cycle = current-branch PR + open count; cycles are spaced by PR_MIN_GAP seconds.
fn pr_cycle(cwd:&Path)->Vec<String> {
    let mut lines=pr_current(cwd).unwrap_or_else(|e|vec![format!("PR error: {e}")]);
    lines.push(open_count(cwd)); lines
}
pub fn pr_once(cwd:&Path)->Vec<String> {
    if !github(cwd) {return vec!["no GitHub origin".into()]}
    pr_cycle(cwd)
}
fn open_count(cwd:&Path)->String {
    match run(cwd,"gh",&["pr","list","--state","open","--json","number","--limit","100"],8).and_then(|s|Ok(serde_json::from_str::<Vec<Value>>(&s)?.len())) {
        Ok(n)=>format!("open PRs: {n}{}",if n==100{"+"}else{""}),Err(e)=>format!("open PRs error: {e}")
    }
}
pub fn prs(cwd:PathBuf,tx:Sender<Event>)->Sender<()> {
    let (send,rx)=mpsc::channel();
    thread::spawn(move||{
        if !github(&cwd) {let _=tx.send(Event::Pr(vec!["no GitHub origin".into()])); return}
        let start=Instant::now(); let mut last:Option<u64>=None; let mut due=0; let mut attempt=0;
        loop {
            let now=start.elapsed().as_secs();
            if now>=due && pr_due(last,now) {
                if tx.send(Event::Pr(pr_cycle(&cwd))).is_err() {break}
                let done=start.elapsed().as_secs(); last=Some(done); due=done+pr_delay(attempt); attempt+=1;
            }
            let wait=due.saturating_sub(start.elapsed().as_secs()).max(1);
            match rx.recv_timeout(Duration::from_secs(wait)) {
                // Triggers (start, HEAD change, turn end, `r`) coalesce and reset the idle backoff.
                Ok(())=>{attempt=0; due=pr_trigger_due(last,start.elapsed().as_secs());}
                Err(mpsc::RecvTimeoutError::Timeout)=>{},Err(_)=>break
            }
        }
    }); send
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_picker_requests_are_bound_focus_then_f9() {
        for pane in ["w1:p1", "bound-other"] {
            assert_eq!(model_picker_requests(pane), vec![
                ("pane.focus", json!({"pane_id":pane})),
                ("pane.send_keys", json!({"pane_id":pane,"keys":["f9"]})),
            ]);
        }
    }
    #[test]
    fn session_files_switch_append_and_truncate() {
        let dir=std::env::temp_dir().join(format!("gs3-tail-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&dir).unwrap();
        let first=dir.join("first.jsonl"); let second=dir.join("second.jsonl");
        let line=b"{\"type\":\"message\",\"message\":{\"role\":\"assistant\",\"usage\":{\"input\":7}}}\n";
        fs::write(&first,line).unwrap(); fs::write(&second,line).unwrap();
        let mut tail=Tail::default(); read_tail(&first,&mut tail).unwrap();
        assert_eq!(tail.session.input,7);
        let mut f=fs::OpenOptions::new().append(true).open(&first).unwrap();
        f.write_all(&line[..20]).unwrap(); read_tail(&first,&mut tail).unwrap(); assert_eq!(tail.session.input,7);
        f.write_all(&line[20..]).unwrap(); read_tail(&first,&mut tail).unwrap(); assert_eq!(tail.session.input,14);
        fs::write(&first,line).unwrap(); read_tail(&first,&mut tail).unwrap(); assert_eq!(tail.session.input,7);
        let mut state=State::new("p".into(),dir.clone()); state.session=tail.session.clone();
        apply(&mut state,Event::Pane(json!({"pane_id":"p","agent_session":{"value":second}})));
        assert_eq!(state.session.input,0); assert!(state.read_session);
        tail=Tail::default(); read_tail(Path::new(state.session_path.as_ref().unwrap()),&mut tail).unwrap();
        let path=state.session_path.clone(); apply(&mut state,Event::Session(path,tail.session)); assert_eq!(state.session.input,7);
        drop(f); fs::remove_file(first).unwrap(); fs::remove_file(second).unwrap(); fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn new_sibling_session_file_triggers_requery() {
        let ev=|kind,p:&str|notify::Event{kind,paths:vec![PathBuf::from(p)],attrs:Default::default()};
        let create=notify::EventKind::Create(notify::event::CreateKind::File);
        let modify=notify::EventKind::Modify(notify::event::ModifyKind::Any);
        let cur=Path::new("/s/dir/a.jsonl");
        assert!(is_new_session_file(&ev(create,"/s/dir/b.jsonl"),cur));
        assert!(!is_new_session_file(&ev(create,"/s/dir/a.jsonl"),cur));
        assert!(!is_new_session_file(&ev(modify,"/s/dir/b.jsonl"),cur));
        assert!(!is_new_session_file(&ev(create,"/s/dir/b.tmp"),cur));
        assert!(!is_new_session_file(&ev(create,"/s/other/b.jsonl"),cur));
    }
    #[test]
    fn subprocesses_disable_git_optional_locks() {
        let out=run(&std::env::temp_dir(),"/bin/sh",&["-c","printf '%s' \"$GIT_OPTIONAL_LOCKS\""],3).unwrap();
        assert_eq!(out,"0");
    }
    #[test]
    fn pane_lookup_adds_workspace_label() {
        let sock=std::env::temp_dir().join(format!("gs3-hud-sock-{}",std::process::id()));
        let _=fs::remove_file(&sock);
        let listener=std::os::unix::net::UnixListener::bind(&sock).unwrap();
        let server=thread::spawn(move||{
            let mut methods=Vec::new();
            for _ in 0..2 {
                let (stream,_)=listener.accept().unwrap(); let mut line=String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                let v:Value=serde_json::from_str(&line).unwrap(); let m=text(&v,"method").to_string();
                let reply=if m=="pane.get" {json!({"id":"hud","result":{"pane":{"pane_id":"w1:p1","workspace_id":"w1"}}})}
                    else {json!({"id":"hud","result":{"workspace":{"workspace_id":"w1","label":"gs3 · repo"}}})};
                writeln!(&stream,"{reply}").unwrap(); methods.push((m,v["params"].clone()));
            }
            methods
        });
        let p=pane(&sock,"w1:p1").unwrap().unwrap();
        assert_eq!(text(&p,"workspace_label"),"gs3 · repo");
        let methods=server.join().unwrap(); fs::remove_file(&sock).unwrap();
        assert_eq!(methods,vec![("pane.get".to_string(),json!({"pane_id":"w1:p1"})),("workspace.get".to_string(),json!({"workspace_id":"w1"}))]);
    }
    #[test]
    fn sibling_creation_requeries_bound_pane_and_resets_session() {
        let dir=std::env::temp_dir().join(format!("gs3-follow-{}",std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let old=dir.join("old.jsonl"); let new=dir.join("new.jsonl");
        for (path,input) in [(&old,99),(&new,7)] {fs::write(path,format!("{}\n",json!({"type":"message","message":{"role":"assistant","usage":{"input":input}}}))).unwrap();}
        let socket=dir.join("herdr.sock");
        let listener=std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let next=new.clone();
        let server=thread::spawn(move||{
            for method in ["pane.get","workspace.get"] {
                let (stream,_)=listener.accept().unwrap(); let mut line=String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                let req:Value=serde_json::from_str(&line).unwrap(); assert_eq!(req["method"],method);
                let result=if method=="pane.get" {assert_eq!(req["params"]["pane_id"],"bound"); json!({"pane":{"pane_id":"bound","workspace_id":"w","agent_session":{"value":next}}})} else {json!({"workspace":{"label":"Probe"}})};
                writeln!(&stream,"{}",json!({"result":result})).unwrap();
            }
        });
        let (tx,rx)=mpsc::channel();
        let local=local(dir.clone(),socket,"bound".into(),tx);
        local.send(Local::Session(Some(old.to_string_lossy().into_owned()))).unwrap();
        let mut state=State::new("bound".into(),dir.clone()); state.session_path=Some(old.to_string_lossy().into_owned());
        apply(&mut state,rx.recv_timeout(Duration::from_secs(3)).unwrap()); assert_eq!(state.session.input,99);
        local.send(Local::Changed(notify::Event{kind:notify::EventKind::Create(notify::event::CreateKind::File),paths:vec![new.clone()],attrs:Default::default()})).unwrap();
        apply(&mut state,rx.recv_timeout(Duration::from_secs(3)).unwrap());
        assert_eq!(state.session_path.as_deref(),new.to_str()); assert_eq!(state.session.input,0); assert!(state.read_session);
        assert_eq!(state.label,"Probe");
        local.send(Local::Session(state.session_path.clone())).unwrap();
        apply(&mut state,rx.recv_timeout(Duration::from_secs(3)).unwrap()); assert_eq!(state.session.input,7);
        server.join().unwrap(); drop(local); fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn context_window_prefers_runtime_model_cache() {
        let agent=std::env::temp_dir().join(format!("gs3-limit-{}",std::process::id())); let dir=agent.join("sessions/--x--");
        fs::create_dir_all(&dir).unwrap();
        let session=dir.join("s.jsonl");
        fs::write(&session,format!("{}\n",json!({"type":"message","message":{"role":"assistant","model":"m","usage":{"totalTokens":10}}}))).unwrap();
        // Pi registers cpamc models from the cache; the store is only a fallback.
        fs::write(agent.join("cpamc-models-cache.json"),json!([{"id":"m","contextWindow":272000}]).to_string()).unwrap();
        fs::write(agent.join("models-store.json"),json!({"cpamc":{"models":[{"id":"m","contextWindow":370000}]}}).to_string()).unwrap();
        let mut t=Tail::default(); read_tail(&session,&mut t).unwrap(); assert_eq!(t.session.limit,Some(272000));
        fs::remove_file(agent.join("cpamc-models-cache.json")).unwrap();
        let mut t=Tail::default(); read_tail(&session,&mut t).unwrap(); assert_eq!(t.session.limit,Some(370000));
        fs::remove_dir_all(&agent).unwrap();
    }
    #[test]
    fn git_snapshot_and_detail_read_a_real_repo() {
        let dir=std::env::temp_dir().join(format!("gs3-git-{}",std::process::id())); let _=fs::remove_dir_all(&dir); fs::create_dir_all(dir.join("sub")).unwrap();
        let sh=|c:&str|{let o=Command::new("/bin/sh").args(["-c",c]).current_dir(&dir).output().unwrap(); assert!(o.status.success(),"{c}: {}",String::from_utf8_lossy(&o.stderr));};
        sh("git init -q -b main && git config user.email t@t && git config user.name t && printf 'a\\nb\\n' > f.txt && git add f.txt && git commit -qm base");
        sh("git checkout -qb feat && printf 'x\\n' > g.txt && git add g.txt && git commit -qm side && git checkout -q main");
        sh("printf 'a\\nc\\n' > f.txt && printf 'new\\n' > sub/n.txt && printf 'S=1\\n' > .env && ln -s /etc/hostname link");
        let g=git_snapshot(&dir.join("sub"));
        assert_eq!(g.error,None); assert_eq!((g.counts.modified,g.counts.untracked),(1,3)); assert_eq!((g.added,g.deleted),(1,1));
        assert!(g.files.iter().any(|f|f.path=="sub/n.txt"),"root-relative paths from a subdirectory: {:?}",g.files);
        assert!(g.graph[0].contains("(HEAD -> main)") && g.commits.len()==1 && g.commits[0].ends_with(" base"),"{:?}",g.graph);
        assert_eq!(g.branches.iter().map(|b|(b.name.as_str(),b.current)).collect::<Vec<_>>(),[("main",true),("feat",false)]);
        let req=|b:Option<&str>,f:Option<&str>,u|DetailRequest{branch:b.map(Into::into),file:f.map(Into::into),untracked:u};
        let d=detail(&dir.join("sub"),&req(None,Some("f.txt"),false)); assert!(d.diff.iter().any(|l|l=="-b") && d.diff.iter().any(|l|l=="+c"),"{:?}",d.diff);
        assert_eq!(detail(&dir,&req(None,Some("sub/n.txt"),true)).diff,["@@ archivo nuevo @@","+new"]);
        assert!(detail(&dir,&req(None,Some(".env"),true)).diff.is_empty(),"secrets are never read");
        assert_eq!(detail(&dir,&req(None,Some("link"),true)).diff,["enlace simbólico (no se sigue)"]);
        let d=detail(&dir,&req(Some("feat"),None,false)); assert_eq!((d.files.len(),d.added,d.error.clone()),(1,1,None)); assert_eq!(d.files[0].path,"g.txt");
        let d=detail(&dir,&req(Some("feat"),Some("g.txt"),false)); assert!(d.diff.contains(&"+x".to_string()));
        assert!(detail(&dir,&req(Some("--output=/tmp/x"),None,false)).error.is_some(),"branch names are never options");
        fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn tree_scan_lists_dirs_first_and_only_opens_requested_dirs() {
        let dir=std::env::temp_dir().join(format!("gs3-tree-{}",std::process::id())); let _=fs::remove_dir_all(&dir);
        for d in ["src/inner","z/node_modules/pkg",".git","target/release"] {fs::create_dir_all(dir.join(d)).unwrap();}
        for f in ["b.txt","A.md","src/m.rs","src/inner/deep.rs","z/y.rs","target/release/bin"] {fs::write(dir.join(f),"x").unwrap();}
        std::os::unix::fs::symlink(dir.join("src"),dir.join("link")).unwrap();
        let t=tree(&dir,vec!["src".into(),"src/inner".into()]);
        let names=|n:&[TreeNode]|n.iter().map(|n|(n.name.clone(),n.dir,n.count)).collect::<Vec<_>>();
        assert_eq!(names(&t.nodes),[("src".into(),true,2),("z".into(),true,1),("A.md".into(),false,0),("b.txt".into(),false,0),("link".into(),false,0)]);
        assert_eq!(names(&t.nodes[0].children),[("inner".into(),true,1),("m.rs".into(),false,0)]);
        assert_eq!(t.nodes[0].children[0].children[0].rel,"src/inner/deep.rs"); assert!(t.nodes[1].children.is_empty(),"closed dirs are counted, not scanned");
        assert_eq!((t.project.as_str(),t.open.len(),t.error.is_none()),(dir.file_name().unwrap().to_str().unwrap(),2,true));
        // A child opened under a closed parent is not scanned; a missing root is an error.
        assert!(tree(&dir,vec!["src/inner".into()]).nodes[0].children.is_empty());
        assert!(tree(&dir.join("missing"),vec![]).error.is_some());
        fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn engram_connection_is_read_only() {
        let path=std::env::temp_dir().join(format!("gs3-hud-readonly-{}.db",std::process::id()));
        {let db=Connection::open(&path).unwrap(); db.execute_batch("CREATE TABLE observations(project TEXT, deleted_at TEXT); INSERT INTO observations VALUES('test',NULL);").unwrap();}
        let db=open_engram(&path).unwrap();
        assert_eq!(db.query_row("SELECT count(*) FROM observations",[],|r|r.get::<_,i64>(0)).unwrap(),1);
        assert!(db.execute("INSERT INTO observations VALUES('bad',NULL)",[]).is_err());
        drop(db); fs::remove_file(path).unwrap();
    }
}
