//! Settings: `TODO_FILE`, `TODO_LIST`, `TODO_JIRA_KEYS` and `TODO_JIRA_URL`, from the environment
//! or the config file; the home directory; paths as messages show them; today's date and the
//! archive cutoff. See docs/designs/rust_port.md, "Configuration", "Upgrading from 0.3" and
//! "Paths".

use std::ffi::OsString;
use std::path::{Component, MAIN_SEPARATOR, Path, PathBuf};

/// Where a setting's value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The environment variable, set and not empty.
    Env,
    /// The config file.
    File,
    /// Neither (or an empty value): the default.
    Default,
}

/// The settings a run uses.
#[derive(Debug)]
pub struct Settings {
    /// The default list; its folder holds all the lists.
    pub todo_file: PathBuf,
    pub todo_file_source: Source,
    /// The list to use instead of the current one; only from the environment, never empty.
    pub todo_list: Option<OsString>,
    /// Jira project keys, space-separated, as given (validated later by `check_jira`).
    pub jira_keys: Vec<u8>,
    /// The Jira browse URL, as given.
    pub jira_url: Vec<u8>,
    /// The config file's path, when there is a place for one.
    pub config_file: Option<PathBuf>,
    /// The home directory, when known.
    pub home: Option<PathBuf>,
    /// Warnings about the settings, without the `t: ` prefix, for the caller to print.
    pub warnings: Vec<String>,
}

/// The settings the config file may set. `TODO_LIST` is left out on purpose: it is a per-run
/// override, and in the file it would make `t use` useless.
const FILE_SETTINGS: [&str; 3] = ["TODO_FILE", "TODO_JIRA_KEYS", "TODO_JIRA_URL"];

/// One `NAME=value` line of the config file.
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    line: usize,
    name: &'static str,
    value: Vec<u8>,
}

fn trim_blanks(mut s: &[u8]) -> &[u8] {
    while let [b' ' | b'\t', rest @ ..] = s {
        s = rest;
    }
    while let [rest @ .., b' ' | b'\t'] = s {
        s = rest;
    }
    s
}

/// The setting lines of a config file, in order, and warnings about the other lines. `shown` is
/// the file's path as messages show it.
fn parse(bytes: &[u8], shown: &str) -> (Vec<Entry>, Vec<String>) {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    let (mut entries, mut warnings) = (Vec::new(), Vec::new());
    for (i, raw) in bytes.split(|&b| b == b'\n').enumerate() {
        let n = i + 1;
        let line = trim_blanks(raw.strip_suffix(b"\r").unwrap_or(raw));
        if line.is_empty() || line[0] == b'#' {
            continue;
        }
        let Some(eq) = line.iter().position(|&b| b == b'=') else {
            warnings.push(format!("{shown}:{n}: not a setting; ignored"));
            continue;
        };
        let name = trim_blanks(&line[..eq]);
        let value = trim_blanks(&line[eq + 1..]);
        if name == b"TODO_LIST" {
            warnings.push(format!(
                "{shown}:{n}: TODO_LIST can't be set here (use t use); ignored"
            ));
        } else if let Some(&name) = FILE_SETTINGS.iter().find(|s| s.as_bytes() == name) {
            if value.contains(&b'$') {
                warnings.push(format!(
                    "{shown}:{n}: values aren't expanded, so $ is taken as it is"
                ));
            }
            entries.push(Entry {
                line: n,
                name,
                value: value.to_vec(),
            });
        } else {
            warnings.push(format!("{shown}:{n}: not a setting; ignored"));
        }
    }
    (entries, warnings)
}

/// The bytes of an environment value or path: as they are on Unix, UTF-8 on Windows.
pub fn os_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        s.as_bytes().to_vec()
    }
    #[cfg(windows)]
    {
        s.to_string_lossy().into_owned().into_bytes()
    }
}

/// A path from bytes: as they are on Unix, UTF-8 on Windows.
pub fn bytes_path(b: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(b))
    }
    #[cfg(windows)]
    {
        PathBuf::from(String::from_utf8_lossy(b).into_owned())
    }
}

/// The rest of a `~/…` value (also `~\…` on Windows), or `None` when it doesn't start with one.
fn after_tilde(value: &[u8]) -> Option<&[u8]> {
    value
        .strip_prefix(b"~/")
        .or_else(|| cfg!(windows).then(|| value.strip_prefix(b"~\\")).flatten())
}

pub const NO_HOME: &str = "can't find your home directory";

