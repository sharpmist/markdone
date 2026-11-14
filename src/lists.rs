//! The lists: every `.md` file in the folder of `TODO_FILE`, the default list. Which list a
//! command uses, and the commands that change the set of lists. See docs/designs/rust_port.md,
//! "List names", and **Which list**, **What a list is**, **Current list**, **Changing the lists**
//! and **Old archives** in bin/EGM.md.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{bytes_path, os_bytes};
use crate::lock::{self, Lock, What};
use crate::store::{self, Files};
use crate::text;

/// What a new list starts with.
pub const EMPTY_LIST: &[u8] = b"## Today\n\n## Later\n\n## Done\n";

/// One list: its name and files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    pub name: Vec<u8>,
    pub files: Files,
    pub lock: PathBuf,
}

impl List {
    pub fn file(&self) -> &Path {
        &self.files.file
    }

    /// The name as messages show it.
    pub fn shown(&self) -> String {
        lossy(&self.name)
    }
}

pub fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// The lists folder and the paths in it.
#[derive(Debug, Clone)]
pub struct Lists {
    /// The default list's file, `TODO_FILE`.
    pub default_file: PathBuf,
    /// The default list's name: its file name up to the last `.`.
    pub default_name: Vec<u8>,
    /// The lists folder.
    pub dir: PathBuf,
    /// The archives folder, `Archive/` in the lists folder.
    pub archive_dir: PathBuf,
    /// Holds the name of the current list (`t use`); none means the default list.
    pub current_file: PathBuf,
    /// Held while lists are added, removed or renamed.
    pub dir_lock_file: PathBuf,
}

/// The number of open items in a list file; 0 when it can't be read, as in 0.3.0.
pub fn open_count(file: &Path) -> usize {
    fs::read(file)
        .map(|b| {
            b.split(|&c| c == b'\n')
                .filter(|l| l.starts_with(b"- [ ] "))
                .count()
        })
        .unwrap_or(0)
}

/// "s" when `n` isn't 1.
fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Whether `name` has the form `Archive <digits>`, which is never a list.
fn is_old_archive_name(name: &[u8]) -> bool {
    name.strip_prefix(b"Archive ")
        .is_some_and(|y| !y.is_empty() && y.iter().all(u8::is_ascii_digit))
}

/// Whether `name` is a Windows device name, alone or followed by `.` and anything, in any case.
fn is_device_name(name: &[u8]) -> bool {
    let stem = name.split(|&b| b == b'.').next().unwrap_or(name);
    let stem = text::lower(stem);
    if matches!(stem.as_str(), "con" | "prn" | "aux" | "nul") {
        return true;
    }
    let mut chars = stem.chars();
    let prefix: String = chars.by_ref().take(3).collect();
    let rest: String = chars.collect();
    (prefix == "com" || prefix == "lpt")
        && (matches!(rest.as_str(), "¹" | "²" | "³")
            || (rest.len() == 1 && rest.as_bytes()[0].is_ascii_digit()))
}

/// Dies unless `name` can be a new list's name.
pub fn check_name(name: &[u8]) -> Result<(), String> {
    let bad = name.is_empty()
        || name
            .iter()
            .any(|&b| b < 0x20 || br#"/\:*?"<>|"#.contains(&b))
        || matches!(name[0], b'.' | b'@')
        || matches!(name.last(), Some(b'.' | b' '))
        || name.ends_with(b".md")
        || is_old_archive_name(name)
        || is_device_name(name);
    if bad {
        return Err(format!(
            r#"'{}' can't be a list name (no / \ : * ? " < > |, no control characters, no leading . or @, no trailing dot, space or .md, not a Windows device name like CON)"#,
            lossy(name)
        ));
    }
    Ok(())
}

/// The name of a file up to its last `.`, like zsh's `:t:r`.
fn stem(file_name: &[u8]) -> &[u8] {
    match file_name.iter().rposition(|&b| b == b'.') {
        Some(i) => &file_name[..i],
        None => file_name,
    }
}

/// Writes `bytes` to `path` through a temp file next to it and a rename, so readers never see half.
fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let r = fs::write(&tmp, bytes).and_then(|()| fs::rename(&tmp, path));
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r
}

