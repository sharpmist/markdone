# src

## Overview

`t`, the whole command, as one Rust program for macOS, Linux and Windows. It replaced the zsh
script of 0.3.0 (`bin/t` at the git tag `v0.3.0`); the design of that port, and every deliberate
difference from 0.3.0, is in [`../docs/designs/rust_port.md`](../docs/designs/rust_port.md).

## Interface

### `main.rs`: dispatch

Commands, dispatched in 0.3.0's order by `[main:run]`:

| Command | Handler |
|---|---|
| `t help`, `-h`, `--help`; `t version`, `--version`; `t commands` | printed without loading the settings |
| `t init [<path> | --obsidian]` | `[init:init]` |
| `t lists [--json]`, `t lists add|rm [--force]|rename …` | `[main:lists_cmd]`, `[lists:Lists]` |
| `t use <list>` | `[main:use_cmd]`, `[lists:Lists::use_list]` |
| `t complete <words>` | `[complete:words]`, and for item commands `[complete:items]` after locking and tidying |
| `t`, `t list [--today|--later|--done [--all]|--json]` | `[main:list_cmd]` |
| `t add [--later] <text>`, `done`, `today`, `later`, `rm`, `reword` | `[main:run]` with `[doc:Doc]` |
| `t move <item> @<list>` | `[main:run]`: both lists locked in path order and tidied |
| `t undo` | `[store:undo]`, before tidy |
| `t pick` | `[main:pick_cmd]`, `[pick:run_fzf]`, `[pick:find_again]` |
| `t edit` | `[main:edit]` |
| `t use` (no name) | `[main:use_cmd]` with fzf |
| `t @<list> <command>` | the command on that list (`[lists:Lists::find]`) |

- `[main:USAGE]`: the usage text, as in 0.3.0.
- `[main:out]`: every write to stdout. A closed pipe ends the run quietly with status 141 on
  Unix, 1 on Windows; any other write error prints `t: could not write the output: <error>` and
  exits 1.
- `[main:tidy]`: reads a list, tidies it, and saves when that changed it; archives first.
- `[main:Locks]`: the locks of a run, at most one per lock file.
- Arguments are bytes on Unix; on Windows one that isn't valid Unicode stops `t` with
  `t: arguments must be valid text`.

### `init.rs`: creating the list

- `[init:init]`: `t init`, `t init <path>`, `t init --obsidian`.
- `[init:obsidian_vaults]`: the vaults in Obsidian's `obsidian.json` (macOS, Linux, Flatpak,
  Snap and Windows locations), most recently opened first.

### `pick.rs`: fzf and programs on `PATH`

- `[pick:find_program]`: the file a program name runs: the first match on `PATH`, with each
  `PATHEXT` extension on Windows.
- `[pick:fzf]`, `[pick:run_fzf]` (`[pick:Choice]`): finding and running fzf; its status as a
  shell reports it (`[pick:status_code]`).
- `[pick:find_again]` (`[pick:Found]`): the item `t pick` chose, found again by its text.

### `complete.rs`: Tab completion data

- `[complete:commands]`: `t commands`, one line per command, `<insert>\t<shown, description,
  example>`, the same table as 0.3.0's.
- `[complete:words]`: what Tab offers after `t <words> ` (`[complete:Complete]`): printed at once,
  or a list's items, which the caller offers after locking and tidying that list.
- `[complete:items]`: the open items an item command can take.
- `[complete:ITEM_CMDS]`: the commands that take an `<item>`.

### `text.rs`: pure text helpers

- `[text:shown]`: an item's text with each web link reduced to its text, for matching and Tab
  completion.
- `[text:render]`: each web link as an OSC 8 hyperlink when styled; unchanged otherwise.
- `[text:bold]`: a name in bold (`ESC[1m … ESC[0m`).
- `[text:styled_stdout]`: whether output is styled: stdout is a terminal and, on Windows, the
  console accepts escape sequences once asked to.
