//! `t init`: creating the list, offering an Obsidian vault for it, and remembering it in the
//! config file. See docs/designs/rust_port.md, "Configuration" (writing it), and **`init` never
//! overwrites**, **Where `init` creates the list** and **`init` asks only in a terminal** in
//! src/EGM.md.

use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::config::{self, Settings, Source, display};
use crate::lists::EMPTY_LIST;

const USAGE: &str = "usage: t init [<path> | --obsidian]";

/// The vault directories Obsidian knows about, most recently opened first, from its own list of
/// vaults (`obsidian.json`, in the macOS, Linux, Flatpak, Snap or Windows location). Vaults whose
/// folder is gone, or whose path contains a newline, are left out; an unreadable or invalid file
/// is skipped.
pub fn obsidian_vaults(home: Option<&Path>) -> Vec<PathBuf> {
    vaults_in(&vault_lists(home))
}

/// Where Obsidian keeps its list of vaults, for this home directory and environment.
fn vault_lists(home: Option<&Path>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(h) = home {
        files.push(h.join("Library/Application Support/obsidian/obsidian.json"));
        let xdg = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|x| !x.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| h.join(".config"));
        files.push(xdg.join("obsidian/obsidian.json"));
        files.push(h.join(".var/app/md.obsidian.Obsidian/config/obsidian/obsidian.json"));
        files.push(h.join("snap/obsidian/current/.config/obsidian/obsidian.json"));
    }
    if cfg!(windows)
        && let Some(appdata) = std::env::var_os("APPDATA").filter(|a| !a.is_empty())
    {
        files.push(
            PathBuf::from(appdata)
                .join("obsidian")
                .join("obsidian.json"),
        );
    }
    files
}

/// The vaults in these `obsidian.json` files, as `obsidian_vaults` describes.
fn vaults_in(files: &[PathBuf]) -> Vec<PathBuf> {
    let mut vaults: Vec<PathBuf> = Vec::new();
    for f in files {
        let Ok(bytes) = fs::read(f) else { continue };
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        let Some(map) = json.get("vaults").and_then(|v| v.as_object()) else {
            continue;
        };
        let mut found: Vec<(f64, PathBuf)> = map
            .values()
            .filter_map(|v| {
                let path = v.get("path")?.as_str()?;
                let ts = v.get("ts").and_then(|t| t.as_f64()).unwrap_or(0.0);
                (!path.contains('\n') && Path::new(path).is_dir())
                    .then(|| (ts, PathBuf::from(path)))
            })
            .collect();
        found.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, p) in found {
            if !vaults.contains(&p) {
                vaults.push(p);
            }
        }
    }
    vaults
}

/// Prints `question` and a space to `to`, then reads one line from stdin, without its line ending.
/// Only called when stdin is a terminal. End of input (Ctrl-D) prints a newline and gives up.
fn ask(question: &str, to: &mut dyn Write) -> Result<String, String> {
    let _ = write!(to, "{question} ");
    let _ = to.flush();
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => {
            let _ = writeln!(to);
            Err("no answer; nothing done".to_string())
        }
        Ok(_) => Ok(line.trim_end_matches(['\n', '\r']).to_string()),
    }
}

fn yes(answer: &str) -> bool {
    answer.is_empty() || answer.starts_with(['y', 'Y'])
}

/// Chooses an Obsidian vault for the list: its directory, or `None` to keep the list where it is.
/// With `offer` (plain `t init` in a terminal), not using a vault is one of the choices; otherwise
/// a vault must be chosen. Asks only when stdin is a terminal; otherwise a single vault is taken
/// and several are an error. Prompts go to stderr.
fn pick_vault(offer: bool, home: Option<&Path>, list: &Path) -> Result<Option<PathBuf>, String> {
    let vaults = obsidian_vaults(home);
    let terminal = io::stdin().is_terminal();
    let mut err = io::stderr();
    if vaults.is_empty() {
        if offer {
            return Ok(None);
        }
        return Err("found no Obsidian vaults (Obsidian lists a vault once you've opened it); use t init <path> instead".to_string());
    }
    if !terminal && vaults.len() > 1 {
        let listed: Vec<String> = vaults
            .iter()
            .map(|v| format!("  {}", v.display()))
            .collect();
        return Err(format!(
            "found {} Obsidian vaults; run t init --obsidian in a terminal to choose one, or t init <path>:\n{}",
            vaults.len(),
            listed.join("\n")
        ));
    }
    if let [only] = vaults.as_slice() {
        if offer {
            let answer = ask(
                &format!(
                    "Keep the list in your Obsidian vault, in {}? [Y/n]",
                    only.join("Todo").display()
                ),
                &mut err,
            )?;
            if !yes(&answer) {
                return Ok(None);
            }
        } else {
            let _ = writeln!(err, "Obsidian vault: {}", only.display());
        }
        return Ok(Some(only.clone()));
    }
    let _ = writeln!(err, "Obsidian vaults:");
    if offer {
        let _ = writeln!(err, "  0  not in a vault ({})", list.display());
    }
    for (i, v) in vaults.iter().enumerate() {
        let _ = writeln!(err, "  {}  {}", i + 1, v.display());
    }
    let answer = ask("Which one should hold the list? [1]", &mut err)?;
    let choice = if answer.is_empty() { "1" } else { &answer };
    let n: Option<usize> = choice
        .bytes()
        .all(|b| b.is_ascii_digit())
        .then(|| choice.parse().ok())
        .flatten();
    match n {
        Some(0) if offer => Ok(None),
        Some(n) if n >= 1 && n <= vaults.len() => Ok(Some(vaults[n - 1].clone())),
        _ => Err(format!("no choice '{answer}'; nothing done")),
    }
}

