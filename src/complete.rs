//! Tab completion data for `shell/t.zsh`: `t commands` and `t complete <words>`, one line per
//! choice, `<text to insert>\t<what fzf shows>`. See the output contracts in bin/EGM.md.

use std::ffi::OsStr;

use crate::doc::{Doc, row};
use crate::lists::Lists;
use crate::text;

/// The commands that take an `<item>`, with their aliases.
pub const ITEM_CMDS: [&[u8]; 11] = [
    b"done", b"d", b"today", b"promote", b"later", b"demote", b"rm", b"remove", b"reword", b"move",
    b"mv",
];

/// One line per command: insert text, shown text, description, example.
const COMMANDS: [(&str, &str, &str, &str); 27] = [
    ("list", "list", "List open items, numbered", "list"),
    (
        "list --today",
        "list --today",
        "List only Today's open items",
        "list --today",
    ),
    (
        "list --later",
        "list --later",
        "List only Later's open items",
        "list --later",
    ),
    (
        "list --json",
        "list --json",
        "List open items as JSON (for scripts)",
        "list --json",
    ),
    (
        "list --done",
        "list --done",
        "List items done in the last 7 days",
        "list --done",
    ),
    (
        "list --done --all",
        "list --done --all",
        "List all done items, incl. archives",
        "list --done --all",
    ),
    (
        "add \"\"",
        "add \"<text>\"",
        "Add an item to Today",
        "add \"Retest logout fix PROJ-123\"",
    ),
    (
        "add --later \"\"",
        "add --later \"<text>\"",
        "Add an item to Later",
        "add --later \"Clean up old branches\"",
    ),
    (
        "done ",
        "done <item>",
        "Mark an item done (number or text)",
        "done \"logout fix\"",
    ),
    (
        "today ",
        "today <item>",
        "Move an item from Later to Today",
        "today 3",
    ),
    (
        "later ",
        "later <item>",
        "Move an item from Today to Later",
        "later 1",
    ),
    (
        "rm ",
        "rm <item>",
        "Remove an item (t undo brings it back)",
        "rm \"old idea\"",
    ),
    (
        "reword ",
        "reword <item> \"<text>\"",
        "Change an item's text",
        "reword 2 \"Retest logout fix\"",
    ),
    (
        "move ",
        "move <item> @<list>",
        "Move an item to another list",
        "move \"build\" @Work",
    ),
    (
        "pick",
        "pick",
        "Pick an open item with fzf, mark it done",
        "pick",
    ),
    ("edit", "edit", "Open the list in your editor", "edit"),
    (
        "undo",
        "undo",
        "Undo the last change (again to redo)",
        "undo",
    ),
    ("init", "init", "Create the list (never overwrites)", "init"),
    (
        "init --obsidian",
        "init --obsidian",
        "Create the list in an Obsidian vault",
        "init --obsidian",
    ),
    (
        "lists",
        "lists",
        "Show your lists; * marks the current one",
        "lists",
    ),
    (
        "lists add \"\"",
        "lists add \"<name>\"",
        "Create another list",
        "lists add \"Work\"",
    ),
    (
        "lists rm ",
        "lists rm <name>",
        "Delete a list (only without open items)",
        "lists rm Work",
    ),
    (
        "lists rename ",
        "lists rename <old> <new>",
        "Rename a list and its archives",
        "lists rename Work Job",
    ),
    ("use ", "use <list>", "Switch the current list", "use Work"),
    (
        "@",
        "@<list> <command>",
        "Run one command on another list",
        "@Work add \"Fix the build\"",
    ),
    ("version", "version", "Show the version", "version"),
    ("help", "help", "Show help", "help"),
];

/// `t commands`: `printf '%s\t%-22s %-42s e.g. t %s\n'` for each command.
pub fn commands() -> Vec<u8> {
    COMMANDS
        .iter()
        .map(|(ins, shown, desc, ex)| format!("{ins}\t{shown:<22} {desc:<42} e.g. t {ex}\n"))
        .collect::<String>()
        .into_bytes()
}

/// What `t complete <words>` does.
#[derive(Debug, PartialEq, Eq)]
pub enum Complete {
    /// Print this and exit, without reading any list.
    Print(Vec<u8>),
    /// Offer the open items of a list (`sel`, or the current one) for this item command; the
    /// caller locks and tidies the list first, as for `t list`.
    Items { sel: Option<Vec<u8>>, cmd: Vec<u8> },
}

