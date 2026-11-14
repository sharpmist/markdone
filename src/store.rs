//! Writing a list safely, undo, and the archives. See docs/designs/rust_port.md, "Saving" and
//! **Writes go through `save`**, **Undo** and **Archive before rewriting** in bin/EGM.md.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::config::bytes_path;

/// The hidden files that belong to a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    /// The list.
    pub file: PathBuf,
    /// The list as it was before `t`'s last write.
    pub bak: PathBuf,
    /// The checksum of what `t` last wrote.
    pub sum: PathBuf,
}

/// Writes `bytes` to `path` in place: opened with create, truncate and write, never replaced, so
/// a symlink stays one and permissions are kept. With `private`, a new file is created readable by
/// its owner only (Unix).
fn write_in_place(path: &Path, bytes: &[u8], private: bool) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut f = options.open(path)?;
    f.write_all(bytes)?;
    f.flush()
}

/// The POSIX `cksum` CRC of `bytes`: CRC-32 (polynomial 0x04C11DB7, not reflected) over the bytes
/// and then their length in as few bytes as it takes, least significant first, inverted.
pub fn cksum(bytes: &[u8]) -> u32 {
    fn step(mut crc: u32, b: u8) -> u32 {
        crc ^= (b as u32) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04C1_1DB7
            } else {
                crc << 1
            };
        }
        crc
    }
    let mut crc = bytes.iter().fold(0, |c, &b| step(c, b));
    let mut len = bytes.len();
    while len > 0 {
        crc = step(crc, (len & 0xff) as u8);
        len >>= 8;
    }
    !crc
}

/// The text `cksum` prints for `bytes` read from stdin, without the newline: `<crc> <length>`.
pub fn cksum_line(bytes: &[u8]) -> String {
    format!("{} {}", cksum(bytes), bytes.len())
}

/// Rewrites the list with `bytes`. First copies the current list to the backup (owner-only when
/// new); if that fails, nothing is written. Then writes the list in place, then the checksum, for
/// undo. Returns a warning when only the checksum couldn't be written.
pub fn save(files: &Files, bytes: &[u8]) -> Result<Option<String>, String> {
    let backup_failed = || {
        format!(
            "could not write the backup {}; list not changed",
            files.bak.display()
        )
    };
    let current = fs::read(&files.file).map_err(|_| backup_failed())?;
    write_in_place(&files.bak, &current, true).map_err(|_| backup_failed())?;
    write_in_place(&files.file, bytes, false).map_err(|_| {
        format!(
            "could not write {} (the previous version is in {})",
            files.file.display(),
            files.bak.display()
        )
    })?;
    let line = cksum_line(bytes) + "\n";
    if write_in_place(&files.sum, line.as_bytes(), true).is_err() {
        let _ = fs::remove_file(&files.sum);
        return Ok(Some(format!(
            "could not write {}; t undo won't undo this change",
            files.sum.display()
        )));
    }
    Ok(None)
}

/// Puts back the list as it was before `t`'s last write, through `save`, so a second undo redoes.
/// Refuses when the list was changed since then, since restoring would throw that change away.
pub fn undo(files: &Files) -> Result<Option<String>, String> {
    let bak = files.bak.display();
    if !files.bak.is_file() {
        return Err("nothing to undo".to_string());
    }
    let Ok(stored) = fs::read(&files.sum) else {
        return Err(format!(
            "can't tell whether the list was changed since t last wrote it; not undoing (t's previous version is in {bak})"
        ));
    };
    let now = fs::read(&files.file)
        .map_err(|_| format!("could not read {}; nothing undone", files.file.display()))?;
    if String::from_utf8_lossy(&stored).trim() != cksum_line(&now) {
        return Err(format!(
            "the list was changed since t last wrote it; not undoing (t's previous version is in {bak})"
        ));
    }
    let previous =
        fs::read(&files.bak).map_err(|_| format!("could not read {bak}; nothing undone"))?;
    save(files, &previous)
}

/// The archive file of a list for a year: `<archive folder>/<list> <year>.md`.
pub fn archive_file(adir: &Path, list: &[u8], year: &[u8]) -> PathBuf {
    adir.join(bytes_path(&[list, b" ", year, b".md"].concat()))
}

