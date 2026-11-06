//! The HUD's own settings (`--config`, e.g. `.gentle-shell/gs3-hud.json`): the ONLY file gs3-hud writes.
use crate::core::text;
use crate::ui::Tab;
use serde_json::{json, Value};
use std::{fs, path::Path};

pub const NAME_MAX: usize = 24;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config { pub name: String, pub hidden: Vec<Tab>, pub collapsed: Vec<String> }
/// Printable characters only, at most NAME_MAX, trimmed.
pub fn clean_name(s: &str) -> String { s.chars().filter(|c| !c.is_control()).take(NAME_MAX).collect::<String>().trim().to_string() }
impl Config {
    pub fn from_json(v: &Value) -> Self {
        let list = |k: &str| v[k].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>();
        // CONFIG itself can never be hidden: it is the only way back.
        let mut hidden: Vec<Tab> = list("hidden").iter().filter_map(|id| Tab::from_id(id)).filter(|t| *t != Tab::Config).collect();
        hidden.sort_by_key(|t| t.index()); hidden.dedup();
        let mut collapsed: Vec<String> = list("collapsed").into_iter().filter(|id| !id.is_empty() && id.len() <= 64).take(64).collect();
        collapsed.sort(); collapsed.dedup();
        Config { name: clean_name(text(v, "name")), hidden, collapsed }
    }
    pub fn to_json(&self) -> Value { json!({"name": self.name, "hidden": self.hidden.iter().map(|t| t.id()).collect::<Vec<_>>(), "collapsed": self.collapsed}) }
    pub fn visible(&self, t: Tab) -> bool { t == Tab::Config || !self.hidden.contains(&t) }
}
/// Missing or invalid file: defaults.
pub fn load(path: &Path) -> Config {
    fs::read(path).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).map(|v| Config::from_json(&v)).unwrap_or_default()
}
/// Atomic: write `<file>.tmp`, then rename over the file. Never creates directories.
pub fn save(path: &Path, c: &Config) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_vec_pretty(&c.to_json()).map_err(|e| e.to_string())?;
    fs::write(&tmp, body).and_then(|_| fs::rename(&tmp, path)).map_err(|e| { let _ = fs::remove_file(&tmp); e.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_is_atomic_and_load_sanitizes() {
        let dir = std::env::temp_dir().join(format!("gs3-config-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap(); let path = dir.join("gs3-hud.json");
        assert_eq!(load(&path), Config::default());
        let c = Config { name: "RamonsDev".into(), hidden: vec![Tab::Tree], collapsed: vec!["status".into()] };
        save(&path, &c).unwrap();
        assert_eq!(load(&path), c);
        let v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(v, json!({"name":"RamonsDev","hidden":["tree"],"collapsed":["status"]}));
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "no temp file left behind");
        fs::write(&path, "{not json").unwrap(); assert_eq!(load(&path), Config::default());
        let dirty = json!({"name":"a\u{7}b very long name that keeps going on","hidden":["config","git","nope","git"],"collapsed":["x","x",""]});
        assert_eq!(Config::from_json(&dirty), Config { name: "ab very long name that k".into(), hidden: vec![Tab::Git], collapsed: vec!["x".into()] });
        // A missing directory is an error, never created.
        assert!(save(&dir.join("missing/gs3-hud.json"), &c).is_err()); assert!(!dir.join("missing").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
