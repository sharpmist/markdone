//! t: personal todo lists kept as Markdown files, one file per list, in one folder.
//!
//! The Rust port of the zsh script `bin/t`; see `docs/designs/rust_port.md`. `init`, `pick`,
//! `edit` and `use` without a name arrive in later steps (S8, S9).

mod complete;
mod config;
mod doc;
mod init;
mod jira;
mod lists;
mod lock;
mod store;
mod text;

use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use complete::Complete;
use config::{Settings, os_bytes};
use doc::{Doc, Error as DocError, Tidied};
use jira::Jira;
use lists::{List, Lists, lossy};
use lock::{Lock, What};

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

/// The exit status when stdout is a closed pipe (`t list | head -1`): what a shell reports for a
/// process killed by SIGPIPE, as the zsh version is. Windows has no SIGPIPE, so 1 there.
#[cfg(unix)]
const BROKEN_PIPE_STATUS: u8 = 141;
#[cfg(windows)]
const BROKEN_PIPE_STATUS: u8 = 1;

/// How a run ends early.
#[derive(Debug)]
enum Fail {
    /// `t: <message>` on stderr, exit 1.
    Die(String),
    /// Exit with this status, printing nothing more.
    Exit(u8),
}

impl From<String> for Fail {
    fn from(m: String) -> Fail {
        Fail::Die(m)
    }
}

impl From<&str> for Fail {
    fn from(m: &str) -> Fail {
        Fail::Die(m.to_string())
    }
}

type R<T = ()> = Result<T, Fail>;

fn die<T>(message: impl Into<String>) -> R<T> {
    Err(Fail::Die(message.into()))
}

/// `t: <message>` on stderr; the run continues.
fn warn(message: &str) {
    eprintln!("t: {message}");
}

fn warn_all(messages: &mut Vec<String>) {
    for m in messages.drain(..) {
        warn(&m);
    }
}

/// Writes to stdout. A closed pipe ends the run quietly; any other error is reported.
fn out(bytes: &[u8]) -> R {
    let mut stdout = io::stdout().lock();
    match stdout.write_all(bytes).and_then(|()| stdout.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Err(Fail::Exit(BROKEN_PIPE_STATUS)),
        Err(e) => die(format!("could not write the output: {e}")),
    }
}

/// Writes a line to stdout.
fn out_line(line: &[u8]) -> R {
    out(&[line, b"\n"].concat())
}

/// The command line: bytes as given on Unix; text on Windows.
fn args() -> R<Vec<Vec<u8>>> {
    let raw: Vec<OsString> = std::env::args_os().skip(1).collect();
    if cfg!(windows) {
        raw.into_iter()
            .map(|a| {
                a.into_string()
                    .map(String::into_bytes)
                    .map_err(|_| Fail::from("arguments must be valid text"))
            })
            .collect()
    } else {
        Ok(raw.iter().map(|a| os_bytes(a)).collect())
    }
}

fn is(a: Option<&Vec<u8>>, words: &[&[u8]]) -> bool {
    a.is_some_and(|a| words.contains(&a.as_slice()))
}

/// The locks a run holds, at most one per lock file (the standard lock belongs to a handle, so a
/// second one in the same run would wait for the first).
#[derive(Default)]
struct Locks(Vec<(PathBuf, Lock)>);

impl Locks {
    fn take(&mut self, path: &Path) -> R {
        if !self.0.iter().any(|(p, _)| p == path) {
            let lock = lock::take(path, What::List)?;
            self.0.push((path.to_path_buf(), lock));
        }
        Ok(())
    }

    fn release(&mut self, path: &Path) {
        self.0.retain(|(p, _)| p != path);
    }
}

/// Everything a list command needs after the list is chosen, locked and tidied.
struct Run {
    lists: Lists,
    list: List,
    doc: Doc,
    jira: Option<Jira>,
    today: String,
    styled: bool,
}

impl Run {
    /// A message or row for stdout, with web links clickable on a terminal.
    fn print(&self, line: &[u8]) -> R {
        out_line(&text::render(line, self.styled))
    }

    fn save(&self, list: &List, doc: &Doc) -> R {
        if let Some(w) = store::save(&list.files, &doc.to_bytes())? {
            warn(&w);
        }
        Ok(())
    }

    /// Text for an item from the command line: linked, one line, not empty.
    fn item_text(&self, text: Vec<u8>, usage: &str) -> R<Vec<u8>> {
        if text.is_empty() {
            return die(usage);
        }
        if text.contains(&b'\n') {
            return die("item must be a single line");
        }
        Ok(match &self.jira {
            Some(j) => jira::linkify(&text, j),
            None => text,
        })
    }

