//! The backup and undo (0.3.0's "backup" and "undo" sections).

mod common;
use common::*;
use std::fs;

fn bak(s: &Sandbox) -> std::path::PathBuf {
    s.lists.join(".Todo.md.bak")
}

fn sum(s: &Sandbox) -> std::path::PathBuf {
    s.lists.join(".Todo.md.sum")
}

#[test]
fn every_write_keeps_a_backup() {
    let s = Sandbox::new(EMPTY);
    s.t(&["add", "one"]);
    assert_eq!(read(&bak(&s)), EMPTY);
    s.t(&["add", "two"]);
    assert_eq!(
        read(&bak(&s)),
        "## Today\n- [ ] one\n\n## Later\n\n## Done\n"
    );
}

#[cfg(unix)]
#[test]
fn the_backup_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let s = Sandbox::new(EMPTY);
    fs::set_permissions(&s.list, fs::Permissions::from_mode(0o644)).unwrap();
    s.t(&["add", "one"]);
    assert_eq!(
        fs::metadata(bak(&s)).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&s.list).unwrap().permissions().mode() & 0o777,
        0o644
    );
}

#[test]
fn undo_and_redo() {
    let s = Sandbox::new(EMPTY);
    assert_eq!(s.t(&["undo"]).all(), "1|t: nothing to undo\n");
    s.t(&["add", "one"]);
    s.t(&["add", "two"]);
    assert_eq!(
        s.t(&["undo"]).all(),
        "0|Undid the last change; run t undo again to redo it\n"
    );
    assert_eq!(s.file(), "## Today\n- [ ] one\n\n## Later\n\n## Done\n");
    s.t(&["undo"]);
    assert_eq!(
        s.file(),
        "## Today\n- [ ] one\n- [ ] two\n\n## Later\n\n## Done\n",
        "redo"
    );
}

#[test]
fn undo_never_throws_away_a_change_made_outside_t() {
    let s = Sandbox::new(EMPTY);
    s.t(&["add", "one"]);
    s.write("## Today\n- [ ] edited by hand\n\n## Later\n\n## Done\n");
    let before = s.file();
    assert_eq!(
        s.t(&["undo"]).all(),
        format!(
            "1|t: the list was changed since t last wrote it; not undoing (t's previous version is in {})\n",
            bak(&s).display()
        )
    );
    assert_eq!(s.file(), before);
    fs::remove_file(sum(&s)).unwrap();
    assert_eq!(
        s.t(&["undo"]).all(),
        format!(
            "1|t: can't tell whether the list was changed since t last wrote it; not undoing (t's previous version is in {})\n",
            bak(&s).display()
        )
    );
}

#[test]
fn undo_runs_before_tidy() {
    // A ticked item would be tidied first, which undo would then revert; undo comes first.
    let s = Sandbox::new(EMPTY);
    s.t(&["add", "one"]);
    s.t(&["undo"]);
    assert_eq!(s.file(), EMPTY);
}

#[test]
fn backup_and_checksum_written_by_0_3_are_undone() {
    let s = Sandbox::new("## Today\n- [ ] a\n\n## Done\n");
    fs::write(bak(&s), "## Today\n\n## Done").unwrap();
    // `cksum <list` as 0.3.0 wrote it.
    fs::write(sum(&s), "302049768 26\n").unwrap();
    assert_eq!(s.t(&["undo"]).status, 0);
    assert_eq!(fs::read(&s.list).unwrap(), b"## Today\n\n## Done");
}
