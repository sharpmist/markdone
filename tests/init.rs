//! `t init` without a terminal (0.3.0's "t init" and "t init with Obsidian vaults" sections; the
//! ones that answer prompts stay in test/run.zsh).

mod common;
use common::*;
use std::fs;
use std::path::PathBuf;

/// A sandbox for a new user: no TODO_FILE, and Obsidian listing these vaults, the last one most
/// recently opened (plus one whose folder is gone).
fn new_user(vaults: &[&str]) -> Sandbox {
    let mut s = Sandbox::new(EMPTY);
    s.unset("TODO_FILE");
    if !vaults.is_empty() {
        let reg = s
            .home
            .join("Library")
            .join("Application Support")
            .join("obsidian");
        fs::create_dir_all(&reg).unwrap();
        let mut entries = Vec::new();
        for (i, v) in vaults.iter().enumerate() {
            let dir = vault(&s, v);
            fs::create_dir_all(&dir).unwrap();
            entries.push(format!(
                r#""v{i}":{{"path":"{}","ts":{}}}"#,
                dir.display().to_string().replace('\\', "\\\\"),
                i + 1
            ));
        }
        let gone = s
            .home
            .join("gone")
            .display()
            .to_string()
            .replace('\\', "\\\\");
        entries.push(format!(r#""gone":{{"path":"{gone}","ts":99}}"#));
        fs::write(
            reg.join("obsidian.json"),
            format!(r#"{{"vaults":{{{}}}}}"#, entries.join(",")),
        )
        .unwrap();
    }
    s
}

fn vault(s: &Sandbox, name: &str) -> PathBuf {
    s.home.join("Vaults").join(name)
}

fn shown_config() -> &'static str {
    if cfg!(windows) {
        r"~\AppData\markdone\config"
    } else {
        "~/.config/markdone/config"
    }
}

fn under_home(parts: &[&str]) -> String {
    let sep = std::path::MAIN_SEPARATOR;
    format!("~{sep}{}", parts.join(&sep.to_string()))
}

#[test]
fn init_creates_the_list_and_its_folders() {
    let mut s = Sandbox::new(EMPTY);
    let list = s.dir.path().join("new dir").join("sub").join("Todo.md");
    s.set("TODO_FILE", &list.display().to_string());
    assert_eq!(
        s.t(&["init"]).all(),
        format!("0|Created {}\n", list.display())
    );
    assert_eq!(read(&list), EMPTY);
    assert_eq!(s.t(&["add", "first"]).stdout, "Added 1 (Today): first\n");
    let before = read(&list);
    assert_eq!(
        s.t(&["init"]).all(),
        format!("1|t: {} already exists; not changed\n", list.display())
    );
    assert_eq!(read(&list), before);
}

#[cfg(unix)]
#[test]
fn init_refuses_a_dangling_symlink_and_names_a_folder_it_cant_create() {
    use std::os::unix::fs::PermissionsExt;
    let mut s = Sandbox::new(EMPTY);
    let target = s.dir.path().join("missing-target.md");
    let link = s.dir.path().join("dangling.md");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    s.set("TODO_FILE", &link.display().to_string());
    assert_eq!(s.t(&["init"]).status, 1);
    assert!(!target.exists());

    let ro = s.dir.path().join("ro");
    fs::create_dir(&ro).unwrap();
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o555)).unwrap();
    s.set(
        "TODO_FILE",
        &ro.join("sub").join("Todo.md").display().to_string(),
    );
    let out = s.t(&["init"]);
    fs::set_permissions(&ro, fs::Permissions::from_mode(0o755)).unwrap();
    if out.status == 1 {
        assert_eq!(
            out.stderr,
            format!(
                "t: could not create the directory {}\n",
                ro.join("sub").display()
            )
        );
    }
}

