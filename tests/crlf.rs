//! Lists with Windows line endings and a byte order mark: new in 0.4.0 (0.3.0 fails on them).

mod common;
use common::*;
use std::fs;

#[test]
fn a_crlf_list_works_and_keeps_its_endings() {
    let t = today();
    let s = Sandbox::new("## Today\r\n- [ ] a\r\n\r\n## Later\r\n\r\n## Done\r\n");
    assert_eq!(
        s.t(&["list"]).stdout,
        "  1  Today  a\n",
        "no CR in the output"
    );
    assert_eq!(s.t(&["add", "x"]).stdout, "Added 2 (Today): x\n");
    assert_eq!(s.t(&["done", "1"]).stdout, "Done: a\n");
    assert_eq!(
        s.file(),
        format!("## Today\r\n- [ ] x\r\n\r\n## Later\r\n\r\n## Done\r\n- [x] {t} a\r\n")
    );
    assert_eq!(
        s.t(&["list", "--json"]).stdout,
        "[\n{\"list\":\"Todo\",\"n\":1,\"section\":\"Today\",\"text\":\"x\",\"jira\":[]}\n]\n"
    );
    assert_eq!(
        s.t(&["done", "x"]).stdout,
        "Done: x\n",
        "matching ignores the CR"
    );
}

#[test]
fn a_ticked_item_in_a_crlf_list_is_tidied() {
    let t = today();
    let s = Sandbox::new("## Today\r\n- [x] ticked\r\n\r\n## Done\r\n");
    assert_eq!(s.t(&[]).all(), "0|");
    assert_eq!(
        s.file(),
        format!("## Today\r\n\r\n## Done\r\n- [x] {t} ticked\r\n")
    );
}

#[test]
fn archives_of_a_crlf_list_are_crlf() {
    let s = Sandbox::new("## Today\r\n\r\n## Done\r\n- [x] 2020-05-01 Old\r\n");
    s.t(&[]);
    assert_eq!(
        fs::read(s.lists.join("Archive").join("Todo 2020.md")).unwrap(),
        b"# Todo archive 2020\r\n\r\n- [x] 2020-05-01 Old\r\n"
    );
}

#[test]
fn moving_between_lists_takes_the_target_files_ending() {
    let s = Sandbox::new("## Today\n- [ ] milk\n\n## Done\n");
    fs::write(s.lists.join("Work.md"), "## Today\r\n\r\n## Done\r\n").unwrap();
    assert_eq!(
        s.t(&["move", "milk", "@Work"]).stdout,
        "Moved to Work (Today) as 1: milk\n"
    );
    assert_eq!(
        read(&s.lists.join("Work.md")),
        "## Today\r\n- [ ] milk\r\n\r\n## Done\r\n"
    );
}

#[test]
fn a_byte_order_mark_is_ignored_and_kept() {
    let s = Sandbox::new("\u{feff}## Today\n\n## Later\n\n## Done\n");
    assert_eq!(s.t(&["add", "x"]).stdout, "Added 1 (Today): x\n");
    assert_eq!(
        s.file(),
        "\u{feff}## Today\n- [ ] x\n\n## Later\n\n## Done\n"
    );
}

#[test]
fn a_list_that_isnt_utf8_works_as_in_0_3() {
    let s = Sandbox::new(EMPTY);
    fs::write(&s.list, b"## Today\n- [ ] caf\xe9\n\n## Done\n").unwrap();
    assert_eq!(s.t(&["add", "x"]).stdout, "Added 2 (Today): x\n");
    assert_eq!(
        fs::read(&s.list).unwrap(),
        b"## Today\n- [ ] caf\xe9\n- [ ] x\n\n## Done\n"
    );
}
