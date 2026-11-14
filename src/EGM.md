# src

## Overview

The Rust port of `t` (design: [`../docs/designs/rust_port.md`](../docs/designs/rust_port.md)).
It is built module by module; until step S12 of the design, the zsh script `bin/t` remains the
command people use, and [`../bin/EGM.md`](../bin/EGM.md) its contract. This file grows with each
step and describes only what exists in `src/` so far.

So far the binary `t` implements `help` and `version`. Every other command prints
`t: '<command>' isn't in the Rust port yet; use bin/t` (or, with no command,
`t: listing isn't in the Rust port yet; use bin/t`) and exits 1.

## Interface

### `main.rs`

- `t help`, `-h`, `--help`: prints 0.3.0's usage text (`[main:USAGE]`), identical to `bin/t`'s;
  a unit test compares the two.
- `t version`, `--version`: `t <version from Cargo.toml>`.
- `[main:out]`: every write to stdout. A closed pipe ends the run quietly with status 141 on
  Unix, 1 on Windows; any other write error prints `t: could not write the output: <error>` and
  exits 1.
- Arguments that aren't valid Unicode: `t: arguments must be valid text`, exit 1 (until step S7
  accepts byte arguments on Unix).

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
- `[lock:take]` creates empty lock files and holds locks on them.
- `[store:save]` writes the list, its backup and its checksum file; `[store:undo]` the same;
  `[store:archive]` creates the archive folder and creates or appends to archive files.
- `[text:styled_stdout]` on Windows may switch the console to escape-sequence mode.
- `[main:out]` writes to stdout; errors and warnings go to stderr.

## Verification

- Run: `cargo test` (unit tests in each module).