    /// The number of the open item `words` names, and the reference as messages show it.
    fn item(&self, cmd: &str, words: &[Vec<u8>]) -> R<(usize, String)> {
        let reference = words.join(&b' ');
        if reference.is_empty() {
            return die(format!("usage: t {cmd} <item>"));
        }
        let shown = lossy(&reference);
        match self.doc.find(&reference) {
            Ok(n) if doc::number(&reference).is_some() => Ok((n, shown)),
            Ok(n) => Ok((n, n.to_string())),
            Err(DocError::Ambiguous(rows)) => {
                let rows: Vec<String> = rows.iter().map(|r| lossy(r)).collect();
                die(format!(
                    "'{shown}' matches {} open items; use more of the text, or the number:\n{}",
                    rows.len(),
                    rows.join("\n")
                ))
            }
            Err(_) => die(format!("no open item matches '{shown}'")),
        }
    }

    fn no_heading(&self, list: &List, section: &[u8]) -> String {
        format!(
            "no '## {}' heading in {}",
            lossy(section),
            list.file().display()
        )
    }
}

/// Reads and tidies a list, saving when tidying changed it, and returns it as it now is. Archives
/// are written first; the list is rewritten only if they all were.
fn tidy(lists: &Lists, list: &List, today: &str, cutoff: &str, jira: Option<&Jira>) -> R<Doc> {
    let bytes =
        fs::read(list.file()).map_err(|_| format!("could not read {}", list.file().display()))?;
    let original = Doc::parse(&bytes);
    let mut doc = original.clone();
    match doc.tidy(today, cutoff, jira) {
        Tidied::NoDoneHeading => {
            warn("ticked items found but no '## Done' heading after them; list not tidied");
            Ok(original)
        }
        Tidied::Done { archived } => {
            let new = doc.to_bytes();
            if new == bytes {
                return Ok(doc);
            }
            if let Err(af) = store::archive(&lists.archive_dir, &list.name, &archived, doc.crlf()) {
                warn(&format!(
                    "could not write {}; list not tidied",
                    af.display()
                ));
                return Ok(original);
            }
            if let Some(w) = store::save(&list.files, &new)? {
                warn(&w);
            }
            // Undoing would put the archived items back, and the next tidy would archive them a
            // second time; so a tidy that archived can't be undone.
            if !archived.is_empty() {
                let _ = fs::remove_file(&list.files.bak);
            }
            Ok(doc)
        }
    }
}