/// Makes `list` the list for every later run, from the terminal or an AI agent, with a
/// `TODO_FILE=` line in the config file. Asks first, and only in a terminal; otherwise it shows
/// the line. Prompt and messages go to stdout, as in 0.3.0.
fn remember_list(list: &Path, s: &Settings, warnings: &mut Vec<String>) -> Result<String, String> {
    let home = s.home.as_deref();
    let line = format!("TODO_FILE={}", display(list, home));
    let Some(config_file) = &s.config_file else {
        return Err(format!(
            "APPDATA isn't set, so there's no config file to save to; set TODO_FILE to {}",
            list.display()
        ));
    };
    let shown = display(config_file, home);
    let existing = match fs::read(config_file) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(_) => return Err(format!("could not read {shown}")),
    };
    if let Some(bytes) = &existing
        && config::todo_file_in(bytes, home).as_deref() == Some(list)
    {
        return Ok(format!("{shown} already points t at this list\n"));
    }
    if !io::stdin().is_terminal() {
        return Ok(format!("To use it, add this to {shown}:\n  {line}\n"));
    }
    let answer = ask(
        &format!(
            "Use this list from now on, also from AI agents? This adds TODO_FILE to {shown}. [Y/n]"
        ),
        &mut io::stdout(),
    )?;
    if !yes(&answer) {
        return Ok(format!(
            "Not saved. To use the list, add this to {shown}:\n  {line}\n"
        ));
    }
    let new = with_todo_file(existing.as_deref().unwrap_or_default(), &line);
    let write = || -> io::Result<()> {
        if let Some(dir) = config_file.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(config_file)?;
        f.write_all(&new)
    };
    write().map_err(|_| format!("could not update {shown}"))?;
    if let Some(v) = std::env::var_os("TODO_FILE").filter(|v| !v.is_empty())
        && Path::new(&v) != list
    {
        warnings.push(format!(
            "TODO_FILE is set in your environment to '{}', which still wins where it's set",
            v.to_string_lossy()
        ));
    }
    Ok(format!("Saved in {shown}: t now uses {}\n", list.display()))
}

/// A config file's bytes with `line` replacing its last `TODO_FILE=` line, or appended (after a
/// newline when the file doesn't end with one). Every other line is kept.
fn with_todo_file(bytes: &[u8], line: &str) -> Vec<u8> {
    let is_setting = |l: &[u8]| {
        let l = l.strip_suffix(b"\r").unwrap_or(l);
        let name: Vec<u8> = l
            .split(|&b| b == b'=')
            .next()
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|b| !matches!(b, b' ' | b'\t'))
            .collect();
        l.contains(&b'=') && name == b"TODO_FILE"
    };
    let mut lines: Vec<Vec<u8>> = bytes.split(|&b| b == b'\n').map(<[u8]>::to_vec).collect();
    let ended = lines.last().is_some_and(Vec::is_empty);
    if ended {
        lines.pop();
    }
    match lines.iter().rposition(|l| is_setting(l)) {
        Some(i) => {
            let cr = lines[i].ends_with(b"\r");
            lines[i] = line.as_bytes().to_vec();
            if cr {
                lines[i].push(b'\r');
            }
        }
        None => lines.push(line.as_bytes().to_vec()),
    }
    let mut out = lines.join(&b'\n');
    out.push(b'\n');
    out
}

