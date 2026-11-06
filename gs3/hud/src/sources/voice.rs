//! Local Discord RPC voice controls for Vesktop only. No credential or RPC error payload reaches the UI.
use super::*;
use std::net::Shutdown;
const CLIENT_ID:&str="207646673902501888";
const MAX_FRAME:usize=1024*1024;
fn encode(op:u32,body:&[u8])->Vec<u8> { [op.to_le_bytes().as_slice(),(body.len() as u32).to_le_bytes().as_slice(),body].concat() }
fn decode(r:&mut impl Read)->Result<(u32,Vec<u8>)> {
    let mut h=[0;8]; r.read_exact(&mut h)?;
    let op=u32::from_le_bytes(h[..4].try_into()?); let n=u32::from_le_bytes(h[4..].try_into()?) as usize;
    if n>MAX_FRAME {return Err(anyhow!("RPC frame too large"))}
    let mut b=vec![0;n]; r.read_exact(&mut b)?; Ok((op,b))
}
fn write_json(s:&mut UnixStream,op:u32,v:&Value)->Result<()> {s.write_all(&encode(op,&serde_json::to_vec(v)?))?; Ok(())}
fn parse_voice(v:&Value)->Option<Voice> {
    if !(text(v,"cmd")=="GET_VOICE_SETTINGS" || text(v,"cmd")=="SET_VOICE_SETTINGS" || (text(v,"cmd")=="DISPATCH" && text(v,"evt")=="VOICE_SETTINGS_UPDATE")) || text(v,"evt")=="ERROR" {return None}
    Some(Voice{online:true,mute:v["data"]["mute"].as_bool()?,deaf:v["data"]["deaf"].as_bool()?})
}
fn candidates(runtime:&Path)->Vec<PathBuf> {
    (0..10).map(|i|runtime.join(format!("discord-ipc-{i}"))).collect()
}
struct Rpc { stream:UnixStream, nonce:u64 }
impl Rpc {
    fn next(&mut self)->Result<Value> {
        loop {let (op,b)=decode(&mut self.stream)?; match op {1=>return Ok(serde_json::from_slice(&b)?),3=>self.stream.write_all(&encode(4,&b))?,_=>return Err(anyhow!("RPC closed"))}}
    }
    fn command(&mut self,cmd:&str,args:Value,evt:Option<&str>,timeout:Duration)->Result<Value> {
        self.nonce+=1; let nonce=self.nonce.to_string(); let mut v=json!({"cmd":cmd,"args":args,"nonce":nonce});
        if let Some(evt)=evt {v["evt"]=evt.into();}
        write_json(&mut self.stream,1,&v)?; let deadline=Instant::now()+timeout;
        loop {
            let remaining=deadline.saturating_duration_since(Instant::now()); if remaining.is_zero() {return Err(anyhow!("RPC timeout"))}
            self.stream.set_read_timeout(Some(remaining))?;
            let v=self.next()?; if v["nonce"].as_str()==Some(&nonce) {
                if text(&v,"evt")=="ERROR" {return Err(anyhow!("RPC rejected"))} return Ok(v)
            }
        }
    }
}
fn save_token(path:&Path,token:&str)->Result<()> {
    let tmp=path.with_extension(format!("json.{}.tmp",std::process::id()));
    let mut file=std::os::unix::fs::OpenOptionsExt::mode(fs::OpenOptions::new().write(true).create_new(true),0o600).open(&tmp)?; // voice-token-save
    file.write_all(&serde_json::to_vec(&json!({"access_token":token}))?)?; file.sync_all()?; drop(file);
    fs::rename(&tmp,path)?; // voice-token-rename
    Ok(())
}
fn connect(socket:&Path,token_path:&Path,authorize:bool)->Result<(Rpc,Voice)> {
    let stream=UnixStream::connect(socket)?; stream.set_read_timeout(Some(Duration::from_millis(700)))?; stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut rpc=Rpc{stream,nonce:0};
    write_json(&mut rpc.stream,0,&json!({"v":1,"client_id":CLIENT_ID}))?;
    if text(&rpc.next()?,"evt")!="READY" {return Err(anyhow!("RPC not ready"))}
    let token=fs::read(token_path).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).and_then(|v|v["access_token"].as_str().map(str::to_owned));
    let timeout=Duration::from_millis(700);
    let authenticated=token.is_some_and(|token|rpc.command("AUTHENTICATE",json!({"access_token":token}),None,timeout).is_ok());
    if !authenticated {
        if !authorize {return Err(anyhow!("RPC needs consent"))}
        let answer=rpc.command("AUTHORIZE",json!({"client_id":CLIENT_ID,"scopes":["rpc","rpc.voice.read","rpc.voice.write"]}),None,Duration::from_secs(120))?;
        let code=answer["data"]["code"].as_str().ok_or_else(||anyhow!("RPC missing code"))?;
        let body=json!({"code":code}).to_string();
        let out=run(&std::env::temp_dir(),"curl",&["--silent","--fail","--max-time","15","--user-agent","gs3-hud/1.0","--header","Content-Type: application/json","--data-binary",&body,"https://streamkit.discord.com/overlay/token"],17).map_err(|_|anyhow!("RPC exchange failed"))?;
        let v:Value=serde_json::from_str(&out).map_err(|_|anyhow!("RPC invalid exchange"))?;
        let token=v["access_token"].as_str().ok_or_else(||anyhow!("RPC missing credential"))?;
        save_token(token_path,token)?;
        rpc.command("AUTHENTICATE",json!({"access_token":token}),None,timeout)?;
    }
    let answer=rpc.command("GET_VOICE_SETTINGS",json!({}),None,timeout)?;
    let voice=parse_voice(&answer).ok_or_else(||anyhow!("RPC invalid settings"))?;
    rpc.command("SUBSCRIBE",json!({}),Some("VOICE_SETTINGS_UPDATE"),timeout)?;
    Ok((rpc,voice))
}
/// Listening-socket inode for `path` in a `/proc/net/unix` table.
fn socket_inode(table:&str,path:&Path)->Option<String> {
    let path=path.to_str()?; table.lines().find(|l|l.split_whitespace().nth(7)==Some(path)).and_then(|l|l.split_whitespace().nth(6)).map(str::to_owned)
}
fn is_vesktop(comm:&str)->bool {comm=="vesktop"||comm=="vesktop.bin"}
/// Only a socket owned by a Vesktop process is accepted (official Discord is not supported).
fn vesktop_socket(path:&Path)->bool {
    let Some(inode)=socket_inode(&fs::read_to_string("/proc/net/unix").unwrap_or_default(),path).map(|i|format!("socket:[{i}]")) else {return false};
    fs::read_dir("/proc").into_iter().flatten().flatten()
        .filter(|e|e.file_name().to_string_lossy().chars().all(|c|c.is_ascii_digit()))
        .filter(|e|fs::read_to_string(e.path().join("comm")).is_ok_and(|c|is_vesktop(c.trim())))
        .any(|e|fs::read_dir(e.path().join("fd")).into_iter().flatten().flatten().any(|fd|fs::read_link(fd.path()).is_ok_and(|l|l.as_os_str()==inode.as_str())))
}
/// The gear runs the `vesktop` launcher at once: a second Electron instance only focuses the running window.
fn vesktop_command()->Command {
    let on_path=std::env::var_os("PATH").and_then(|p|std::env::split_paths(&p).map(|d|d.join("vesktop")).find(|p|p.is_file()));
    Command::new(on_path.unwrap_or_else(||dirs::home_dir().unwrap_or_default().join(".local/bin/vesktop")))
}
fn launch_vesktop() {
    let mut cmd=vesktop_command();
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {if let Some(dir)=std::env::var_os("XDG_RUNTIME_DIR") {cmd.env("DBUS_SESSION_BUS_ADDRESS",format!("unix:path={}/bus",PathBuf::from(dir).display()));}}
    use std::os::unix::process::CommandExt;
    if let Ok(mut child)=cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn() { // voice-gear-spawn
        thread::spawn(move||{let _=child.wait();});
    }
}
/// Like Discord's own mic button: while deafened it undeafens (Vesktop ignores a bare mute then).
fn voice_args(cmd:VoiceCmd,state:Voice)->Value {
    match cmd {VoiceCmd::Mute if state.deaf=>json!({"mute":false,"deaf":false}), VoiceCmd::Mute=>json!({"mute":!state.mute}), _=>json!({"deaf":!state.deaf})}
}
enum Message { Cmd(VoiceCmd), Frame(u64,Result<(u32,Vec<u8>)>) }
pub fn voice(tx:Sender<Event>)->Sender<VoiceCmd> {
    let (send,commands)=mpsc::channel(); let (events,rx)=mpsc::channel(); let bridge=events.clone();
    let (gear,gear_rx)=mpsc::channel::<()>();
    thread::spawn(move||{let mut last:Option<Instant>=None; while gear_rx.recv().is_ok() {while gear_rx.try_recv().is_ok() {} if last.is_none_or(|t|t.elapsed()>=Duration::from_secs(1)) {last=Some(Instant::now()); launch_vesktop();}}});
    thread::spawn(move||{for cmd in commands {if cmd==VoiceCmd::Open {let _=gear.send(());} if bridge.send(Message::Cmd(cmd)).is_err() {break}}});
    thread::spawn(move||{
        let Some(runtime)=std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from) else {return};
        let token=dirs::home_dir().unwrap_or_default().join(".config/gs3/discord-token.json");
        let mut consent=false; let mut generation=0; let mut sent=Voice::default();
        loop {
            let found=candidates(&runtime).into_iter().filter(|p|p.exists() && vesktop_socket(p)).find_map(|p|connect(&p,&token,consent).ok());
            consent=false;
            if let Some((mut rpc,mut state))=found {
                generation+=1; let id=generation; let events=events.clone();
                let Ok(mut reader)=rpc.stream.try_clone() else {return}; let _=reader.set_read_timeout(None);
                thread::spawn(move||loop {let result=decode(&mut reader); let failed=result.is_err(); if events.send(Message::Frame(id,result)).is_err()||failed {break}});
                if state!=sent {sent=state; if tx.send(Event::Voice(state)).is_err() {return}}
                let mut pending:Option<(String,Instant)>=None;
                loop {
                    let msg=match pending.as_ref() {Some((_,deadline))=>rx.recv_timeout(deadline.saturating_duration_since(Instant::now())),None=>rx.recv().map_err(|_|mpsc::RecvTimeoutError::Disconnected)};
                    match msg {
                        Ok(Message::Cmd(cmd)) if pending.is_none() && cmd!=VoiceCmd::Open => {
                            rpc.nonce+=1; let nonce=rpc.nonce.to_string(); let args=voice_args(cmd,state);
                            if write_json(&mut rpc.stream,1,&json!({"cmd":"SET_VOICE_SETTINGS","args":args,"nonce":nonce})).is_err() {break}
                            pending=Some((nonce,Instant::now()+Duration::from_secs(2)));
                        }
                        Ok(Message::Frame(g,result)) if g==id => {
                            let Ok((op,b))=result else {break};
                            if op==3 {if rpc.stream.write_all(&encode(4,&b)).is_err() {break} continue}
                            if op!=1 {break} let Ok(v)=serde_json::from_slice::<Value>(&b) else {break};
                            if let Some(next)=parse_voice(&v) {state=next; if state!=sent {sent=state; if tx.send(Event::Voice(state)).is_err() {return}}}
                            if pending.as_ref().is_some_and(|(n,_)|v["nonce"].as_str()==Some(n)) {if text(&v,"evt")=="ERROR" {break} pending=None;}
                        }
                        // A lost reply only frees the in-flight slot; the subscription still reports the truth.
                        Err(mpsc::RecvTimeoutError::Timeout)=>pending=None,
                        Err(mpsc::RecvTimeoutError::Disconnected)=>return,
                        _=>{}
                    }
                }
                let _=rpc.stream.shutdown(Shutdown::Both);
            }
            if sent.online {sent=Voice::default(); if tx.send(Event::Voice(sent)).is_err() {return}}
            let deadline=Instant::now()+Duration::from_secs(2); // reconnect backoff, never poll a connected client
            while Instant::now()<deadline {
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(Message::Cmd(cmd)) if cmd!=VoiceCmd::Open=>{consent=true; break},
                    Err(mpsc::RecvTimeoutError::Disconnected)=>return, Err(_)=>break, _=>{}
                }
            }
        }
    }); send
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    #[test]
    fn voice_frames_dispatch_and_candidates() {
        let v = json!({"hello":"🎙"});
        let bytes = encode(1, &serde_json::to_vec(&v).unwrap());
        assert_eq!(&bytes[..4], &1u32.to_le_bytes());
        let (op, body) = decode(&mut bytes.as_slice()).unwrap();
        assert_eq!(op, 1); assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(), v);
        assert!(decode(&mut [1u8,0].as_slice()).is_err());
        let mut huge = [1u32.to_le_bytes(), (MAX_FRAME as u32 + 1).to_le_bytes()].concat();
        assert!(decode(&mut huge.as_slice()).is_err()); huge.clear();
        assert_eq!(parse_voice(&json!({"cmd":"DISPATCH","evt":"VOICE_SETTINGS_UPDATE","data":{"mute":true,"deaf":false}})), Some(Voice{online:true,mute:true,deaf:false}));
        assert_eq!(parse_voice(&json!({"cmd":"DISPATCH","evt":"OTHER","data":{"mute":true,"deaf":false}})), None);
        assert_eq!(parse_voice(&json!({"cmd":"GET_VOICE_SETTINGS","data":{"mute":true}})), None);
        let c = candidates(Path::new("/run/test"));
        assert_eq!(c.len(),10); assert_eq!(c[0],Path::new("/run/test/discord-ipc-0")); assert_eq!(c[9],Path::new("/run/test/discord-ipc-9"));
        let table="Num RefCount Protocol Flags Type St Inode Path\n0: 00000002 00000000 00010000 0001 01 7880 /run/x/discord-ipc-0\n0: 00000002 00000000 00010000 0001 01 9911 /run/x/discord-ipc-1\n";
        assert_eq!(socket_inode(table,Path::new("/run/x/discord-ipc-1")).as_deref(),Some("9911"));
        assert_eq!(socket_inode(table,Path::new("/run/x/discord-ipc-2")),None);
        assert!(is_vesktop("vesktop") && is_vesktop("vesktop.bin") && !is_vesktop("Discord"));
        assert!(!vesktop_socket(Path::new("/run/test/discord-ipc-0")), "a socket nobody owns is not Vesktop");
        let v=|mute,deaf|Voice{online:true,mute,deaf};
        assert_eq!(voice_args(VoiceCmd::Mute,v(false,false)),json!({"mute":true}));
        assert_eq!(voice_args(VoiceCmd::Mute,v(true,false)),json!({"mute":false}));
        assert_eq!(voice_args(VoiceCmd::Mute,v(false,true)),json!({"mute":false,"deaf":false}));
        assert_eq!(voice_args(VoiceCmd::Deaf,v(false,true)),json!({"deaf":false}));
        assert_eq!(voice_args(VoiceCmd::Deaf,v(true,false)),json!({"deaf":true}));
        assert!(vesktop_command().get_program().to_string_lossy().ends_with("vesktop"));
    }
    #[test]
    fn voice_fake_rpc_round_trip() {
        let dir = std::env::temp_dir().join(format!("gs3-voice-{}-{}",std::process::id(),crate::core::now_secs()));
        fs::create_dir(&dir).unwrap(); let socket=dir.join("ipc"); let token=dir.join("test-token.json");
        save_token(&token,"fake-test-value").unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&token).unwrap().permissions().mode() & 0o777,0o600);
        let listener=UnixListener::bind(&socket).unwrap();
        let server=thread::spawn(move|| {
            let (mut s, _)=listener.accept().unwrap(); s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let (op, b)=decode(&mut s).unwrap(); assert_eq!(op,0);
            assert_eq!(serde_json::from_slice::<Value>(&b).unwrap(),json!({"v":1,"client_id":CLIENT_ID}));
            write_json(&mut s,1,&json!({"cmd":"DISPATCH","evt":"READY"})).unwrap();
            for cmd in ["AUTHENTICATE","GET_VOICE_SETTINGS","SUBSCRIBE","SET_VOICE_SETTINGS"] {
                let (_, b)=decode(&mut s).unwrap(); let v:Value=serde_json::from_slice(&b).unwrap(); assert_eq!(v["cmd"],cmd);
                match cmd {"AUTHENTICATE"=>assert_eq!(v["args"]["access_token"],"fake-test-value"), "SUBSCRIBE"=>assert_eq!(v["evt"],"VOICE_SETTINGS_UPDATE"), "SET_VOICE_SETTINGS"=>assert_eq!(v["args"],json!({"mute":true})), _=>{}}
                if cmd=="GET_VOICE_SETTINGS" {s.write_all(&encode(3,b"ping")).unwrap(); let (op,b)=decode(&mut s).unwrap(); assert_eq!((op,b),(4,b"ping".to_vec()));}
                write_json(&mut s,1,&json!({"cmd":cmd,"nonce":v["nonce"],"data":{"mute":cmd=="SET_VOICE_SETTINGS","deaf":false}})).unwrap();
            }
            write_json(&mut s,1,&json!({"cmd":"DISPATCH","evt":"VOICE_SETTINGS_UPDATE","data":{"mute":true,"deaf":true}})).unwrap();
        });
        let (mut rpc, initial)=connect(&socket,&token,false).unwrap(); assert_eq!(initial,Voice{online:true,mute:false,deaf:false});
        let answer=rpc.command("SET_VOICE_SETTINGS",json!({"mute":!initial.mute}),None,Duration::from_secs(2)).unwrap();
        assert_eq!(parse_voice(&answer),Some(Voice{online:true,mute:true,deaf:false}));
        assert_eq!(parse_voice(&rpc.next().unwrap()),Some(Voice{online:true,mute:true,deaf:true}));
        server.join().unwrap(); drop(rpc);
        fs::remove_dir_all(dir).unwrap();
    }
}
