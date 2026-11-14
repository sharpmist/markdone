//! Several lists, `t move`, and old archives (0.3.0's "several lists" and "t move" sections).

mod common;
use common::*;
use std::fs;

#[test]
fn lists_add_and_at_list() {
    let s = Sandbox::new(EMPTY);
    assert_eq!(s.t(&["lists"]).all(), "0|* Todo   0 open\n");
    assert_eq!(
        s.t(&["lists", "add", "Work"]).all(),
        "0|Created the list Work; use it with t @Work … or t use Work\n"
    );
    assert_eq!(read(&s.lists.join("Work.md")), EMPTY);
    assert_eq!(
        s.t(&["lists", "add", "work"]).all(),
        "1|t: there's already a list called Work\n"
    );
    for bad in [
        "a/b",
        ".hidden",
        "@at",
        "x.md",
        "Archive 2020",
        "",
        "a:b",
        "CON",
        "a*b",
    ] {
        assert_eq!(s.t(&["lists", "add", bad]).status, 1, "{bad:?}");
    }
    assert_eq!(s.t(&["lists", "add", "Side projects"]).status, 0);
    assert_eq!(
        s.t(&["@work", "add", "Fix the build"]).all(),
        "0|Added 1 (Today): Fix the build\n"
    );
    assert_eq!(s.file(), EMPTY, "the default list is untouched");
    assert_eq!(
        s.t(&["@Side projects", "add", "-l", "Write the post"])
            .stdout,
        "Added 1 (Later): Write the post\n"
    );
    assert_eq!(
        s.t(&["lists"]).stdout,
        "  Side projects   1 open\n* Todo            0 open\n  Work            1 open\n"
    );
    assert_eq!(
        s.t(&["@Nope", "list"]).all(),
        "1|t: no list called 'Nope'; t lists shows them\n"
    );
    assert_eq!(s.t(&["@", "list"]).status, 1);
    assert_eq!(
        s.t(&["@Work", "lists"]).all(),
        "1|t: t lists works on the lists themselves; it doesn't take @Work\n"
    );
    assert_eq!(s.t(&["@Work", "init"]).status, 1);
}

#[test]
fn use_and_todo_list() {
    let s = Sandbox::new(EMPTY);
    s.t(&["lists", "add", "Work"]);
    s.t(&["@Work", "add", "Fix the build"]);
    assert_eq!(s.t(&["use", "work"]).all(), "0|Now using Work (1 open)\n");
    assert_eq!(read(&s.lists.join(".current")), "Work\n");
    assert_eq!(s.t(&["add", "Second"]).stdout, "Added 2 (Today): Second\n");
    assert_eq!(
        s.t(&["list"]).stdout,
        "  1  Today  Fix the build\n  2  Today  Second\n"
    );
    assert_eq!(
        s.t_with(&["list", "--json"], &[("TODO_LIST", "Todo")])
            .stdout,
        "[]\n"
    );
    assert_eq!(
        s.t_with(&["list"], &[("TODO_LIST", "Nope")]).all(),
        "1|t: TODO_LIST is 'Nope', but there's no such list; t lists shows them\n"
    );
    let out = s.t_with(&["use", "Work"], &[("TODO_LIST", "Todo")]);
    assert_eq!(out.stdout, "Now using Work (2 open)\n");
    assert_eq!(
        out.stderr,
        "t: TODO_LIST is set to 'Todo', which still wins where it's set\n"
    );
    assert_eq!(
        s.t(&["use", "Nope"]).all(),
        "1|t: no list called 'Nope'; t lists shows them\n"
    );
    assert_eq!(s.t(&["use"]).status, 1);
    assert_eq!(
        s.t(&["@Work", "list", "--json"]).stdout,
        "[\n{\"list\":\"Work\",\"n\":1,\"section\":\"Today\",\"text\":\"Fix the build\",\"jira\":[]},\n{\"list\":\"Work\",\"n\":2,\"section\":\"Today\",\"text\":\"Second\",\"jira\":[]}\n]\n"
    );
    assert_eq!(
        s.t(&["lists", "--json"]).stdout,
        "[\n{\"name\":\"Todo\",\"open\":0,\"current\":false},\n{\"name\":\"Work\",\"open\":2,\"current\":true}\n]\n"
    );
    // A TODO_LIST that names no list is an error for t lists too (0.3.0 listed without a *).
    assert_eq!(
        s.t_with(&["lists"], &[("TODO_LIST", "Nope")]).all(),
        "1|t: TODO_LIST is 'Nope', but there's no such list; t lists shows them\n"
    );
}

