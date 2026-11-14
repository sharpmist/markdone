//! `t commands` and `t complete` (the Tab completion contract with shell/t.zsh).

mod common;
use common::*;

#[test]
fn commands_one_tab_separated_line_each() {
    let s = Sandbox::new(EMPTY);
    let out = s.t(&["commands"]);
    assert_eq!(out.status, 0);
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(lines.len(), 27);
    assert!(lines.iter().all(|l| l.matches('\t').count() == 1));
    assert!(lines.contains(
        &"add \"\"\tadd \"<text>\"           Add an item to Today                       e.g. t add \"Retest logout fix PROJ-123\""
    ));
}

#[test]
fn complete() {
    let s = Sandbox::new(EMPTY);
    s.t(&["lists", "add", "Work"]);
    s.t(&["@Work", "add", "Ship it"]);
    assert_eq!(
        s.t(&["complete", "@"]).stdout,
        "@Todo \t* Todo   0 open\n@Work \t  Work   1 open\n"
    );
    assert_eq!(
        s.t(&["complete", "use"]).stdout,
        "Todo \t* Todo   0 open\nWork \t  Work   1 open\n"
    );
    assert_eq!(s.t(&["complete", "lists", "rm"]).stdout.lines().count(), 2);
    assert_eq!(
        s.t(&["complete", "@Work", "done"]).stdout,
        "1\t  1  Today  Ship it\n"
    );
    assert_eq!(
        s.t(&["complete", "@Work"]).stdout,
        s.t(&["commands"]).stdout
    );
    let subs: Vec<String> = s
        .t(&["complete", "lists"])
        .stdout
        .lines()
        .map(|l| l.split('\t').next().unwrap().to_string())
        .collect();
    assert_eq!(subs, ["add \"\"", "rm ", "rename ", "--json"]);
    assert_eq!(s.t(&["complete", "@Nope", "done"]).status, 1);
    assert_eq!(
        s.t(&["complete", "move", "1"]).stdout,
        "@Todo \t* Todo   0 open\n@Work \t  Work   1 open\n"
    );
    assert_eq!(s.t(&["complete", "bogus"]).all(), "0|");
    assert_eq!(s.t(&["complete"]).all(), "0|");
    // A TODO_LIST naming no list: nothing, quietly (0.3.0 printed an error).
    assert_eq!(
        s.t_with(&["complete", "use"], &[("TODO_LIST", "Nope")])
            .all(),
        "0|"
    );
}

#[test]
fn complete_items_by_command() {
    let s = Sandbox::new(&format!(
        "## Today\n- [ ] Buy milk {}\n\n## Later\n- [ ] Someday\n\n## Done\n",
        l("PROJ-1")
    ));
    assert_eq!(
        s.t(&["complete", "move"]).stdout,
        "1\t  1  Today  Buy milk PROJ-1\n2\t  2  Later  Someday\n"
    );
    assert_eq!(
        s.t(&["complete", "today"]).stdout,
        "2\t  2  Later  Someday\n"
    );
    assert_eq!(
        s.t(&["complete", "later"]).stdout,
        "1\t  1  Today  Buy milk PROJ-1\n"
    );
    assert_eq!(
        s.t(&["complete", "reword"]).stdout,
        "1 \"\"\t  1  Today  Buy milk PROJ-1\n2 \"\"\t  2  Later  Someday\n"
    );
}

#[test]
fn list_names_are_quoted_for_the_shell() {
    let s = Sandbox::new(EMPTY);
    s.t(&["lists", "add", "Side projects"]);
    s.t(&["lists", "add", "it's"]);
    let out = s.t(&["complete", "@"]).stdout;
    assert!(out.contains("@'Side projects' \t"), "{out}");
    assert!(out.contains("@it\\'s \t"), "{out}");
}
