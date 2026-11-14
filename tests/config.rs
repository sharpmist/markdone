//! Settings from the config file and the environment, and the upgrade warning (new in 0.4.0;
//! replaces 0.3.0's "configuration from ~/.zshenv" section).

mod common;
use common::*;
use std::fs;

fn write_config(s: &Sandbox, text: &str) {
    let c = s.config_file();
    fs::create_dir_all(c.parent().unwrap()).unwrap();
    fs::write(&c, text).unwrap();
}

#[test]
fn todo_file_from_the_config_file() {
    let mut s = Sandbox::new(EMPTY);
    s.unset("TODO_FILE");
    write_config(
        &s,
        &format!("# my settings\nTODO_FILE={}\n", s.list.display()),
    );
    assert_eq!(s.t(&["add", "mine"]).all(), "0|Added 1 (Today): mine\n");
    assert!(s.file().contains("mine"));
}

#[test]
fn the_environment_wins() {
    let s = Sandbox::new(EMPTY);
    let other = s.lists.join("Other.md");
    fs::write(&other, EMPTY).unwrap();
    write_config(&s, &format!("TODO_FILE={}\n", other.display()));
    s.t(&["add", "env"]);
    assert!(s.file().contains("env"));
    assert_eq!(read(&other), EMPTY);
}

#[test]
fn tilde_means_the_home_directory() {
    let mut s = Sandbox::new(EMPTY);
    s.unset("TODO_FILE");
    let notes = s.home.join("notes");
    fs::create_dir_all(&notes).unwrap();
    fs::write(notes.join("Todo.md"), EMPTY).unwrap();
    write_config(&s, "TODO_FILE=~/notes/Todo.md\n");
    assert_eq!(s.t(&["add", "x"]).stdout, "Added 1 (Today): x\n");
    assert!(read(&notes.join("Todo.md")).contains("- [ ] x"));
}

#[test]
fn jira_settings_from_the_file_and_off_for_one_run() {
    let mut s = Sandbox::new(EMPTY);
    s.unset("TODO_JIRA_KEYS");
    s.unset("TODO_JIRA_URL");
    write_config(&s, &format!("TODO_JIRA_KEYS = PROJ\nTODO_JIRA_URL={URL}\n"));
    assert_eq!(
        s.t(&["add", "PROJ-1"]).stdout,
        format!("Added 1 (Today): {}\n", l("PROJ-1"))
    );
    assert_eq!(
        s.t_with(&["add", "PROJ-2"], &[("TODO_JIRA_KEYS", "")])
            .stdout,
        "Added 2 (Today): PROJ-2\n"
    );
}

#[test]
fn an_empty_todo_file_means_the_default() {
    let mut s = Sandbox::new(EMPTY);
    s.set("TODO_FILE", "");
    let default = s.home.join("todo").join("Todo.md");
    assert_eq!(
        s.t(&[]).all(),
        format!(
            "1|t: {} not found; create it with 't init', or set TODO_FILE to your list\n",
            default.display()
        )
    );
}

#[test]
fn bad_lines_warn_with_file_and_line() {
    let s = Sandbox::new(EMPTY);
    write_config(&s, "nonsense\nTODO_LIST=Work\nTODO_FILE=relative.md\n");
    let shown = if cfg!(windows) {
        r"~\AppData\markdone\config"
    } else {
        "~/.config/markdone/config"
    };
    let out = s.t(&["list"]);
    assert_eq!(
        out.stderr,
        format!(
            "t: {shown}:1: not a setting; ignored\nt: {shown}:2: TODO_LIST can't be set here (use t use); ignored\nt: {shown}:3: TODO_FILE must be an absolute path or start with ~/; ignored\n"
        )
    );
    assert_eq!(out.status, 0);
    assert_eq!(s.t(&["help"]).stderr, "", "help doesn't read the settings");
}

#[cfg(unix)]
#[test]
fn an_unreadable_config_file_stops_t() {
    use std::os::unix::fs::PermissionsExt;
    let s = Sandbox::new(EMPTY);
    write_config(&s, "");
    fs::set_permissions(s.config_file(), fs::Permissions::from_mode(0o000)).unwrap();
    let out = s.t(&["list"]);
    fs::set_permissions(s.config_file(), fs::Permissions::from_mode(0o644)).unwrap();
    // Running as root can read anything; then there's nothing to check.
    if out.status == 1 {
        assert_eq!(out.stderr, "t: could not read ~/.config/markdone/config\n");
    }
    assert_eq!(s.t(&["version"]).status, 0);
}

#[cfg(unix)]
#[test]
fn the_upgrade_warning_shows_the_line_to_move() {
    let mut s = Sandbox::new(EMPTY);
    s.unset("TODO_FILE");
    fs::write(
        s.home.join(".zshenv"),
        "# markdone: where t keeps the list (set by t init)\nexport TODO_FILE=\"${TODO_FILE:-$HOME/notes/Todo.md}\"\n",
    )
    .unwrap();
    let out = s.t(&["list"]);
    assert_eq!(out.status, 1);
    assert!(
        out.stderr.starts_with(
            "t: ~/.zshenv sets TODO_FILE, but t no longer reads it; put this line in ~/.config/markdone/config: TODO_FILE=~/notes/Todo.md\n"
        ),
        "{}",
        out.stderr
    );
    // Not when TODO_FILE comes from the environment (as in zsh, which reads .zshenv).
    s.set("TODO_FILE", &s.list.display().to_string());
    assert_eq!(s.t(&["list"]).stderr, "");
    // Another TODO_FILE line: the path is a placeholder.
    s.unset("TODO_FILE");
    fs::write(s.home.join(".zshenv"), "export TODO_FILE=/x/Todo.md\n").unwrap();
    assert!(s.t(&["list"]).stderr.contains("TODO_FILE=<path>\n"));
}