fn run() -> R {
    let all = args()?;
    let mut a: &[Vec<u8>] = &all;

    // @<list> as the first argument: the list this one command works on.
    let mut sel: Option<Vec<u8>> = None;
    if let Some(first) = a.first()
        && first.first() == Some(&b'@')
    {
        if first.len() == 1 {
            return die("usage: t @<list> <command>, e.g. t @work add \"Fix the build\"");
        }
        sel = Some(first[1..].to_vec());
        a = &a[1..];
    }
    let cmd: Vec<u8> = a.first().cloned().unwrap_or_default();

    // Commands that need no settings.
    match cmd.as_slice() {
        b"-h" | b"--help" | b"help" => return out(USAGE.as_bytes()),
        b"--version" | b"version" => {
            return out_line(format!("t {}", env!("CARGO_PKG_VERSION")).as_bytes());
        }
        b"commands" => return out(&complete::commands()),
        _ => {}
    }
    if let (b"init" | b"lists" | b"use", Some(s)) = (cmd.as_slice(), &sel) {
        return die(format!(
            "t {} works on the lists themselves; it doesn't take @{}",
            lossy(&cmd),
            lossy(s)
        ));
    }

    let settings = config::load()?;
    for w in &settings.warnings {
        warn(w);
    }
    if let Some(w) = config::upgrade_warning(&settings) {
        warn(&w);
    }
    let lists = Lists::new(&settings.todo_file);
    let todo_list = settings.todo_list.as_deref();

    // Commands that work on the lists themselves.
    let mut complete_cmd = None;
    match cmd.as_slice() {
        b"init" => {
            let mut warnings = Vec::new();
            let result = init::init(&a[1..], &settings, &mut warnings);
            if let Ok(text) = &result {
                out(text.as_bytes())?;
            }
            warn_all(&mut warnings);
            result?;
            return Ok(());
        }
        b"lists" => return lists_cmd(&lists, &settings, &a[1..]),
        b"use" => return use_cmd(&lists, &settings, &a[1..]),
        b"complete" => match complete::words(&lists, todo_list, &a[1..]) {
            Complete::Print(text) => return out(&text),
            Complete::Items { sel: s, cmd: c } => {
                if s.is_some() {
                    sel = s;
                }
                complete_cmd = Some(c);
            }
        },
        _ => {}
    }

    // The list this run works on.
    let mut warnings = Vec::new();
    let name = match &sel {
        Some(s) => lists
            .find(s)
            .ok_or_else(|| format!("no list called '{}'; t lists shows them", lossy(s)))?,
        None => lists.current(todo_list, &mut warnings)?,
    };
    warn_all(&mut warnings);
    let list = lists.list(&name);
    if !list.file().is_file() {
        if name == lists.default_name {
            return die(format!(
                "{} not found; create it with 't init', or set TODO_FILE to your list",
                list.file().display()
            ));
        }
        return die(format!(
            "the list {} is gone ({} not found)",
            list.shown(),
            list.file().display()
        ));
    }

    // t move <item> @<list>: the target list is found now, as both lists are locked below.
    let moving = is(a.first(), &[b"move", b"mv"]);
    let dest = if moving {
        let last = a
            .last()
            .filter(|l| a.len() >= 3 && l.len() > 1 && l[0] == b'@');
        let Some(last) = last else {
            return die("usage: t move <item> @<list>");
        };
        let d = lists
            .find(&last[1..])
            .ok_or_else(|| format!("no list called '{}'; t lists shows them", lossy(&last[1..])))?;
        if d == name {
            return die(format!("the item is already in {}", list.shown()));
        }
        Some(lists.list(&d))
    } else {
        None
    };

    // Archives from before there were several lists belong to the default list; move them under
    // its lock, keeping that lock when this run needs it anyway.
    let mut locks = Locks::default();
    if !lists.old_archives().is_empty() {
        let default = lists.list(&lists.default_name);
        locks.take(&default.lock)?;
        lists.migrate_archives(&mut warnings);
        warn_all(&mut warnings);
        let needed = name == lists.default_name
            || dest.as_ref().is_some_and(|d| d.name == lists.default_name);
        if !needed {
            locks.release(&default.lock);
        }
    }

    // Lock: both lists for move, in the byte order of their files' paths, so two moves in
    // opposite directions can't each hold one lock and wait for the other.
    match &dest {
        Some(d) => {
            let (first, second) =
                if os_bytes(list.file().as_os_str()) < os_bytes(d.file().as_os_str()) {
                    (&list, d)
                } else {
                    (d, &list)
                };
            locks.take(&first.lock)?;
            locks.take(&second.lock)?;
        }
        None => locks.take(&list.lock)?,
    }

    // undo runs before tidy: tidying first would be a change of its own, which undo would revert.
    if cmd == b"undo" {
        if let Some(w) = store::undo(&list.files)? {
            warn(&w);
        }
        return out_line(b"Undid the last change; run t undo again to redo it");
    }

    let (jira, jira_warning) = jira::check_jira(&settings.jira_keys, &settings.jira_url);
    if let Some(w) = jira_warning {
        warn(&w);
    }
    let today_date = config::today();
    let today = config::ymd(today_date);
    let cutoff = config::cutoff(today_date);
    let dest_doc = match &dest {
        Some(d) => Some(tidy(&lists, d, &today, &cutoff, jira.as_ref())?),
        None => None,
    };
    let doc = tidy(&lists, &list, &today, &cutoff, jira.as_ref())?;
    let run = Run {
        lists,
        list,
        doc,
        jira,
        today,
        styled: text::styled_stdout(),
    };

    let rest = if a.is_empty() { a } else { &a[1..] };
    match cmd.as_slice() {
        b"" | b"list" | b"ls" | b"l" => list_cmd(&run, rest),
        b"add" | b"a" => {
            let (section, words): (&[u8], _) = if is(rest.first(), &[b"--later", b"-l"]) {
                (b"Later", &rest[1..])
            } else {
                (b"Today", rest)
            };
            let text = run.item_text(words.join(&b' '), "usage: t add [--later] <text>")?;
            let mut doc = run.doc.clone();
            let n = doc
                .insert(section, &text)
                .map_err(|_| run.no_heading(&run.list, section))?;
            run.save(&run.list, &doc)?;
            run.print(
                &[
                    format!("Added {n} ({}): ", lossy(section)).as_bytes(),
                    &text,
                ]
                .concat(),
            )
        }
        b"done" | b"d" => {
            let (n, shown) = run.item("done", rest)?;
            let mut doc = run.doc.clone();
            let text = doc
                .finish(n, &run.today)
                .map_err(|_| format!("no open item {shown} (or no '## Done' heading)"))?;
            run.save(&run.list, &doc)?;
            run.print(&[b"Done: ", text.as_slice()].concat())
        }
        b"today" | b"promote" => move_within(&run, b"Today", "today", rest),
        b"later" | b"demote" => move_within(&run, b"Later", "later", rest),
        b"rm" | b"remove" => {
            let (n, shown) = run.item("rm", rest)?;
            let mut doc = run.doc.clone();
            let text = doc.remove(n).map_err(|_| format!("no open item {shown}"))?;
            run.save(&run.list, &doc)?;
            run.print(&[b"Removed: ", text.as_slice()].concat())
        }
        b"reword" => {
            let usage = "usage: t reword <item> <new text>";
            let new_words = rest.get(1..).unwrap_or_default();
            if rest.first().is_none_or(|r| r.is_empty()) || new_words.join(&b' ').is_empty() {
                return die(usage);
            }
            let (n, shown) = run.item("reword", &rest[..1])?;
            let text = run.item_text(new_words.join(&b' '), usage)?;
            let mut doc = run.doc.clone();
            let section = doc
                .reword(n, &text)
                .map_err(|_| format!("no open item {shown}"))?;
            run.save(&run.list, &doc)?;
            run.print(
                &[
                    format!("Reworded {n} ({}): ", lossy(&section)).as_bytes(),
                    &text,
                ]
                .concat(),
            )
        }
        b"move" | b"mv" => {
            let dest = dest.expect("found above");
            let mut dest_doc = dest_doc.expect("tidied above");
            let (n, shown) = run.item("move", &rest[..rest.len() - 1])?;
            let item = run
                .doc
                .take(n)
                .map_err(|_| format!("no open item {shown}"))?;
            // Added to the target first, then removed here, each saved: a failure in between
            // leaves the item in both lists, never in neither.
            let at = dest_doc
                .insert(&item.section, &item.text)
                .map_err(|_| format!("{}; nothing moved", run.no_heading(&dest, &item.section)))?;
            run.save(&dest, &dest_doc)?;
            let mut doc = run.doc.clone();
            doc.remove(n).map_err(|_| {
                format!(
                    "could not remove item {shown} from {}; it's now in both {} and {}",
                    run.list.file().display(),
                    run.list.shown(),
                    dest.shown()
                )
            })?;
            run.save(&run.list, &doc)?;
            run.print(
                &[
                    format!(
                        "Moved to {} ({}) as {at}: ",
                        dest.shown(),
                        lossy(&item.section)
                    )
                    .as_bytes(),
                    &item.text,
                ]
                .concat(),
            )
        }
        b"complete" => out(&complete::items(
            &run.doc,
            &complete_cmd.expect("set above"),
        )),
        b"pick" | b"f" => die("t pick isn't in the Rust port yet; use bin/t"),
        b"edit" | b"e" => die("t edit isn't in the Rust port yet; use bin/t"),
        _ => {
            eprintln!("t: unknown command '{}'", lossy(&cmd));
            eprint!("{USAGE}");
            Err(Fail::Exit(1))
        }
    }
}

