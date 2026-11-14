//! Adding, listing, done, today, later, rm, reword, items by text, and failures that must leave
//! the list unchanged (0.3.0's sections of the same names, and its edge cases).

mod common;
use common::*;

#[test]
fn the_sandbox_is_its_own_world() {
    let s = Sandbox::new(EMPTY);
    assert!(s.home.starts_with(s.dir.path()));
    assert!(s.config_file().starts_with(s.dir.path()));
    assert!(!s.config_file().exists());
    assert_eq!(s.t(&["list"]).all(), "0|");
}

#[test]
fn adding_and_listing() {
    let s = Sandbox::new(EMPTY);
    assert_eq!(s.t(&["add", "first"]).all(), "0|Added 1 (Today): first\n");
    assert_eq!(
        s.t(&["add", "--later", "some day"]).stdout,
        "Added 2 (Later): some day\n"
    );
    assert_eq!(
        s.t(&["a", "-l", "short"]).stdout,
        "Added 3 (Later): short\n"
    );
    assert_eq!(s.t(&["a", "second"]).stdout, "Added 2 (Today): second\n");
    let weird = r#"Ticket & "quotes" back\slash \n $HOME `x`"#;
    assert_eq!(
        s.t(&["add", weird]).stdout,
        format!("Added 3 (Today): {weird}\n")
    );
    assert_eq!(
        s.t(&["add", "two", "words"]).stdout,
        "Added 4 (Today): two words\n"
    );
    let list = format!(
        "  1  Today  first\n  2  Today  second\n  3  Today  {weird}\n  4  Today  two words\n  5  Later  some day\n  6  Later  short\n"
    );
    assert_eq!(s.t(&["list"]).stdout, list);
    assert_eq!(s.t(&[]).stdout, list);
    assert_eq!(s.t(&["ls"]).stdout, list);
    assert_eq!(s.t(&["l"]).stdout, list);
    assert_eq!(
        s.t(&["list", "--later"]).stdout,
        "  5  Later  some day\n  6  Later  short\n"
    );
    assert_eq!(s.t(&["ls", "--later", "--today"]).stdout, list);
}

#[test]
fn done() {
    let s = Sandbox::new(EMPTY);
    s.t(&["add", "first"]);
    s.t(&["add", "second"]);
    s.t(&["add", "-l", "some day"]);
    assert_eq!(s.t(&["done", "2"]).all(), "0|Done: second\n");
    assert_eq!(s.t(&["d", "1"]).stdout, "Done: first\n");
    let t = today();
    assert_eq!(
        s.file(),
        format!(
            "## Today\n\n## Later\n- [ ] some day\n\n## Done\n- [x] {t} first\n- [x] {t} second\n"
        )
    );
}

#[test]
fn errors_leave_the_file_untouched() {
    let s = Sandbox::new(EMPTY);
    s.t(&["add", "one"]);
    let before = s.file();
    for args in [
        &["add"][..],
        &["add", "--later"],
        &["done"],
        &["done", "x"],
        &["done", "99"],
        &["bogus"],
        &["add", "two\nlines"],
        &["today", "9"],
        &["rm", "9"],
        &["reword", "1"],
        &["list", "--bogus"],
    ] {
        assert_eq!(s.t(args).status, 1, "{args:?}");
    }
    assert_eq!(s.file(), before);
    assert_eq!(
        s.t(&["add", "two\nlines"]).stderr,
        "t: item must be a single line\n"
    );
    assert_eq!(
        s.t(&["done", "99"]).stderr,
        "t: no open item 99 (or no '## Done' heading)\n"
    );
    let bogus = s.t(&["bogus"]);
    assert!(
        bogus
            .stderr
            .starts_with("t: unknown command 'bogus'\nUsage:\n")
    );
    assert_eq!(bogus.stdout, "");
}

#[test]
fn help_and_version_need_no_list() {
    let mut s = Sandbox::new(EMPTY);
    s.set("TODO_FILE", "/nonexistent/Todo.md");
    let help = s.t(&["help"]);
    assert_eq!(help.status, 0);
    assert!(help.stdout.contains("t add [--later] <text>"));
    assert_eq!(s.t(&["--help"]).stdout, help.stdout);
    assert_eq!(s.t(&["-h"]).stdout, help.stdout);
    let v = s.t(&["version"]);
    assert_eq!(v.status, 0);
    assert!(v.stdout.starts_with("t 0."));
    assert_eq!(s.t(&["--version"]).stdout, v.stdout);
}