/// The year of an archived line, `- [x] YYYY-…`.
fn year_of(line: &[u8]) -> &[u8] {
    line.get(6..10).unwrap_or(&[])
}

/// Appends archived lines to the list's archives, one file per year of their date, each year's
/// lines sorted by bytes. A new archive file starts with `# <list> archive <year>` and a blank
/// line, with the list's line ending (`crlf`); an existing one keeps its own. Stops at the first
/// file that can't be written and returns it, so the caller can warn and leave the list as it
/// was (earlier years may then hold the lines twice after the next tidy; never none).
pub fn archive(adir: &Path, list: &[u8], lines: &[Vec<u8>], crlf: bool) -> Result<(), PathBuf> {
    let mut years: Vec<&[u8]> = lines.iter().map(|l| year_of(l)).collect();
    years.sort();
    years.dedup();
    for year in years {
        let af = archive_file(adir, list, year);
        let mut items: Vec<&Vec<u8>> = lines.iter().filter(|l| year_of(l) == year).collect();
        items.sort();
        let write = || -> io::Result<()> {
            let (exists, crlf) = match fs::read(&af) {
                Ok(existing) => (true, ending_of(&existing).unwrap_or(crlf)),
                Err(e) if e.kind() == io::ErrorKind::NotFound => (false, crlf),
                Err(e) => return Err(e),
            };
            let eol: &[u8] = if crlf { b"\r\n" } else { b"\n" };
            let mut out = Vec::new();
            if !exists {
                fs::create_dir_all(adir)?;
                out.extend_from_slice(&[b"# ", list, b" archive ", year].concat());
                out.extend_from_slice(eol);
                out.extend_from_slice(eol);
            }
            for l in &items {
                out.extend_from_slice(l);
                out.extend_from_slice(eol);
            }
            let mut f = OpenOptions::new().append(true).create(true).open(&af)?;
            f.write_all(&out)
        };
        write().map_err(|_| af.clone())?;
    }
    Ok(())
}

/// Whether a file's first line ends in CRLF; `None` when it has no newline.
fn ending_of(bytes: &[u8]) -> Option<bool> {
    let nl = bytes.iter().position(|&b| b == b'\n')?;
    Some(nl > 0 && bytes[nl - 1] == b'\r')
}