- `[text:json_string]`: a JSON string; invalid UTF-8 becomes U+FFFD; `"`, `\` and U+0000 to
  U+001F are escaped, the latter as `\u00xx`.
- `[text:lower]`, `[text:char_count]`, `[text:pad]`: case folding, character counts and
  padding for case-insensitive matching and aligned columns.
- `[text:shell_quote]`: a list name quoted as zsh's `${(q-)name}` does.

### `jira.rs`: Jira linking

- `[jira:check_jira]`: validates `TODO_JIRA_KEYS` and `TODO_JIRA_URL`; returns the settings
  (`[jira:Jira]`) when linking is on, else a warning (none when the keys are blank). The
  warnings are 0.3.0's three.
- `[jira:linkify]`: an item's text with each bare key turned into `[KEY-123](<url>KEY-123)`.
- `[jira:keys_in]`: the keys in an item's text, for `t list --json`.

### `doc.rs`: the lines of a list

- `[doc:Doc]`: a list in memory. `[doc:Doc::parse]` and `[doc:Doc::to_bytes]` turn bytes into
  lines and back.
- `[doc:Doc::open_items]` (`[doc:Item]`: number, section, text), `[doc:Doc::rows]` (`t list`
  rows, optionally one section's), `[doc:Doc::done_items]`, `[doc:Doc::find]` (an `<item>`
  reference to a number), `[doc:Doc::take]`.
- Changes, each checked before anything changes, so on error the list is as it was:
  `[doc:Doc::insert]`, `[doc:Doc::finish]`, `[doc:Doc::remove]`, `[doc:Doc::reword]`,
  `[doc:Doc::move_within]`, `[doc:Doc::tidy]` (`[doc:Tidied]`).
- `[doc:Error]`: why a change can't be made; callers turn it into 0.3.0's messages.
- `[doc:row]` (`printf "%3d  %-6s %s"`), `[doc:number]`, `[doc:done_rows]` (`t list --done`).

### `lists.rs`: the lists

- `[lists:Lists]`: the lists folder of `TODO_FILE` and the paths in it; `[lists:Lists::list]`
  gives a `[lists:List]` (name, `[store:Files]`, lock file) for a name.
- `[lists:Lists::names]`, `[lists:Lists::find]`, `[lists:Lists::current]`, `[lists:check_name]`,
  `[lists:open_count]`.
- `[lists:Lists::show]`, `[lists:Lists::lines]`, `[lists:Lists::json]`: `t lists`, plain, tabbed
  (for Tab completion) and `--json`.
- `[lists:Lists::add]`, `[lists:Lists::remove]`, `[lists:Lists::rename]`,
  `[lists:Lists::use_list]`: `t lists add|rm|rename` and `t use <name>`, each under
  `[lists:Lists::dir_lock]`.
- `[lists:Lists::old_archives]`, `[lists:Lists::migrate_archives]`: archives from before there
  were several lists.

### `lock.rs`: one run at a time

- `[lock:take]`: the lock on a lock file (`[lock:What]`: a list's or the lists folder's), waiting
  up to 10 s, checking every 0.05 s. `[lock:Lock]` holds it until dropped or the process ends.

### `store.rs`: saving, undo, archives

- `[store:Files]`: a list's file, backup and checksum file.
- `[store:save]`: rewrites a list with backup and checksum.
- `[store:undo]`: restores the backup when the list still matches the checksum.
- `[store:archive]`, `[store:archive_file]`: appends archived lines to `<list> <year>.md`.
- `[store:archived_items]`: the done items of a list's archives, for `t list --done --all`.
- `[store:cksum]`, `[store:cksum_line]`: the POSIX `cksum` value and line.

### `config.rs`: settings

- `[config:load]`: the settings of this run (`[config:Settings]`): `todo_file` and where it came
  from (`[config:Source]`: `Env`, `File`, `Default`), `todo_list`, `jira_keys`, `jira_url`, the
  config file's path, the home directory, and warnings for the caller to print with `t: `.
- `[config:display]`, `[config:under_home]`: a path as messages show it (`~` plus the rest when
  under the home directory).
- `[config:today]`, `[config:ymd]`, `[config:cutoff]`: today's local date and the archive cutoff
  (7 days earlier), as `YYYY-MM-DD`.
- `[config:os_bytes]`, `[config:bytes_path]`: bytes of environment values and paths (as they are
  on Unix, UTF-8 on Windows).

## Invariants

- **Web links.** `[text]` not empty and without `]`, then `(http://` or `(https://`, at least
  one byte, none of them `)` or ASCII white space (space, tab, LF, VT, FF, CR), then `)`. They
  are found left to right without overlaps, each starting at the leftmost `[` that begins one.
- **Jira linking is all or nothing.** Keys are split at spaces, tabs, newlines and NUL; each must
  match `^[A-Z][A-Z0-9_]*$`, and the URL must be `http://` or `https://`, at least two more
  bytes, ending in `/`. Otherwise linking is off, with a warning.
- **What linking touches.** Scanning left to right, Markdown links (the text may be empty),
  inline code and bare URLs (up to ASCII white space) are copied unchanged; a key is one of the
  configured keys, `-` and ASCII digits, with no word byte, `/` or `-` before it and no word byte
  or `-` after the digits ("word" is ASCII letters, digits and `_`). Linking its own output
  changes nothing.
- **Keys for JSON** use the key test alone, at every position, so keys inside links and code
  count; each key once, in order of first appearance.
- **Dispatch order.** A first `@<list>` is taken off. `help`, `version` and `commands` need no
  settings. Then the settings are loaded (their warnings and the upgrade warning printed), and
  `init`, `lists`, `use` and `complete` (unless it offers items) run without reading a list.
  Otherwise: the list is chosen and must exist; `move` finds its target; old archives are moved
  under the default list's lock (kept when the run needs that list anyway); the list is locked
  (both, in byte order of their paths, for `move`); `undo` runs and exits; `check_jira`; the
  target (for `move`) and the list are tidied; the command runs. An unknown command is found
  only then: `t: unknown command '<word>'` and the usage on stderr, exit 1.
- **Writes.** Every command builds the new list in memory and saves it through `[store:save]`
  only when the change succeeded. `move` saves the target first, then the source.
- **Messages.** Success messages and errors are 0.3.0's; messages with item text go through
  `[text:render]`.
- **Upgrade warning.** When `TODO_FILE` comes from the default and `${ZDOTDIR:-$HOME}/.zshenv`
  mentions `TODO_FILE` (macOS and Linux): `<.zshenv> sets TODO_FILE, but t no longer reads it;
  put this line in <config file>: TODO_FILE=<path>`, the path from 0.3's marked line, else
  `<path>`.
- **`init` never overwrites.** It stops when anything, a dangling symlink included, is at the
  target; the one exception is an existing regular file in a vault, used as it is. Otherwise it
  creates the folders and writes the three headings.
- **Where `init` creates the list.** `t init <path>`: at `<path>` made absolute against the
  working directory, `.` and `..` removed by position, symlinks left alone; then it shows the
  config line for it when it isn't the list `t` uses already, and never asks or writes.
  `t init --obsidian`: `<vault>/Todo/Todo.md`. Plain `t init`: at `TODO_FILE`, but when
  `TODO_FILE` comes from the default and stdin and stdout are a terminal, it first offers the
  vaults.
- **`init` asks only in a terminal.** Without one, a single vault is taken and several are an
  error listing them, and the config line is shown, not written. Vault prompts go to stderr; the
  question about the config file and its messages go to stdout, as in 0.3.0. An answer that is
  empty or starts with `y`/`Y` is yes; end of input stops with `no answer; nothing done`.
- **Remembering the list.** After a yes, `TODO_FILE=<path>` (`~` and the rest under the home
  directory) replaces the config file's last `TODO_FILE` line, or is appended; other lines stay;
  the file is written in place. When the config already names the list, nothing is asked. When
  `TODO_FILE` is set in the environment to something else, a warning says it still wins.
- **Picking.** `t pick` needs fzf (found first, else 0.3.0's message) and an open item. fzf gets
  the unstyled `t list` rows; the list's lock is released while it is open. A cancelled fzf ends
  the run with fzf's status. Then the lock is taken again, the list tidied again, and the chosen
  item found by its exact text: at the chosen number if unchanged, else the only open item with
  that text; none, or several and none at the number, stops with 0.3.0's messages.
- **`t use` alone** needs a terminal on stdin and stdout and fzf; it offers the `t lists` lines
  and passes fzf's status through when cancelled.
- **Editing.** The editor is `EDITOR` (empty or unset: `nvim`, `notepad` on Windows); a value
  naming an existing file is the program, otherwise it is split at spaces, tabs and newlines.
  On Unix `t` execs it after tidying, which closes the lock's file; on Windows `t` releases the
  lock, waits for the editor and exits with its status. An editor that can't be started:
  `t: could not run the editor '<program>'`, exit 127.
- **Bytes.** `doc.rs` works on bytes; only matching decodes them (invalid UTF-8 as U+FFFD).
- **Lines.** Split at LF; a CR before the LF is the line's ending, not its content. An empty file
  has no lines. A leading UTF-8 byte order mark belongs to no line and is written back. The
  file's line ending is CRLF when the first line ends in CRLF. Lines `t` writes or changes get
  the file's ending; untouched lines keep theirs; a last line without LF gets the file's ending.
  Parsing and writing an unchanged list gives back the same bytes, except that a missing final
  newline is added.
- **Sections.** A line starting with `## ` starts a section named by the rest of it. Items before
  any heading are in section "". Open items (`- [ ] `) are numbered from 1 across all sections.
  `- [X] ` and indented items are ordinary lines.
- **Insert position.** At the end of the first section with the name, after its last non-blank
  line (blank: only spaces and tabs).
- **Done.** `finish` writes `- [x] <today> <text>` directly under the first `## Done` and fails
  when the item, its text or the heading is missing.
- **Tidy.** Links Jira keys on `- [ ] ` and `- [x] ` lines; gives today's date to done items whose
  10 bytes after `- [x] ` don't have the shape `NNNN-NN-NN`; moves ticked items outside a Done
  section to the top of the next `## Done` after them; takes dated done items whose date sorts
  before the cutoff out for the archives. When a ticked item has no `## Done` after it, nothing
  changes.
- **Item references.** Digits only are a number (leading zeros allowed); other text must be a
  case-insensitive, literal substring of exactly one open item's text as a terminal shows it.
- **What a list is.** The regular files (not symlinks, not hidden) ending in `.md` directly in the
  lists folder, except `Archive <digits>`, plus the default list when its file exists (a symlink
  there counts); names sorted by bytes. A list's name is its file name without `.md`; the
  default list's is its file name up to the last `.`. Hidden files are named after the full file
  name (`.Todo.md.lock`, `.Todo.md.bak`, `.Todo.md.sum`).
- **Which list.** `find` takes an exact name, else the only list matching ignoring case.
  `current`: `TODO_LIST` (no match is an error, also for `t lists` and `t lists rm`), else the
  name in `.current` (trailing CR and LF dropped; a gone list warns and falls back), else the
  default list.
- **New names** (`lists add`, `lists rename`) must not be empty, contain `/ \ : * ? " < > |` or
  a byte below 0x20, start with `.` or `@`, end with `.`, a space or `.md`, be `Archive <digits>`,
  be a Windows device name (`CON`, `PRN`, `AUX`, `NUL`, `COM0`–`COM9`, `LPT0`–`LPT9`, `COM¹`–`COM³`,
  `LPT¹`–`LPT³`, any case, alone or before a `.`), or match another list ignoring case.
- **Changing the lists.** `add`, `remove`, `rename` and `use_list` hold the folder's lock;
  `remove` and `rename` also take the list's lock, after the folder's, and release it before
  deleting its lock file. `remove` refuses the default list, the current list and, unless forced,
  a list with open items, and keeps the archives. `rename` refuses the default list, moves the
  file, its backup and checksum and its archives, and keeps `t use`'s choice. `.current` is
  written through a temp file and a rename. Messages are 0.3.0's.
- **Old archives.** `Archive <digits>.md` in the lists folder moves to
  `Archive/<default list> <year>.md`, or its done items are appended there sorted; a failure
  warns and leaves the file.
- **Locks.** The lock file is opened for reading and writing, created empty when missing, never
  truncated. A lock belongs to an open handle, not to the process: a run must hold at most one
  lock per lock file, since a second handle waits and fails. Messages are 0.3.0's
  (`could not create the lock file …`, `another t has been using the list …`,
  `another t has been changing the lists …`, `could not lock …`).
- **Writes go through `save`.** It reads the current list and writes it to the backup (created
  0600 on Unix); if that fails, nothing is written. It then writes the list in place (truncate and
  write, never rename), then `<crc> <length>` and LF to the checksum file (0600 on Unix); if only
  that fails, it deletes the checksum file and warns. Messages are 0.3.0's.
- **Undo.** Restores the backup byte for byte through `save` only when the list's `cksum` line
  equals the stored one (both trimmed); a second undo redoes. The format is 0.3.0's, so its
  backups and checksums work.
- **Archives.** One file per list and year of the item's date, `# <list> archive <year>` and a
  blank line when new; each year's lines appended sorted by bytes, with the archive's own line
  ending (a new one takes the list's). The first file that can't be written stops archiving and
  is reported, so the caller leaves the list as it was.
- **Settings sources.** `TODO_FILE`, `TODO_JIRA_KEYS` and `TODO_JIRA_URL` come from the
  environment variable when it is set, else the config file, else the default. An empty
  environment value stops the lookup and means the default (`~/todo/Todo.md`; Jira linking
  off). `TODO_LIST` comes only from the environment; empty means unset.
- **Config file place.** `$XDG_CONFIG_HOME/markdone/config` (when set and not empty), else
  `~/.config/markdone/config`, on macOS and Linux; `%APPDATA%\markdone\config` on Windows, with
  a warning when `APPDATA` isn't set.
- **Config file format.** A leading UTF-8 byte order mark is skipped; a CR before LF is dropped;
  blank lines and `#` lines are skipped; other lines are `NAME=value`, name and value trimmed of
  spaces and tabs, the value everything after the first `=`, no quotes or expansion. The last
  line for a name wins. `TODO_FILE` must be absolute or start with `~/` (also `~\` on Windows),
  else the line is ignored. Each ignored or doubtful line gives one warning naming the file and
  line number.
- **Home directory.** Needed only for the default list and `~` values; when it is needed and
  unknown, loading fails with `can't find your home directory`.
- **Dates.** The cutoff is the local date 7 days before today; dates are compared as strings.

## Side Effects

- `[config:load]` reads the environment and the config file; it never writes.
- `[init:init]` creates the list and its folders, reads Obsidian's `obsidian.json`, and writes
  the config file (only after a yes in a terminal).
- `[lock:take]` creates empty lock files and holds locks on them.
- `[lists:Lists]` creates, deletes and renames list files, their hidden files and archives,
  writes or deletes `.current`, creates `.lists.lock`, and moves old archives.
- `[store:save]` writes the list, its backup and its checksum file; `[store:undo]` the same;
  `[store:archive]` creates the archive folder and creates or appends to archive files.
- `[text:styled_stdout]` on Windows may switch the console to escape-sequence mode.
- `[main:out]` writes to stdout; errors and warnings go to stderr.

## Verification

- Run: `cargo test`: unit tests in each module, and integration tests in `../tests/` that run
  the built `t` in a sandbox (`../tests/common/mod.rs`) whose home directory, config folders and
  lists are all inside a temporary directory, with the environment cleared.
- Also run `test/run.zsh` on macOS or Linux after `cargo build`: the checks that need zsh or a
  terminal, against `target/debug/t`. It must end with `N passed, 0 failed`.