#[test]
fn today_and_later() {
    let s = Sandbox::new("## Today\n- [ ] A\n\n## Later\n- [ ] B\n- [ ] C PROJ-5\n\n## Done\n");
    assert_eq!(
        s.t(&["today", "3"]).stdout,
        format!("Moved to Today as 2: C {}\n", l("PROJ-5"))
    );
    assert_eq!(s.t(&["promote", "3"]).stdout, "Moved to Today as 3: B\n");
    assert_eq!(
        s.file(),
        format!(
            "## Today\n- [ ] A\n- [ ] C {}\n- [ ] B\n\n## Later\n\n## Done\n",
            l("PROJ-5")
        )
    );
    let before = s.file();
    assert_eq!(
        s.t(&["today", "1"]).all(),
        "1|t: item 1 is already in Today\n"
    );
    assert_eq!(s.t(&["today", "9"]).status, 1);
    assert_eq!(s.t(&["today", "x"]).status, 1);
    assert_eq!(s.t(&["later"]).all(), "1|t: usage: t later <item>\n");
    assert_eq!(s.file(), before);
    assert_eq!(s.t(&["later", "1"]).stdout, "Moved to Later as 3: A\n");
    assert_eq!(
        s.t(&["demote", "1"]).stdout,
        "Moved to Later as 3: C [PROJ-5](https://jira.example.com/browse/PROJ-5)\n"
    );

    let s = Sandbox::new("## Today\n- [ ] A\n\n## Done\n");
    assert_eq!(
        s.t(&["later", "1"]).all(),
        format!("1|t: no '## Later' heading in {}\n", s.list.display())
    );
    assert_eq!(s.file(), "## Today\n- [ ] A\n\n## Done\n", "item not lost");
}

#[test]
fn items_by_text() {
    let p7 = l("PROJ-7");
    let s = Sandbox::new(
        "## Today\n- [ ] Fix login bug\n- [ ] Write docs (v2.1)\n\n## Later\n- [ ] Fix logout PROJ-7\n\n## Done\n",
    );
    assert_eq!(s.t(&["done", "write"]).stdout, "Done: Write docs (v2.1)\n");
    s.t(&["undo"]);
    assert_eq!(
        s.t(&["done", "LOGIN", "BUG"]).stdout,
        "Done: Fix login bug\n"
    );
    s.t(&["undo"]);
    assert_eq!(s.t(&["d", "(v2"]).stdout, "Done: Write docs (v2.1)\n");
    s.t(&["undo"]);
    assert_eq!(
        s.t(&["later", "docs"]).stdout,
        "Moved to Later as 3: Write docs (v2.1)\n"
    );
    assert_eq!(
        s.t(&["today", "logout"]).stdout,
        format!("Moved to Today as 2: Fix logout {p7}\n")
    );
    assert_eq!(
        s.t(&["done", "PROJ-7"]).stdout,
        format!("Done: Fix logout {p7}\n")
    );
    s.t(&["undo"]);
    let before = s.file();
    assert_eq!(
        s.t(&["done", "fix"]).all(),
        format!(
            "1|t: 'fix' matches 2 open items; use more of the text, or the number:\n  1  Today  Fix login bug\n  2  Today  Fix logout {p7}\n"
        )
    );
    assert_eq!(
        s.t(&["done", "f.x"]).all(),
        "1|t: no open item matches 'f.x'\n"
    );
    assert_eq!(s.t(&["done"]).all(), "1|t: usage: t done <item>\n");
    assert_eq!(
        s.t(&["today", "login"]).all(),
        "1|t: item 1 is already in Today\n"
    );
    assert_eq!(s.file(), before);
}

#[test]
fn text_matches_what_a_terminal_shows() {
    let s = Sandbox::new(
        "## Today\n- [ ] Fix logout PROJ-7\n- [ ] Read [the guide](https://docs.example.com/guide) on https://example.org/faq\n- [ ] Ask about [local notes](notes.md)\n\n## Done\n",
    );
    for url in ["jira.example", "docs.example", "browse"] {
        assert_eq!(
            s.t(&["done", url]).all(),
            format!("1|t: no open item matches '{url}'\n")
        );
    }
    for (r, done) in [
        ("logout proj-7", "Done: Fix logout"),
        ("the guide on", "Done: Read [the guide]"),
        ("example.org", "Done: Read [the guide]"),
        ("notes.md", "Done: Ask about"),
    ] {
        let out = s.t(&["done", r]);
        assert!(out.stdout.starts_with(done), "{r}: {out:?}");
        s.t(&["undo"]);
    }
}