#[test]
fn undo_archives_and_locks_are_per_list() {
    let d8 = days_ago(8);
    let s = Sandbox::new(EMPTY);
    s.t(&["lists", "add", "Work"]);
    s.t(&["use", "Work"]);
    s.t(&["add", "one"]);
    s.t(&["@Todo", "add", "Home thing"]);
    s.t(&["add", "two"]);
    s.t(&["@Todo", "undo"]);
    assert_eq!(s.file(), EMPTY);
    assert_eq!(s.t(&["list"]).stdout.lines().count(), 2);
    let work = s.lists.join("Work.md");
    fs::write(&work, format!("{}- [x] {d8} Old work\n", read(&work))).unwrap();
    s.t(&["list"]);
    assert!(
        s.lists
            .join("Archive")
            .join(format!("Work {}.md", &d8[..4]))
            .is_file()
    );
    assert_eq!(s.t(&["@Todo", "list", "--done", "--all"]).stdout, "");
    assert_eq!(
        s.t(&["list", "--done", "--all"]).stdout,
        format!("{d8}  Old work\n")
    );
}

#[test]
fn a_vanished_current_list_falls_back() {
    let s = Sandbox::new(EMPTY);
    s.t(&["lists", "add", "Work"]);
    s.t(&["use", "Work"]);
    fs::remove_file(s.lists.join("Work.md")).unwrap();
    assert_eq!(
        s.t(&["list"]).all(),
        "0|t: the current list 'Work' is gone; using Todo (t use picks another)\n"
    );
}

#[test]
fn rename_and_remove() {
    let s = Sandbox::new(EMPTY);
    s.t(&["lists", "add", "Work"]);
    s.t(&["@Work", "add", "a"]);
    s.t(&["use", "Work"]);
    assert_eq!(
        s.t(&["lists", "rename", "Work", "Job"]).all(),
        "0|Renamed the list Work to Job\n"
    );
    assert!(s.lists.join("Job.md").is_file() && !s.lists.join("Work.md").exists());
    assert_eq!(read(&s.lists.join(".current")), "Job\n");
    assert_eq!(s.t(&["lists", "rename", "Todo", "Home"]).status, 1);
    assert_eq!(
        s.t(&["lists", "rm", "Todo"]).all(),
        "1|t: Todo is your default list (TODO_FILE); it can't be deleted\n"
    );
    assert_eq!(
        s.t(&["lists", "rm", "Job"]).all(),
        "1|t: Job is the current list; switch with t use first\n"
    );
    s.t(&["use", "Todo"]);
    assert_eq!(
        s.t(&["lists", "rm", "Job"]).all(),
        "1|t: Job has 1 open item; finish or move them first, or use t lists rm --force Job\n"
    );
    assert_eq!(
        s.t(&["lists", "rm", "--force", "job"]).all(),
        format!(
            "0|Deleted the list Job with its 1 open item; its archives stay in {}\n",
            s.lists.join("Archive").display()
        )
    );
    assert!(!s.lists.join("Job.md").exists() && !s.lists.join(".Job.md.lock").exists());
    assert_eq!(s.t(&["lists", "rm", "Nope"]).status, 1);
    assert_eq!(
        s.t(&["lists", "add"]).all(),
        "1|t: usage: t lists add <name>\n"
    );
    assert_eq!(
        s.t(&["lists", "rm"]).all(),
        "1|t: usage: t lists rm [--force] <name>\n"
    );
    assert_eq!(
        s.t(&["lists", "rename", "x"]).all(),
        "1|t: usage: t lists rename <old name> <new name>\n"
    );
    assert_eq!(
        s.t(&["lists", "bogus"]).all(),
        "1|t: usage: t lists [--json] | t lists add|rm|rename …\n"
    );
}

