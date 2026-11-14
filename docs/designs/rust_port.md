# Port `t` to Rust

*   **Author:** sharpmist
*   **Status:** Proposed
*   **Date:** 2026-11-14

--------------------------------------------------------------------------------

## Overview

`t` is a todo command that keeps lists in Markdown files. Version 0.3.0 is one zsh script,
`bin/t`, that calls awk and perl, and runs on macOS and Linux. This design replaces the script
with one Rust program that builds to a native binary for macOS, Linux and Windows.

Commands, output, exit codes, the file format and the hidden files stay as they are. The
differences are listed, all of them, in [What changes](#what-changes). The main ones: settings
come from the environment or a config file instead of `~/.zshenv`, lists with Windows line
endings work, and nothing but fzf and the editor is run as a separate program.

**Reference for current behaviour.** `bin/t` at the git tag `v0.3.0`, with its contract
[`bin/EGM.md`](../../bin/EGM.md) (EGM files are this repository's per-directory contracts: what a
directory offers, what must stay true, and what it changes). Where the two differ, `bin/t` wins.
This design names the contract's invariants by their bold names instead of repeating them, and
says "as in 0.3.0" for message texts it keeps; the exact text is in `bin/t` at `v0.3.0`. Both
files are deleted late in the port (step S12), so the tag stays the reference.

## The Problem

`t` can't run on Windows without WSL, because zsh doesn't run there natively. A person on
Windows, or an AI agent started from PowerShell, can't use it. Three things in the script are
tied to a Unix shell:

*   The script needs zsh, awk and perl, and standard tools such as `cksum` and `mktemp`.
*   Locking uses `zsystem flock`, a zsh module.
*   Settings live in `~/.zshenv`. This works only because every run of `t` starts zsh, which
    reads that file.

A native binary removes all three. The author chose Rust over Go and TypeScript; the reasons are
under [Alternatives Considered](#alternatives-considered).

## Goals

*   `t` runs natively on macOS, Linux and Windows from one code base, with no interpreter or
    other program needed at run time. fzf stays optional.
*   For the same list and the same input, every command prints the same text on stdout and
    stderr and exits with the same status as 0.3.0, except for the rows of
    [What changes](#what-changes). Extra messages that zsh or `mkdir` print on some failures
    today are not part of this.
*   A list written by 0.3.0, with its backup, checksum, lock file and archives, works with 0.4.0
    without conversion, and the other way round, as long as the two versions aren't used on the
    same list at the same time.
*   Every behaviour that doesn't need a terminal or zsh is tested on all three systems in CI.
*   Two runs of `t` on one list take turns, on all three systems.

## Non-Goals

*   New commands or options.
*   Tab completion for shells other than zsh. `shell/t.zsh` keeps working unchanged.
*   Package manager releases (Homebrew, Scoop, winget, crates.io).
*   Keeping the zsh script as a second implementation.
*   Taking settings from `~/.zshenv`. (The upgrade warning looks into it, see
    [Upgrading from 0.3](#upgrading-from-03); it never takes a value from it.)
*   Locking that excludes 0.3.0 on Linux (see [Locking](#locking)).

## Open Questions

None. Decisions made during review are recorded where they apply, and the alternatives that were
rejected are under [Alternatives Considered](#alternatives-considered).

## Critical User Journeys (CUJs)

### CUJ 1: First use on Windows

*   **Given**: A Windows computer with a Rust toolchain and no zsh, awk or perl.
*   **When**: The person runs `cargo install --path .` in a clone, then `t init`, `t add "Buy
    milk"` and `t list` in PowerShell.
*   **Then**: `t init` creates `C:\Users\<name>\todo\Todo.md` with the headings `## Today`,
    `## Later` and `## Done`, and prints `Created C:\Users\<name>\todo\Todo.md`. `t add` prints
    `Added 1 (Today): Buy milk`. `t list` prints `  1  Today  Buy milk`.

### CUJ 2: Upgrade from 0.3 with settings in `~/.zshenv`

*   **Given**: macOS. `~/.zshenv` has the line 0.3's `t init` wrote,
    `export TODO_FILE="${TODO_FILE:-$HOME/notes/Todo.md}"`. No config file. 0.4.0 is installed.
*   **When**: An agent started from bash runs `t list`.
*   **Then**: `t` doesn't see `TODO_FILE`, so it looks for `~/todo/Todo.md`, doesn't find it, and
    exits 1 with the 0.3.0 "not found" message, preceded by the warning
    `t: ~/.zshenv sets TODO_FILE, but t no longer reads it; put this line in
    ~/.config/markdone/config: TODO_FILE=~/notes/Todo.md`. After the person adds that line, the
    same command lists the items. Run from zsh, `t` works before and after, because zsh exports
    the variable and the warning only appears when `TODO_FILE` comes from the default.

### CUJ 3: A person and an agent use one list at once

*   **Given**: A list with three open items. The person has `t pick` open in fzf.
*   **When**: An agent runs `t add "New"`, then the person chooses item 2 in fzf.
*   **Then**: `t add` returns at once, because `pick` doesn't hold the lock while fzf is open.
    `pick` then marks the chosen item done, found again by its text. Both changes are in the file.

### CUJ 4: One list, synced between macOS and Windows

*   **Given**: A list in an Obsidian vault, synced to a Windows computer where an editor saved it
    with CRLF line endings.
*   **When**: `t add "x"` and `t done 1` run on either system.
*   **Then**: The headings and items are recognised, the lines `t` writes end in CRLF like the
    file's first line, and every line `t` doesn't change stays byte for byte as it was. (0.3.0
    fails on this file: `t add` stops with `no '## Today' heading`.)

## Technical Plan (Architecture)

### Shape

One Cargo package at the repository root, named `markdone`, with one binary named `t`. `src/`
replaces `bin/`. Lines in the diagram mean "calls".

```text
                       argv, environment
                              |
                           main.rs
          dispatch, usage, edit, exit codes, broken pipe
                              |
    +-----------+-------------+------------+-----------+----------+
    |           |             |            |           |          |
 config.rs   lists.rs      store.rs     init.rs   complete.rs  pick.rs
 settings,   which list,   read, save,  create a  t commands,  fzf
 config      its files,    undo,        list,     t complete
 file        lists add/    archives     vaults,
             rm/rename,       |         config
             use              |         file
    |           |          lock.rs
    +-----------+-------------+------------+-----------+----------+
                              |
                 doc.rs     jira.rs     text.rs
           (lines of a list) (Jira keys) (links, terminal styling,
                                          JSON, case, padding, quoting)
```

`doc.rs`, `jira.rs` and `text.rs` are pure: they take bytes and return bytes, and touch no files
or environment. Files are read and written only in `config.rs`, `lists.rs`, `lock.rs`,
`store.rs` and `init.rs`. `main.rs` starts the editor, `pick.rs` starts fzf, and `complete.rs`
only calls the others.

### Modules

| File | Replaces in `bin/t` | Holds |
|---|---|---|
| `src/main.rs` | the `@<list>` block, both `case` blocks, `usage`, `warn`, `die`, the `edit` line | Argument handling and dispatch in 0.3.0's order (see [Dispatch](#dispatch)); `edit`; exit codes; broken-pipe handling. |
| `src/config.rs` | the variables at the top, `days_ago` | The four settings, from the environment or the config file; home directory; default list path; lists folder; today's date and the archive cutoff. |
| `src/lists.rs` | `set_list`, `list_names`, `find_list`, `check_name`, `current_name`, `lists_cmd`, `lists_add`, `lists_rm`, `lists_rename`, `use_cmd`, `migrate_archives`, `open_count`, `list_choices` | A `List` value (name, file, lock file, backup, checksum file) and the commands that change the set of lists. |
| `src/lock.rs` | `lock`, `dir_lock` | Taking a lock with a timeout; releasing it for `pick` and before deleting a lock file. |
| `src/doc.rs` | the awk in `tidy`, `list`, `list_done`, `insert_item`, `add`, `item`, `finish`, `remove`, `reword`, `move`, `move_to`, `complete_items`, `pick`, `list_json` | Functions over the lines of a list (see [The list in memory](#the-list-in-memory)). |
| `src/jira.rs` | `check_jira`, `linkify`, the key regex in `list_json` | Jira settings checks, linking, finding keys for JSON. |
| `src/text.rs` | `render`, `AWK_SHOWN`, `js`, the perl `js` in `list_json`, `${(q-)…}`, `${(L)…}` | Web links, terminal styling, JSON strings, case folding, padding, shell quoting. |
| `src/store.rs` | `save`, `undo`, the archive loop in `tidy`, `cksum` | Reading a list, writing it with backup and checksum, undo, appending to archives. |
| `src/init.rs` | `init`, `obsidian_vaults`, `pick_vault`, `ask`, `remember_list`, `zshenv_line` | Creating a list, finding Obsidian vaults, prompts, writing the config file. |
| `src/complete.rs` | `commands`, `complete_cmd`, `ITEM_CMDS` | The command table and what Tab offers. |
| `src/pick.rs` | `pick`, the fzf call in `use_cmd` | Finding and running fzf. |

### Dependencies

*   `chrono` (default features off, `clock` on): today's date in the local time zone, which
    honours `TZ` as `date +%F` does. The standard library has no local time zone support.
*   `serde_json`: reading Obsidian's `obsidian.json`. JSON output is written by hand, as today.
*   `windows-sys` (Windows only, feature `Win32_System_Console`): switching the console to
    escape-sequence mode for terminal styling.
*   `tempfile`, for tests only.
*   File locks come from the standard library (`File::try_lock`, stable since Rust 1.89), so
    `Cargo.toml` sets `rust-version = "1.89"` and CI builds with 1.89 as well as with stable.
*   No argument parser crate: the command line has shapes such as `@<list>` first and `@<list>`
    last, and the error texts must match 0.3.0's. No regex crate: it has no lookbehind, which
    the Jira pattern `(?<![\w/-])((?:KEYS)-\d+)(?![\w-])` needs, so the scanner is written by
    hand.

`Cargo.lock` is committed. The release profile uses `strip = true`, `lto = true` and
`codegen-units = 1`. A new dependency needs the repository owner's approval in its pull request.

### Errors and exit codes

*   Functions return `Result`; `main` turns an error into `t: <message>` on stderr and exit 1, as
    `die` does. Code never panics on user input or I/O errors; a panic is a bug.
*   Warnings are `t: <message>` on stderr, and the run continues, as `warn` does.
*   Output to stdout goes through one writer. When stdout is a closed pipe (`t list | head -1`),
    `t` stops at once with no message and exits 141 on Unix (what a shell reports for a process
    killed by SIGPIPE, as 0.3.0 is) and 1 on Windows.
*   `t pick` and `t use` without a name exit with fzf's status when nothing is chosen.
*   An editor that can't be started: `t: could not run the editor '<program>'`, exit 127 (zsh's
    status for a command that isn't found).

### Dispatch

The order of 0.3.0, kept because tests and users depend on it:

1.  A first argument `@<list>` is taken off. An empty name is an error (as in 0.3.0).
2.  Commands that need no list: `help`, `-h`, `--help`, `version`, `--version`, `commands`,
    `init`, `lists`, `use`, and `complete` unless it offers a list's items. `init`, `lists` and
    `use` with `@<list>` are an error.
3.  The settings are loaded (see [Configuration](#configuration)), and the upgrade warning is
    printed when it applies.
4.  The list is chosen (`@<list>`, else **Which list**). Its file must exist.
5.  For `move`, the target list is found.
6.  Old archives are migrated (see **Old archives**), under the default list's lock.
7.  The list is locked; for `move`, both lists, in the byte order of their file paths.
8.  `undo` runs and exits.
9.  `check_jira`, then tidy.
10. The command. An unknown command is found only here, after tidy:
    `t: unknown command '<word>'` and the usage on stderr, exit 1.

**One lock per file per run.** The standard lock belongs to an open file handle, not to the
process, so the same run can't lock a file twice: a second handle would wait 10 s and fail. A run
therefore holds at most one `Lock` per lock file. Step 6 keeps the default list's lock when the
default list is also the list (or one of the two lists, for `move`) of step 7, and releases it
before step 7 otherwise. Step 7 reuses a lock it already holds.

### What stays the same

Every invariant in `bin/EGM.md` holds for the Rust program unless [What changes](#what-changes)
names it. In particular: the output contracts, the success messages, every `die` and `warn`
text, exit status 1 for errors, arguments joined with single spaces, item numbers, the insert
position, archive before rewrite, undo before tidy, and the names and places of the hidden files.

The hidden files are named after the list's full file name: for `Todo.md` they are
`.Todo.md.lock`, `.Todo.md.bak` and `.Todo.md.sum`, next to it. (`bin/EGM.md` says
`.<list name>.lock` in one place; the code uses the file name.) `.current` and `.lists.lock` are
in the lists folder; archives are `Archive/<list> <year>.md`.

These messages are not in `bin/EGM.md` but are kept as in 0.3.0: `Moved to <list> (<section>) as
<n>: <text>`, `Created the list <name>; use it with t @<name> … or t use <name>`, `Deleted the
list <name>…`, `Renamed the list <old> to <new>`, `Now using <name> (<n> open)`,
`<file> already exists; using it as it is`, `Obsidian vault: <dir>`.

### What changes

| Today (in `bin/t` 0.3.0) | In 0.4.0 |
|---|---|
| Settings reach `t` through `~/.zshenv`; **Remembering the list**: `t init` writes a marked line to `${ZDOTDIR:-$HOME}/.zshenv`, and leaves the file alone when another line there mentions `TODO_FILE` | Settings come from the environment or a config file; `t init` writes or replaces the `TODO_FILE=` line in the config file. See [Configuration](#configuration). |
| No warning about settings | The upgrade warning, and warnings about lines in the config file. See [Upgrading from 0.3](#upgrading-from-03) and [New messages](#new-messages). |
| Lists with CRLF line endings fail (`no '## Today' heading`), and `t list` shows `Today\r` | Supported. See [Line endings](#line-endings). |
| A UTF-8 byte order mark before the first heading stops it from matching | Ignored for matching and written back. |
| **One run at a time**: an fcntl lock (`zsystem flock`) | `File::try_lock` (flock on Unix, `LockFileEx` on Windows), tried every 0.05 s for up to 10 s, same messages. See [Locking](#locking). |
| **Unlocking**: `exec` closes the lock's descriptor | Unix: the same (the standard library opens files close-on-exec). Windows: `t edit` releases the lock, starts the editor, waits, and exits with its status. |
| **Startup**: `zmodload` must succeed | Gone. |
| **Writes go through `save`**: the result is built in a `mktemp` file | Built in memory. Backup, in-place write and checksum unchanged; see [Saving](#saving). |
| **Undo**: `cksum` | The same POSIX cksum value and `.sum` text, computed in the program. Existing `.sum` files keep working. |
| **Archive cutoff**: nothing is archived when the date can't be computed, with a warning | The date is always computed; the warning is gone. |
| `Jira linking failed (is perl installed?)` errors | Gone; linking can't fail. |
| Runs awk, perl, `date`, `cmp`, `cksum`, `cut`, `sort`, `grep`, `cat`, `mktemp`, `mkdir`, `mv`, `rm` | Runs only fzf (`pick`, `use` alone) and the editor (`edit`). |
| `t list \| head -1`: killed quietly by SIGPIPE | Stops quietly, exit 141 on Unix, 1 on Windows. |
| List names sort by the locale; archive lines sort with `sort` | Both sort by bytes. Archive lines are `- [x] ` and then the date, so their date order is the same. |
| Case-insensitive matching (`<item>` text, list names) follows awk's `tolower` and zsh's `${(L)}`, which depend on the locale | Unicode lower-casing (`str::to_lowercase`) on both sides, for item text and list names. |
| `%-6s` pads by bytes in BSD awk and mawk, by characters in gawk; `t lists` pads by characters | Padding counts characters everywhere. |
| `t lists --json` escapes only `\`, `"`, tab (`\t`) and newline (`\n`); `t list --json` writes invalid UTF-8 bytes as they are | Both escape `"`, `\` and U+0000 to U+001F as `\u00xx` (lowercase hex), and write invalid UTF-8 bytes as U+FFFD, so the output is always valid JSON. A list name can contain such characters today (`t lists add` accepts a name with ESC), so `t lists --json` output changes for those names. |
| **What a list is**: `check_name` refuses an empty name, `/`, tab, newline, a leading `.` or `@`, a trailing `.md`, `Archive <year>` | Also refuses `\ : * ? " < >` and `\|`, any character U+0000 to U+001F, a trailing `.` or space, and Windows device names (see [List names](#list-names)), on every system, so a list made on one system syncs to another. Existing lists with such names are still found and used. New error text in [New messages](#new-messages). |
| `TODO_LIST=nosuch t lists` prints the error, lists without a `*`, exits 0; `t lists rm` then doesn't refuse the current list; `t complete use` with a bad `TODO_LIST` prints an error and exits 0 (all because `die` inside `$(…)` doesn't stop zsh) | `t lists` and `t lists rm` stop with the `TODO_LIST` error, exit 1. `t complete` prints nothing and exits 0. |
| An exported `tabbed` variable switches `t lists` to tab-separated output | No effect (it was an internal flag). |
| `T_VERSION` in the script | The version in `Cargo.toml`. |
| `t edit`: `${=EDITOR:-nvim}` | The same on macOS and Linux; on Windows the default is `notepad`. See [Platform differences](#platform-differences). |
| Web links in `render` stop at `)` and white space; in `shown` at `)`, space and tab | Both stop at `)` and ASCII white space (space, tab, LF, VT, FF, CR). |

### Configuration

**Sources.** Each of `TODO_FILE`, `TODO_JIRA_KEYS` and `TODO_JIRA_URL` is taken from the first of
these that has it:

1.  The environment variable, when it is set. An empty value counts: it stops the lookup and
    means the default. (So `TODO_FILE= t list` uses `~/todo/Todo.md` as in 0.3.0, and
    `TODO_JIRA_KEYS= t list` turns linking off for one run even when the config file sets keys,
    as README recommends.)
2.  The config file.
3.  The default: `~/todo/Todo.md` for `TODO_FILE`, unset for the others.

`TODO_LIST` comes only from the environment, as a per-run override; in the config file it would
make `t use` useless. "`TODO_FILE` comes from the default" means step 3 (or an empty value in
step 1) supplied it. Plain `t init` offers Obsidian vaults only in that case, and `t init <path>`
compares `<path>` with the resolved `TODO_FILE`.

**Where.** On macOS and Linux: `$XDG_CONFIG_HOME/markdone/config` when `XDG_CONFIG_HOME` is set
and not empty, else `~/.config/markdone/config`. On Windows: `%APPDATA%\markdone\config`. When
`APPDATA` isn't set, `t` warns once (`APPDATA isn't set, so the config file isn't read`) and
carries on without a config file.

**Format.**

```text
# comment
TODO_FILE=~/notes/Todo.md
TODO_JIRA_KEYS=PROJ OPS
TODO_JIRA_URL=https://your-company.atlassian.net/browse/
```

*   A leading UTF-8 byte order mark is skipped. Lines end at LF; a CR before it is dropped.
*   Blank lines (empty or only spaces and tabs) and lines whose first non-blank character is `#`
    are skipped.
*   Other lines are `NAME=value`. The name is the text before the first `=`, with spaces and
    tabs trimmed; the value is the text after it, trimmed the same way. No quotes and no
    expansion.
*   When a name appears more than once, the last line wins.
*   `TODO_FILE` must be absolute, or start with `~/` (or `~\` on Windows), which means the home
    directory. Otherwise the line is ignored with a warning.
*   A value containing `$` is kept, with a warning that it isn't expanded.
*   Any other line is ignored with a warning. Warnings name the file and line number; see
    [New messages](#new-messages). They are printed on every run that loads the settings.
*   A missing config file is not an error. One that exists but can't be read stops `t`:
    `t: could not read <config file>`, exit 1. `help`, `version` and `commands` don't load the
    settings, so they still work.

**Writing it (`t init`).** Replaces 0.3.0's `remember_list`, with the same flow and streams
(prompt and messages on stdout):

1.  When the config file already has `TODO_FILE=<this list>`: `<config file> already points t at
    this list`, and stop.
2.  Without a terminal on stdin: `To use it, add this to <config file>:` and `  TODO_FILE=<path>`
    on the next line, and stop.
3.  Ask `Use this list from now on, also from AI agents? This adds TODO_FILE to <config file>.
    [Y/n]`. An answer starting with `y` or `Y`, or an empty one, is yes (the answer's trailing
    CR and LF are dropped first). On no: `Not saved. To use the list, add this to <config file>:`
    and the line, and stop.
4.  Create the directory if needed. Replace the last existing `TODO_FILE=` line, or append the
    line when there is none; keep every other line. Write the file in place (truncate and
    write), so a config file that is a symlink stays one.
5.  `Saved in <config file>: t now uses <file>`.
6.  When `TODO_FILE` is set in the environment to something else:
    `t: TODO_FILE is set in your environment to '<value>', which still wins where it's set`
    on stderr.

`<path>` is written as `~/…` when the list is under the home directory, else in full;
`<config file>` is shown the same way. `t init <path>` prints the hint of step 2 when `<path>`
isn't the list `t` uses already, and never asks or writes, as in 0.3.0.

### Upgrading from 0.3

On macOS and Linux, after step 3 of [Dispatch](#dispatch): when `TODO_FILE` comes from the
default and `${ZDOTDIR:-$HOME}/.zshenv` has a line containing `TODO_FILE`, `t` warns:

`t: ~/.zshenv sets TODO_FILE, but t no longer reads it; put this line in <config file>: TODO_FILE=<path>`

`<path>` comes from the line 0.3's `t init` wrote: the marker comment
`# markdone: where t keeps the list (set by t init)` followed by
`export TODO_FILE="${TODO_FILE:-<value>}"`. The value is unescaped (`\\ \" \$` and backquote)
and a leading `$HOME` becomes `~`. Without that marked line, `<path>` is printed as the literal
text `<path>`. This is the only use of `.zshenv`; no value is taken from it.

In zsh, the variable is exported, so the warning only appears where the settings really differ:
programs not started from zsh. README's "Upgrading from 0.3" tells people to move all four
settings, including `TODO_JIRA_KEYS` and `TODO_JIRA_URL`, to the config file, and not to use 0.3
and 0.4 on one list at the same time.

### The list in memory

`store.rs` reads a list as bytes. `doc.rs` works on bytes, so a list that isn't valid UTF-8 works
as in 0.3.0. Every structural marker (`## `, `- [ ] `, `- [x] `, dates) is ASCII.

**Lines.** The file is split at LF. A CR before the LF is the line's ending, not its content. An
empty file has no lines. A UTF-8 byte order mark at the start of the file belongs to no line: it
is ignored for matching and written back first. A last line without LF is a line; when the file
is rewritten it gets the file's line ending (0.3.0 adds a newline too, because awk ends every
line it prints with one).

**Sections.** Rules of 0.3.0:

*   A line starting with `## ` starts a section named by the rest of the line. `#` and `###`
    lines are ordinary lines. A heading with a trailing space is a different name.
*   `## Today`, `## Later` and `## Done` are the sections commands refer to. Items before the
    first heading are in section "" (shown as blank in `t list`). Items under any other heading
    are listed, numbered and can be moved with `today` and `later`.
*   An open item is a line starting with `- [ ] `; a done item one starting with `- [x] `.
    `- [X] ` and indented items are ordinary lines. The item's text is the rest of the line.
*   Open items are numbered from 1, from the top of the file, across all sections.

**Dates.** A done item is dated when the 10 bytes after `- [x] ` have the shape `NNNN-NN-NN`
(digits and dashes; no check that it is a real date, and nothing required after it). The archive
cutoff is the local date 7 days ago as `YYYY-MM-DD`, counted back from today's noon. A dated item
is archived when its 10-byte date is less than the cutoff as a string. Its archive year is the
first 4 bytes of the date.

**Functions.** Each takes lines and returns new lines plus a result, or an error naming the
0.3.0 message; none writes a file.

*   `tidy(lines, today, cutoff, jira)` returns the new lines, the archived lines, and whether a
    ticked item was left without a `## Done` after it (then the caller warns as in 0.3.0 and
    keeps the list as it was). It links Jira keys on item lines, gives today's date to undated
    done items anywhere, moves ticked items from other sections to the top of the next `## Done`
    after them in file order, and takes dated items older than the cutoff out of `## Done`,
    including the ones just moved there.
*   `insert(lines, section, text)`: append `- [ ] text` at the end of the section, before its
    trailing blank lines (empty, or spaces and tabs only, ignoring the CR); returns the item's
    number. Fails when the heading is missing.
*   `finish(lines, n, today)`: remove open item `n` and write `- [x] <today> <text>` directly under
    the first `## Done`; returns the text. Fails when item `n` doesn't exist, its text is empty,
    or there is no `## Done`.
*   `remove(lines, n)`, `reword(lines, n, text)` (returns the number and section),
    `move_within(lines, n, section)` (returns the new number; fails when the item is already
    there), `take(lines, n)` (returns section and text without changing anything, for the
    source half of `t move`).
*   `find(lines, reference)`: a reference of digits only is the number (leading zeros allowed;
    a number too large to be an item is "no open item <reference as typed>"). Other text is
    matched as in **Item references**, with both sides lower-cased by `str::to_lowercase` after
    decoding (invalid bytes as U+FFFD). Errors list the matching items as 0.3.0 does.
*   `open_items(lines)` returns `(number, section, text)`; `done_items(lines)` returns the
    `- [x] ` lines of `## Done`.

**`t list --done`.** `store.rs` collects the list's done items, then (with `--all`) the
`- [x] ` lines of its archive files in byte order of their names. These are sorted by the 10-byte
date, newest first; the sort is stable, so items with the same date keep this input order. Each
is printed as the date, two spaces, and the text after the date and its following byte.

### Line endings

*   A file's line ending is CRLF when its first line ends in CRLF, else LF. An empty file, or one
    with a single line and no LF, uses LF.
*   Lines `t` writes (a new, reworded, moved or done item, a line moved by tidy, a new archive
    heading and the blank line after it, archived lines) end with the ending of the file they
    are written to. An archive file uses its own first line's ending; a new archive file uses
    the list's.
*   Lines `t` doesn't change keep their own ending, so a file with mixed endings keeps them.
*   Item text never includes the CR: not in `t list`, JSON, messages, matching, completion, or
    the text `pick` compares.

### Jira linking and display

Two kinds of link, used consistently below:

*   A **Markdown link** is `[`, any bytes except `]`, `](`, any bytes except `)`, then `)`.
*   A **web link** is a Markdown link whose text is not empty and whose target is `http://` or
    `https://` followed by one or more bytes, none of them `)` or ASCII white space (space, tab,
    LF, VT, FF, CR).

**Linking (`jira.rs`).** Applies to item lines in tidy and to the text of `add` and `reword`. The
scanner walks the text from the left:

1.  At a Markdown link, copy it unchanged.
2.  At a backquote, if another backquote follows, copy up to and including it unchanged.
3.  At `http://` or `https://`, copy up to the next ASCII white space unchanged.
4.  At a key: one of `TODO_JIRA_KEYS` starts here, followed by `-` and one or more ASCII digits,
    the byte before is not an ASCII letter or digit, `_`, `/` or `-`, and the byte after the
    digits is not an ASCII letter or digit, `_` or `-`. Write `[KEY-123](<TODO_JIRA_URL>KEY-123)`
    with the URL put directly in front of the key.
5.  Otherwise copy one byte.

**Keys for JSON.** The `jira` array of `t list --json` uses only rule 4's test, at every position
of the stored text, without rules 1 to 3: keys inside backquotes and inside links count. Each key
appears once, in order of first appearance.

**Display (`text.rs`).** "Styled" output turns each web link into an OSC 8 hyperlink, the bytes
`ESC ] 8 ; ; <target> ESC \ <text> ESC ] 8 ; ; ESC \`, and shows the list name in bold
(`ESC [1m <name> ESC [0m`) above `t list` when there is more than one list. Output is styled when
stdout is a terminal and, on Windows, switching the console to escape-sequence mode succeeds.
Styled output is used for `t list`, `t list --done`, and the `Added`, `Done`, `Removed`,
`Reworded` and `Moved` messages; nothing else. `shown(text)` reduces each web link to its text,
for matching `<item>` and for completion.

### Locking

*   The lock file is opened for reading and writing, created when missing, never truncated.
*   `try_lock` is called every 0.05 s until it succeeds or 10 s pass; then 0.3.0's timeout
    message for a list or for the lists folder.
*   A run holds a lock by keeping its `File` open until it exits. The kernel releases it however
    the process ends. `pick` releases before starting fzf and locks again afterwards.
*   fzf and the editor don't inherit the lock: the standard library opens files close-on-exec on
    Unix and non-inheritable on Windows.
*   `lists rm` and `lists rename` release the list's lock before deleting the lock file, so the
    deletion also works on Windows.
*   **Compatibility with 0.3.0.** 0.3.0 locks with fcntl. On macOS, fcntl and flock locks on the
    same file exclude each other (tested). On Linux they don't, so 0.3.0 and 0.4.0 running at
    the same time on one list can drop each other's changes. This is accepted; README tells
    people not to mix the versions.

### Saving

`save(list, lines)`, as in **Writes go through `save`**:

1.  Read the list's current bytes and write them to the backup, opened with create, truncate and
    write; on Unix, a new backup is created with mode 0600. (Not `fs::copy`, which copies the
    list's permissions.) If this fails: 0.3.0's message, and nothing is written.
2.  Write the new bytes to the list in place: open with truncate and write, never rename.
3.  Write the checksum: `<crc> <length>` and LF, the output format of POSIX `cksum`, mode 0600
    when created. If this fails, delete the checksum file and warn as in 0.3.0.

Tidy saves only when its result differs from the file. `undo` compares the stored checksum with
the list's, both with surrounding white space trimmed, and restores the backup byte for byte
through `save`. `.current` holds the name and LF; it is read with trailing CR and LF dropped, and
written to `.current.<process id>` and renamed.

### List names

A list's name is its file name without the last `.md`. The lists are the regular files (not
symlinks, not hidden) ending in `.md` directly in the lists folder, as zsh's `*.md(N.)` gives
today, plus the default list when its file exists (a symlink there counts). Names of the form
`Archive <digits>` are not lists.

A new name (for `lists add` and `lists rename`) must not:

*   be empty, contain `/ \ : * ? " < > |` or any character U+0000 to U+001F;
*   start with `.` or `@`, end with `.`, a space or `.md`;
*   be `Archive <digits>`;
*   be a Windows device name, in any case, alone or followed by `.` and anything: `CON`, `PRN`,
    `AUX`, `NUL`, `COM0` to `COM9`, `LPT0` to `LPT9`, and `COM¹`, `COM²`, `COM³`, `LPT¹`, `LPT²`,
    `LPT³` (from Microsoft's "Naming Files, Paths, and Namespaces");
*   match an existing list's name, compared with `str::to_lowercase`.

**Shell quoting** of list names, in `t complete` and in the `lists add` message, matches zsh's
`${(q-)name}`. Test vectors, produced with zsh 5.9:

| Name | Quoted |
|---|---|
| `Work` | `Work` |
| `my list` | `'my list'` |
| `it's` | `it\'s` |
| `a+b`, `x,y`, `k:v`, `a@b`, `50%`, `a=b`, `Café`, `a.b-c_d` | unchanged |
| `~x` | `'~x'` |
| `a#b` | `'a#b'` |

Step S7 adds vectors for every other ASCII punctuation character, produced the same way.

### Paths

*   **Home directory**: `std::env::home_dir()` (`HOME` on Unix, `USERPROFILE` on Windows; not
    deprecated in current Rust). When it is needed and unknown:
    `t: can't find your home directory`, exit 1.
*   **`t init <path>`** makes `<path>` absolute against the current directory and removes `.` and
    `..` by their position, without resolving symlinks, like zsh's `${1:a}`.
*   **The default list's name** is its file name up to the last `.`, like zsh's `:t:r`.
*   **"Under the home directory"** compares path components; on Windows without regard to case.
    Messages use the system's own separator.
*   **Lock order** for `move` compares the two files' paths as bytes (on Windows, as UTF-8).

### Platform differences

Code specific to one system sits behind `cfg(unix)` or `cfg(windows)` only in these places:

| Where | macOS and Linux | Windows |
|---|---|---|
| Config file (`config.rs`) | `$XDG_CONFIG_HOME/markdone/config` or `~/.config/markdone/config` | `%APPDATA%\markdone\config` |
| `~` in config values (`config.rs`) | `~/` | `~/` or `~\` |
| Obsidian's vault list (`init.rs`) | the macOS, Linux, Flatpak and Snap locations of 0.3.0 (`bin/t` `obsidian_vaults`) | `%APPDATA%\obsidian\obsidian.json` |
| Backup and checksum files (`store.rs`) | mode 0600 when created | default permissions |
| Arguments and paths (`main.rs`, `config.rs`) | bytes, as given | UTF-16 converted to UTF-8; an argument that isn't valid Unicode stops `t`: `t: arguments must be valid text` |
| Terminal styling (`text.rs`) | stdout is a terminal | also, `SetConsoleMode` with escape-sequence processing succeeds |
| `t edit` (`main.rs`) | `exec`; default `nvim` | release the lock, spawn, wait; default `notepad` |
| Finding fzf and the editor (`pick.rs`, `main.rs`) | search `PATH` | search `PATH` with each extension in `PATHEXT` |
| Upgrade warning (`main.rs`) | checks `.zshenv` | none |

Obsidian vaults are listed as in 0.3.0: most recently opened first, without vaults whose folder
is gone or whose path contains a newline; an unreadable or invalid `obsidian.json` is skipped
without a message.

**`EDITOR`.** Empty or unset means the default. When the whole value names an existing file, it
is the program (so `C:\Program Files\Editor\editor.exe` works). Otherwise it is split at spaces,
tabs and newlines, like zsh's `${=EDITOR}`; the first word is the program, found on `PATH`. On
Windows, a `.cmd` or `.bat` program (such as `code`, which is `code.cmd`) is run by the standard
library through `cmd.exe` with its argument escaping.

**fzf.** Found on `PATH` before anything else, as 0.3.0 checks first; otherwise 0.3.0's message.
`pick` feeds it the unstyled `t list` output (no list-name line, no escape sequences), as 0.3.0
does by piping, so the first word of the chosen line is the item number.

**Windows console.** Prompts drop the trailing CR and LF of an answer. `t` writes UTF-8; README
tells PowerShell users to set `[Console]::OutputEncoding` to UTF-8 to read non-ASCII text, and
notes that Windows PowerShell 5.1 drops `"` inside arguments to programs.

**File sharing on Windows.** Writing a list another program holds open without sharing fails
with 0.3.0's "could not write" message; no retry.

### New messages

Every message that 0.3.0 doesn't have, verbatim (`<config file>` and paths shown with `~` when
under the home directory):

| When | Text (stderr unless noted) |
|---|---|
| Config file can't be read | `t: could not read <config file>` (exit 1) |
| Config line that isn't `NAME=value` with a known name | `t: <config file>:<n>: not a setting; ignored` |
| `TODO_LIST` in the config file | `t: <config file>:<n>: TODO_LIST can't be set here (use t use); ignored` |
| Relative `TODO_FILE` | `t: <config file>:<n>: TODO_FILE must be an absolute path or start with ~/; ignored` |
| `$` in a value | `t: <config file>:<n>: values aren't expanded, so $ is taken as it is` |
| No `APPDATA` (Windows) | `t: APPDATA isn't set, so the config file isn't read` |
| Upgrade warning | see [Upgrading from 0.3](#upgrading-from-03) |
| `t init` prompt and messages (stdout) | see [Configuration](#configuration) |
| `TODO_FILE` set in the environment after saving | `t: TODO_FILE is set in your environment to '<value>', which still wins where it's set` |
| Unknown home directory | `t: can't find your home directory` (exit 1) |
| Invalid arguments (Windows) | `t: arguments must be valid text` (exit 1) |
| Editor can't be started | `t: could not run the editor '<program>'` (exit 127) |
| Bad list name | `t: '<name>' can't be a list name (no / \ : * ? " < > \|, no control characters, no leading . or @, no trailing dot, space or .md, not a Windows device name like CON)` (exit 1) |
| `install`: no cargo | `install: cargo isn't installed; get Rust from https://rustup.rs and run ./install again` (exit 1) |
| `install`: build failed | `install: cargo build failed; see the messages above` (exit 1) |
| `install`: last line (stdout) | `Done. Open a new terminal, then run 't init' to create your list (or set TODO_FILE first). After a git pull, run ./install again to rebuild t.` |

## Alternatives Considered

### Alternative A: Port to Go

*   **Pros**: One static binary; simple cross-compiling.
*   **Cons**: File locks need a library or per-system code.
*   **Reason for Rejection**: Rejected by author: preferred Rust, whose standard library has file
    locks for all three systems.

### Alternative B: Port to TypeScript

*   **Pros**: Quick to write; many contributors know it.
*   **Cons**: Needs Node or Bun at run time, or a bundled executable of roughly 50–100 MB. Every
    run starts a JavaScript runtime, on every Tab press and every agent call. Node's standard
    library has no kernel file lock, so "one run at a time" would rest on lock files that can go
    stale after a crash.
*   **Reason for Rejection**: Rejected by author: run-time dependency, start-up cost and locking.

### Alternative C: Keep zsh and support Windows through WSL

*   **Pros**: Almost no work.
*   **Cons**: PowerShell and agents on Windows would call `wsl t`; lists on the Windows drive go
    through a slower file system.
*   **Reason for Rejection**: Rejected by author: native Windows was the aim.

### Alternative D: Keep the zsh script next to the Rust program

*   **Pros**: No upgrade step for people on 0.3.
*   **Cons**: Every change made twice; the two lock in ways that don't exclude each other on
    Linux.
*   **Reason for Rejection**: Rejected by author: one implementation.

### Alternative E: Read the settings from `~/.zshenv`

*   **Pros**: No upgrade step.
*   **Cons**: `~/.zshenv` is a shell script; reading it properly means running zsh, which Windows
    doesn't have.
*   **Reason for Rejection**: Rejected by author: a config file, plus a warning that shows the
    line to move.

### Alternative F: A TOML config file

*   **Pros**: A known format.
*   **Cons**: One more dependency for four string settings; values need quotes.
*   **Reason for Rejection**: Rejected by author: `NAME=value` lines use the names people already
    know from the environment variables.

### Alternative G: Write the list by renaming a temp file over it

*   **Pros**: The write is atomic.
*   **Cons**: Replaces the file: a list that is a symlink stops being one, permissions and owner
    must be copied, and sync tools and editors watching the file see a delete and a create.
*   **Reason for Rejection**: Rejected by author: keeps 0.3.0's in-place write with a backup.

### Alternative H: Also take an fcntl lock on Unix

*   **Pros**: 0.3.0 and 0.4.0 would exclude each other on Linux too, and 0.3.0's lock tests would
    keep working unchanged.
*   **Cons**: A dependency (`rustix` or `libc`). fcntl drops every lock a process holds on a file
    when any handle to that file closes, so a later change that opens the lock file a second
    time would silently drop the lock.
*   **Reason for Rejection**: Rejected by author: standard lock only; the versions aren't meant
    to run side by side.

### Alternative I: Refuse lists that aren't valid UTF-8

*   **Pros**: Simpler code with Rust strings.
*   **Cons**: A list in a legacy encoding that works with 0.3.0 would stop working.
*   **Reason for Rejection**: Rejected by author: work on bytes, as 0.3.0 does.

### Alternative J: Leave CRLF lists for later

*   **Pros**: A smaller, strict port.
*   **Cons**: A Windows user whose editor writes CRLF can't use `t`.
*   **Reason for Rejection**: Rejected by author: CRLF is supported in 0.4.0.

--------------------------------------------------------------------------------

## Non-Functional Requirements

*   **Start-up**: `t list` on a list of 200 items with Jira linking on, output to a pipe,
    finishes in under 20 ms on an Apple-silicon Mac, measured as the median of 20 runs with
    `hyperfine`.
*   **Safety of the list**: a command that fails before it saves leaves the list unchanged. Tidy
    runs before the command and may already have saved (so `t done 99` can tidy and then fail);
    `t move` can leave an item in both lists after a failure; archives can get a duplicate line
    after a crash. These are 0.3.0's rules (**Tidy runs first**, **Moving between lists**,
    **Never lose an item**), tested in `tests/items.rs`, `tests/lists.rs` and `tests/tidy.rs`.
*   **Security**: no network access. fzf and the editor are started with an argument list, not a
    command line, so list contents and names are never run as commands. The one exception is a
    `.cmd` or `.bat` editor on Windows, which the standard library runs through `cmd.exe`, with
    the argument escaping it applies since Rust 1.77.2.
*   **Rollback**: `main` keeps 0.3.0 until the final merge. The file format, hidden files and
    `.sum` format are unchanged, so going back needs only the old script and the old
    `~/.zshenv` lines. While both are installed, don't use them on the same list at once (see
    [Locking](#locking)).

--------------------------------------------------------------------------------

## Additional Resources

*   [`bin/t` at `v0.3.0`](https://github.com/sharpmist/markdone/blob/v0.3.0/bin/t) and
    [`bin/EGM.md`](../../bin/EGM.md): the reference for current behaviour.
*   [`EGM.md`](../../EGM.md), [`shell/EGM.md`](../../shell/EGM.md),
    [`AGENTS.md`](../../AGENTS.md), [`README.md`](../../README.md), [`install`](../../install),
    [`shell/t.zsh`](../../shell/t.zsh), [`.github/workflows/test.yml`](../../.github/workflows/test.yml).
*   [`test/run.zsh`](../../test/run.zsh): 365 checks of 0.3.0 (`365 passed, 0 failed` on macOS
    at `v0.3.0`), the reference for the port's tests.

## Future Improvements

*   **Tab completion in PowerShell, bash and fish**: `t commands` and `t complete` already give a
    shell everything it needs.
*   **Linux arm64 release binaries**, signed macOS and Windows binaries, and package manager
    releases.

--------------------------------------------------------------------------------

## Detailed Implementation Plan

### How the work runs

*   Work lands on the `develop` branch, one GitHub issue per step below, in order, in milestone
    `0.4.0 Rust port` with the label `rust-port`. Commits reference their issue.
*   Each step writes its tests first, from this document and from the 0.3.0 checks named in
    [Where each `test/run.zsh` section goes](#where-each-testrunzsh-section-goes), and every
    step leaves `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
    `cargo test --locked` and `test/run.zsh` passing.
*   A module is declared in `src/main.rs` in the step that adds it, with `#[allow(dead_code)]` on
    its `mod` line until S7 wires it into dispatch and removes the attribute. Steps write both
    `cfg(unix)` and `cfg(windows)` branches of their own code; S10 tests the Windows ones.
*   `src/EGM.md` is created in S2 and gains each module's interface, invariants and side effects
    in the step that adds the module.
*   **Test suite during the port.** `test/run.zsh` keeps testing `bin/t` until S12. From S1 it
    finds the repository from its own path and takes an absolute `T_BIN` (default
    `$repo/bin/t`). Checks that only apply to one implementation run only for it: `impl=zsh`
    when `T_BIN` is `$repo/bin/t`, else `impl=rust`. From S9, CI also runs
    `T_BIN=$PWD/target/debug/t test/run.zsh` on macOS and Ubuntu; that run must pass from then
    on (the parity gate).
*   The version is `0.4.0-dev` until S14 sets `0.4.0`.

| Step | Issue | Files |
|---|---|---|
| S1 | Scaffold, CI, test suite safety | `Cargo.toml`, `Cargo.lock`, `.gitignore`, `src/main.rs`, `.github/workflows/test.yml`, `test/run.zsh` |
| S2 | Text helpers and settings | `src/text.rs`, `src/config.rs`, `src/main.rs`, `src/EGM.md` |
| S3 | Jira linking | `src/jira.rs`, `src/main.rs`, `src/EGM.md` |
| S4 | The list in memory | `src/doc.rs`, `src/main.rs`, `src/EGM.md` |
| S5 | Locking, saving, undo, archives | `src/lock.rs`, `src/store.rs`, `src/main.rs`, `src/EGM.md` |
| S6 | Lists | `src/lists.rs`, `src/main.rs`, `src/EGM.md` |
| S7 | Dispatch and list commands | `src/main.rs`, `src/complete.rs`, `src/EGM.md`, `tests/common/mod.rs`, `tests/items.rs`, `tests/tidy.rs`, `tests/crlf.rs`, `tests/jira.rs`, `tests/lists.rs`, `tests/complete.rs`, `tests/undo.rs`, `tests/locking.rs`, `tests/config.rs` |
| S8 | `init` and the config file | `src/init.rs`, `src/main.rs`, `src/EGM.md`, `tests/init.rs`, `test/run.zsh` |
| S9 | `pick`, `use` alone, `edit`; parity gate | `src/pick.rs`, `src/main.rs`, `src/EGM.md`, `test/run.zsh`, `.github/workflows/test.yml` |
| S10 | Windows | `tests/platform.rs`, fixes in the `cfg(windows)` places of [Platform differences](#platform-differences) |
| S11 | Move tests to Rust | `test/run.zsh`, `tests/*.rs` |
| S12 | Remove the script; install | `bin/t`, `bin/EGM.md`, `install`, `test/run.zsh`, `.github/workflows/test.yml`, `shell/EGM.md`, `EGM.md` |
| S13 | Docs; drift check | `README.md`, `AGENTS.md`, `EGM.md`, `src/EGM.md`, `docs/designs/SITEMAP.md` |
| S14 | Release | `.github/workflows/release.yml`, `Cargo.toml`, `Cargo.lock` |

### Target Files to Create/Modify

#### 1. `Cargo.toml`, `Cargo.lock` (Create)

*   **Steps**: S1; dependencies added in the step that first uses them (S2 `chrono`, S8
    `serde_json`, S2 `windows-sys`, S5 `tempfile`); S14 sets the version.
*   **Proposed Changes**: Package `markdone`, version `0.4.0-dev`, edition 2024,
    `rust-version = "1.89"`, `[[bin]] name = "t"`, `path = "src/main.rs"`. Release profile as
    under [Dependencies](#dependencies). The lock file is committed because this is a program.

#### 2. `.gitignore` (Create)

*   **Steps**: S1.
*   **Proposed Changes**: `/target`.

#### 3. `src/main.rs` (Create)

*   **Steps**: S1 (`help`, `version`), then `mod` lines in S2 to S6, dispatch in S7, `init` in
    S8, `pick`, `use` alone and `edit` in S9.
*   **Proposed Changes**: `usage()` with 0.3.0's text. `warn`, `die`, the stdout writer and exit
    codes as under [Errors and exit codes](#errors-and-exit-codes). [Dispatch](#dispatch) with
    its lock rule. `edit` as under [Platform differences](#platform-differences). The upgrade
    warning.

#### 4. `src/text.rs` (Create)

*   **Steps**: S2.
*   **Proposed Changes**: Web link matching, `shown`, styled output and the OSC 8 bytes, the
    Windows console mode switch, JSON strings, lower-casing, padding by characters, shell quoting
    with the test vectors of [List names](#list-names). Unit tests for each.

#### 5. `src/config.rs` (Create)

*   **Steps**: S2.
*   **Proposed Changes**: Settings with where each value came from, as under
    [Configuration](#configuration); the config file reader and its warnings; the home directory;
    today's date and the cutoff. Unit tests for the reader, the order of sources and empty values.

#### 6. `src/jira.rs` (Create)

*   **Steps**: S3.
*   **Proposed Changes**: `check_jira` with 0.3.0's three warnings, the scanner, keys for JSON.
    Unit tests for every case of the 0.3.0 sections "Jira links" and "list --json".

#### 7. `src/doc.rs` (Create)

*   **Steps**: S4.
*   **Proposed Changes**: The functions of [The list in memory](#the-list-in-memory) and the
    rules of [Line endings](#line-endings). Unit tests for every case of the 0.3.0 sections
    "adding and listing", "done", "tidy", "today", "later", "list --today / --later", "items by
    text", "rm", "reword", "listing done items" and "edge cases", plus CRLF, BOM, mixed endings,
    missing final newline, empty file and non-UTF-8 bytes.

#### 8. `src/lock.rs`, `src/store.rs` (Create)

*   **Steps**: S5.
*   **Proposed Changes**: [Locking](#locking) and [Saving](#saving); archive files; reading a
    list's archives for `--done --all`. `cksum` tested against the `cksum` command's output for
    an empty input, a short one and one over 64 KiB.

#### 9. `src/lists.rs` (Create)

*   **Steps**: S6.
*   **Proposed Changes**: `List`, the rules of [List names](#list-names), `current_name`,
    `lists` (plain and `--json`), `lists add`, `lists rm [--force]`, `lists rename`, `use <name>`,
    old-archive migration, open counts. The folder's lock first, then a list's.

#### 10. `src/complete.rs` (Create)

*   **Steps**: S7.
*   **Proposed Changes**: The `t commands` table and line format, `t complete <words>` as in
    `bin/EGM.md`'s output contracts.

#### 11. `src/init.rs` (Create)

*   **Steps**: S8.
*   **Proposed Changes**: `init`, Obsidian vaults, `pick_vault` (prompts on stderr, as in
    0.3.0), `ask` and the config file writer (prompt and messages on stdout, as in 0.3.0).

#### 12. `src/pick.rs` (Create)

*   **Steps**: S9.
*   **Proposed Changes**: Finding fzf; `pick` (release the lock, run fzf with 0.3.0's arguments
    on the unstyled list, lock, tidy, find the item again by its stored text); the chooser for
    `t use` alone.

#### 13. `tests/` (Create)

*   **Steps**: S7 creates the files and moves in the cases its commands cover; S8 adds
    `tests/init.rs`; S10 adds `tests/platform.rs`; S11 moves the remaining cases.
*   **Proposed Changes**: `tests/common/mod.rs` makes a sandbox: a temp directory with `HOME`,
    `USERPROFILE`, `XDG_CONFIG_HOME` and `APPDATA` inside it, all other variables cleared except
    `PATH`, `SystemRoot` and `PATHEXT` (Windows), and `TODO_FILE` inside it; it runs the binary
    from `env!("CARGO_BIN_EXE_t")` and returns stdout, stderr and the status. Its first test
    checks the sandbox, as the zsh suite's first check does. `tests/locking.rs` holds the lock
    from the test process with `File::lock`. `tests/platform.rs` holds Windows-only cases: the
    config location, a missing `APPDATA`, `edit` with a `.cmd` editor and with a path containing
    spaces, and arguments.

#### 14. `test/run.zsh` (Modify)

*   **Steps**: S1, S8, S9, S11, S12.
*   **Proposed Changes**:
    *   S1: `repo=${0:A:h:h}`, used for `README.md`, `shell/t.zsh` and `install` instead of
        `${T_BIN:h:h}`; `T_BIN` absolute, overridable, default `$repo/bin/t`; `impl` as above;
        `XDG_CONFIG_HOME` and `APPDATA` exported into the throwaway home before the first run of
        `t`, with a safety check that the config file used is inside it.
    *   S8: config-file checks (`impl=rust`) beside the `~/.zshenv` and `t init` checks
        (`impl=zsh`); the perl-failure checks run only for `impl=zsh`.
    *   S9: the lock helpers (`hold` and the probes in the edit and pick checks) use
        `python3 -c 'import fcntl; fcntl.lockf(...)'` for `impl=zsh` and `fcntl.flock` for
        `impl=rust`; "pick without fzf" no longer links awk and perl into its `PATH` for
        `impl=rust`.
    *   S11: the sections moved to `tests/*.rs` are deleted.
    *   S12: the `impl=zsh` checks are deleted; `T_BIN` defaults to `$repo/target/debug/t`,
        and the suite stops with a message when it isn't built; `install` checks use
        `--no-build` with a binary built beforehand.

#### 15. `install` (Modify)

*   **Steps**: S12.
*   **Proposed Changes**: Runs `cargo build --release --locked` in the clone unless
    `--no-build` is given; the two new failure messages. Links `target/release/t`. A link to
    `$repo/bin/t` (0.3's) counts as this `t`: install replaces it, `--uninstall` removes it. The
    header comment and the last line change as in [New messages](#new-messages). Everything else
    stays.

#### 16. `.github/workflows/test.yml` (Modify)

*   **Steps**: S1, S9, S12.
*   **Proposed Changes**: S1: runs on pushes to `develop` too; a Rust job on `ubuntu-latest`,
    `macos-latest` and `windows-latest` with `cargo fmt --check`,
    `cargo clippy --all-targets -- -D warnings` and `cargo test --locked`, plus a build with Rust
    1.89 on Ubuntu; the existing zsh jobs stay. S9: macOS and Ubuntu also run
    `cargo build --locked` and `T_BIN=$PWD/target/debug/t test/run.zsh`. S12: the awk matrix and
    the `bin/t` jobs are removed; the tag check compares the tag with `target/debug/t --version`.

#### 17. `.github/workflows/release.yml` (Create)

*   **Steps**: S14.
*   **Proposed Changes**: On a `v*` tag, with `permissions: contents: write`: builds
    `x86_64-unknown-linux-musl` (with `musl-tools`), `aarch64-apple-darwin`,
    `x86_64-apple-darwin` (target added with `rustup`) and `x86_64-pc-windows-msvc`; packs the
    binary, `LICENSE`, `README.md` and `shell/t.zsh` as `t-<version>-<target>.tar.gz` (`.zip`
    for Windows); writes `SHA256SUMS`; creates a GitHub release with them. Binaries are unsigned;
    README says how to allow them on macOS and Windows.

#### 18. `bin/t`, `bin/EGM.md` (Delete)

*   **Steps**: S12, once the Rust program passes every test.

#### 19. `src/EGM.md` (Create)

*   **Steps**: S2 to S9, then S13.
*   **Proposed Changes**: The contract of the Rust program: the sections of `bin/EGM.md`,
    handlers named by module and function, and the rules of this design that change it.

#### 20. `EGM.md`, `shell/EGM.md` (Modify)

*   **Steps**: S12 (`bin` becomes `src`; the link in `shell/EGM.md` points at `src/EGM.md`), S13
    (the rest).
*   **Proposed Changes**: Root: the "Configuration only through environment variables"
    invariant becomes environment variables or the config file; Portability names Rust and the
    three systems; System Verification names `cargo test` and `test/run.zsh`.

#### 21. `README.md` (Modify)

*   **Steps**: S13.
*   **Proposed Changes**: Requirements (no zsh, awk or perl to run; a Rust toolchain to build),
    Install (`./install` on macOS and Linux, `cargo install --path .` anywhere, release
    binaries), Configure (the config file; environment variables still win; empty values),
    Keeping the list in Obsidian (config file instead of `~/.zshenv`), Windows (console encoding,
    PowerShell quoting, unsigned binaries), a new "Upgrading from 0.3" section, Test
    (`cargo test`, then `test/run.zsh`).

#### 22. `AGENTS.md` (Modify)

*   **Steps**: S13.
*   **Proposed Changes**: "What this is" names the Rust layout. "Never touch a real todo list"
    uses a throwaway `HOME` and `XDG_CONFIG_HOME` (and `APPDATA` on Windows). "Tests":
    `cargo test` and `test/run.zsh` before every commit. "Keep it portable": the three systems,
    platform code only in the places of the design, new dependencies approved by the repository
    owner. "Changing the code": the checklist names the Rust files. "Commits and pull requests":
    designs in `docs/designs/`, work tracked in GitHub issues, work on `develop`.

#### 23. `docs/designs/SITEMAP.md` (Modify)

*   **Steps**: created with this design; S13 sets the design's status to Implemented.

`shell/t.zsh`, `CLAUDE.md` and `LICENSE` don't change.

### Where each `test/run.zsh` section goes

| 0.3.0 section | Goes to |
|---|---|
| the tests' own safety | stays, plus the sandbox check in `tests/common` |
| adding and listing, done, errors leave the file untouched, today, later, list --today / --later, items by text, rm, reword, edge cases | `tests/items.rs` |
| tidy, listing done items | `tests/tidy.rs` |
| list --json, Jira links | `tests/jira.rs`; checks that need a terminal (`on_tty`) stay; perl-failure checks removed in S12 (perl is gone) |
| t commands, t complete | `tests/complete.rs` |
| several lists, t move, old archives (part of the 1057 section) | `tests/lists.rs` |
| Tab completion widget | stays |
| configuration from ~/.zshenv | removed in S12 (zsh only); replaced by `tests/config.rs` |
| edit | stays (Unix `exec`); Windows in `tests/platform.rs` |
| locking | `tests/locking.rs`; the lock probes in the edit and pick checks stay |
| pick, t use alone in a terminal | stay (fake fzf, terminal) |
| backup, undo | `tests/undo.rs` |
| the README demo stays true | stays |
| t init, t init with Obsidian vaults | `tests/init.rs`; checks with terminal prompts stay |
| ./install | stays |

A check that exists in 0.3.0 and nowhere after S12 is listed, with its reason, in the pull
request that removes it.

### Verification Plan

*   **Compilation/Build**: `cargo build --release --locked` on macOS, Linux and Windows; a build
    with Rust 1.89.
*   **Unit and integration tests**: `cargo test --locked` on all three systems in CI.
*   **Static Analysis & Linting**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
*   **Coverage**: every 0.3.0 check passes against the Rust binary, has a Rust test that replaces
    it, or is listed as removed with its reason (see the table above).
*   **E2E/Integration**: from S9, `T_BIN=$PWD/target/debug/t test/run.zsh` ends with
    `N passed, 0 failed` on macOS and Ubuntu. By hand, on a throwaway list only: a list with
    `.bak` and `.sum` written by 0.3.0 is undone by 0.4.0's `t undo`; two `t add` runs started
    together both land in the list; old archives plus `t move` to and from the default list
    (the one-lock-per-file rule).
*   **Drift**: before the pull request into `main`, this design and `src/EGM.md` are checked
    against the code (`/egm:egm-drift-report`).

--------------------------------------------------------------------------------

*Drafted with EGM*