/// The `- [x] ` lines of a list's archives (`<list> <digits>.md`), file by file in byte order of
/// their names, for `t list --done --all`. Returns warnings for files that can't be read.
pub fn archived_items(adir: &Path, list: &[u8]) -> (Vec<Vec<u8>>, Vec<String>) {
    let mut names: Vec<(Vec<u8>, PathBuf)> = Vec::new();
    if let Ok(entries) = fs::read_dir(adir) {
        for e in entries.flatten() {
            let name = crate::config::os_bytes(&e.file_name());
            let year = name
                .strip_prefix(list)
                .and_then(|r| r.strip_prefix(b" "))
                .and_then(|r| r.strip_suffix(b".md"));
            if year.is_some_and(|y| !y.is_empty() && y.iter().all(u8::is_ascii_digit)) {
                names.push((name, e.path()));
            }
        }
    }
    names.sort();
    let (mut items, mut warnings) = (Vec::new(), Vec::new());
    for (_, path) in names {
        match fs::read(&path) {
            Ok(bytes) => {
                for line in bytes.split(|&b| b == b'\n') {
                    let line = line.strip_suffix(b"\r").unwrap_or(line);
                    if line.starts_with(b"- [x] ") {
                        items.push(line.to_vec());
                    }
                }
            }
            Err(_) => warnings.push(format!("could not read {}", path.display())),
        }
    }
    (items, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(dir: &Path) -> Files {
        Files {
            file: dir.join("Todo.md"),
            bak: dir.join(".Todo.md.bak"),
            sum: dir.join(".Todo.md.sum"),
        }
    }

    #[test]
    fn cksum_matches_the_cksum_command() {
        // Values printed by `cksum` (macOS and GNU agree).
        assert_eq!(cksum_line(b""), "4294967295 0");
        assert_eq!(cksum_line(b"hello\n"), "3015617425 6");
        assert_eq!(
            cksum_line(b"## Today\n- [ ] a\n\n## Done\n"),
            "302049768 26"
        );
        assert_eq!(cksum_line(&[b'x'; 70000]), "4215398528 70000");
    }

    #[cfg(unix)]
    #[test]
    fn cksum_matches_the_command_here_too() {
        use std::process::{Command, Stdio};
        let data: Vec<u8> = (0..100_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let mut child = Command::new("cksum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(&data).unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(
            String::from_utf8(out.stdout).unwrap().trim(),
            cksum_line(&data)
        );
    }

    #[test]
    fn save_backs_up_writes_and_records_the_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let f = files(dir.path());
        fs::write(&f.file, b"old\n").unwrap();
        assert_eq!(save(&f, b"new\n"), Ok(None));
        assert_eq!(fs::read(&f.file).unwrap(), b"new\n");
        assert_eq!(fs::read(&f.bak).unwrap(), b"old\n");
        assert_eq!(
            fs::read_to_string(&f.sum).unwrap(),
            cksum_line(b"new\n") + "\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn backup_and_checksum_are_private_and_the_list_keeps_its_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let f = files(dir.path());
        fs::write(&f.file, b"old\n").unwrap();
        fs::set_permissions(&f.file, fs::Permissions::from_mode(0o644)).unwrap();
        save(&f, b"new\n").unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&f.bak), 0o600);
        assert_eq!(mode(&f.sum), 0o600);
        assert_eq!(mode(&f.file), 0o644);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_list_stays_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.md");
        fs::write(&real, b"old\n").unwrap();
        let f = files(dir.path());
        std::os::unix::fs::symlink(&real, &f.file).unwrap();
        save(&f, b"new\n").unwrap();
        assert!(
            fs::symlink_metadata(&f.file)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&real).unwrap(), b"new\n");
    }

    #[test]
    fn a_failed_backup_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = files(dir.path());
        fs::write(&f.file, b"old\n").unwrap();
        f.bak = dir.path().join("missing").join(".Todo.md.bak");
        assert_eq!(
            save(&f, b"new\n").unwrap_err(),
            format!(
                "could not write the backup {}; list not changed",
                f.bak.display()
            )
        );
        assert_eq!(fs::read(&f.file).unwrap(), b"old\n");
    }

    #[test]
    fn a_failed_checksum_warns_and_leaves_none() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = files(dir.path());
        fs::write(&f.file, b"old\n").unwrap();
        f.sum = dir.path().join("missing").join(".Todo.md.sum");
        assert_eq!(
            save(&f, b"new\n").unwrap(),
            Some(format!(
                "could not write {}; t undo won't undo this change",
                f.sum.display()
            ))
        );
        assert_eq!(fs::read(&f.file).unwrap(), b"new\n");
    }

    #[test]
    fn undo_and_redo() {
        let dir = tempfile::tempdir().unwrap();
        let f = files(dir.path());
        assert_eq!(undo(&f).unwrap_err(), "nothing to undo");
        fs::write(&f.file, b"one\n").unwrap();
        save(&f, b"two\n").unwrap();
        undo(&f).unwrap();
        assert_eq!(fs::read(&f.file).unwrap(), b"one\n");
        undo(&f).unwrap();
        assert_eq!(fs::read(&f.file).unwrap(), b"two\n", "a second undo redoes");
        // A change made outside t since its last write is never thrown away.
        fs::write(&f.file, b"edited\n").unwrap();
        assert_eq!(
            undo(&f).unwrap_err(),
            format!(
                "the list was changed since t last wrote it; not undoing (t's previous version is in {})",
                f.bak.display()
            )
        );
        fs::remove_file(&f.sum).unwrap();
        assert_eq!(
            undo(&f).unwrap_err(),
            format!(
                "can't tell whether the list was changed since t last wrote it; not undoing (t's previous version is in {})",
                f.bak.display()
            )
        );
    }

    #[test]
    fn undo_accepts_files_written_by_0_3() {
        // 0.3.0 wrote `cksum <list >sum`: the line and a newline. Undo also restores bytes exactly.
        let dir = tempfile::tempdir().unwrap();
        let f = files(dir.path());
        fs::write(&f.file, b"## Today\n- [ ] a\n\n## Done\n").unwrap();
        fs::write(&f.bak, b"## Today\n\n## Done").unwrap();
        fs::write(&f.sum, b"302049768 26\n").unwrap();
        undo(&f).unwrap();
        assert_eq!(fs::read(&f.file).unwrap(), b"## Today\n\n## Done");
    }

    #[test]
    fn archives_per_year_sorted_and_appended() {
        let dir = tempfile::tempdir().unwrap();
        let adir = dir.path().join("Archive");
        fs::create_dir(&adir).unwrap();
        fs::write(
            adir.join("Todo 2026.md"),
            b"# Archive 2026\n\n- [x] 2026-01-01 Existing\n",
        )
        .unwrap();
        let lines = vec![
            b"- [x] 2026-11-06 Old ticked later".to_vec(),
            b"- [x] 2026-11-06 Eight days".to_vec(),
            b"- [x] 2020-05-01 Ancient".to_vec(),
        ];
        archive(&adir, b"Todo", &lines, false).unwrap();
        assert_eq!(
            fs::read_to_string(adir.join("Todo 2026.md")).unwrap(),
            "# Archive 2026\n\n- [x] 2026-01-01 Existing\n- [x] 2026-11-06 Eight days\n- [x] 2026-11-06 Old ticked later\n"
        );
        assert_eq!(
            fs::read_to_string(adir.join("Todo 2020.md")).unwrap(),
            "# Todo archive 2020\n\n- [x] 2020-05-01 Ancient\n"
        );
    }

    #[test]
    fn archives_keep_their_line_endings() {
        let dir = tempfile::tempdir().unwrap();
        let adir = dir.path().join("Archive");
        archive(&adir, b"Todo", &[b"- [x] 2020-05-01 A".to_vec()], true).unwrap();
        assert_eq!(
            fs::read(adir.join("Todo 2020.md")).unwrap(),
            b"# Todo archive 2020\r\n\r\n- [x] 2020-05-01 A\r\n"
        );
        fs::write(adir.join("Todo 2021.md"), b"# Todo archive 2021\n\n").unwrap();
        archive(&adir, b"Todo", &[b"- [x] 2021-05-01 B".to_vec()], true).unwrap();
        assert_eq!(
            fs::read(adir.join("Todo 2021.md")).unwrap(),
            b"# Todo archive 2021\n\n- [x] 2021-05-01 B\n",
            "an existing archive keeps its own ending"
        );
    }

    #[test]
    fn a_failed_archive_write_names_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let adir = dir.path().join("Archive");
        fs::write(&adir, b"a file, not a folder").unwrap();
        let err = archive(&adir, b"Todo", &[b"- [x] 2020-05-01 A".to_vec()], false).unwrap_err();
        assert_eq!(err, adir.join("Todo 2020.md"));
    }

    #[test]
    fn archived_items_for_done_all() {
        let dir = tempfile::tempdir().unwrap();
        let adir = dir.path();
        fs::write(
            adir.join("Todo 2020.md"),
            b"# Todo archive 2020\n\n- [x] 2020-05-01 Ancient\n- [x] 2020-06-01 Later ancient\n",
        )
        .unwrap();
        fs::write(
            adir.join("Todo 2026.md"),
            b"# t\r\n\r\n- [x] 2026-11-06 Charlie\r\n",
        )
        .unwrap();
        fs::write(
            adir.join("Todo notes.md"),
            b"- [x] 2099-01-01 Not an archive\n",
        )
        .unwrap();
        fs::write(adir.join("Work 2026.md"), b"- [x] 2026-01-01 Other list\n").unwrap();
        let (items, warnings) = archived_items(adir, b"Todo");
        assert!(warnings.is_empty());
        assert_eq!(
            items,
            vec![
                b"- [x] 2020-05-01 Ancient".to_vec(),
                b"- [x] 2020-06-01 Later ancient".to_vec(),
                b"- [x] 2026-11-06 Charlie".to_vec(),
            ]
        );
        assert_eq!(
            archived_items(&adir.join("none"), b"Todo"),
            (vec![], vec![])
        );
    }
}