impl Lists {
    pub fn new(default_file: &Path) -> Lists {
        let dir = match default_file.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let file_name = default_file.file_name().map(os_bytes).unwrap_or_default();
        Lists {
            default_file: default_file.to_path_buf(),
            default_name: stem(&file_name).to_vec(),
            archive_dir: dir.join("Archive"),
            current_file: dir.join(".current"),
            dir_lock_file: dir.join(".lists.lock"),
            dir,
        }
    }

    /// The list called `name` (which need not exist): the default list's file is `TODO_FILE`;
    /// another list's is `<lists folder>/<name>.md`. Its hidden files sit next to it, named after
    /// its file name: `.<file>.lock`, `.<file>.bak`, `.<file>.sum`.
    pub fn list(&self, name: &[u8]) -> List {
        let file = if name == self.default_name.as_slice() {
            self.default_file.clone()
        } else {
            self.dir.join(bytes_path(&[name, b".md"].concat()))
        };
        let parent = file.parent().map(Path::to_path_buf).unwrap_or_default();
        let hidden = |ext: &str| {
            let mut n = b".".to_vec();
            n.extend(os_bytes(file.file_name().unwrap_or_default()));
            n.extend_from_slice(ext.as_bytes());
            parent.join(bytes_path(&n))
        };
        List {
            name: name.to_vec(),
            lock: hidden(".lock"),
            files: Files {
                bak: hidden(".bak"),
                sum: hidden(".sum"),
                file,
            },
        }
    }