/// `t init [<path> | --obsidian]`: creates the list with its three headings, never over anything
/// already there (the one exception: an existing list in a vault is used as it is). Returns what
/// to print on stdout; warnings go to `warnings`.
pub fn init(args: &[Vec<u8>], s: &Settings, warnings: &mut Vec<String>) -> Result<String, String> {
    let home = s.home.as_deref();
    let mut list = s.todo_file.clone();
    let mut vault = None;
    let mut in_vault = false;
    let path_given = match args {
        [] => {
            if s.todo_file_source == Source::Default
                && io::stdin().is_terminal()
                && io::stdout().is_terminal()
            {
                vault = pick_vault(true, home, &list)?;
                in_vault = vault.is_some();
            }
            false
        }
        [a] if a == b"--obsidian" => {
            vault = pick_vault(false, home, &list)?;
            in_vault = true;
            false
        }
        [a] if a.first() == Some(&b'-') => return Err(USAGE.to_string()),
        [a] => {
            list = config::absolute(&config::bytes_path(a));
            true
        }
        _ => return Err(USAGE.to_string()),
    };
    if let Some(v) = &vault {
        list = v.join("Todo").join("Todo.md");
    }
    let mut out = String::new();
    if list.exists() || list.is_symlink() {
        if !(in_vault && list.is_file()) {
            return Err(format!("{} already exists; not changed", list.display()));
        }
        out.push_str(&format!(
            "{} already exists; using it as it is\n",
            list.display()
        ));
    } else {
        let dir = list.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(dir)
            .map_err(|_| format!("could not create the directory {}", dir.display()))?;
        fs::write(&list, EMPTY_LIST).map_err(|_| format!("could not write {}", list.display()))?;
        out.push_str(&format!("Created {}\n", list.display()));
    }
    // What has been printed so far goes out before any question.
    print!("{out}");
    let _ = io::stdout().flush();
    out.clear();
    if in_vault {
        out.push_str(&remember_list(&list, s, warnings)?);
    } else if path_given && list != config::absolute(&s.todo_file) {
        let shown = s
            .config_file
            .as_deref()
            .map(|c| display(c, home))
            .unwrap_or_else(|| "your environment".to_string());
        out.push_str(&format!(
            "To use it, add this to {shown}:\n  TODO_FILE={}\n",
            display(&list, home)
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_file_line_replaced_or_appended() {
        let line = "TODO_FILE=~/new/Todo.md";
        assert_eq!(with_todo_file(b"", line), b"TODO_FILE=~/new/Todo.md\n");
        assert_eq!(
            with_todo_file(b"TODO_JIRA_KEYS=PROJ", line),
            b"TODO_JIRA_KEYS=PROJ\nTODO_FILE=~/new/Todo.md\n"
        );
        assert_eq!(
            with_todo_file(
                b"# mine\nTODO_FILE=/a\nTODO_JIRA_KEYS=P\n TODO_FILE = /b\n",
                line
            ),
            b"# mine\nTODO_FILE=/a\nTODO_JIRA_KEYS=P\nTODO_FILE=~/new/Todo.md\n"
        );
        assert_eq!(
            with_todo_file(b"TODO_FILE=/a\r\n", line),
            b"TODO_FILE=~/new/Todo.md\r\n"
        );
        assert_eq!(
            with_todo_file(b"# TODO_FILE is set below\n", line),
            b"# TODO_FILE is set below\nTODO_FILE=~/new/Todo.md\n"
        );
    }

    #[test]
    fn answers() {
        assert!(yes("") && yes("y") && yes("Yes") && yes("Y"));
        assert!(!yes("n") && !yes("no"));
    }

    #[test]
    fn vaults_newest_first_without_missing_folders() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let reg = home.join("Library/Application Support/obsidian");
        fs::create_dir_all(&reg).unwrap();
        for v in ["Work", "Notes"] {
            fs::create_dir_all(home.join("Vaults").join(v)).unwrap();
        }
        let json = format!(
            r#"{{"vaults":{{"a":{{"path":"{w}","ts":1}},"b":{{"path":"{n}","ts":2}},"c":{{"path":"{g}","ts":99}}}}}}"#,
            w = home.join("Vaults/Work").display(),
            n = home.join("Vaults/Notes").display(),
            g = home.join("gone").display()
        )
        .replace('\\', "\\\\");
        let file = reg.join("obsidian.json");
        fs::write(&file, json).unwrap();
        let files = [file.clone(), home.join("missing.json")];
        assert_eq!(
            vaults_in(&files),
            [home.join("Vaults/Notes"), home.join("Vaults/Work")]
        );
        fs::write(&file, "not json").unwrap();
        assert!(vaults_in(&files).is_empty());
        assert!(vault_lists(Some(home)).contains(&file));
    }
}
