//! Flameshot launcher: an explicit click on the flame icon opens its interactive area selector. No output is read.
use super::*;
/// Flameshot is installed as a Flatpak; `gui` starts the interactive capture (or asks the running daemon to).
fn flameshot_command()->Command {
    let mut cmd=Command::new("flatpak");
    cmd.args(["run","--branch=stable","--arch=x86_64","--command=flameshot","org.flameshot.Flameshot","gui"]);
    cmd
}
/// Clicks are coalesced and at least 1 s apart, so a double click never opens two selectors.
pub fn screenshot()->Sender<()> {
    let (send,rx)=mpsc::channel::<()>();
    thread::spawn(move||{
        let mut last:Option<Instant>=None;
        while rx.recv().is_ok() {
            while rx.try_recv().is_ok() {}
            if last.is_some_and(|t|t.elapsed()<Duration::from_secs(1)) {continue}
            last=Some(Instant::now());
            let mut cmd=flameshot_command();
            if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {if let Some(dir)=std::env::var_os("XDG_RUNTIME_DIR") {cmd.env("DBUS_SESSION_BUS_ADDRESS",format!("unix:path={}/bus",PathBuf::from(dir).display()));}}
            use std::os::unix::process::CommandExt;
            if let Ok(mut child)=cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn() { // screenshot-spawn
                thread::spawn(move||{let _=child.wait();});
            }
        }
    });
    send
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opens_the_flameshot_gui() {
        let cmd=flameshot_command();
        assert_eq!(cmd.get_program(),"flatpak");
        assert_eq!(cmd.get_args().last().unwrap(),"gui");
    }
}