    /// The names of the lists, sorted by bytes: the regular files (not symlinks, not hidden)
    /// ending in `.md` directly in the lists folder, except `Archive <digits>`, plus the default
    /// list when its file exists (a symlink there counts).
    pub fn names(&self) -> Vec<Vec<u8>> {
        let mut names = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for e in entries.flatten() {
                let file_name = os_bytes(&e.file_name());
                let Some(name) = file_name.strip_suffix(b".md") else {
                    continue;
                };
                let regular = e.file_type().is_ok_and(|t| t.is_file());
                if regular && !file_name.starts_with(b".") && !is_old_archive_name(name) {
                    names.push(name.to_vec());
                }
            }
        }
        if self.default_file.is_file() {
            names.push(self.default_name.clone());
        }
        names.sort();
        names.dedup();
        names
    }

    /// The list `name` means: an exact match, else the only list whose name matches ignoring case.
    pub fn find(&self, name: &[u8]) -> Option<Vec<u8>> {
        let names = self.names();
        if names.iter().any(|n| n == name) {
            return Some(name.to_vec());
        }
        let want = text::lower(name);
        let hits: Vec<&Vec<u8>> = names.iter().filter(|n| text::lower(n) == want).collect();
        match hits.as_slice() {
            [one] => Some(one.to_vec()),
            _ => None,
        }
    }

    /// The name stored by `t use`, without trailing newlines; `None` when there is none.
    fn stored_current(&self) -> Option<Vec<u8>> {
        let mut c = fs::read(&self.current_file).ok()?;
        while matches!(c.last(), Some(b'\n' | b'\r')) {
            c.pop();
        }
        (!c.is_empty()).then_some(c)
    }

    /// The list a command uses when it names none: `TODO_LIST` (a name that matches no list is an
    /// error), else the one `t use` chose (a gone one falls back with a warning), else the default.
    pub fn current(
        &self,
        todo_list: Option<&OsStr>,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<u8>, String> {
        if let Some(v) = todo_list {
            let v = os_bytes(v);
            return self.find(&v).ok_or_else(|| {
                format!(
                    "TODO_LIST is '{}', but there's no such list; t lists shows them",
                    lossy(&v)
                )
            });
        }
        if let Some(c) = self.stored_current() {
            if let Some(n) = self.find(&c) {
                return Ok(n);
            }
            warnings.push(format!(
                "the current list '{}' is gone; using {} (t use picks another)",
                lossy(&c),
                lossy(&self.default_name)
            ));
        }
        Ok(self.default_name.clone())
    }

    /// Takes the lists folder's lock, for `t lists add|rm|rename` and `t use`.
    pub fn dir_lock(&self) -> Result<Lock, String> {
        if !self.dir.is_dir() {
            return Err(format!(
                "{} not found; create your first list with 't init'",
                self.dir.display()
            ));
        }
        lock::take(&self.dir_lock_file, What::Folder)
    }

    /// `t lists` lines: `<* or space> <name padded> <n> open`, sorted by name; with `tabbed`, each
    /// starts with the name and a tab (for Tab completion).
    pub fn lines(&self, current: &[u8], tabbed: bool) -> Vec<Vec<u8>> {
        let names = self.names();
        let width = names.iter().map(|n| text::char_count(n)).max().unwrap_or(0);
        names
            .iter()
            .map(|n| {
                let mut line = Vec::new();
                if tabbed {
                    line.extend_from_slice(n);
                    line.push(b'\t');
                }
                line.push(if n == current { b'*' } else { b' ' });
                line.push(b' ');
                line.extend(text::pad(n, width));
                line.extend(format!(" {:>3} open", open_count(self.list(n).file())).into_bytes());
                line
            })
            .collect()
    }

    /// `t lists`: the lines above, or an error when there are no lists.
    pub fn show(&self, current: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        let lines = self.lines(current, false);
        if lines.is_empty() {
            return Err(format!(
                "no lists in {}; create one with 't init'",
                self.dir.display()
            ));
        }
        Ok(lines)
    }

    /// `t lists --json`: `[{"name":"Todo","open":3,"current":true}]`, one object per line.
    pub fn json(&self, current: &[u8]) -> String {
        let objects: Vec<String> = self
            .names()
            .iter()
            .map(|n| {
                format!(
                    r#"{{"name":{},"open":{},"current":{}}}"#,
                    text::json_string(n),
                    open_count(self.list(n).file()),
                    n == current
                )
            })
            .collect();
        if objects.is_empty() {
            "[]\n".to_string()
        } else {
            format!("[\n{}\n]\n", objects.join(",\n"))
        }
    }

    /// `t lists add <name>`: creates a list with the three headings. Never overwrites.
    pub fn add(&self, name: &[u8]) -> Result<String, String> {
        check_name(name)?;
        let _dir = self.dir_lock()?;
        if let Some(other) = self.find(name) {
            return Err(format!("there's already a list called {}", lossy(&other)));
        }
        let list = self.list(name);
        let f = list.file();
        if f.exists() || f.is_symlink() {
            return Err(format!("{} already exists; not changed", f.display()));
        }
        fs::write(f, EMPTY_LIST).map_err(|_| format!("could not write {}", f.display()))?;
        let q = lossy(&text::shell_quote(name));
        Ok(format!(
            "Created the list {}; use it with t @{q} … or t use {q}",
            list.shown()
        ))
    }

    /// `t lists rm [--force] <name>`: deletes a list's file and hidden files; its archives stay.
    /// Refuses the default list, the current list, and a list with open items unless `force`.
    pub fn remove(
        &self,
        name: &[u8],
        force: bool,
        todo_list: Option<&OsStr>,
    ) -> Result<String, String> {
        let _dir = self.dir_lock()?;
        let n = self.find(name).ok_or_else(|| no_such_list(name))?;
        let shown = lossy(&n);
        if n == self.default_name {
            return Err(format!(
                "{shown} is your default list (TODO_FILE); it can't be deleted"
            ));
        }
        if n == self.current(todo_list, &mut Vec::new())? {
            return Err(format!(
                "{shown} is the current list; switch with t use first"
            ));
        }
        let list = self.list(&n);
        let held = lock::take(&list.lock, What::List)?;
        let o = open_count(list.file());
        if o > 0 && !force {
            return Err(format!(
                "{shown} has {o} open item{}; finish or move them first, or use t lists rm --force {shown}",
                plural(o)
            ));
        }
        let rm = |p: &Path| match fs::remove_file(p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(()),
            _ => Ok(()),
        };
        let failed = || format!("could not delete {}", list.file().display());
        rm(list.file()).map_err(|()| failed())?;
        rm(&list.files.bak).map_err(|()| failed())?;
        rm(&list.files.sum).map_err(|()| failed())?;
        // Released before its file is deleted, which Windows needs.
        drop(held);
        rm(&list.lock).map_err(|()| failed())?;
        let with = if o > 0 {
            format!(" with its {o} open item{}", plural(o))
        } else {
            String::new()
        };
        Ok(format!(
            "Deleted the list {shown}{with}; its archives stay in {}",
            self.archive_dir.display()
        ))
    }

    /// `t lists rename <old> <new>`: renames a list, its hidden files and its archives, and keeps
    /// it current if `t use` chose it. The default list can't be renamed: `TODO_FILE` names it.
    pub fn rename(
        &self,
        old: &[u8],
        new: &[u8],
        warnings: &mut Vec<String>,
    ) -> Result<String, String> {
        check_name(new)?;
        let _dir = self.dir_lock()?;
        let n = self.find(old).ok_or_else(|| no_such_list(old))?;
        if n == self.default_name {
            return Err(format!(
                "{} is your default list, named by TODO_FILE; it can't be renamed",
                lossy(&n)
            ));
        }
        if let Some(other) = self.find(new)
            && text::lower(&other) != text::lower(&n)
        {
            return Err(format!("there's already a list called {}", lossy(&other)));
        }
        let was_current = self
            .stored_current()
            .is_some_and(|c| text::lower(&c) == text::lower(&n));
        let from = self.list(&n);
        let held = lock::take(&from.lock, What::List)?;
        let to = self.list(new);
        fs::rename(from.file(), to.file())
            .map_err(|_| format!("could not rename {}", from.file().display()))?;
        if from.files.bak.exists() {
            let _ = fs::rename(&from.files.bak, &to.files.bak);
        }
        if from.files.sum.exists() {
            let _ = fs::rename(&from.files.sum, &to.files.sum);
        }
        drop(held);
        let _ = fs::remove_file(&from.lock);
        for (path, year) in self.archives_of(&n) {
            let target = store::archive_file(&self.archive_dir, new, &year);
            if fs::rename(&path, &target).is_err() {
                warnings.push(format!("could not rename {}", path.display()));
            }
        }
        if was_current && write_atomically(&self.current_file, &[new, b"\n"].concat()).is_err() {
            warnings.push(format!("could not update {}", self.current_file.display()));
        }
        Ok(format!("Renamed the list {} to {}", lossy(&n), lossy(new)))
    }

    /// A list's archive files, `<list> <digits>.md` (regular files), with their years.
    fn archives_of(&self, name: &[u8]) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.archive_dir) {
            for e in entries.flatten() {
                let file_name = os_bytes(&e.file_name());
                let year = file_name
                    .strip_prefix(name)
                    .and_then(|r| r.strip_prefix(b" "))
                    .and_then(|r| r.strip_suffix(b".md"));
                if let Some(y) = year
                    && !y.is_empty()
                    && y.iter().all(u8::is_ascii_digit)
                    && e.file_type().is_ok_and(|t| t.is_file())
                {
                    out.push((e.path(), y.to_vec()));
                }
            }
        }
        out.sort();
        out
    }

    /// `t use <name>`: makes `<name>` the current list for every terminal and agent that doesn't
    /// name one; the default list clears the choice. Returns the message.
    pub fn use_list(
        &self,
        name: &[u8],
        todo_list: Option<&OsStr>,
        warnings: &mut Vec<String>,
    ) -> Result<String, String> {
        let _dir = self.dir_lock()?;
        let n = self.find(name).ok_or_else(|| no_such_list(name))?;
        if n == self.default_name {
            match fs::remove_file(&self.current_file) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    return Err(format!("could not update {}", self.current_file.display()));
                }
                _ => {}
            }
        } else {
            write_atomically(&self.current_file, &[n.as_slice(), b"\n"].concat())
                .map_err(|_| format!("could not write {}", self.current_file.display()))?;
        }
        if let Some(v) = todo_list {
            warnings.push(format!(
                "TODO_LIST is set to '{}', which still wins where it's set",
                v.to_string_lossy()
            ));
        }
        let list = self.list(&n);
        Ok(format!(
            "Now using {} ({} open)",
            list.shown(),
            open_count(list.file())
        ))
    }

    /// Archives from before there were several lists: `<lists folder>/Archive <year>.md`.
    pub fn old_archives(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for e in entries.flatten() {
                let file_name = os_bytes(&e.file_name());
                if let Some(name) = file_name.strip_suffix(b".md")
                    && is_old_archive_name(name)
                    && e.file_type().is_ok_and(|t| t.is_file())
                {
                    out.push((e.path(), name[8..].to_vec()));
                }
            }
        }
        out.sort();
        out
    }

    /// Moves old archives to `Archive/<default list> <year>.md`, appending their items (sorted)
    /// when that file exists. Call under the default list's lock. A failure warns and leaves the
    /// old file.
    pub fn migrate_archives(&self, warnings: &mut Vec<String>) {
        for (old, year) in self.old_archives() {
            let to = store::archive_file(&self.archive_dir, &self.default_name, &year);
            if fs::create_dir_all(&self.archive_dir).is_err() {
                warnings.push(format!(
                    "could not create {}; old archives left where they are",
                    self.archive_dir.display()
                ));
                return;
            }
            if to.exists() {
                let moved = fs::read(&old).ok().and_then(|bytes| {
                    let mut items: Vec<Vec<u8>> = bytes
                        .split(|&b| b == b'\n')
                        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
                        .filter(|l| l.starts_with(b"- [x] "))
                        .map(<[u8]>::to_vec)
                        .collect();
                    items.sort();
                    store::append_lines(&to, &items).ok()?;
                    fs::remove_file(&old).ok()
                });
                if moved.is_none() {
                    warnings.push(format!(
                        "could not move {} into {}",
                        old.display(),
                        to.display()
                    ));
                    return;
                }
            } else if fs::rename(&old, &to).is_err() {
                warnings.push(format!(
                    "could not move {} to {}",
                    old.display(),
                    to.display()
                ));
                return;
            }
        }
    }
}

