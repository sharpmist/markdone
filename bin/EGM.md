# bin

## Overview

`t` is the whole todo command: one zsh script that reads and rewrites a single Markdown list
(`## Today`, `## Later`, `## Done`), using awk and perl for the text processing. Every command
except `help`, `commands` and `init` first checks that the list exists, validates the Jira
settings and tidies the list, then runs.

## Interface

Commands, dispatched by the two `case "$1"` blocks at the bottom of `t`:

| Command                              | Aliases          | Handler                         |
| ------------------------------------ | ---------------- | ------------------------------- |
| `t`, `t list`                        | `ls`, `l`        | `[t:list]`                      |
| `t list --today`, `t list --later`   |                  | `[t:list]` with a section name  |
| `t list --done [--all]`              |                  | `[t:list_done]`                 |
| `t add [--later] <text>`             | `a`, `-l`        | `[t:add]`, `[t:insert_item]`    |
| `t done <n>`                         | `d`              | `[t:finish]`                    |
| `t today <n>`                        | `promote`        | `[t:move]`                      |
| `t later <n>`                        | `demote`         | `[t:move]`                      |
| `t pick`                             | `f`              | `[t:list]`, fzf, `[t:finish]`   |
| `t edit`                             | `e`              | `exec ${=EDITOR:-nvim} "$f"`    |
| `t init`                             |                  | `[t:init]`                      |
| `t help`                             | `-h`, `--help`   | `[t:usage]`                     |
| `t commands`                         |                  | `[t:commands]`                  |

Environment variables read at startup:

- `TODO_FILE`: the list file, default `$HOME/todo/Todo.md` (`[t:f]`).
- `TODO_JIRA_KEYS`, `TODO_JIRA_URL`: Jira linking settings (`[t:JIRA_KEYS]`, `[t:JIRA_URL]`),
  validated by `[t:check_jira]`.
- `EDITOR`: used by `t edit`, split on spaces, default `nvim`.

Internal helpers:

- `[t:tidy]`: dates and moves ticked items to `## Done`, archives old Done items, links Jira keys.
- `[t:linkify]`: turns bare Jira keys into Markdown links; `linkify all` processes every line.
- `[t:render]`: shows Markdown links as OSC 8 hyperlinks when stdout is a terminal.
- `[t:save]`: rewrites the list from a temp file.
- `[t:days_ago]`: date `n` days ago as `YYYY-MM-DD`, using `zsh/datetime`.
- `[t:warn]`, `[t:die]`: print `t: <message>` to stderr; `die` exits with status 1.

Output contracts:

- `t commands`: one line per command, `<text to insert after "t ">\t<display text>`.
  [`../shell/t.zsh`](../shell/EGM.md) parses it.
- `t list`: one line per open item, `<n>  <section> <text>` (`printf "%3d  %-6s %s\n"`).
  `t pick` takes the item number from the first field of the chosen line.
- `t list --done`: `<YYYY-MM-DD>  <text>`, newest first.
- Success messages: `Added <n> (<section>): <text>`, `Done: <text>`,
  `Moved to <section> as <n>: <text>`, `Created <file>`.

## Invariants

- **The list must exist.** Only `t init` creates it: every command except `help`, `commands` and
  `init` dies when `$f` is not a file (`[t:f]` check before `check_jira`).
- **`init` never overwrites.** `[t:init]` dies when anything, including a dangling symlink, is
  already at `$f`. Otherwise it creates the directory (`mkdir -p`) and writes the three headings.
- **Tidy runs first.** `check_jira` and `tidy` run before the second `case` dispatch, so every
  list command, including `edit`, sees a tidied list. `help`, `commands` and `init` exit before
  that; `help` and `commands` never touch the list.
