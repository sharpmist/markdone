//! Tidy (dating, moving, archiving) and `t list --done` (0.3.0's sections of the same names).

mod common;
use common::*;
use std::fs;

#[test]
fn tidy_dates_moves_and_archives() {
    let (t, d7, d8) = (today(), days_ago(7), days_ago(8));
    let s = Sandbox::new(&format!(
        "## Today\n- [ ] Open one\n- [x] Ticked in an editor\n\n## Later\n- [x] {d8} Old ticked later\n\n## Done\n- [x] {d7} Seven days\n- [x] Undated\n- [x] {d8} Eight days\n- [x] 2020-05-01 Ancient\n"
    ));
    let y = &d8[..4];
    // An old-style archive next to the list is moved to Archive/ first.
    fs::write(
        s.lists.join(format!("Archive {y}.md")),
        format!("# Archive {y}\n\n- [x] {y}-01-01 Existing\n"),
    )
    .unwrap();
    assert_eq!(s.t(&[]).all(), "0|  1  Today  Open one\n");
    assert_eq!(
        s.file(),
        format!(
            "## Today\n- [ ] Open one\n\n## Later\n\n## Done\n- [x] {t} Ticked in an editor\n- [x] {d7} Seven days\n- [x] {t} Undated\n"
        )
    );
    assert_eq!(
        read(&s.lists.join("Archive").join(format!("Todo {y}.md"))),
        format!(
            "# Archive {y}\n\n- [x] {y}-01-01 Existing\n- [x] {d8} Eight days\n- [x] {d8} Old ticked later\n"
        )
    );
    assert!(!s.lists.join(format!("Archive {y}.md")).exists());
    assert_eq!(
        read(&s.lists.join("Archive").join("Todo 2020.md")),
        "# Todo archive 2020\n\n- [x] 2020-05-01 Ancient\n"
    );
    let before = s.file();
    s.t(&[]);
    assert_eq!(s.file(), before, "a second run changes nothing");
    // A tidy that archived can't be undone (the items would be archived twice).
    assert_eq!(s.t(&["undo"]).all(), "1|t: nothing to undo\n");
}

#[cfg(unix)]
#[test]
fn a_failed_archive_write_keeps_the_item_in_the_list() {
    use std::os::unix::fs::PermissionsExt;
    let s = Sandbox::new("## Today\n- [x] Ticked\n\n## Done\n- [x] 2020-06-01 Old item\n");
    let adir = s.lists.join("Archive");
    fs::create_dir(&adir).unwrap();
    let af = adir.join("Todo 2020.md");
    fs::write(&af, "# Todo archive 2020\n\n").unwrap();
    fs::set_permissions(&af, fs::Permissions::from_mode(0o444)).unwrap();
    let before = s.file();
    let out = s.t(&[]);
    assert_eq!(
        out.stderr,
        format!("t: could not write {}; list not tidied\n", af.display())
    );
    assert_eq!(s.file(), before);
    assert_eq!(read(&af), "# Todo archive 2020\n\n");
    fs::set_permissions(&af, fs::Permissions::from_mode(0o644)).unwrap();
    s.t(&[]);
    assert!(read(&af).contains("- [x] 2020-06-01 Old item"));
}

#[test]
fn list_done() {
    let (t, d7, d8) = (today(), days_ago(7), days_ago(8));
    let y = &d8[..4];
    let s = Sandbox::new(&format!(
        "## Today\n- [ ] Open one\n- [x] Ticked in an editor\n\n## Done\n- [x] {t} Alpha\n- [x] {d7} Bravo\n"
    ));
    fs::write(
        s.lists.join(format!("Archive {y}.md")),
        format!("# Archive {y}\n\n- [x] {d8} Charlie\n"),
    )
    .unwrap();
    fs::write(
        s.lists.join("Archive 2020.md"),
        "# Archive 2020\n\n- [x] 2020-05-01 Ancient\n- [x] 2020-06-01 Later ancient\n",
    )
    .unwrap();
    fs::write(
        s.lists.join("Archive notes.md"),
        "- [x] 2099-01-01 Not an archive\n",
    )
    .unwrap();
    assert_eq!(
        s.t(&["list", "--done"]).stdout,
        format!("{t}  Ticked in an editor\n{t}  Alpha\n{d7}  Bravo\n")
    );
    let all = format!(
        "{t}  Ticked in an editor\n{t}  Alpha\n{d7}  Bravo\n{d8}  Charlie\n2020-06-01  Later ancient\n2020-05-01  Ancient\n"
    );
    assert_eq!(s.t(&["list", "--done", "--all"]).stdout, all);
    assert_eq!(s.t(&["ls", "--all", "--done"]).stdout, all);
    let s = Sandbox::new(EMPTY);
    assert_eq!(s.t(&["list", "--done"]).all(), "0|");
}