/// Builds the settings from an environment lookup, the home directory and the config file (its
/// path and contents, when it exists). Fails only when the home directory is needed and unknown.
fn resolve(
    env: impl Fn(&str) -> Option<OsString>,
    home: Option<PathBuf>,
    config_file: Option<PathBuf>,
    config_bytes: Option<&[u8]>,
    mut warnings: Vec<String>,
) -> Result<Settings, String> {
    let shown_config = config_file
        .as_deref()
        .map(|p| display(p, home.as_deref()))
        .unwrap_or_default();
    let entries = match config_bytes {
        Some(bytes) => {
            let (entries, w) = parse(bytes, &shown_config);
            warnings.extend(w);
            entries
        }
        None => Vec::new(),
    };

    // TODO_FILE: the environment (empty means the default), else the last valid file line.
    let from_file = {
        let mut found = None;
        for e in entries.iter().filter(|e| e.name == "TODO_FILE") {
            if e.value.is_empty() {
                found = Some(None);
            } else if let Some(rest) = after_tilde(&e.value) {
                let home = home.as_ref().ok_or(NO_HOME)?;
                found = Some(Some(home.join(bytes_path(rest))));
            } else if bytes_path(&e.value).is_absolute() {
                found = Some(Some(bytes_path(&e.value)));
            } else {
                warnings.push(format!(
                    "{shown_config}:{}: TODO_FILE must be an absolute path or start with ~/; ignored",
                    e.line
                ));
            }
        }
        found
    };
    let (todo_file, todo_file_source) = match env("TODO_FILE") {
        Some(v) if !v.is_empty() => (PathBuf::from(v), Source::Env),
        Some(_) => (default_list(home.as_deref())?, Source::Default),
        None => match from_file {
            Some(Some(p)) => (p, Source::File),
            _ => (default_list(home.as_deref())?, Source::Default),
        },
    };

    // The Jira settings: the environment when set (empty means off), else the last file line.
    let setting = |name: &str| -> Vec<u8> {
        match env(name) {
            Some(v) => os_bytes(&v),
            None => entries
                .iter()
                .rfind(|e| e.name == name)
                .map(|e| e.value.clone())
                .unwrap_or_default(),
        }
    };
    let jira_keys = setting("TODO_JIRA_KEYS");
    let jira_url = setting("TODO_JIRA_URL");
    let todo_list = env("TODO_LIST").filter(|v| !v.is_empty());

    Ok(Settings {
        todo_file,
        todo_file_source,
        todo_list,
        jira_keys,
        jira_url,
        config_file,
        home,
        warnings,
    })
}

/// The default list, `~/todo/Todo.md`.
fn default_list(home: Option<&Path>) -> Result<PathBuf, String> {
    Ok(home.ok_or(NO_HOME)?.join("todo").join("Todo.md"))
}

/// Where the config file is, and a warning when there is no place for one.
fn config_path(
    env: &impl Fn(&str) -> Option<OsString>,
    home: Option<&Path>,
) -> (Option<PathBuf>, Option<String>) {
    #[cfg(unix)]
    {
        let base = match env("XDG_CONFIG_HOME") {
            Some(x) if !x.is_empty() => Some(PathBuf::from(x)),
            _ => home.map(|h| h.join(".config")),
        };
        (base.map(|b| b.join("markdone").join("config")), None)
    }
    #[cfg(windows)]
    {
        let _ = home;
        match env("APPDATA") {
            Some(a) if !a.is_empty() => {
                (Some(PathBuf::from(a).join("markdone").join("config")), None)
            }
            _ => (
                None,
                Some("APPDATA isn't set, so the config file isn't read".to_string()),
            ),
        }
    }
}

/// The settings of this run, from the real environment and config file.
pub fn load() -> Result<Settings, String> {
    let env = |name: &str| std::env::var_os(name);
    let home = std::env::home_dir().filter(|h| !h.as_os_str().is_empty());
    let (config_file, warning) = config_path(&env, home.as_deref());
    let bytes = match &config_file {
        Some(path) => match std::fs::read(path) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(format!("could not read {}", display(path, home.as_deref()))),
        },
        None => None,
    };
    resolve(
        env,
        home,
        config_file,
        bytes.as_deref(),
        warning.into_iter().collect(),
    )
}

/// The part of `path` under `home`, comparing components (without regard to case on Windows),
/// or `None` when it isn't under it.
pub fn under_home(path: &Path, home: &Path) -> Option<PathBuf> {
    let same = |a: Component, b: Component| {
        if cfg!(windows) {
            a.as_os_str().to_string_lossy().to_lowercase()
                == b.as_os_str().to_string_lossy().to_lowercase()
        } else {
            a == b
        }
    };
    let mut p = path.components();
    for h in home.components() {
        if !p.next().is_some_and(|c| same(c, h)) {
            return None;
        }
    }
    let rest: PathBuf = p.collect();
    (!rest.as_os_str().is_empty()).then_some(rest)
}

