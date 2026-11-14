//! Finding programs on `PATH` and running fzf, for `t pick` and `t use` without a name; and finding
//! the item `t pick` chose again after fzf closes. See docs/designs/rust_port.md, "Platform
//! differences" (fzf, `EDITOR`), and **Picking** in bin/EGM.md.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::doc::Doc;

/// The file `name` runs when started without a path: the first match on `PATH`. On Windows a name
/// without an extension also matches with each extension in `PATHEXT`.
pub fn find_program(name: &Path) -> Option<PathBuf> {
    if name.components().count() > 1 || name.is_absolute() {
        return runnable(name).then(|| name.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    let extensions: Vec<OsString> = if cfg!(windows) && name.extension().is_none() {
        std::env::var_os("PATHEXT")
            .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into())
            .to_string_lossy()
            .split(';')
            .filter(|e| !e.is_empty())
            .map(OsString::from)
            .collect()
    } else {
        vec![OsString::new()]
    };
    for dir in std::env::split_paths(&path) {
        for ext in &extensions {
            let mut file = name.as_os_str().to_owned();
            file.push(ext);
            let candidate = dir.join(file);
            if runnable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(unix)]
fn runnable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(windows)]
fn runnable(p: &Path) -> bool {
    p.is_file()
}

/// fzf, found on `PATH`.
pub fn fzf() -> Option<PathBuf> {
    find_program(Path::new("fzf"))
}

/// What fzf returned: its exit status, as a shell reports it, and the chosen line.
pub struct Choice {
    pub status: i32,
    pub line: Vec<u8>,
}

/// Runs fzf with `args`, feeding it `lines`, one per line. fzf draws on the terminal itself.
pub fn run_fzf(fzf: &Path, args: &[&str], lines: &[Vec<u8>]) -> Result<Choice, String> {
    let mut child = Command::new(fzf)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|_| format!("could not run {}", fzf.display()))?;
    let mut input = Vec::new();
    for l in lines {
        input.extend_from_slice(l);
        input.push(b'\n');
    }
    if let Some(mut stdin) = child.stdin.take() {
        // fzf may exit before reading everything (Esc); that's not an error here.
        let _ = stdin.write_all(&input);
    }
    let out = child
        .wait_with_output()
        .map_err(|_| format!("could not run {}", fzf.display()))?;
    let mut line = out.stdout;
    while matches!(line.last(), Some(b'\n' | b'\r')) {
        line.pop();
    }
    Ok(Choice {
        status: status_code(&out.status),
        line,
    })
}

/// An exit status as a shell reports it: the code, or 128 plus the signal that ended it.
pub fn status_code(status: &std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return 128 + sig;
        }
    }
    status.code().unwrap_or(1)
}

/// What became of the item `t pick` chose, found by its exact stored text in the list as it is
/// after fzf closed.
#[derive(Debug, PartialEq, Eq)]
pub enum Found {
    /// This open item: the one at the chosen number if its text is unchanged, else the only open
    /// item with that text.
    Item(usize),
    /// No open item has the text any more.
    Gone,
    /// Several open items have the text, none of them at the chosen number.
    Ambiguous(usize),
}

pub fn find_again(doc: &Doc, n: usize, text: &[u8]) -> Found {
    let same: Vec<usize> = doc
        .open_items()
        .into_iter()
        .filter(|it| it.text == text)
        .map(|it| it.n)
        .collect();
    match same.as_slice() {
        _ if same.contains(&n) => Found::Item(n),
        [] => Found::Gone,
        [one] => Found::Item(*one),
        _ => Found::Ambiguous(same.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_again_by_text() {
        let doc = Doc::parse(b"## Today\n- [ ] same\n- [ ] B\n## Later\n- [ ] same\n- [ ] C\n");
        assert_eq!(find_again(&doc, 3, b"same"), Found::Item(3));
        assert_eq!(find_again(&doc, 2, b"C"), Found::Item(4), "numbers shifted");
        assert_eq!(find_again(&doc, 2, b"gone"), Found::Gone);
        assert_eq!(find_again(&doc, 2, b"same"), Found::Ambiguous(2));
    }

    #[test]
    fn programs_with_a_path_are_checked_as_they_are() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        assert_eq!(find_program(&missing), None);
    }
}