/// `t today` and `t later`: moves an item to the end of the other section, in one write.
fn move_within(run: &Run, to: &[u8], cmd: &str, rest: &[Vec<u8>]) -> R {
    let (n, shown) = run.item(cmd, rest)?;
    let mut doc = run.doc.clone();
    let (at, text) = doc.move_within(n, to).map_err(|e| match e {
        DocError::AlreadyIn(s) => format!("item {shown} is already in {}", lossy(&s)),
        DocError::NoHeading(s) => run.no_heading(&run.list, &s),
        _ => format!("no open item {shown}"),
    })?;
    run.save(&run.list, &doc)?;
    run.print(
        &[
            format!("Moved to {} as {at}: ", lossy(to)).as_bytes(),
            &text,
        ]
        .concat(),
    )
}

/// `t [list] [--today | --later | --done [--all] | --json]`.
fn list_cmd(run: &Run, options: &[Vec<u8>]) -> R {
    let (mut done, mut all, mut today, mut later, mut json) = (false, false, false, false, false);
    for o in options {
        match o.as_slice() {
            b"--done" => done = true,
            b"--all" => all = true,
            b"--today" => today = true,
            b"--later" => later = true,
            b"--json" => json = true,
            _ => {
                return die(format!(
                    "unknown option '{}' for list (expected --today, --later, --done, --all, --json)",
                    lossy(o)
                ));
            }
        }
    }
    if all && !done {
        return die("--all only works together with --done");
    }
    if done && (today || later) {
        return die("--today and --later list open items; they don't combine with --done");
    }
    if done && json {
        return die("--json lists open items; it doesn't combine with --done");
    }
    let section: Option<&[u8]> = match (today, later) {
        (true, false) => Some(b"Today"),
        (false, true) => Some(b"Later"),
        _ => None,
    };
    if done {
        let mut items = run.doc.done_items();
        if all {
            let (archived, mut warnings) =
                store::archived_items(&run.lists.archive_dir, &run.list.name);
            warn_all(&mut warnings);
            items.extend(archived);
        }
        for r in doc::done_rows(items) {
            run.print(&r)?;
        }
        return Ok(());
    }
    if json {
        let objects: Vec<String> = run
            .doc
            .open_items()
            .into_iter()
            .filter(|it| section.is_none_or(|s| s == it.section))
            .map(|it| {
                let keys: Vec<String> = match &run.jira {
                    Some(j) => jira::keys_in(&it.text, j)
                        .iter()
                        .map(|k| text::json_string(k))
                        .collect(),
                    None => Vec::new(),
                };
                format!(
                    r#"{{"list":{},"n":{},"section":{},"text":{},"jira":[{}]}}"#,
                    text::json_string(&run.list.name),
                    it.n,
                    text::json_string(&it.section),
                    text::json_string(&it.text),
                    keys.join(",")
                )
            })
            .collect();
        return out(if objects.is_empty() {
            "[]\n".to_string()
        } else {
            format!("[\n{}\n]\n", objects.join(",\n"))
        }
        .as_bytes());
    }
    // With several lists, a terminal shows which one this is; pipes and agents get only items.
    if run.styled && run.lists.names().len() > 1 {
        out_line(&text::bold(&run.list.name))?;
    }
    for r in run.doc.rows(section) {
        run.print(&r)?;
    }
    Ok(())
}

