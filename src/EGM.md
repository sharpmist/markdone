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
- `[text:styled_stdout]` on Windows may switch the console to escape-sequence mode.
- `[main:out]` writes to stdout; errors and warnings go to stderr.

## Verification

- Run: `cargo test` (unit tests in each module).