/// The lists, `<prefix><name, quoted for the shell> \t<the t lists line>`. Nothing when there are
/// no lists or `TODO_LIST` names none.
fn list_choices(lists: &Lists, todo_list: Option<&OsStr>, prefix: &[u8]) -> Vec<u8> {
    let Ok(current) = lists.current(todo_list, &mut Vec::new()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in lists.lines(&current, true) {
        let tab = line.iter().position(|&b| b == b'\t').unwrap_or(line.len());
        out.extend_from_slice(prefix);
        out.extend(text::shell_quote(&line[..tab]));
        out.extend_from_slice(b" ");
        out.extend_from_slice(&line[tab..]);
        out.push(b'\n');
    }
    out
}

/// What Tab offers after `t <words> `:
///   `@`                         the lists, inserted as `@<name>`
///   `@<list>`                   the commands, as after `t `
///   `[@<list>] <item command>`  that list's open items
///   `use`, `lists rm`, `lists rename`  the lists
///   `move <item>`               the lists, as `@<name>`
///   `lists`                     its subcommands
///   anything else               nothing
pub fn words(lists: &Lists, todo_list: Option<&OsStr>, words: &[Vec<u8>]) -> Complete {
    let mut w = words;
    let mut sel = None;
    if let Some(first) = w.first()
        && first.len() > 1
        && first[0] == b'@'
    {
        sel = Some(first[1..].to_vec());
        w = &w[1..];
    }
    let joined = w.join(&b' ');
    let print = Complete::Print;
    match joined.as_slice() {
        b"@" => return print(list_choices(lists, todo_list, b"@")),
        b"" => {
            return print(if sel.is_some() {
                commands()
            } else {
                Vec::new()
            });
        }
        b"use" | b"lists rm" | b"lists remove" | b"lists rename" => {
            return print(list_choices(lists, todo_list, b""));
        }
        b"lists" => {
            return print(
                b"add \"\"\tadd \"<name>\"          Create a list\nrm \trm <name>             Delete a list\nrename \trename <old> <new>    Rename a list\n--json\t--json                The lists as JSON\n"
                    .to_vec(),
            );
        }
        _ => {}
    }
    // `move <item> …` offers the lists after the item; `move` alone offers the items (below).
    if w.len() >= 2 && matches!(w[0].as_slice(), b"move" | b"mv") {
        return print(if w.len() == 2 {
            list_choices(lists, todo_list, b"@")
        } else {
            Vec::new()
        });
    }
    if w.len() == 1 && ITEM_CMDS.contains(&w[0].as_slice()) {
        return Complete::Items {
            sel,
            cmd: w[0].clone(),
        };
    }
    print(Vec::new())
}

/// The open items `cmd` can take, `<number>[ ""]\t<the t list row, links shown as their text>`:
/// `today` offers Later's items and `later` Today's; for `reword` the insert ends in `""` so the
/// cursor lands between the quotes for the new text.
pub fn items(doc: &Doc, cmd: &[u8]) -> Vec<u8> {
    let (only, suffix): (Option<&[u8]>, &str) = match cmd {
        b"today" | b"promote" => (Some(b"Later"), ""),
        b"later" | b"demote" => (Some(b"Today"), ""),
        b"reword" => (None, " \"\""),
        _ => (None, ""),
    };
    let mut out = Vec::new();
    for it in doc.open_items() {
        if only.is_some_and(|s| s != it.section) {
            continue;
        }
        out.extend(format!("{}{suffix}\t", it.n).into_bytes());
        out.extend(row(it.n, &it.section, &text::shown(&it.text)));
        out.push(b'\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_match_the_zsh_version() {
        // bin/t prints the same table with printf; compare until the script is removed (S12).
        let script = include_str!("../bin/t").replace("\r\n", "\n");
        let start = script.find("  for ins shown desc ex in \\\n").unwrap();
        let end = start + script[start..].find("  do\n").unwrap();
        let rows: Vec<&str> = script[start..end].lines().skip(1).collect();
        assert_eq!(rows.len(), COMMANDS.len(), "one row per command");
        let text = String::from_utf8(commands()).unwrap();
        for (line, (ins, shown, desc, ex)) in text.lines().zip(COMMANDS) {
            assert!(line.starts_with(&format!("{ins}\t")));
            assert!(line.ends_with(&format!(" e.g. t {ex}")));
            assert!(line.contains(shown) && line.contains(desc));
        }
        for (r, (ins, shown, desc, ex)) in rows.iter().zip(COMMANDS) {
            for field in [ins, shown, desc, ex] {
                let quoted = if field.contains('\'') {
                    format!("\"{field}\"")
                } else {
                    format!("'{field}'")
                };
                assert!(r.contains(&quoted), "{field:?} not in {r:?}");
            }
        }
    }

    #[test]
    fn item_choices() {
        let doc = Doc::parse(
            b"## Today\n- [ ] Ship [PROJ-1](https://jira.example.com/browse/PROJ-1)\n## Later\n- [ ] Later one\n",
        );
        assert_eq!(
            items(&doc, b"done"),
            b"1\t  1  Today  Ship PROJ-1\n2\t  2  Later  Later one\n".to_vec()
        );
        assert_eq!(
            items(&doc, b"today"),
            b"2\t  2  Later  Later one\n".to_vec()
        );
        assert_eq!(
            items(&doc, b"demote"),
            b"1\t  1  Today  Ship PROJ-1\n".to_vec()
        );
        assert_eq!(
            items(&doc, b"reword"),
            b"1 \"\"\t  1  Today  Ship PROJ-1\n2 \"\"\t  2  Later  Later one\n".to_vec()
        );
    }
}