fn no_such_list(name: &[u8]) -> String {
    format!("no list called '{}'; t lists shows them", lossy(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Folder {
        _dir: tempfile::TempDir,
        lists: Lists,
    }

    fn folder() -> Folder {
        let dir = tempfile::tempdir().unwrap();
        let todo = dir.path().join("Todo.md");
        fs::write(&todo, EMPTY_LIST).unwrap();
        let lists = Lists::new(&todo);
        Folder { _dir: dir, lists }
    }

    fn names(l: &Lists) -> Vec<String> {
        l.names().iter().map(|n| lossy(n)).collect()
    }

    fn show(l: &Lists) -> String {
        let cur = l.current(None, &mut Vec::new()).unwrap();
        l.show(&cur)
            .unwrap()
            .iter()
            .map(|b| lossy(b) + "\n")
            .collect()
    }

    #[test]
    fn hidden_files_are_named_after_the_file() {
        let f = folder();
        let l = f.lists.list(b"Todo");
        assert_eq!(l.file(), f.lists.dir.join("Todo.md"));
        assert_eq!(l.lock, f.lists.dir.join(".Todo.md.lock"));
        assert_eq!(l.files.bak, f.lists.dir.join(".Todo.md.bak"));
        assert_eq!(l.files.sum, f.lists.dir.join(".Todo.md.sum"));
        assert_eq!(f.lists.list(b"Work").file(), f.lists.dir.join("Work.md"));
        assert_eq!(f.lists.default_name, b"Todo");
    }

    #[test]
    fn what_a_list_is() {
        let f = folder();
        let d = &f.lists.dir;
        fs::write(d.join("Work.md"), b"").unwrap();
        fs::write(d.join(".hidden.md"), b"").unwrap();
        fs::write(d.join("Archive 2020.md"), b"").unwrap();
        fs::write(d.join("notes.txt"), b"").unwrap();
        fs::write(d.join("a.b.md"), b"").unwrap();
        fs::create_dir(d.join("folder.md")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(d.join("Work.md"), d.join("Link.md")).unwrap();
        assert_eq!(names(&f.lists), ["Todo", "Work", "a.b"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_default_list_counts() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.txt");
        fs::write(&real, EMPTY_LIST).unwrap();
        let todo = dir.path().join("Todo.md");
        std::os::unix::fs::symlink(&real, &todo).unwrap();
        assert_eq!(names(&Lists::new(&todo)), ["Todo"]);
    }

    #[test]
    fn find_exact_or_the_only_case_insensitive_match() {
        let f = folder();
        fs::write(f.lists.dir.join("Work.md"), b"").unwrap();
        assert_eq!(f.lists.find(b"Work"), Some(b"Work".to_vec()));
        assert_eq!(f.lists.find(b"work"), Some(b"Work".to_vec()));
        assert_eq!(f.lists.find(b"Nope"), None);
    }

    #[test]
    fn check_name_rules() {
        for good in [
            "Work",
            "Side projects",
            "a.b",
            "Café",
            "COM10",
            "Console",
            "x-y_z",
        ] {
            assert_eq!(check_name(good.as_bytes()), Ok(()), "{good:?}");
        }
        for bad in [
            "",
            "a/b",
            r"a\b",
            "a:b",
            "a*b",
            "a?b",
            "a\"b",
            "a<b",
            "a>b",
            "a|b",
            "a\tb",
            "a\nb",
            ".hidden",
            "@at",
            "x.md",
            "trailing.",
            "trailing ",
            "Archive 2020",
            "CON",
            "con",
            "Nul",
            "COM1",
            "lpt9",
            "COM0",
            "LPT0",
            "COM¹",
            "LPT³",
            "CON.x",
            "aux.txt",
        ] {
            assert!(check_name(bad.as_bytes()).is_err(), "{bad:?}");
        }
        assert_eq!(
            check_name(b"a/b").unwrap_err(),
            r#"'a/b' can't be a list name (no / \ : * ? " < > |, no control characters, no leading . or @, no trailing dot, space or .md, not a Windows device name like CON)"#
        );
    }

    #[test]
    fn lists_add_show_and_json() {
        let f = folder();
        assert_eq!(show(&f.lists), "* Todo   0 open\n");
        assert_eq!(
            f.lists.add(b"Work").unwrap(),
            "Created the list Work; use it with t @Work … or t use Work"
        );
        assert_eq!(fs::read(f.lists.dir.join("Work.md")).unwrap(), EMPTY_LIST);
        assert_eq!(
            f.lists.add(b"work").unwrap_err(),
            "there's already a list called Work"
        );
        assert_eq!(
            f.lists.add(b"Side projects").unwrap(),
            "Created the list Side projects; use it with t @'Side projects' … or t use 'Side projects'"
        );
        fs::write(
            f.lists.dir.join("Work.md"),
            b"## Today\n- [ ] a\r\n- [ ] b\n",
        )
        .unwrap();
        assert_eq!(
            show(&f.lists),
            "  Side projects   0 open\n* Todo            0 open\n  Work            2 open\n"
        );
        assert_eq!(
            f.lists.json(b"Work"),
            "[\n{\"name\":\"Side projects\",\"open\":0,\"current\":false},\n{\"name\":\"Todo\",\"open\":0,\"current\":false},\n{\"name\":\"Work\",\"open\":2,\"current\":true}\n]\n"
        );
        let tabbed = f.lists.lines(b"Todo", true);
        assert_eq!(lossy(&tabbed[1]), "Todo\t* Todo            0 open");
    }

    #[test]
    fn no_lists() {
        let dir = tempfile::tempdir().unwrap();
        let lists = Lists::new(&dir.path().join("Todo.md"));
        assert_eq!(lists.json(b"Todo"), "[]\n");
        assert_eq!(
            lists.show(b"Todo").unwrap_err(),
            format!(
                "no lists in {}; create one with 't init'",
                dir.path().display()
            )
        );
        let gone = Lists::new(&dir.path().join("missing").join("Todo.md"));
        assert_eq!(
            gone.add(b"Work").unwrap_err(),
            format!(
                "{} not found; create your first list with 't init'",
                dir.path().join("missing").display()
            )
        );
    }

    #[test]
    fn current_list() {
        let f = folder();
        f.lists.add(b"Work").unwrap();
        let mut w = Vec::new();
        assert_eq!(f.lists.current(None, &mut w).unwrap(), b"Todo");
        assert_eq!(
            f.lists.use_list(b"work", None, &mut w).unwrap(),
            "Now using Work (0 open)"
        );
        assert_eq!(fs::read(&f.lists.current_file).unwrap(), b"Work\n");
        assert_eq!(f.lists.current(None, &mut w).unwrap(), b"Work");
        assert_eq!(
            f.lists.current(Some(OsStr::new("todo")), &mut w).unwrap(),
            b"Todo"
        );
        assert_eq!(
            f.lists
                .current(Some(OsStr::new("Nope")), &mut w)
                .unwrap_err(),
            "TODO_LIST is 'Nope', but there's no such list; t lists shows them"
        );
        assert!(w.is_empty());
        f.lists
            .use_list(b"Work", Some(OsStr::new("Todo")), &mut w)
            .unwrap();
        assert_eq!(
            w,
            ["TODO_LIST is set to 'Todo', which still wins where it's set"]
        );
        assert_eq!(
            f.lists.use_list(b"Nope", None, &mut w).unwrap_err(),
            "no list called 'Nope'; t lists shows them"
        );
        // A vanished current list falls back to the default, with a warning.
        fs::remove_file(f.lists.dir.join("Work.md")).unwrap();
        let mut w = Vec::new();
        assert_eq!(f.lists.current(None, &mut w).unwrap(), b"Todo");
        assert_eq!(
            w,
            ["the current list 'Work' is gone; using Todo (t use picks another)"]
        );
        // Using the default list clears the choice.
        f.lists.add(b"Work").unwrap();
        f.lists.use_list(b"Todo", None, &mut w).unwrap();
        assert!(!f.lists.current_file.exists());
        // A CRLF ending in .current is dropped.
        fs::write(&f.lists.current_file, b"Work\r\n").unwrap();
        assert_eq!(f.lists.current(None, &mut w).unwrap(), b"Work");
    }

    #[test]
    fn rename_moves_files_archives_and_the_current_choice() {
        let f = folder();
        let d = f.lists.dir.clone();
        f.lists.add(b"Work").unwrap();
        fs::write(d.join(".Work.md.bak"), b"bak").unwrap();
        fs::write(d.join(".Work.md.sum"), b"sum").unwrap();
        fs::create_dir(d.join("Archive")).unwrap();
        fs::write(d.join("Archive").join("Work 2026.md"), b"a").unwrap();
        fs::write(d.join("Archive").join("Workshop 2026.md"), b"other").unwrap();
        f.lists.use_list(b"Work", None, &mut Vec::new()).unwrap();
        let mut w = Vec::new();
        assert_eq!(
            f.lists.rename(b"Work", b"Job", &mut w).unwrap(),
            "Renamed the list Work to Job"
        );
        assert!(w.is_empty());
        assert!(d.join("Job.md").is_file() && !d.join("Work.md").exists());
        assert_eq!(fs::read(d.join(".Job.md.bak")).unwrap(), b"bak");
        assert_eq!(fs::read(d.join(".Job.md.sum")).unwrap(), b"sum");
        assert!(!d.join(".Work.md.lock").exists());
        assert!(d.join("Archive").join("Job 2026.md").is_file());
        assert!(d.join("Archive").join("Workshop 2026.md").is_file());
        assert_eq!(fs::read(&f.lists.current_file).unwrap(), b"Job\n");
        assert!(f.lists.rename(b"Todo", b"Home", &mut w).is_err());
        f.lists.add(b"Side").unwrap();
        assert_eq!(
            f.lists.rename(b"Job", b"side", &mut w).unwrap_err(),
            "there's already a list called Side"
        );
        // Changing only the case is allowed.
        assert!(f.lists.rename(b"Side", b"side", &mut w).is_ok());
    }

    #[test]
    fn remove_refuses_default_current_and_open_items() {
        let f = folder();
        let d = f.lists.dir.clone();
        f.lists.add(b"Job").unwrap();
        fs::write(d.join("Job.md"), b"## Today\n- [ ] a\n- [ ] b\n- [ ] c\n").unwrap();
        fs::write(d.join(".Job.md.bak"), b"").unwrap();
        assert_eq!(
            f.lists.remove(b"Todo", false, None).unwrap_err(),
            "Todo is your default list (TODO_FILE); it can't be deleted"
        );
        f.lists.use_list(b"Job", None, &mut Vec::new()).unwrap();
        assert_eq!(
            f.lists.remove(b"Job", false, None).unwrap_err(),
            "Job is the current list; switch with t use first"
        );
        f.lists.use_list(b"Todo", None, &mut Vec::new()).unwrap();
        assert_eq!(
            f.lists.remove(b"Job", false, None).unwrap_err(),
            "Job has 3 open items; finish or move them first, or use t lists rm --force Job"
        );
        assert_eq!(
            f.lists.remove(b"job", true, None).unwrap(),
            format!(
                "Deleted the list Job with its 3 open items; its archives stay in {}",
                d.join("Archive").display()
            )
        );
        assert!(!d.join("Job.md").exists() && !d.join(".Job.md.bak").exists());
        assert!(!d.join(".Job.md.lock").exists());
        f.lists.add(b"Empty").unwrap();
        assert_eq!(
            f.lists.remove(b"Empty", false, None).unwrap(),
            format!(
                "Deleted the list Empty; its archives stay in {}",
                d.join("Archive").display()
            )
        );
        assert!(f.lists.remove(b"Nope", false, None).is_err());
        // A TODO_LIST naming no list is an error here too (0.3.0 ignored it).
        f.lists.add(b"Other").unwrap();
        assert_eq!(
            f.lists
                .remove(b"Other", false, Some(OsStr::new("Nope")))
                .unwrap_err(),
            "TODO_LIST is 'Nope', but there's no such list; t lists shows them"
        );
    }

    #[test]
    fn old_archives_move_to_the_archive_folder() {
        let f = folder();
        let d = f.lists.dir.clone();
        fs::write(
            d.join("Archive 2020.md"),
            b"# Archive 2020\n\n- [x] 2020-05-01 A\n",
        )
        .unwrap();
        fs::write(
            d.join("Archive 2021.md"),
            b"# Archive 2021\n\n- [x] 2021-06-01 B\n- [x] 2021-01-01 C\n",
        )
        .unwrap();
        fs::create_dir(d.join("Archive")).unwrap();
        fs::write(
            d.join("Archive").join("Todo 2021.md"),
            b"# Todo archive 2021\n\n- [x] 2021-03-01 D\n",
        )
        .unwrap();
        assert_eq!(f.lists.old_archives().len(), 2);
        let mut w = Vec::new();
        f.lists.migrate_archives(&mut w);
        assert!(w.is_empty(), "{w:?}");
        assert!(f.lists.old_archives().is_empty());
        assert_eq!(
            fs::read_to_string(d.join("Archive").join("Todo 2020.md")).unwrap(),
            "# Archive 2020\n\n- [x] 2020-05-01 A\n"
        );
        assert_eq!(
            fs::read_to_string(d.join("Archive").join("Todo 2021.md")).unwrap(),
            "# Todo archive 2021\n\n- [x] 2021-03-01 D\n- [x] 2021-01-01 C\n- [x] 2021-06-01 B\n"
        );
    }
}