/// `t lists [--json]`, `t lists add|rm|rename …`.
fn lists_cmd(lists: &Lists, settings: &Settings, a: &[Vec<u8>]) -> R {
    let todo_list = settings.todo_list.as_deref();
    let rest = a.get(1..).unwrap_or_default();
    let mut warnings = Vec::new();
    match a.first().map(Vec::as_slice) {
        Some(b"add") => {
            if rest.len() != 1 {
                return die("usage: t lists add <name>");
            }
            out_line(lists.add(&rest[0])?.as_bytes())
        }
        Some(b"rm" | b"remove") => {
            let (force, names) = match rest.first().map(Vec::as_slice) {
                Some(b"--force") => (true, &rest[1..]),
                _ => (false, rest),
            };
            if names.len() != 1 {
                return die("usage: t lists rm [--force] <name>");
            }
            out_line(lists.remove(&names[0], force, todo_list)?.as_bytes())
        }
        Some(b"rename") => {
            if rest.len() != 2 {
                return die("usage: t lists rename <old name> <new name>");
            }
            let message = lists.rename(&rest[0], &rest[1], &mut warnings);
            warn_all(&mut warnings);
            out_line(message?.as_bytes())
        }
        _ => {
            if !(a.is_empty() || (a.len() == 1 && a[0] == b"--json")) {
                return die("usage: t lists [--json] | t lists add|rm|rename …");
            }
            let current = lists.current(todo_list, &mut warnings)?;
            warn_all(&mut warnings);
            if a.is_empty() {
                for line in lists.show(&current)? {
                    out_line(&line)?;
                }
                Ok(())
            } else {
                out(lists.json(&current).as_bytes())
            }
        }
    }
}

/// `t use <list>` (`t use` alone, with fzf, arrives in step S9).
fn use_cmd(lists: &Lists, settings: &Settings, a: &[Vec<u8>]) -> R {
    if a.len() > 1 {
        return die("usage: t use <list>");
    }
    let Some(name) = a.first() else {
        return die(
            "usage: t use <list> (in a terminal with fzf, t use alone picks one); t lists shows them",
        );
    };
    let mut warnings = Vec::new();
    let message = lists.use_list(name, settings.todo_list.as_deref(), &mut warnings)?;
    out_line(message.as_bytes())?;
    warn_all(&mut warnings);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(Fail::Die(message)) => {
            warn(&message);
            ExitCode::FAILURE
        }
        Err(Fail::Exit(code)) => ExitCode::from(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_matches_the_zsh_version() {
        // bin/t prints its usage from a heredoc; the Rust text must stay identical until the
        // script is removed (step S12). Git on Windows may check bin/t out with CRLF line
        // endings; compare the text only.
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