/// `path` as messages show it: `~` and the rest when it is under the home directory, else in full.
pub fn display(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|h| under_home(path, h)) {
        Some(rest) => format!("~{MAIN_SEPARATOR}{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// The marker 0.3's `t init` wrote above its `TODO_FILE` line in `.zshenv`.
const ZSHENV_MARK: &[u8] = b"# markdone: where t keeps the list (set by t init)";

/// The path in 0.3's marked `.zshenv` line, `export TODO_FILE="${TODO_FILE:-<value>}"`, unescaped
/// and with a leading `$HOME` as `~`; `None` without such a line.
fn marked_path(zshenv: &[u8]) -> Option<String> {
    let lines: Vec<&[u8]> = zshenv
        .split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
        .collect();
    let at = lines.windows(2).rposition(|w| w[0] == ZSHENV_MARK)?;
    let value = lines[at + 1]
        .strip_prefix(b"export TODO_FILE=\"${TODO_FILE:-")?
        .strip_suffix(b"}\"")?;
    let mut out = Vec::new();
    let mut i = 0;
    while i < value.len() {
        if value[i] == b'\\' && i + 1 < value.len() {
            i += 1;
        }
        out.push(value[i]);
        i += 1;
    }
    let mut path = String::from_utf8_lossy(&out).into_owned();
    if let Some(rest) = path.strip_prefix("$HOME") {
        path = format!("~{rest}");
    }
    Some(path)
}

/// The warning for people upgrading from 0.3 (macOS and Linux): when `TODO_FILE` comes from the
/// default but `${ZDOTDIR:-$HOME}/.zshenv` mentions `TODO_FILE`, which `t` no longer reads. No
/// value is taken from `.zshenv`.
pub fn upgrade_warning(s: &Settings) -> Option<String> {
    if cfg!(windows) || s.todo_file_source != Source::Default {
        return None;
    }
    let zdot = std::env::var_os("ZDOTDIR")
        .filter(|z| !z.is_empty())
        .map(PathBuf::from)
        .or_else(|| s.home.clone())?;
    let zshenv = zdot.join(".zshenv");
    let bytes = std::fs::read(&zshenv).ok()?;
    if !bytes.windows(9).any(|w| w == b"TODO_FILE") {
        return None;
    }
    let path = marked_path(&bytes).unwrap_or_else(|| "<path>".to_string());
    let config = s
        .config_file
        .as_deref()
        .map(|c| display(c, s.home.as_deref()))
        .unwrap_or_else(|| "the config file".to_string());
    Some(format!(
        "{} sets TODO_FILE, but t no longer reads it; put this line in {config}: TODO_FILE={path}",
        display(&zshenv, s.home.as_deref())
    ))
}

/// Today in the local time zone (which honours `TZ`, as `date +%F` does).
pub fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

/// A date as `YYYY-MM-DD`.
pub fn ymd(d: chrono::NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// The archive cutoff: the date 7 days before `today`, as `YYYY-MM-DD`. Done items dated before
/// it (comparing the strings) are archived.
pub fn cutoff(today: chrono::NaiveDate) -> String {
    ymd(today
        .checked_sub_days(chrono::Days::new(7))
        .unwrap_or(chrono::NaiveDate::MIN))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let map: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        move |k: &str| map.get(k).cloned()
    }

    fn home() -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(r"C:\Users\me")
        } else {
            PathBuf::from("/home/me")
        }
    }

    fn abs(p: &str) -> String {
        if cfg!(windows) {
            format!(r"C:{}", p.replace('/', "\\"))
        } else {
            p.to_string()
        }
    }

    fn cfg_file() -> PathBuf {
        home().join(".config").join("markdone").join("config")
    }

    fn settings(env: &[(&str, &str)], file: Option<&str>) -> Settings {
        resolve(
            env_of(env),
            Some(home()),
            Some(cfg_file()),
            file.map(str::as_bytes),
            Vec::new(),
        )
        .unwrap()
    }

    fn default_list() -> PathBuf {
        home().join("todo").join("Todo.md")
    }

    #[test]
    fn defaults() {
        let s = settings(&[], None);
        assert_eq!(s.todo_file, default_list());
        assert_eq!(s.todo_file_source, Source::Default);
        assert_eq!(s.todo_list, None);
        assert!(s.jira_keys.is_empty() && s.jira_url.is_empty());
        assert!(s.warnings.is_empty());
    }

    #[test]
    fn the_environment_wins_over_the_file() {
        let file = format!(
            "TODO_FILE={}\nTODO_JIRA_KEYS=FILE\nTODO_JIRA_URL=https://file/\n",
            abs("/notes/Todo.md")
        );
        let s = settings(
            &[
                ("TODO_FILE", &abs("/env/Todo.md")),
                ("TODO_JIRA_KEYS", "ENV"),
            ],
            Some(&file),
        );
        assert_eq!(s.todo_file, PathBuf::from(abs("/env/Todo.md")));
        assert_eq!(s.todo_file_source, Source::Env);
        assert_eq!(s.jira_keys, b"ENV");
        assert_eq!(s.jira_url, b"https://file/");
    }

    #[test]
    fn the_file_wins_over_the_default() {
        let s = settings(&[], Some(&format!("TODO_FILE={}\n", abs("/notes/Todo.md"))));
        assert_eq!(s.todo_file, PathBuf::from(abs("/notes/Todo.md")));
        assert_eq!(s.todo_file_source, Source::File);
    }

    #[test]
    fn empty_values_mean_the_default() {
        let file = format!("TODO_FILE={}\nTODO_JIRA_KEYS=FILE\n", abs("/notes/Todo.md"));
        // An empty TODO_FILE in the environment stops the lookup: the default, not the file.
        let s = settings(&[("TODO_FILE", ""), ("TODO_JIRA_KEYS", "")], Some(&file));
        assert_eq!(s.todo_file, default_list());
        assert_eq!(s.todo_file_source, Source::Default);
        // An empty TODO_JIRA_KEYS turns linking off for one run, even with keys in the file.
        assert!(s.jira_keys.is_empty());
        // An empty TODO_FILE line in the file means the default too.
        let s = settings(&[], Some("TODO_FILE=\n"));
        assert_eq!(s.todo_file_source, Source::Default);
        // An empty TODO_LIST is unset.
        assert_eq!(settings(&[("TODO_LIST", "")], None).todo_list, None);
        assert_eq!(
            settings(&[("TODO_LIST", "Work")], None).todo_list,
            Some(OsString::from("Work"))
        );
    }

    #[test]
    fn tilde_means_home() {
        let s = settings(&[], Some("TODO_FILE=~/notes/Todo.md"));
        assert_eq!(s.todo_file, home().join("notes").join("Todo.md"));
        if cfg!(windows) {
            let s = settings(&[], Some(r"TODO_FILE=~\notes\Todo.md"));
            assert_eq!(s.todo_file, home().join("notes").join("Todo.md"));
        }
    }

    #[test]
    fn format_details() {
        let file = format!(
            "\u{feff}# a comment\r\n\r\n   \t\r\n  # indented comment\n  TODO_JIRA_KEYS = PROJ OPS  \r\nTODO_JIRA_URL=https://a/=b\nTODO_FILE={}\nTODO_FILE={}\n",
            abs("/first/Todo.md"),
            abs("/second/Todo.md")
        );
        let s = settings(&[], Some(&file));
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
        assert_eq!(s.jira_keys, b"PROJ OPS");
        assert_eq!(
            s.jira_url, b"https://a/=b",
            "the value is everything after the first ="
        );
        assert_eq!(
            s.todo_file,
            PathBuf::from(abs("/second/Todo.md")),
            "the last line wins"
        );
    }

    #[test]
    fn warnings_name_the_file_and_line() {
        let shown = if cfg!(windows) {
            r"~\.config\markdone\config"
        } else {
            "~/.config/markdone/config"
        };
        let file = "no equals sign\nTODO_LIST=Work\nOTHER=1\nTODO_FILE=relative/Todo.md\nTODO_JIRA_URL=$JIRA\nexport TODO_FILE=x\n";
        let s = settings(&[], Some(file));
        assert_eq!(
            s.warnings,
            [
                format!("{shown}:1: not a setting; ignored"),
                format!("{shown}:2: TODO_LIST can't be set here (use t use); ignored"),
                format!("{shown}:3: not a setting; ignored"),
                format!("{shown}:5: values aren't expanded, so $ is taken as it is"),
                format!("{shown}:6: not a setting; ignored"),
                format!("{shown}:4: TODO_FILE must be an absolute path or start with ~/; ignored"),
            ]
        );
        assert_eq!(
            s.todo_file_source,
            Source::Default,
            "the relative line is ignored"
        );
        assert_eq!(s.todo_list, None, "TODO_LIST never comes from the file");
        assert_eq!(s.jira_url, b"$JIRA", "kept as it is");
    }

    #[test]
    fn an_ignored_line_leaves_the_earlier_value() {
        let file = format!("TODO_FILE={}\nTODO_FILE=relative\n", abs("/a/Todo.md"));
        assert_eq!(
            settings(&[], Some(&file)).todo_file,
            PathBuf::from(abs("/a/Todo.md"))
        );
    }

    #[test]
    fn no_home_is_an_error_only_when_needed() {
        let r = resolve(env_of(&[]), None, None, None, Vec::new());
        assert_eq!(r.unwrap_err(), NO_HOME);
        let r = resolve(
            env_of(&[("TODO_FILE", &abs("/x/Todo.md"))]),
            None,
            None,
            None,
            Vec::new(),
        );
        assert_eq!(r.unwrap().todo_file, PathBuf::from(abs("/x/Todo.md")));
    }

    #[cfg(unix)]
    #[test]
    fn config_file_location() {
        let h = home();
        let (p, w) = config_path(&env_of(&[]), Some(&h));
        assert_eq!(p, Some(h.join(".config/markdone/config")));
        assert_eq!(w, None);
        let (p, _) = config_path(&env_of(&[("XDG_CONFIG_HOME", "/xdg")]), Some(&h));
        assert_eq!(p, Some(PathBuf::from("/xdg/markdone/config")));
        let (p, _) = config_path(&env_of(&[("XDG_CONFIG_HOME", "")]), Some(&h));
        assert_eq!(p, Some(h.join(".config/markdone/config")));
        assert_eq!(config_path(&env_of(&[]), None).0, None);
    }

    #[cfg(windows)]
    #[test]
    fn config_file_location() {
        let h = home();
        let (p, w) = config_path(
            &env_of(&[("APPDATA", r"C:\Users\me\AppData\Roaming")]),
            Some(&h),
        );
        assert_eq!(
            p,
            Some(PathBuf::from(
                r"C:\Users\me\AppData\Roaming\markdone\config"
            ))
        );
        assert_eq!(w, None);
        let (p, w) = config_path(&env_of(&[]), Some(&h));
        assert_eq!(p, None);
        assert_eq!(
            w.as_deref(),
            Some("APPDATA isn't set, so the config file isn't read")
        );
    }

    #[test]
    fn display_uses_tilde_under_home() {
        let h = home();
        let sep = MAIN_SEPARATOR;
        assert_eq!(
            display(&h.join("notes").join("Todo.md"), Some(&h)),
            format!("~{sep}notes{sep}Todo.md")
        );
        // Not a string prefix: /home/meow is not under /home/me.
        let other = PathBuf::from(format!("{}ow", h.display())).join("x");
        assert_eq!(display(&other, Some(&h)), other.display().to_string());
        assert_eq!(display(&h, Some(&h)), h.display().to_string());
        assert_eq!(
            display(&h.join("x"), None),
            h.join("x").display().to_string()
        );
        if cfg!(windows) {
            let upper = PathBuf::from(r"c:\USERS\ME\notes");
            assert_eq!(display(&upper, Some(&h)), r"~\notes");
        }
    }

    #[test]
    fn the_path_in_0_3s_marked_zshenv_line() {
        let zshenv = b"export PATH=x\n\n# markdone: where t keeps the list (set by t init)\nexport TODO_FILE=\"${TODO_FILE:-$HOME/notes/My \\\"Todo\\\" \\$1.md}\"\n";
        assert_eq!(
            marked_path(zshenv).as_deref(),
            Some("~/notes/My \"Todo\" $1.md")
        );
        assert_eq!(
            marked_path(b"# markdone: where t keeps the list (set by t init)\r\nexport TODO_FILE=\"${TODO_FILE:-/srv/Todo.md}\"\r\n").as_deref(),
            Some("/srv/Todo.md")
        );
        assert_eq!(marked_path(b"export TODO_FILE=/x/Todo.md\n"), None);
    }

    #[test]
    fn dates() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 3, 5).unwrap();
        assert_eq!(ymd(d), "2026-03-05");
        assert_eq!(cutoff(d), "2026-02-26");
        let d = chrono::NaiveDate::from_ymd_opt(2026, 1, 3).unwrap();
        assert_eq!(cutoff(d), "2025-12-27");
    }

    #[cfg(unix)]
    #[test]
    fn bytes_round_trip_on_unix() {
        use std::os::unix::ffi::OsStrExt;
        let raw = b"/tmp/caf\xe9/Todo.md";
        let p = bytes_path(raw);
        assert_eq!(p.as_os_str().as_bytes(), raw);
        assert_eq!(os_bytes(p.as_os_str()), raw);
    }
}
