//! Jira links and `t list --json` (0.3.0's sections of the same names).

mod common;
use common::*;

#[test]
fn json() {
    let s = Sandbox::new(EMPTY);
    assert_eq!(s.t(&["list", "--json"]).all(), "0|[]\n");
    let t = today();
    let s = Sandbox::new(&format!(
        "## Today\n- [ ] Say \"hi\" \\ back\tslash PROJ-7, again PROJ-7, and OPS-1\n\n## Later\n- [ ] Later one\n\n## Done\n- [x] {t} done\n"
    ));
    let (p7, o1) = (l("PROJ-7"), l("OPS-1"));
    assert_eq!(
        s.t(&["list", "--json"]).stdout,
        format!(
            "[\n{{\"list\":\"Todo\",\"n\":1,\"section\":\"Today\",\"text\":\"Say \\\"hi\\\" \\\\ back\\u0009slash {p7}, again {p7}, and {o1}\",\"jira\":[\"PROJ-7\",\"OPS-1\"]}},\n{{\"list\":\"Todo\",\"n\":2,\"section\":\"Later\",\"text\":\"Later one\",\"jira\":[]}}\n]\n"
        )
    );
    assert_eq!(
        s.t(&["list", "--later", "--json"]).stdout,
        "[\n{\"list\":\"Todo\",\"n\":2,\"section\":\"Later\",\"text\":\"Later one\",\"jira\":[]}\n]\n"
    );
    let off = s.t_with(&["list", "--today", "--json"], &[("TODO_JIRA_KEYS", "")]);
    assert!(off.stdout.contains("\"jira\":[]"));
}

#[test]
fn add_links_keys_and_leaves_the_rest() {
    let s = Sandbox::new(EMPTY);
    assert_eq!(
        s.t(&["add", "Retest PROJ-1877 and OPS-12."]).stdout,
        format!(
            "Added 1 (Today): Retest {} and {}.\n",
            l("PROJ-1877"),
            l("OPS-12")
        )
    );
    let plain = "Leave UTF-8, ISO-8601, ABC-12, xPROJ-4, PROJ-5a, `PROJ-3`, https://example.com/PROJ-2 and [see PROJ-6](https://example.com) alone";
    assert_eq!(
        s.t(&["add", plain]).stdout,
        format!("Added 2 (Today): {plain}\n")
    );
}

#[test]
fn keys_typed_in_an_editor_are_linked_by_tidy() {
    let t = today();
    let s = Sandbox::new(&format!(
        "## Today\n- [ ] Typed in an editor PROJ-10\nNotes about PROJ-11\n\n## Done\n- [x] {t} Shipped WEB-7\n"
    ));
    assert_eq!(
        s.t(&[]).stdout,
        format!("  1  Today  Typed in an editor {}\n", l("PROJ-10"))
    );
    assert_eq!(
        s.file(),
        format!(
            "## Today\n- [ ] Typed in an editor {}\nNotes about PROJ-11\n\n## Done\n- [x] {t} Shipped {}\n",
            l("PROJ-10"),
            l("WEB-7")
        )
    );
    let before = s.file();
    s.t(&[]);
    assert_eq!(s.file(), before, "linking is idempotent");
}

#[test]
fn linking_off() {
    let off = "Fix offset -42 for PROJ-1 and OPS-2";
    for keys in ["", " "] {
        let s = Sandbox::new(EMPTY);
        let out = s.t_with(&["add", off], &[("TODO_JIRA_KEYS", keys)]);
        assert_eq!(out.all(), format!("0|Added 1 (Today): {off}\n"));
    }
    let mut s = Sandbox::new(EMPTY);
    s.unset("TODO_JIRA_KEYS");
    assert_eq!(
        s.t(&["add", off]).all(),
        format!("0|Added 1 (Today): {off}\n")
    );

    let s = Sandbox::new(&format!(
        "## Today\n- [ ] Linked {} and bare PROJ-2\n\n## Done\n",
        l("PROJ-1")
    ));
    let before = s.file();
    s.t_with(&[], &[("TODO_JIRA_KEYS", "")]);
    assert_eq!(s.file(), before);
}

#[test]
fn wrong_settings_warn_and_turn_linking_off() {
    let off = "Fix offset -42 for PROJ-1 and OPS-2";
    let cases = [
        (
            ("TODO_JIRA_URL", "https://x.example/browse"),
            "TODO_JIRA_URL must start with http:// or https:// and end with / (got 'https://x.example/browse'); Jira linking is off",
        ),
        (
            ("TODO_JIRA_URL", "x.example/browse/"),
            "TODO_JIRA_URL must start with http:// or https:// and end with / (got 'x.example/browse/'); Jira linking is off",
        ),
        (
            ("TODO_JIRA_URL", ""),
            "TODO_JIRA_KEYS is set but TODO_JIRA_URL isn't; Jira linking is off",
        ),
        (
            ("TODO_JIRA_KEYS", "PROJ,OPS WEB"),
            "TODO_JIRA_KEYS: 'PROJ,OPS' isn't a Jira project key (use uppercase keys separated by spaces); Jira linking is off",
        ),
        (
            ("TODO_JIRA_KEYS", "proj A.B"),
            "TODO_JIRA_KEYS: 'proj', 'A.B' isn't a Jira project key (use uppercase keys separated by spaces); Jira linking is off",
        ),
    ];
    for (setting, warning) in cases {
        let s = Sandbox::new(EMPTY);
        let out = s.t_with(&["add", off], &[setting]);
        assert_eq!(out.stderr, format!("t: {warning}\n"), "{setting:?}");
        assert_eq!(out.stdout, format!("Added 1 (Today): {off}\n"));
    }
    let s = Sandbox::new(EMPTY);
    let out = s.t_with(
        &["add", "PROJ-1 P2_X-2 PROJX-3"],
        &[("TODO_JIRA_KEYS", "  PROJ   P2_X  ")],
    );
    assert_eq!(
        out.all(),
        format!(
            "0|Added 1 (Today): {} {} PROJX-3\n",
            l("PROJ-1"),
            l("P2_X-2")
        )
    );
}
