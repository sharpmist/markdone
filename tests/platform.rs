//! What differs between systems (docs/designs/rust_port.md, "Platform differences"): the config
//! file's place, starting the editor, and arguments. The editor cases run on every system, with a
//! shell script as the editor on Unix and a .cmd file on Windows.

mod common;
use common::*;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn the_config_file_is_read_from_its_place() {
    let mut s = Sandbox::new(EMPTY);
    s.unset("TODO_FILE");
    let c = s.config_file();
    fs::create_dir_all(c.parent().unwrap()).unwrap();
    fs::write(&c, format!("TODO_FILE={}\n", s.list.display())).unwrap();
    assert_eq!(s.t(&["add", "x"]).all(), "0|Added 1 (Today): x\n");
    if cfg!(windows) {
        assert!(
            c.starts_with(s.home.join("AppData")),
            "%APPDATA%\\markdone\\config"
        );
    } else {
        assert!(
            c.starts_with(s.home.join(".config")),
            "$XDG_CONFIG_HOME/markdone/config"
        );
    }
}

#[cfg(windows)]
#[test]
fn without_appdata_there_is_no_config_file() {
    let s = Sandbox::new(EMPTY);
    let out = s.command(&["list"]).env_remove("APPDATA").output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "t: APPDATA isn't set, so the config file isn't read\n"
    );
    assert!(out.status.success());
}

/// Writes an editor that records its arguments, one per line, in `out.txt` next to it.
fn editor(dir: &Path, name: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    if cfg!(windows) {
        let p = dir.join(format!("{name}.cmd"));
        fs::write(&p, "@echo off\r\necho %*> \"%~dp0out.txt\"\r\n").unwrap();
        p
    } else {
        let p = dir.join(name);
        fs::write(
            &p,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$(dirname \"$0\")/out.txt\"\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        }
        p
    }
}

fn path_with(dir: &Path) -> String {
    let old = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(std::env::split_paths(&old));
    std::env::join_paths(paths)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn an_editor_found_on_path_gets_its_arguments_and_the_list() {
    let s = Sandbox::new(EMPTY);
    let bin = s.dir.path().join("bin");
    editor(&bin, "ed");
    // On Windows, "ed" finds ed.cmd through PATHEXT.
    let out = s.t_with(
        &["edit"],
        &[("PATH", &path_with(&bin)), ("EDITOR", "ed --wait")],
    );
    assert_eq!(out.all(), "0|");
    let recorded = read(&bin.join("out.txt"));
    assert!(recorded.contains("--wait"), "{recorded:?}");
    assert!(recorded.contains("Todo.md"), "{recorded:?}");
}

#[test]
fn an_editor_at_a_path_with_spaces() {
    let s = Sandbox::new(EMPTY);
    let dir = s.dir.path().join("my editors");
    let ed = editor(&dir, "ed");
    let out = s.t_with(&["edit"], &[("EDITOR", &ed.display().to_string())]);
    assert_eq!(out.all(), "0|");
    assert!(read(&dir.join("out.txt")).contains("Todo.md"));
}

#[test]
fn an_editor_that_cant_be_started() {
    let s = Sandbox::new(EMPTY);
    let out = s.t_with(&["edit"], &[("EDITOR", "no-such-editor-anywhere")]);
    assert_eq!(
        out.all(),
        "127|t: could not run the editor 'no-such-editor-anywhere'\n"
    );
}

#[test]
fn the_editor_doesnt_hold_the_lock() {
    // The editor takes the list's lock itself; it gets it, so t no longer holds it.
    let s = Sandbox::new(EMPTY);
    let bin = s.dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let probe = env!("CARGO_BIN_EXE_t");
    // `t add` from inside the editor would wait 10 s and fail if t still held the lock.
    let script = if cfg!(windows) {
        let p = bin.join("ed.cmd");
        fs::write(
            &p,
            format!("@echo off\r\n\"{probe}\" add from-the-editor\r\n"),
        )
        .unwrap();
        p
    } else {
        let p = bin.join("ed");
        fs::write(&p, format!("#!/bin/sh\n\"{probe}\" add from-the-editor\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        }
        p
    };
    let start = std::time::Instant::now();
    let out = s.t_with(&["edit"], &[("EDITOR", &script.display().to_string())]);
    assert!(start.elapsed().as_secs() < 5, "{out:?}");
    assert_eq!(out.stdout, "Added 1 (Today): from-the-editor\n");
    assert!(s.file().contains("from-the-editor"));
}

#[cfg(unix)]
#[test]
fn arguments_are_bytes_on_unix() {
    use std::os::unix::ffi::OsStrExt;
    let s = Sandbox::new(EMPTY);
    let out = s
        .command(&["add"])
        .arg(std::ffi::OsStr::from_bytes(b"caf\xe9"))
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        fs::read(&s.list).unwrap(),
        b"## Today\n- [ ] caf\xe9\n\n## Later\n\n## Done\n"
    );
}

#[cfg(windows)]
#[test]
fn arguments_must_be_text_on_windows() {
    use std::os::windows::ffi::OsStringExt;
    let s = Sandbox::new(EMPTY);
    let bad = std::ffi::OsString::from_wide(&[0x61, 0xD800]);
    let out = s.command(&["add"]).arg(bad).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "t: arguments must be valid text\n"
    );
    assert_eq!(s.file(), EMPTY);
}