#[test]
fn init_path_never_asks_and_shows_the_config_line() {
    let s = new_user(&[]);
    // A relative path is taken from the working directory as the system reports it (on macOS
    // the temporary folder /var/… is reported as /private/var/…).
    let cwd = if cfg!(unix) {
        s.dir.path().canonicalize().unwrap()
    } else {
        s.dir.path().to_path_buf()
    };
    let out = s.t(&["init", "lists/Todo.md"]);
    let made = cwd.join("lists").join("Todo.md");
    // The sandbox already has lists/Todo.md, so take another name.
    assert_eq!(
        out.all(),
        format!("1|t: {} already exists; not changed\n", made.display())
    );
    let out = s.t(&["init", "./new/../more/Todo.md"]);
    let made = cwd.join("more").join("Todo.md");
    assert_eq!(out.status, 0, "{out:?}");
    assert!(out.stdout.starts_with(&format!(
        "Created {}\nTo use it, add this to {}:\n  TODO_FILE=",
        made.display(),
        shown_config()
    )));
    assert_eq!(read(&made), EMPTY);
    assert!(!s.config_file().exists(), "never writes the config file");
    // Under the home directory, the line uses ~.
    let out = s.t(&[
        "init",
        &s.home.join("notes").join("Todo.md").display().to_string(),
    ]);
    assert!(out.stdout.ends_with(&format!(
        "  TODO_FILE={}\n",
        under_home(&["notes", "Todo.md"])
    )));
}

#[test]
fn init_usage() {
    let s = new_user(&[]);
    let usage = "1|t: usage: t init [<path> | --obsidian]\n";
    assert_eq!(s.t(&["init", "a", "b"]).all(), usage);
    assert_eq!(s.t(&["init", "--bogus"]).all(), usage);
    assert_eq!(s.t(&["init", "--obsidian", "extra"]).all(), usage);
}

#[test]
fn init_obsidian_without_vaults_fails() {
    let s = new_user(&[]);
    assert_eq!(
        s.t(&["init", "--obsidian"]).all(),
        "1|t: found no Obsidian vaults (Obsidian lists a vault once you've opened it); use t init <path> instead\n"
    );
}

#[test]
fn init_obsidian_without_a_terminal_takes_the_only_vault() {
    let s = new_user(&["Solo"]);
    let list = vault(&s, "Solo").join("Todo").join("Todo.md");
    let out = s.t(&["init", "--obsidian"]);
    assert_eq!(out.status, 0);
    assert_eq!(
        out.stderr,
        format!("Obsidian vault: {}\n", vault(&s, "Solo").display())
    );
    assert_eq!(
        out.stdout,
        format!(
            "Created {}\nTo use it, add this to {}:\n  TODO_FILE={}\n",
            list.display(),
            shown_config(),
            under_home(&["Vaults", "Solo", "Todo", "Todo.md"])
        )
    );
    assert_eq!(read(&list), EMPTY);
    assert!(!s.config_file().exists());
    // Again: the existing list is used as it is.
    let out = s.t(&["init", "--obsidian"]);
    assert!(out.stdout.starts_with(&format!(
        "{} already exists; using it as it is\n",
        list.display()
    )));
}

#[test]
fn init_obsidian_without_a_terminal_and_several_vaults_fails() {
    let s = new_user(&["Work", "Notes"]);
    assert_eq!(
        s.t(&["init", "--obsidian"]).all(),
        format!(
            "1|t: found 2 Obsidian vaults; run t init --obsidian in a terminal to choose one, or t init <path>:\n  {}\n  {}\n",
            vault(&s, "Notes").display(),
            vault(&s, "Work").display()
        )
    );
    assert!(!vault(&s, "Work").join("Todo").exists());
}

#[test]
fn plain_init_without_a_terminal_asks_nothing() {
    let s = new_user(&["Solo"]);
    let list = s.home.join("todo").join("Todo.md");
    assert_eq!(
        s.t(&["init"]).all(),
        format!("0|Created {}\n", list.display())
    );
}

#[test]
fn a_config_that_already_names_the_list() {
    let s = new_user(&["Solo"]);
    let c = s.config_file();
    fs::create_dir_all(c.parent().unwrap()).unwrap();
    fs::write(&c, "TODO_FILE=~/Vaults/Solo/Todo/Todo.md\n").unwrap();
    let out = s.t(&["init", "--obsidian"]);
    assert!(
        out.stdout.ends_with(&format!(
            "{} already points t at this list\n",
            shown_config()
        )),
        "{out:?}"
    );
    assert_eq!(
        s.t(&["add", "x"]).stdout,
        "Added 1 (Today): x\n",
        "t uses it"
    );
}