- **Writes go through `save`.** `[t:add]`, `[t:finish]`, `[t:move]` and `[t:tidy]` build the result
  in a `mktemp` file and call `[t:save]` only when the step producing it succeeded. `save` writes
  with `cat "$1" >"$f"`, so the file itself (symlink, permissions) is kept. The write is not
  atomic.
- **Startup.** `zmodload zsh/datetime` must succeed, or `t` exits 1 before doing anything.
- **`list` options.** Only `--today`, `--later`, `--done` and `--all` are accepted. `--all` needs
  `--done`; `--done` can't be combined with `--today` or `--later`; `--today --later` together
  list everything.
- **Archive before rewriting.** `[t:tidy]` appends to every `Archive <year>.md` first and calls
  `save` only if all of those writes worked; otherwise it warns and leaves the list untidied. A
  new archive file starts with a `# Archive <year>` heading; items are appended sorted, one file
  per year of the item's date.
- **Undated Done items.** `[t:tidy]` gives today's date to any `- [x] ` line without a
  `YYYY-MM-DD` date, in `## Done` as well as outside it.
- **`## Done` must follow ticked items.** In `[t:tidy]`, ticked items outside Done are moved only
  when a `## Done` heading comes after them. If any are left over, awk exits non-zero and the list
  is not changed.
- **Archive cutoff.** A Done item is archived when its date (characters 7-16 of the line) sorts
  below `days_ago 7`. If that date can't be computed, nothing is archived.
- **Item numbers.** `n` counts `- [ ] ` lines from the top of the file, across all sections.
  `[t:list]`, `[t:finish]` and `[t:move]` all count the same way; a section filter in `list` keeps
  the numbers.
- **Item text.** `[t:add]` joins its arguments with spaces (`"$*"`), and rejects empty text and
  text containing a newline.
- **Moves.** `[t:move]` dies when item `n` doesn't exist or is already in the target section, and
  removes and re-inserts the item in a single write.
- **Insert position.** `[t:insert_item]` appends at the end of the section, before its trailing
  blank lines, and fails when the section heading is missing.
- **Done items.** `[t:finish]` writes `- [x] YYYY-MM-DD <text>` directly under `## Done` and fails
  when item `n` or the heading doesn't exist.
- **Jira linking is all or nothing.** `[t:check_jira]` turns linking off, with a warning, when a
  key doesn't match `^[A-Z][A-Z0-9_]*$`, when the URL is missing, or when the URL doesn't match
  `http(s|)://?*/`. `[t:linkify]` leaves existing Markdown links, inline code and URLs alone, and
  only links the listed keys.
- **Portability.** `#!/usr/bin/env zsh`, POSIX awk only, no GNU- or BSD-only tool flags; date
  arithmetic goes through `[t:days_ago]`.

## Side Effects

- Rewrites `$TODO_FILE` in place (`[t:save]`), from `add`, `done`, `today`, `later`, `pick` and
  from `[t:tidy]` on any list command when tidying changes the list.
- Creates or appends to `Archive <year>.md` in the list's directory (`[t:tidy]`); reads them for
  `t list --done --all` (`[t:list_done]`).
- Creates `$TODO_FILE` and its directory (`[t:init]`), only when nothing is there yet.
- Creates temp files with `mktemp` and removes them before returning.
- Runs external programs: `awk`, `date`, `cmp`, `cut`, `sort`, `grep`, `cat`, `mktemp`; `perl`
  when Jira linking is on (`[t:linkify]`) or stdout is a terminal (`[t:render]`); `fzf` for
  `t pick`; `$EDITOR` (via `exec`) for `t edit`.
- Writes OSC 8 escape sequences to stdout when it is a terminal (`[t:render]`).
- Writes warnings and errors to stderr prefixed with `t: `; exits 1 on `[t:die]` and on an unknown
  command (after printing the usage to stderr). `t pick` exits with fzf's status when nothing is
  chosen.
- No network access and no locking: concurrent runs are not serialised.

## Verification

- Run: `test/run.zsh` (from the repository root)