#[test]
fn rm() {
    let t = today();
    let s = Sandbox::new(&format!(
        "## Today\n- [ ] keep\n- [ ] old idea\n\n## Later\n- [ ] later idea\n\n## Done\n- [x] {t} done item\n"
    ));
    assert_eq!(s.t(&["rm", "2"]).all(), "0|Removed: old idea\n");
    assert_eq!(s.t(&["remove", "later"]).all(), "0|Removed: later idea\n");
    assert_eq!(
        s.file(),
        format!("## Today\n- [ ] keep\n\n## Later\n\n## Done\n- [x] {t} done item\n")
    );
    s.t(&["undo"]);
    assert!(s.file().contains("- [ ] later idea"));
    let before = s.file();
    assert_eq!(s.t(&["rm", "9"]).all(), "1|t: no open item 9\n");
    assert_eq!(
        s.t(&["rm", "done item"]).all(),
        "1|t: no open item matches 'done item'\n"
    );
    assert_eq!(s.t(&["rm"]).all(), "1|t: usage: t rm <item>\n");
    assert_eq!(s.file(), before);
}

#[test]
fn reword() {
    let s = Sandbox::new(
        "## Today\n- [ ] first\n- [ ] Retest logout\n\n## Later\n- [ ] third\n\n## Done\n",
    );
    assert_eq!(
        s.t(&["reword", "2", "Retest logout fix PROJ-9"]).all(),
        format!("0|Reworded 2 (Today): Retest logout fix {}\n", l("PROJ-9"))
    );
    assert_eq!(
        s.t(&["reword", "third", "Plan", "the", "next", "release"])
            .all(),
        "0|Reworded 3 (Later): Plan the next release\n"
    );
    assert_eq!(
        s.file(),
        format!(
            "## Today\n- [ ] first\n- [ ] Retest logout fix {}\n\n## Later\n- [ ] Plan the next release\n\n## Done\n",
            l("PROJ-9")
        )
    );
    let before = s.file();
    let usage = "1|t: usage: t reword <item> <new text>\n";
    assert_eq!(s.t(&["reword", "first"]).all(), usage);
    assert_eq!(s.t(&["reword"]).all(), usage);
    assert_eq!(s.t(&["reword", "9", "x"]).all(), "1|t: no open item 9\n");
    assert_eq!(
        s.t(&["reword", "1", "two\nlines"]).all(),
        "1|t: item must be a single line\n"
    );
    assert_eq!(s.file(), before);
}

#[test]
fn edge_cases() {
    let s = Sandbox::new("## Today\n- [x] orphan\n");
    assert_eq!(
        s.t(&[]).all(),
        "0|t: ticked items found but no '## Done' heading after them; list not tidied\n"
    );
    assert_eq!(s.file(), "## Today\n- [x] orphan\n");

    let s = Sandbox::new("## Today\n\n## Done\n\n## Later");
    assert_eq!(s.t(&["add", "-l", "x"]).stdout, "Added 1 (Later): x\n");
    assert_eq!(s.file(), "## Today\n\n## Done\n\n## Later\n- [ ] x\n");

    let s = Sandbox::new("## Later\n\n## Done\n");
    assert_eq!(s.t(&["add", "x"]).status, 1);

    let mut s = Sandbox::new(EMPTY);
    let missing = s.dir.path().join("nonexistent").join("Todo.md");
    s.set("TODO_FILE", &missing.display().to_string());
    assert_eq!(
        s.t(&[]).all(),
        format!(
            "1|t: {} not found; create it with 't init', or set TODO_FILE to your list\n",
            missing.display()
        )
    );
}

#[test]
fn list_options() {
    let s = Sandbox::new(EMPTY);
    assert_eq!(
        s.t(&["list", "--all"]).all(),
        "1|t: --all only works together with --done\n"
    );
    assert_eq!(
        s.t(&["list", "--today", "--done"]).all(),
        "1|t: --today and --later list open items; they don't combine with --done\n"
    );
    assert_eq!(
        s.t(&["list", "--done", "--json"]).all(),
        "1|t: --json lists open items; it doesn't combine with --done\n"
    );
    assert_eq!(
        s.t(&["list", "--bogus"]).all(),
        "1|t: unknown option '--bogus' for list (expected --today, --later, --done, --all, --json)\n"
    );
}

#[test]
fn a_closed_pipe_ends_the_run_quietly() {
    // Like `t list | head -c 16`: far more output than a pipe buffers, and a reader that stops.
    use std::io::Read;
    use std::process::Stdio;
    let mut text = String::from("## Today\n");
    for i in 0..20_000 {
        text.push_str(&format!("- [ ] item number {i}\n"));
    }
    text.push_str("\n## Later\n\n## Done\n");
    let s = Sandbox::new(&text);
    let mut child = s
        .command(&["list"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut first = [0u8; 16];
    child
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut first)
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stderr), "", "no panic message");
    let expected = if cfg!(windows) { 1 } else { 141 };
    assert_eq!(out.status.code(), Some(expected));
}