#[test]
fn move_between_lists() {
    let s = Sandbox::new("## Today\n- [ ] Buy milk\n\n## Later\n- [ ] Someday\n\n## Done\n");
    s.t(&["lists", "add", "Work"]);
    assert_eq!(
        s.t(&["move", "milk", "@Work"]).all(),
        "0|Moved to Work (Today) as 1: Buy milk\n"
    );
    assert_eq!(
        s.t(&["mv", "1", "@work"]).stdout,
        "Moved to Work (Later) as 2: Someday\n"
    );
    assert_eq!(s.file(), "## Today\n\n## Later\n\n## Done\n");
    assert_eq!(
        read(&s.lists.join("Work.md")),
        "## Today\n- [ ] Buy milk\n\n## Later\n- [ ] Someday\n\n## Done\n"
    );
    // Each list's undo covers its own half.
    s.t(&["undo"]);
    assert!(s.file().contains("Someday"));
    assert_eq!(
        s.t(&["@Work", "move", "milk", "@Work"]).all(),
        "1|t: the item is already in Work\n"
    );
    assert_eq!(
        s.t(&["move", "x"]).all(),
        "1|t: usage: t move <item> @<list>\n"
    );
    assert_eq!(
        s.t(&["move", "x", "@Nope"]).all(),
        "1|t: no list called 'Nope'; t lists shows them\n"
    );
    fs::write(s.lists.join("Later only.md"), "## Later\n\n## Done\n").unwrap();
    s.t(&["add", "milk again"]);
    let before = s.file();
    let out = s.t(&["move", "milk again", "@Later only"]);
    assert_eq!(
        out.all(),
        format!(
            "1|t: no '## Today' heading in {}; nothing moved\n",
            s.lists.join("Later only.md").display()
        )
    );
    assert_eq!(s.file(), before);
    assert_eq!(
        read(&s.lists.join("Later only.md")),
        "## Later\n\n## Done\n"
    );
}

#[test]
fn old_archives_with_move_to_and_from_the_default_list() {
    // The default list is locked for the migration and again for the move: one lock per file.
    let s = Sandbox::new("## Today\n- [ ] milk\n\n## Done\n");
    fs::write(
        s.lists.join("Work.md"),
        "## Today\n- [ ] report\n\n## Done\n",
    )
    .unwrap();
    fs::write(
        s.lists.join("Archive 2020.md"),
        "# Archive 2020\n\n- [x] 2020-05-01 Ancient\n",
    )
    .unwrap();
    let start = std::time::Instant::now();
    assert_eq!(
        s.t(&["move", "milk", "@Work"]).all(),
        "0|Moved to Work (Today) as 2: milk\n"
    );
    assert!(start.elapsed().as_secs() < 5, "no wait for its own lock");
    assert!(s.lists.join("Archive").join("Todo 2020.md").is_file());
    fs::write(
        s.lists.join("Archive 2021.md"),
        "# Archive 2021\n\n- [x] 2021-05-01 Old\n",
    )
    .unwrap();
    assert_eq!(
        s.t(&["@Work", "move", "report", "@Todo"]).all(),
        "0|Moved to Todo (Today) as 1: report\n"
    );
    fs::write(
        s.lists.join("Archive 2022.md"),
        "# Archive 2022\n\n- [x] 2022-05-01 Old\n",
    )
    .unwrap();
    assert_eq!(s.t(&["@Work", "list"]).all(), "0|  1  Today  milk\n");
    assert!(s.lists.join("Archive").join("Todo 2022.md").is_file());
    assert_eq!(s.t(&["list", "--done", "--all"]).stdout.lines().count(), 3);
}

#[test]
fn a_name_like_a_pattern_matches_only_that_list() {
    let s = Sandbox::new(EMPTY);
    s.t(&["lists", "add", "Work"]);
    s.t(&["lists", "add", "W[o]rk"]);
    assert_eq!(
        s.t(&["@w[o]rk", "add", "star"]).stdout,
        "Added 1 (Today): star\n"
    );
    assert_eq!(s.t(&["@Work", "list"]).stdout, "");
}

#[test]
fn rename_keeps_the_current_choice_while_todo_list_says_otherwise() {
    let s = Sandbox::new(EMPTY);
    s.t(&["lists", "add", "Work"]);
    s.t(&["use", "Work"]);
    s.t_with(
        &["lists", "rename", "Work", "Job"],
        &[("TODO_LIST", "Todo")],
    );
    assert_eq!(read(&s.lists.join(".current")), "Job\n");
}
