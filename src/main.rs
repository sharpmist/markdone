//! t: personal todo lists kept as Markdown files, one file per list, in one folder.
//!
//! The Rust port of the zsh script `bin/t`; see `docs/designs/rust_port.md`. Until the port is
//! done, only `help` and `version` are implemented here.

// Modules are declared as each port step adds them; the attribute goes once dispatch uses them
// (step S7).
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod doc;
#[allow(dead_code)]
mod jira;
#[allow(dead_code)]
mod lock;
#[allow(dead_code)]
mod store;
#[allow(dead_code)]
mod text;

use std::io::{self, Write};
use std::process::ExitCode;

/// 0.3.0's usage text, unchanged (`usage()` in `bin/t`).
const USAGE: &str = r#"Usage:
  t [list]                  list open items, numbered          (aliases: ls, l)
  t list --today | --later  list only Today's or Later's open items (same numbers as t list)
  t list --done [--all]     list done items, newest first; --all includes the archives
  t list --json             open items as JSON (also with --today or --later)
  t add [--later] <text>    add an item to Today, or to Later  (alias: a; -l = --later)
  t done <item>             mark an item done                  (alias: d)
  t today <item>            move an item from Later to Today   (alias: promote)
  t later <item>            move an item from Today to Later   (alias: demote)
  t rm <item>               remove an open item                (alias: remove)
  t reword <item> <text>    replace an open item's text, keeping its place
  t move <item> @<list>     move an item to another list, same section (alias: mv)
  t pick                    choose an item with fzf, mark done (alias: f)
  t edit                    open the list in $EDITOR           (alias: e)
  t undo                    undo the last change; run it again to redo
  t init [<path>]           create the list with its three headings (never overwrites)
  t init --obsidian         create the list in an Obsidian vault you pick, in its Todo folder

  t lists [--json]          show the lists, with open items; * marks the current one
  t lists add <name>        create another list, next to the default one
  t lists rm [--force] <name>   delete a list (with open items only with --force)
  t lists rename <old> <new>    rename a list and its archives
  t use [<list>]            switch the current list (alone: pick one with fzf)
  t @<list> <command>       run one command on another list, e.g. t @work add "Fix the build"

  t help                    show this help                     (-h, --help)
  t version                 show the version                   (--version)
  t commands                list commands with examples, tab-separated (used by Tab completion)
  t complete <words>        list what Tab offers after t <words>, tab-separated (used by Tab completion)

<item> is a number from t list, or text that matches exactly one open item
(case-insensitive, any part of it), e.g. t done "logout fix".
For reword, quote an <item> given as text with spaces: t reword "logout" "New text".

A command without @<list> uses TODO_LIST if set, else the list t use chose,
else the default list (TODO_FILE). Every .md file in its folder is a list.

Commands that read a list first tidy it: items ticked in an editor get
today's date and move to Done; Done items older than 7 days move to
"Archive/<list> <year>.md" in the lists folder; Jira keys like PROJ-123 become
links when TODO_JIRA_KEYS and TODO_JIRA_URL are set.
"#;

/// The exit status when stdout is a closed pipe (`t help | head -1`): what a shell reports for a
/// process killed by SIGPIPE, as the zsh version is. Windows has no SIGPIPE, so 1 there.
#[cfg(unix)]
const BROKEN_PIPE_STATUS: u8 = 141;
#[cfg(windows)]
const BROKEN_PIPE_STATUS: u8 = 1;

/// Writes to stdout. A closed pipe ends the run quietly; any other error is reported.
fn out(text: &str) -> Result<(), ExitCode> {
    let mut stdout = io::stdout().lock();
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Err(ExitCode::from(BROKEN_PIPE_STATUS)),
        Err(e) => {
            eprintln!("t: could not write the output: {e}");
            Err(ExitCode::FAILURE)
        }
    }
}

fn run(args: &[String]) -> Result<(), ExitCode> {
    match args.first().map(String::as_str) {
        Some("-h" | "--help" | "help") => out(USAGE),
        Some("--version" | "version") => out(&format!("t {}\n", env!("CARGO_PKG_VERSION"))),
        Some(cmd) => {
            eprintln!("t: '{cmd}' isn't in the Rust port yet; use bin/t");
            Err(ExitCode::FAILURE)
        }
        None => {
            eprintln!("t: listing isn't in the Rust port yet; use bin/t");
            Err(ExitCode::FAILURE)
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = match std::env::args_os()
        .skip(1)
        .map(|a| a.into_string())
        .collect::<Result<_, _>>()
    {
        Ok(args) => args,
        // Byte arguments on Unix are handled when the list commands arrive (step S7).
        Err(_) => {
            eprintln!("t: arguments must be valid text");
            return ExitCode::FAILURE;
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_matches_the_zsh_version() {
        // bin/t prints its usage from a heredoc; the Rust text must stay identical until the
        // script is removed (step S12).
        // Git on Windows may check bin/t out with CRLF line endings; compare the text only.
        let script = include_str!("../bin/t").replace("\r\n", "\n");
        let usage = USAGE.replace("\r\n", "\n");
        let start = script
            .find("cat <<'EOF'\n")
            .expect("usage heredoc in bin/t")
            + 12;
        let end = start + script[start..].find("\nEOF\n").expect("end of heredoc") + 1;
        assert_eq!(usage, &script[start..end]);
    }
}
